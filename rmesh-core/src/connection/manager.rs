use anyhow::{Context, Result, bail, ensure};
use meshtastic::Message;
use meshtastic::api::state::Configured;
use meshtastic::api::{ConnectedStreamApi, StreamApi};
use meshtastic::packet::{PacketReceiver, PacketRouter};
use meshtastic::utils;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use crate::state::{
    AirQualityMetrics, BluetoothConfig, ChannelInfo, DeviceConfig, DeviceMetrics, DeviceState,
    DisplayConfig, EnvironmentMetrics, LoraConfig, MyNodeInfo, NetworkConfig, NodeInfo, Position,
    PositionConfig, PowerConfig, TelemetryData, TextMessage, User,
};

/// want_config nonce the firmware treats as "send the configuration without the node DB".
/// A randomly generated id must never collide with it.
const NODELESS_WANT_CONFIG_ID: u32 = 69420;

/// A simple packet router that doesn't handle incoming packets
struct NoOpRouter;

impl PacketRouter<(), std::io::Error> for NoOpRouter {
    fn handle_packet_from_radio(
        &mut self,
        _packet: meshtastic::protobufs::FromRadio,
    ) -> std::result::Result<(), std::io::Error> {
        Ok(())
    }

    fn handle_mesh_packet(
        &mut self,
        _packet: meshtastic::protobufs::MeshPacket,
    ) -> std::result::Result<(), std::io::Error> {
        Ok(())
    }

    fn source_node_id(&self) -> meshtastic::types::NodeId {
        0u32.into()
    }
}

/// Pending traceroutes by request packet id: the route, or the routing error that ended it.
type RouteWaiters = Arc<
    Mutex<
        HashMap<u32, oneshot::Sender<std::result::Result<crate::mesh::TracerouteResult, String>>>,
    >,
>;

pub struct ConnectionManager {
    port: Option<String>,
    ble: Option<String>,
    timeout: Duration,
    api: Option<ConnectedStreamApi<Configured>>,
    /// Every packet the processor sees is also copied to these, for commands that watch
    /// the stream (`message monitor`, `position track`).
    packet_subscribers: Arc<Mutex<Vec<mpsc::UnboundedSender<meshtastic::protobufs::FromRadio>>>>,
    device_state: Arc<Mutex<DeviceState>>,
    packet_processor: Option<JoinHandle<()>>,
    ack_waiters: Arc<Mutex<HashMap<u32, oneshot::Sender<bool>>>>,
    route_waiters: RouteWaiters,
    admin_session_passkey: Arc<Mutex<Option<Vec<u8>>>>,
}

impl ConnectionManager {
    pub async fn new(port: Option<String>, ble: Option<String>, timeout: Duration) -> Result<Self> {
        Ok(Self {
            port,
            ble,
            timeout,
            api: None,
            packet_subscribers: Arc::new(Mutex::new(Vec::new())),
            device_state: Arc::new(Mutex::new(DeviceState::new())),
            packet_processor: None,
            ack_waiters: Arc::new(Mutex::new(HashMap::new())),
            route_waiters: Arc::new(Mutex::new(HashMap::new())),
            admin_session_passkey: Arc::new(Mutex::new(None)),
        })
    }

    pub async fn connect(&mut self) -> Result<()> {
        info!("Establishing connection to Meshtastic device...");

        // Tear down any previous session first. Reconnecting without disconnecting left the
        // old processor task running: it shares device_state, so packets still queued from
        // the previous radio would land *after* begin_config_dump resets the state and
        // repopulate it — including my_node_info, which admin writes address by.
        if self.api.is_some() || self.packet_processor.is_some() {
            debug!("Tearing down the previous connection before reconnecting");
            if let Err(e) = self.disconnect().await {
                debug!("Error while closing the previous connection: {e}");
            }
        }

        // Create StreamApi instance
        let stream_api = StreamApi::new();

        // Determine connection type and connect
        let (packet_receiver, connected_api) = if let Some(_ble_addr) = &self.ble {
            #[cfg(feature = "bluetooth")]
            {
                info!("Connecting via Bluetooth to {addr}", addr = _ble_addr);
                // Parse BLE address string into BleId - try as MAC address first, then as name
                let ble_id = utils::stream::BleId::from_mac_address(_ble_addr)
                    .unwrap_or_else(|_| utils::stream::BleId::from_name(_ble_addr));
                let stream = utils::stream::build_ble_stream(&ble_id, Duration::from_secs(10))
                    .await
                    .context("Failed to connect via Bluetooth")?;
                stream_api.connect(stream).await
            }
            #[cfg(not(feature = "bluetooth"))]
            {
                bail!("Bluetooth support not compiled. Build with --features bluetooth");
            }
        } else if let Some(port) = &self.port {
            if port.contains(':') || port.starts_with("192.") || port.starts_with("10.") {
                // TCP connection
                info!("Connecting via TCP to {port}");
                let stream = utils::stream::build_tcp_stream(port.clone())
                    .await
                    .context("Failed to connect via TCP")?;
                stream_api.connect(stream).await
            } else {
                // Serial connection
                info!("Connecting via serial port {port}");
                let mut stream = utils::stream::build_serial_stream(
                    port.clone(),
                    None, // Use default baud rate
                    None, // Use default DTR
                    None, // Use default RTS
                )
                .context("Failed to connect via serial")?;

                // Send wake sequence to force device resync (similar to Python implementation)
                // This helps the device wake up and resync its serial state machine
                use tokio::io::AsyncWriteExt;
                let wake_sequence = vec![0xc3; 32]; // START2 byte repeated
                if let Err(e) = stream.stream.write_all(&wake_sequence).await {
                    debug!("Failed to send wake sequence: {e}");
                }
                if let Err(e) = stream.stream.flush().await {
                    debug!("Failed to flush wake sequence: {e}");
                }

                // Add a brief delay for serial port stabilization
                // This helps avoid initial sync errors with stale data
                tokio::time::sleep(Duration::from_millis(100)).await;

                stream_api.connect(stream).await
            }
        } else {
            // Auto-detect serial port
            info!("Auto-detecting serial port...");
            let ports =
                utils::stream::available_serial_ports().context("Failed to list serial ports")?;

            ensure!(
                !ports.is_empty(),
                "No serial ports found. Please specify --port or --ble"
            );

            let port_name = ports[0].clone();
            info!("Using auto-detected port: {port_name}");

            let mut stream = utils::stream::build_serial_stream(
                port_name, None, // Use default baud rate
                None, // Use default DTR
                None, // Use default RTS
            )
            .context("Failed to connect to auto-detected serial port")?;

            // Send wake sequence to force device resync (similar to Python implementation)
            // This helps the device wake up and resync its serial state machine
            use tokio::io::AsyncWriteExt;
            let wake_sequence = vec![0xc3; 32]; // START2 byte repeated
            if let Err(e) = stream.stream.write_all(&wake_sequence).await {
                debug!("Failed to send wake sequence: {e}");
            }
            if let Err(e) = stream.stream.flush().await {
                debug!("Failed to flush wake sequence: {e}");
            }

            // Add a brief delay for serial port stabilization
            // This helps avoid initial sync errors with stale data
            tokio::time::sleep(Duration::from_millis(100)).await;

            stream_api.connect(stream).await
        };

        // Configure the connection
        info!("Configuring connection...");
        // generate_rand_id draws from the whole u32 range, so it can land on the sentinel
        // the firmware reads as "send the config without the node DB" and hand back an
        // empty node list. The reference client steps past it the same way.
        let mut config_id = utils::generate_rand_id::<u32>();
        if config_id == NODELESS_WANT_CONFIG_ID {
            config_id += 1;
        }
        let configured_api = connected_api
            .configure(config_id)
            .await
            .context("Failed to configure connection")?;

        // Store the configured API
        self.api = Some(configured_api);

        // Same reasoning as in `disconnect`, for a manager that is reconnected without one.
        self.clear_session_key().await;

        // Record which dump we are waiting for before any packet can be processed.
        // `disconnect` leaves device_state intact, so a reconnect would otherwise inherit
        // the previous session's completion flag and skip the wait entirely.
        self.device_state.lock().await.begin_config_dump(config_id);

        // Start packet processing
        self.start_packet_processing(packet_receiver).await;

        // The radio dumps my_info, metadata, channels, config and the whole node DB in
        // response to want_config, then terminates it with ConfigCompleteId. Wait for that
        // marker rather than a fixed delay: how long the dump takes scales with the size of
        // the node DB, and reading device state early yields a half-populated view.
        self.wait_for_config_complete().await;

        info!("Connection established and configured successfully");
        Ok(())
    }

    async fn start_packet_processing(&mut self, mut receiver: PacketReceiver) {
        let device_state = self.device_state.clone();
        let ack_waiters = self.ack_waiters.clone();
        let route_waiters = self.route_waiters.clone();
        let admin_session_passkey = self.admin_session_passkey.clone();
        let packet_subscribers = self.packet_subscribers.clone();

        // Spawn a background task to process packets
        let handle = tokio::spawn(async move {
            info!("Starting packet processing loop");

            while let Some(packet) = receiver.recv().await {
                // Before processing, which consumes the packet and can fail. A subscriber
                // whose receiver is gone is dropped here.
                packet_subscribers
                    .lock()
                    .await
                    .retain(|subscriber| subscriber.send(packet.clone()).is_ok());

                if let Err(e) = process_from_radio_packet(
                    packet,
                    device_state.clone(),
                    ack_waiters.clone(),
                    route_waiters.clone(),
                    admin_session_passkey.clone(),
                )
                .await
                {
                    warn!("Error processing packet: {e}");
                }
            }

            // The radio is gone (unplugged, or the link dropped): end every watcher too, or
            // `message monitor` would wait forever on a stream nothing feeds.
            packet_subscribers.lock().await.clear();
            info!("Packet processing loop ended");
        });

        self.packet_processor = Some(handle);
    }

    /// Block until the radio signals the end of its initial configuration dump.
    ///
    /// Falls back to a warning rather than an error: commands that only send (such as
    /// `message send`) still work against a device that never emits ConfigCompleteId.
    async fn wait_for_config_complete(&self) {
        // Deliberately bounded even when the caller passes zero. Treating zero as "no
        // limit" wedges forever against a peer that stays connected but never finishes its
        // dump: the liveness check below sees a live task, so nothing breaks the loop. The
        // CLI rejects zero outright; a library caller that passes it gets an immediate
        // warning and degraded state, which is recoverable where a hang is not.
        let budget = self.timeout;
        let start = std::time::Instant::now();

        while start.elapsed() < budget {
            if self.device_state.lock().await.config_complete {
                debug!(
                    "Initial config dump completed in {elapsed:?}",
                    elapsed = start.elapsed()
                );
                return;
            }

            // Once the processing task has ended nothing can set the flag, so sitting out
            // the rest of the budget would stall every command for the full timeout.
            if self
                .packet_processor
                .as_ref()
                .is_some_and(|handle| handle.is_finished())
            {
                warn!("Connection closed before the initial config dump completed");
                return;
            }

            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        warn!(
            "Timed out after {budget:?} waiting for the initial config dump; \
             device state may be incomplete"
        );
    }

    pub fn is_connected(&self) -> bool {
        self.api.is_some()
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(processor) = self.packet_processor.take() {
            processor.abort();
        }
        // Close every subscriber's stream, so a watcher ends instead of waiting forever.
        self.packet_subscribers.lock().await.clear();

        if let Some(mut api) = self.api.take() {
            // The firmware turns Bluetooth off while a serial client is attached and only turns
            // it back on when the client says it is done, or after 15 idle minutes.
            if let Err(e) = api
                .send_to_radio_packet(Some(
                    meshtastic::protobufs::to_radio::PayloadVariant::Disconnect(true),
                ))
                .await
            {
                debug!("Failed to tell the radio we are disconnecting: {e}");
            }
            // A fixed grace period, because StreamApi::disconnect cancels the writer without
            // draining its queue. Drain it there instead if 100 ms ever proves short.
            tokio::time::sleep(Duration::from_millis(100)).await;
            api.disconnect().await?;
        }

        // The passkey authorises admin writes against the radio that issued it. Keeping it
        // would let `ensure_session_key` short-circuit on the next connection and send one
        // radio's credential to another, which that radio rejects — silently, on the paths
        // that do not read anything back.
        self.clear_session_key().await;

        Ok(())
    }

    pub fn get_api(&mut self) -> Result<&mut ConnectedStreamApi<Configured>> {
        self.api.as_mut().context("Not connected")
    }

    /// How long callers may wait on the radio, from `--timeout`.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub async fn get_device_state(&self) -> DeviceState {
        self.device_state.lock().await.clone()
    }

    pub fn get_device_state_ref(&self) -> Arc<Mutex<DeviceState>> {
        self.device_state.clone()
    }

    /// A stream of every packet received from now on, alongside the manager's own
    /// processing. It closes on disconnect.
    pub async fn subscribe_packets(&self) -> PacketReceiver {
        let (subscriber, receiver) = mpsc::unbounded_channel();
        self.packet_subscribers.lock().await.push(subscriber);
        receiver
    }

    pub async fn send_traceroute(
        &mut self,
        destination: u32,
    ) -> Result<crate::mesh::TracerouteResult> {
        // The radio's own hop limit, as the reference client uses; 3 is the firmware default.
        let hop_limit = self
            .device_state
            .lock()
            .await
            .lora_config
            .as_ref()
            .map_or(3, |lora| lora.hop_limit);
        // Nonzero: the firmware assigns its own id to a packet sent with 0, and the reply
        // would then name an id we never saw.
        let packet_id = rand::random::<u32>().max(1);

        let (tx, rx) = oneshot::channel();
        self.route_waiters.lock().await.insert(packet_id, tx);

        let api = self.get_api()?;
        api.send_to_radio_packet(Some(
            meshtastic::protobufs::to_radio::PayloadVariant::Packet(traceroute_packet(
                destination,
                packet_id,
                hop_limit,
            )),
        ))
        .await?;
        debug!("Sent traceroute to {destination:08x} as packet {packet_id}");

        // The reference client allows 20 s per hop.
        let wait = Duration::from_secs(20 * u64::from(hop_limit.max(1)));
        let reply = tokio::time::timeout(wait, rx).await;
        self.route_waiters.lock().await.remove(&packet_id);
        match reply {
            Ok(Ok(Ok(route))) => Ok(route),
            Ok(Ok(Err(reason))) => bail!("Traceroute to {destination:08x} failed: {reason}"),
            Ok(Err(_)) => bail!("Traceroute to {destination:08x} was abandoned"),
            Err(_) => bail!("No traceroute reply from {destination:08x} within {wait:?}"),
        }
    }

    pub async fn send_text_with_ack(
        &mut self,
        text: String,
        destination: u32,
        channel: u8,
        timeout_secs: u64,
    ) -> Result<bool> {
        // Generate a unique packet ID for tracking
        let packet_id = rand::random::<u32>();

        // Create a oneshot channel for ACK notification
        let (tx, rx) = oneshot::channel();

        // Register the ACK waiter
        {
            let mut waiters = self.ack_waiters.lock().await;
            waiters.insert(packet_id, tx);
        }

        // Create a no-op packet router for sending
        let mut router = NoOpRouter;

        // Send the message with want_ack set to true
        let api = self.get_api()?;
        api.send_mesh_packet(
            &mut router,
            text.into_bytes().into(),
            meshtastic::protobufs::PortNum::TextMessageApp,
            if destination == 0xFFFFFFFF {
                meshtastic::packet::PacketDestination::Broadcast
            } else {
                meshtastic::packet::PacketDestination::Node(destination.into())
            },
            (channel as u32).into(),
            true,  // want_ack
            false, // want_response
            false, // echo_response
            Some(packet_id),
            None, // emoji
        )
        .await?;

        debug!("Sent message with ID {packet_id} and ACK request");

        // Wait for ACK with timeout
        match tokio::time::timeout(Duration::from_secs(timeout_secs), rx).await {
            Ok(Ok(ack)) => Ok(ack),
            Ok(Err(_)) => {
                // Channel was closed without receiving ACK
                debug!("ACK channel closed for packet {packet_id}");
                Ok(false)
            }
            Err(_) => {
                // Timeout occurred, clean up the waiter
                let mut waiters = self.ack_waiters.lock().await;
                waiters.remove(&packet_id);
                debug!("ACK timeout for packet {packet_id}");
                Ok(false)
            }
        }
    }

    /// Request a session key from the device for admin operations
    pub async fn ensure_session_key(&mut self) -> Result<()> {
        // Check if we already have a session key
        {
            let session_key = self.admin_session_passkey.lock().await;
            if session_key.is_some() {
                debug!("Session key already exists");
                return Ok(());
            }
        }

        info!("Requesting admin session key...");

        // Resolved before the mutable api borrow below.
        let local_node = self.local_node_num().await?;

        let api = self.get_api()?;

        // Create admin message for session key request
        let admin_msg = meshtastic::protobufs::AdminMessage {
            payload_variant: Some(
                meshtastic::protobufs::admin_message::PayloadVariant::GetConfigRequest(
                    meshtastic::protobufs::admin_message::ConfigType::SessionkeyConfig as i32,
                ),
            ),
            session_passkey: Vec::new(),
        };

        // Create mesh packet
        let mesh_packet = meshtastic::protobufs::MeshPacket {
            payload_variant: Some(meshtastic::protobufs::mesh_packet::PayloadVariant::Decoded(
                meshtastic::protobufs::Data {
                    portnum: meshtastic::protobufs::PortNum::AdminApp as i32,
                    payload: admin_msg.encode_to_vec(),
                    want_response: true,
                    ..Default::default()
                },
            )),
            to: local_node,
            ..Default::default()
        };

        // Send session key request
        api.send_to_radio_packet(Some(
            meshtastic::protobufs::to_radio::PayloadVariant::Packet(mesh_packet),
        ))
        .await?;

        // Wait for the session key to be received
        let timeout = Duration::from_secs(5);
        let start = std::time::Instant::now();

        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;

            let session_key = self.admin_session_passkey.lock().await;
            if session_key.is_some() {
                info!("Session key received successfully");
                return Ok(());
            }

            if start.elapsed() > timeout {
                bail!("Timeout waiting for session key");
            }
        }
    }

    /// Get the current session key if available
    /// The attached radio's own node number.
    ///
    /// Admin messages must be addressed to it, never to 0. PKI-capable firmware encrypts
    /// admin traffic to the destination's public key and node 0 has none, so the radio
    /// answers PKI_SEND_FAIL_PUBLIC_KEY and drops the request — which made every admin
    /// call (session key, config set, reboot) time out with nothing to show for it.
    pub async fn local_node_num(&self) -> Result<u32> {
        self.device_state
            .lock()
            .await
            .my_node_info
            .as_ref()
            .map(|info| info.node_num)
            .context("No node info from the radio yet; cannot address an admin message to it")
    }

    pub async fn get_session_key(&self) -> Option<Vec<u8>> {
        self.admin_session_passkey.lock().await.clone()
    }

    /// Set the session key (used when receiving admin responses)
    pub async fn set_session_key(&self, key: Vec<u8>) {
        let mut session_key = self.admin_session_passkey.lock().await;
        *session_key = Some(key);
        debug!("Session key updated");
    }

    /// Clear the session key (used on disconnect or authentication failure)
    pub async fn clear_session_key(&self) {
        let mut session_key = self.admin_session_passkey.lock().await;
        *session_key = None;
        debug!("Session key cleared");
    }
}

async fn process_from_radio_packet(
    from_radio: meshtastic::protobufs::FromRadio,
    device_state: Arc<Mutex<DeviceState>>,
    ack_waiters: Arc<Mutex<HashMap<u32, oneshot::Sender<bool>>>>,
    route_waiters: RouteWaiters,
    admin_session_passkey: Arc<Mutex<Option<Vec<u8>>>>,
) -> Result<()> {
    let payload_variant = match from_radio.payload_variant {
        Some(variant) => variant,
        None => return Ok(()), // Ignore empty packets
    };

    match payload_variant {
        meshtastic::protobufs::from_radio::PayloadVariant::MyInfo(my_info) => {
            let mut state = device_state.lock().await;
            state.set_my_node_info(MyNodeInfo {
                node_num: my_info.my_node_num,
                node_id: format!("{num:08x}", num = my_info.my_node_num),
                reboot_count: my_info.reboot_count,
                min_app_version: my_info.min_app_version,
                device_id: hex::encode(my_info.device_id),
            });
            debug!("Updated my node info");
        }

        meshtastic::protobufs::from_radio::PayloadVariant::NodeInfo(node_info) => {
            let mut state = device_state.lock().await;
            let user = node_info.user.clone().unwrap_or_default();
            let last_heard = node_info.last_heard as u64;
            let last_heard_iso =
                chrono::DateTime::from_timestamp(last_heard as i64, 0).map(|dt| dt.to_rfc3339());

            state.update_node(
                node_info.num,
                NodeInfo {
                    id: format!("{num:08x}", num = node_info.num),
                    num: node_info.num,
                    user: User {
                        id: user.id.clone(),
                        long_name: user.long_name.clone(),
                        short_name: user.short_name.clone(),
                        // Not user.hw_model(): that accessor maps a board newer than the
                        // vendored protobufs back to UNSET, so `info nodes` would report
                        // every such radio as having no hardware model.
                        hw_model: crate::state::hardware_model_name(user.hw_model),
                    },
                    last_heard: Some(last_heard),
                    last_heard_iso,
                    snr: Some(node_info.snr),
                    rssi: Some(0), // NodeInfo doesn't have RSSI
                },
            );
            debug!("Updated node info for {num}", num = node_info.num);
        }

        meshtastic::protobufs::from_radio::PayloadVariant::Channel(channel) => {
            let mut state = device_state.lock().await;
            state.update_channel(ChannelInfo {
                index: channel.index as u32,
                name: channel
                    .settings
                    .as_ref()
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| format!("Channel {index}", index = channel.index)),
                role: format!("{role:?}", role = channel.role()),
                has_psk: channel
                    .settings
                    .as_ref()
                    .map(|s| !s.psk.is_empty())
                    .unwrap_or_default(),
                settings: channel.settings,
            });
            debug!("Updated channel {index}", index = channel.index);
        }

        meshtastic::protobufs::from_radio::PayloadVariant::Packet(mesh_packet) => {
            process_mesh_packet(
                mesh_packet,
                device_state,
                ack_waiters,
                route_waiters,
                admin_session_passkey,
            )
            .await?;
        }

        meshtastic::protobufs::from_radio::PayloadVariant::Config(config) => {
            debug!("Received Config packet during initial connection");
            process_config_response(config, device_state).await?;
        }

        meshtastic::protobufs::from_radio::PayloadVariant::Metadata(metadata) => {
            let mut state = device_state.lock().await;
            debug!(
                "Updated device metadata (firmware {version})",
                version = metadata.firmware_version
            );
            state.metadata = Some(metadata);
        }

        meshtastic::protobufs::from_radio::PayloadVariant::ConfigCompleteId(id) => {
            let mut state = device_state.lock().await;
            if state.want_config_id == Some(id) {
                info!("Config complete received with ID: {id}");
                state.config_complete = true;
            } else {
                // A dump requested by an earlier client can still be draining out of the
                // radio; accepting its marker would end our wait on a partial state.
                debug!(
                    "Ignoring ConfigCompleteId {id} from an earlier session (waiting for {want:?})",
                    want = state.want_config_id
                );
            }
        }

        meshtastic::protobufs::from_radio::PayloadVariant::ClientNotification(notification) => {
            // The radio refuses some requests outright, such as a second traceroute within
            // 30 s, and says why here instead of with a routing error.
            let waiter = match notification.reply_id {
                Some(id) => route_waiters.lock().await.remove(&id),
                None => None,
            };
            match waiter {
                Some(sender) => {
                    // A dropped receiver means the caller already gave up.
                    let _ = sender.send(Err(notification.message));
                }
                None => warn!("Radio: {message}", message = notification.message),
            }
        }

        variant => {
            // Other packet types not yet handled
            debug!("Unhandled FromRadio packet variant: {variant:?}");
        }
    }

    Ok(())
}

async fn process_mesh_packet(
    mesh_packet: meshtastic::protobufs::MeshPacket,
    device_state: Arc<Mutex<DeviceState>>,
    ack_waiters: Arc<Mutex<HashMap<u32, oneshot::Sender<bool>>>>,
    route_waiters: RouteWaiters,
    admin_session_passkey: Arc<Mutex<Option<Vec<u8>>>>,
) -> Result<()> {
    let payload_variant = match mesh_packet.payload_variant {
        Some(variant) => variant,
        None => return Ok(()),
    };

    let packet_data = match &payload_variant {
        meshtastic::protobufs::mesh_packet::PayloadVariant::Decoded(decoded) => decoded,
        meshtastic::protobufs::mesh_packet::PayloadVariant::Encrypted(_) => {
            // Can't process encrypted packets
            return Ok(());
        }
    };

    match packet_data.portnum() {
        meshtastic::protobufs::PortNum::TextMessageApp => {
            let text = String::from_utf8_lossy(&packet_data.payload).to_string();
            let mut state = device_state.lock().await;

            state.add_message(TextMessage {
                from: format!("{from:08x}", from = mesh_packet.from),
                from_node: mesh_packet.from,
                to: format!("{to:08x}", to = mesh_packet.to),
                to_node: mesh_packet.to,
                channel: mesh_packet.channel,
                text,
                time: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                snr: Some(mesh_packet.rx_snr),
                rssi: Some(mesh_packet.rx_rssi),
                acknowledged: false,
            });
            debug!(
                "Received text message from {from:08x}",
                from = mesh_packet.from
            );
        }

        meshtastic::protobufs::PortNum::PositionApp => {
            if let Ok(position_proto) =
                meshtastic::protobufs::Position::decode(packet_data.payload.as_slice())
            {
                let mut state = device_state.lock().await;

                if let (Some(lat), Some(lon)) =
                    (position_proto.latitude_i, position_proto.longitude_i)
                {
                    state.update_position(
                        mesh_packet.from,
                        Position {
                            node_id: format!("{from:08x}", from = mesh_packet.from),
                            node_num: mesh_packet.from,
                            latitude: lat as f64 / 1e7,
                            longitude: lon as f64 / 1e7,
                            altitude: position_proto.altitude,
                            time: if position_proto.time > 0 {
                                chrono::DateTime::from_timestamp(position_proto.time as i64, 0)
                                    .map(|dt| dt.to_rfc3339())
                            } else {
                                None
                            },
                            last_updated: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        },
                    );
                    debug!("Updated position for {from:08x}", from = mesh_packet.from);
                }
            }
        }

        meshtastic::protobufs::PortNum::TelemetryApp => {
            if let Ok(telemetry) =
                meshtastic::protobufs::Telemetry::decode(packet_data.payload.as_slice())
            {
                let mut state = device_state.lock().await;

                let mut telemetry_data = TelemetryData {
                    node_num: mesh_packet.from,
                    time: telemetry.time as u64,
                    device_metrics: None,
                    environment_metrics: None,
                    air_quality_metrics: None,
                };

                // Process the telemetry variant
                if let Some(variant) = telemetry.variant {
                    match variant {
                        meshtastic::protobufs::telemetry::Variant::DeviceMetrics(m) => {
                            telemetry_data.device_metrics = Some(DeviceMetrics {
                                battery_level: m.battery_level,
                                voltage: m.voltage,
                                channel_utilization: m.channel_utilization,
                                air_util_tx: m.air_util_tx,
                                uptime_seconds: m.uptime_seconds,
                            });
                        }
                        meshtastic::protobufs::telemetry::Variant::EnvironmentMetrics(m) => {
                            telemetry_data.environment_metrics = Some(EnvironmentMetrics {
                                temperature: m.temperature,
                                relative_humidity: m.relative_humidity,
                                barometric_pressure: m.barometric_pressure,
                                gas_resistance: m.gas_resistance,
                                iaq: m.iaq,
                                distance: m.distance,
                                lux: m.lux,
                                white_lux: m.white_lux,
                                ir_lux: m.ir_lux,
                                uv_lux: m.uv_lux,
                                wind_direction: m.wind_direction,
                                wind_speed: m.wind_speed,
                                weight: m.weight,
                            });
                        }
                        meshtastic::protobufs::telemetry::Variant::AirQualityMetrics(m) => {
                            telemetry_data.air_quality_metrics = Some(AirQualityMetrics {
                                pm10_standard: m.pm10_standard,
                                pm25_standard: m.pm25_standard,
                                pm100_standard: m.pm100_standard,
                                pm10_environmental: m.pm10_environmental,
                                pm25_environmental: m.pm25_environmental,
                                pm100_environmental: m.pm100_environmental,
                                particles_03um: m.particles_03um,
                                particles_05um: m.particles_05um,
                                particles_10um: m.particles_10um,
                                particles_25um: m.particles_25um,
                                particles_50um: m.particles_50um,
                                particles_100um: m.particles_100um,
                            });
                        }
                        variant => {
                            // Other telemetry types not yet handled
                            debug!("Unhandled telemetry variant: {variant:?}");
                        }
                    }
                }

                state.update_telemetry(mesh_packet.from, telemetry_data);
                debug!("Updated telemetry for {from:08x}", from = mesh_packet.from);
            }
        }

        meshtastic::protobufs::PortNum::AdminApp => {
            debug!("Received AdminApp packet");
            if let Ok(admin_msg) =
                meshtastic::protobufs::AdminMessage::decode(packet_data.payload.as_slice())
            {
                debug!("Decoded admin message: {admin_msg:?}");

                // Extract and store the session passkey if present
                if !admin_msg.session_passkey.is_empty() {
                    let mut session_key = admin_session_passkey.lock().await;
                    *session_key = Some(admin_msg.session_passkey.clone());
                    info!("Received and stored admin session passkey");
                }

                match admin_msg.payload_variant {
                    Some(
                        meshtastic::protobufs::admin_message::PayloadVariant::GetConfigResponse(
                            config,
                        ),
                    ) => {
                        debug!("Processing config response");
                        process_config_response(config, device_state).await?;
                    }
                    // Without this the reply to GetChannelRequest is dropped, so a channel
                    // readback can never observe anything and every channel write reports
                    // failure however well it went.
                    Some(
                        meshtastic::protobufs::admin_message::PayloadVariant::GetChannelResponse(
                            channel,
                        ),
                    ) => {
                        let mut state = device_state.lock().await;
                        debug!(
                            "Processing channel response for {index}",
                            index = channel.index
                        );
                        state.update_channel(ChannelInfo {
                            index: channel.index as u32,
                            name: channel
                                .settings
                                .as_ref()
                                .map(|s| s.name.clone())
                                .unwrap_or_default(),
                            role: format!("{role:?}", role = channel.role()),
                            has_psk: channel
                                .settings
                                .as_ref()
                                .map(|s| !s.psk.is_empty())
                                .unwrap_or_default(),
                            settings: channel.settings,
                        });
                    }
                    _ => {}
                }
            } else {
                debug!("Failed to decode admin message");
            }
        }

        meshtastic::protobufs::PortNum::TracerouteApp => {
            // A reply names our request's packet id; a request passing through does not.
            let waiter = match packet_data.request_id {
                0 => None,
                id => route_waiters.lock().await.remove(&id),
            };
            if let Some(sender) = waiter {
                let reply = match meshtastic::protobufs::RouteDiscovery::decode(
                    packet_data.payload.as_slice(),
                ) {
                    Ok(discovery) => Ok(traceroute_result(
                        &discovery,
                        mesh_packet.to,
                        mesh_packet.from,
                        mesh_packet.hop_start,
                        &device_state.lock().await.nodes,
                    )),
                    Err(e) => Err(format!("undecodable reply: {e}")),
                };
                if sender.send(reply).is_err() {
                    debug!(
                        "Traceroute reply {request_id} arrived after its caller gave up",
                        request_id = packet_data.request_id
                    );
                }
            }
        }

        meshtastic::protobufs::PortNum::RoutingApp => {
            // Handle routing packets (including ACKs)
            if let Ok(routing) =
                meshtastic::protobufs::Routing::decode(packet_data.payload.as_slice())
                && let Some(variant) = routing.variant
            {
                match variant {
                    meshtastic::protobufs::routing::Variant::ErrorReason(reason) => {
                        debug!("Routing error: {reason:?}");
                        // NONE is an acknowledgement, not a failure: the traceroute reply may
                        // still be on its way. Any other reason ends that wait.
                        let failed = reason != meshtastic::protobufs::routing::Error::None as i32;
                        let waiter = match packet_data.request_id {
                            id if failed && id != 0 => route_waiters.lock().await.remove(&id),
                            _ => None,
                        };
                        if let Some(sender) = waiter {
                            let reason = meshtastic::protobufs::routing::Error::try_from(reason)
                                .map(|e| e.as_str_name().to_string())
                                .unwrap_or_else(|_| format!("routing error {reason}"));
                            // A dropped receiver means the caller already gave up.
                            let _ = sender.send(Err(reason));
                        }
                    }
                    variant => {
                        debug!("Unhandled routing variant: {variant:?}");
                    }
                }
            }

            // Check if this is an ACK by looking at the request_id
            if packet_data.request_id != 0 {
                let mut waiters = ack_waiters.lock().await;
                if let Some(sender) = waiters.remove(&packet_data.request_id) {
                    if sender.send(true).is_err() {
                        debug!(
                            "ACK receiver dropped for packet {request_id}",
                            request_id = packet_data.request_id
                        );
                    } else {
                        debug!(
                            "Received ACK for packet {request_id}",
                            request_id = packet_data.request_id
                        );
                    }
                }
            }
        }

        portnum => {
            // Other port types not yet handled
            debug!(
                "Unhandled mesh packet with portnum {portnum:?} from {from:08x}",
                from = mesh_packet.from
            );
        }
    }

    // Also check for ACKs in any packet type if they have a request_id
    if mesh_packet.id != 0 && mesh_packet.want_ack {
        // This packet wants an ACK, but we're not handling that here
    } else if mesh_packet.id != 0 {
        // Check if this might be an implicit ACK
        if let meshtastic::protobufs::mesh_packet::PayloadVariant::Decoded(ref data) =
            payload_variant
            && data.request_id != 0
        {
            let mut waiters = ack_waiters.lock().await;
            if let Some(sender) = waiters.remove(&data.request_id) {
                if sender.send(true).is_err() {
                    debug!(
                        "Implicit ACK receiver dropped for packet {request_id}",
                        request_id = data.request_id
                    );
                } else {
                    debug!(
                        "Received implicit ACK for packet {request_id}",
                        request_id = data.request_id
                    );
                }
            }
        }
    }

    Ok(())
}

async fn process_config_response(
    config: meshtastic::protobufs::Config,
    device_state: Arc<Mutex<DeviceState>>,
) -> Result<()> {
    let mut state = device_state.lock().await;

    if let Some(payload) = config.payload_variant {
        match payload {
            meshtastic::protobufs::config::PayloadVariant::Device(device_config) => {
                state.raw_device_config = Some(device_config.clone());
                state.device_config = Some(DeviceConfig {
                    role: format!("{role:?}", role = device_config.role()),
                    button_gpio: device_config.button_gpio,
                    buzzer_gpio: device_config.buzzer_gpio,
                    rebroadcast_mode: format!("{mode:?}", mode = device_config.rebroadcast_mode()),
                    node_info_broadcast_secs: device_config.node_info_broadcast_secs,
                    tzdef: if device_config.tzdef.is_empty() {
                        None
                    } else {
                        Some(device_config.tzdef)
                    },
                    disable_triple_click: device_config.disable_triple_click,
                });
                debug!("Updated device config");
            }
            meshtastic::protobufs::config::PayloadVariant::Position(position_config) => {
                state.position_config = Some(PositionConfig {
                    position_broadcast_secs: position_config.position_broadcast_secs,
                    position_broadcast_smart_enabled: position_config
                        .position_broadcast_smart_enabled,
                    fixed_position: position_config.fixed_position,
                    gps_enabled: position_config.gps_mode()
                        != meshtastic::protobufs::config::position_config::GpsMode::Disabled,
                    gps_mode: format!("{mode:?}", mode = position_config.gps_mode()),
                });
                debug!("Updated position config");
            }
            meshtastic::protobufs::config::PayloadVariant::Power(power_config) => {
                state.power_config = Some(PowerConfig {
                    is_power_saving: power_config.is_power_saving,
                    on_battery_shutdown_after_secs: power_config.on_battery_shutdown_after_secs,
                    adc_multiplier_override: power_config.adc_multiplier_override,
                    wait_bluetooth_secs: power_config.wait_bluetooth_secs,
                    sds_secs: power_config.sds_secs,
                    ls_secs: power_config.ls_secs,
                    min_wake_secs: power_config.min_wake_secs,
                });
                debug!("Updated power config");
            }
            meshtastic::protobufs::config::PayloadVariant::Network(network_config) => {
                state.network_config = Some(NetworkConfig {
                    wifi_enabled: network_config.wifi_enabled,
                    wifi_ssid: network_config.wifi_ssid,
                    wifi_psk: network_config.wifi_psk,
                    ntp_server: network_config.ntp_server,
                    eth_enabled: network_config.eth_enabled,
                    ipv4_config: network_config
                        .ipv4_config
                        .as_ref()
                        .map(|config| format!("{config:?}")),
                });
                debug!("Updated network config");
            }
            meshtastic::protobufs::config::PayloadVariant::Display(display_config) => {
                // One narrow allow per deprecated field, deliberately not one covering the
                // whole struct: a blanket allow here silenced gps_format's own deprecation
                // (upstream marked it Unused in 2.7.4) and hid that rmesh was reporting a
                // dead field. Keep each suppression pinned to the field it excuses so the
                // next deprecation still shows up as a build warning.
                #[allow(deprecated)]
                let gps_format = format!("{format:?}", format = display_config.gps_format());
                #[allow(deprecated)]
                let compass_north_top = display_config.compass_north_top;

                let display = DisplayConfig {
                    screen_on_secs: display_config.screen_on_secs,
                    gps_format,
                    auto_screen_carousel_secs: display_config.auto_screen_carousel_secs,
                    compass_north_top,
                    compass_orientation: format!(
                        "{orientation:?}",
                        orientation = display_config.compass_orientation()
                    ),
                    flip_screen: display_config.flip_screen,
                    units: format!("{units:?}", units = display_config.units()),
                    displaymode: format!("{mode:?}", mode = display_config.displaymode()),
                    heading_bold: display_config.heading_bold,
                    wake_on_tap_or_motion: display_config.wake_on_tap_or_motion,
                };
                state.display_config = Some(display);
                debug!("Updated display config");
            }
            meshtastic::protobufs::config::PayloadVariant::Lora(lora_config) => {
                state.raw_lora_config = Some(lora_config.clone());
                // The protobuf name, rather than a hand-written table: every regen of the
                // protobufs adds regions, and an exhaustive match turns that into a build
                // break. This also matches what the reference client prints, and what
                // `config set lora.region` accepts back.
                let region_str = crate::state::region_name(lora_config.region);

                state.lora_config = Some(LoraConfig {
                    use_preset: lora_config.use_preset,
                    modem_preset: format!("{preset:?}", preset = lora_config.modem_preset()),
                    bandwidth: lora_config.bandwidth,
                    spread_factor: lora_config.spread_factor,
                    coding_rate: lora_config.coding_rate,
                    frequency_offset: lora_config.frequency_offset,
                    region: region_str.to_string(),
                    hop_limit: lora_config.hop_limit,
                    tx_enabled: lora_config.tx_enabled,
                    tx_power: lora_config.tx_power,
                    channel_num: lora_config.channel_num,
                    ignore_mqtt: lora_config.ignore_mqtt,
                });
                debug!("Updated LoRa config");
            }
            meshtastic::protobufs::config::PayloadVariant::Bluetooth(bluetooth_config) => {
                state.bluetooth_config = Some(BluetoothConfig {
                    enabled: bluetooth_config.enabled,
                    mode: format!("{mode:?}", mode = bluetooth_config.mode()),
                    fixed_pin: bluetooth_config.fixed_pin,
                    device_logging_enabled: false, // Not available in current protobuf
                });
                debug!("Updated Bluetooth config");
            }
            meshtastic::protobufs::config::PayloadVariant::Security(_security_config) => {
                // Security config not yet handled
                debug!("Security config received but not yet handled");
            }
            meshtastic::protobufs::config::PayloadVariant::Sessionkey(_sessionkey_config) => {
                // Sessionkey config not yet handled
                debug!("Sessionkey config received but not yet handled");
            }
            meshtastic::protobufs::config::PayloadVariant::DeviceUi(_device_ui_config) => {
                // DeviceUI config not yet handled
                debug!("DeviceUI config received but not yet handled");
            }
        }
    }

    Ok(())
}

/// The traceroute request the reference client sends: an empty RouteDiscovery on the
/// traceroute port, asking for a reply. `request_id` must stay 0 — the firmware reads a
/// nonzero one as "this is a reply" and does not answer.
fn traceroute_packet(
    destination: u32,
    id: u32,
    hop_limit: u32,
) -> meshtastic::protobufs::MeshPacket {
    meshtastic::protobufs::MeshPacket {
        to: destination,
        id,
        hop_limit,
        priority: meshtastic::protobufs::mesh_packet::Priority::Reliable as i32,
        payload_variant: Some(meshtastic::protobufs::mesh_packet::PayloadVariant::Decoded(
            meshtastic::protobufs::Data {
                portnum: meshtastic::protobufs::PortNum::TracerouteApp as i32,
                payload: meshtastic::protobufs::RouteDiscovery::default().encode_to_vec(),
                want_response: true,
                ..Default::default()
            },
        )),
        ..Default::default()
    }
}

/// Both paths of a traceroute reply sent by `dest` to `us`. The firmware lists only the
/// hops in between, records each SNR in quarter-dB with `i8::MIN` for "unknown", and
/// lists a hop that did not record itself as the broadcast address.
fn traceroute_result(
    reply: &meshtastic::protobufs::RouteDiscovery,
    us: u32,
    dest: u32,
    hop_start: u32,
    nodes: &HashMap<u32, NodeInfo>,
) -> crate::mesh::TracerouteResult {
    let path = |from: u32, via: &[u32], to: u32, snrs: &[i32]| {
        // One SNR per node after the first, or the list cannot be lined up with the hops.
        let snrs = if snrs.len() == via.len() + 1 {
            snrs
        } else {
            &[]
        };
        std::iter::once(from)
            .chain(via.iter().copied())
            .chain(std::iter::once(to))
            .enumerate()
            .map(|(hop, node_id)| crate::mesh::RouteHop {
                node_id,
                node_name: nodes
                    .get(&node_id)
                    .map(|n| n.user.long_name.clone())
                    .unwrap_or_else(|| "Unknown".to_string()),
                hop_number: hop as u32,
                snr: hop
                    .checked_sub(1)
                    .and_then(|i| snrs.get(i))
                    .filter(|&&snr| snr != i32::from(i8::MIN))
                    .map(|&snr| snr as f32 / 4.0),
            })
            .collect::<Vec<_>>()
    };
    crate::mesh::TracerouteResult {
        towards: path(us, &reply.route, dest, &reply.snr_towards),
        // Recorded only when the reply carries an SNR for every hop back, ours included, and
        // a hop_start: without one the firmware cannot fill in hops that did not record
        // themselves, so the list may be silently short. The reference client checks both.
        back: (hop_start > 0 && reply.snr_back.len() == reply.route_back.len() + 1)
            .then(|| path(dest, &reply.route_back, us, &reply.snr_back)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reconnecting without disconnecting must not leave the previous processor running:
    /// it shares `device_state` and would write the old radio's queued packets into the
    /// state the new dump has just reset.
    #[tokio::test]
    async fn reconnect_stops_the_previous_packet_processor() {
        let mut manager = ConnectionManager::new(None, None, Duration::from_secs(1))
            .await
            .expect("manager");

        // Stand in for a live session: a task holding device_state, as the real processor
        // does, plus the handle connect() checks.
        let state = manager.get_device_state_ref();
        let handle = tokio::spawn(async move {
            loop {
                state.lock().await.config_complete = true;
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        });
        manager.packet_processor = Some(handle);

        // connect() fails here (no port), but only after the teardown it now performs.
        let _ = manager.connect().await;

        assert!(
            manager.packet_processor.is_none(),
            "the previous processor must be shut down before a new session starts"
        );
    }

    /// disconnect() must put ToRadio{disconnect} on the wire before the stream closes,
    /// or the firmware keeps Bluetooth off for 15 minutes after every serial session.
    #[tokio::test]
    async fn disconnect_tells_the_radio_before_closing() {
        use tokio::io::AsyncReadExt;

        let (client, mut radio) = tokio::io::duplex(4096);
        let (_receiver, api) = StreamApi::new()
            .connect(meshtastic::api::StreamHandle::from_stream(client))
            .await;
        let mut manager = ConnectionManager::new(None, None, Duration::from_secs(1))
            .await
            .expect("manager");
        manager.api = Some(api.configure(1).await.expect("configure"));

        manager.disconnect().await.expect("disconnect");

        let mut written = Vec::new();
        tokio::time::timeout(Duration::from_secs(5), radio.read_to_end(&mut written))
            .await
            .expect("stream closed")
            .expect("read");
        let payload = meshtastic::protobufs::ToRadio {
            payload_variant: Some(meshtastic::protobufs::to_radio::PayloadVariant::Disconnect(
                true,
            )),
        }
        .encode_to_vec();
        let mut frame = vec![0x94, 0xc3, 0, payload.len() as u8];
        frame.extend(payload);
        assert!(
            written.ends_with(&frame),
            "the last frame written must be the disconnect, got {written:02x?}"
        );
    }

    /// Feed a single FromRadio payload through the handler against a fresh state.
    async fn feed(
        state: &Arc<Mutex<DeviceState>>,
        variant: meshtastic::protobufs::from_radio::PayloadVariant,
    ) {
        process_from_radio_packet(
            meshtastic::protobufs::FromRadio {
                id: 0,
                payload_variant: Some(variant),
            },
            state.clone(),
            Arc::new(Mutex::new(HashMap::new())),
            Arc::new(Mutex::new(HashMap::new())),
            Arc::new(Mutex::new(None)),
        )
        .await
        .expect("handler should accept the packet");
    }

    /// The firmware version comes from DeviceMetadata; my_node_info.min_app_version is a
    /// different quantity and must not be used as a stand-in for it.
    #[tokio::test]
    async fn metadata_supplies_the_firmware_version() {
        let state = Arc::new(Mutex::new(DeviceState::new()));

        feed(
            &state,
            meshtastic::protobufs::from_radio::PayloadVariant::Metadata(
                meshtastic::protobufs::DeviceMetadata {
                    firmware_version: "2.7.26.54e0d8d".to_string(),
                    ..Default::default()
                },
            ),
        )
        .await;

        let metadata = state.lock().await.metadata.clone();
        assert_eq!(
            metadata.expect("metadata stored").firmware_version,
            "2.7.26.54e0d8d"
        );
    }

    /// Only the dump this session asked for may end the wait; a marker left over from an
    /// earlier client would otherwise release connect() on a half-populated state.
    #[tokio::test]
    async fn config_complete_accepts_only_the_requested_dump() {
        let state = Arc::new(Mutex::new(DeviceState::new()));
        state.lock().await.want_config_id = Some(42);

        feed(
            &state,
            meshtastic::protobufs::from_radio::PayloadVariant::ConfigCompleteId(7),
        )
        .await;
        assert!(
            !state.lock().await.config_complete,
            "a stale ConfigCompleteId must not end the wait"
        );

        feed(
            &state,
            meshtastic::protobufs::from_radio::PayloadVariant::ConfigCompleteId(42),
        )
        .await;
        assert!(
            state.lock().await.config_complete,
            "the requested ConfigCompleteId must end the wait"
        );
    }

    /// The processor owns the radio's packet stream, so watchers (`message monitor`,
    /// `position track`) only see packets if it hands them a copy — without taking them
    /// away from its own processing.
    #[tokio::test]
    async fn subscribers_see_packets_the_processor_also_handles() -> Result<()> {
        let mut manager = ConnectionManager::new(None, None, Duration::from_secs(1)).await?;
        let (radio, packets) = mpsc::unbounded_channel();
        manager.start_packet_processing(packets).await;
        let mut watcher = manager.subscribe_packets().await;

        radio.send(meshtastic::protobufs::FromRadio {
            id: 0,
            payload_variant: Some(meshtastic::protobufs::from_radio::PayloadVariant::Metadata(
                meshtastic::protobufs::DeviceMetadata {
                    firmware_version: "2.7.26.54e0d8d".to_string(),
                    ..Default::default()
                },
            )),
        })?;

        let seen = tokio::time::timeout(Duration::from_secs(5), watcher.recv())
            .await
            .context("the watcher must receive the packet")?;
        assert!(seen.is_some());
        let state = manager.get_device_state_ref();
        tokio::time::timeout(Duration::from_secs(5), async {
            while state.lock().await.metadata.is_none() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .context("the processor must still handle the packet")?;

        manager.disconnect().await?;
        let closed = tokio::time::timeout(Duration::from_secs(5), watcher.recv()).await;
        assert!(
            matches!(closed, Ok(None)),
            "disconnect must close the watcher's stream"
        );
        Ok(())
    }

    /// A radio that goes away (unplugged, link dropped) ends the processor; its watchers
    /// must end with it, or `message monitor` waits forever on a stream nothing feeds.
    #[tokio::test]
    async fn a_dead_radio_closes_watchers() -> Result<()> {
        let mut manager = ConnectionManager::new(None, None, Duration::from_secs(1)).await?;
        let (radio, packets) = mpsc::unbounded_channel::<meshtastic::protobufs::FromRadio>();
        manager.start_packet_processing(packets).await;
        let mut watcher = manager.subscribe_packets().await;

        drop(radio);

        let closed = tokio::time::timeout(Duration::from_secs(5), watcher.recv()).await;
        assert!(
            matches!(closed, Ok(None)),
            "the watcher's stream must close with the radio's"
        );
        Ok(())
    }

    /// What the reference client sends. A nonzero `request_id` makes the firmware treat the
    /// request as a reply and never answer, which is how rmesh's traceroute used to fail.
    #[test]
    fn traceroute_request_matches_the_reference_client() -> Result<()> {
        let packet = traceroute_packet(0x7e9bb193, 42, 3);
        assert_eq!(
            (packet.to, packet.id, packet.hop_limit),
            (0x7e9bb193, 42, 3)
        );
        assert_eq!(packet.hop_start, 0, "the firmware sets hop_start itself");
        let Some(meshtastic::protobufs::mesh_packet::PayloadVariant::Decoded(data)) =
            packet.payload_variant
        else {
            bail!("the request must be decoded");
        };
        assert_eq!(
            data.portnum(),
            meshtastic::protobufs::PortNum::TracerouteApp
        );
        assert!(data.want_response);
        assert_eq!(data.request_id, 0);
        assert!(
            data.payload.is_empty(),
            "an empty RouteDiscovery, not one wrapped in Routing: {payload:02x?}",
            payload = data.payload
        );
        Ok(())
    }

    fn mesh_packet(
        from: u32,
        hop_start: u32,
        portnum: meshtastic::protobufs::PortNum,
        request_id: u32,
        payload: Vec<u8>,
    ) -> meshtastic::protobufs::FromRadio {
        meshtastic::protobufs::FromRadio {
            id: 0,
            payload_variant: Some(meshtastic::protobufs::from_radio::PayloadVariant::Packet(
                meshtastic::protobufs::MeshPacket {
                    from,
                    to: 0x5c15c784,
                    hop_start,
                    payload_variant: Some(
                        meshtastic::protobufs::mesh_packet::PayloadVariant::Decoded(
                            meshtastic::protobufs::Data {
                                portnum: portnum as i32,
                                payload,
                                request_id,
                                ..Default::default()
                            },
                        ),
                    ),
                    ..Default::default()
                },
            )),
        }
    }

    fn routing_error(
        request_id: u32,
        error: meshtastic::protobufs::routing::Error,
    ) -> meshtastic::protobufs::FromRadio {
        mesh_packet(
            0x5c15c784,
            0,
            meshtastic::protobufs::PortNum::RoutingApp,
            request_id,
            meshtastic::protobufs::Routing {
                variant: Some(meshtastic::protobufs::routing::Variant::ErrorReason(
                    error as i32,
                )),
            }
            .encode_to_vec(),
        )
    }

    type RouteOutcome = std::result::Result<crate::mesh::TracerouteResult, String>;

    async fn route_waiter(
        route_waiters: &RouteWaiters,
        id: u32,
    ) -> oneshot::Receiver<RouteOutcome> {
        let (tx, rx) = oneshot::channel();
        route_waiters.lock().await.insert(id, tx);
        rx
    }

    async fn outcome(rx: oneshot::Receiver<RouteOutcome>) -> Result<RouteOutcome> {
        Ok(tokio::time::timeout(Duration::from_secs(5), rx)
            .await
            .context("the waiter must resolve")??)
    }

    async fn feed_with_route_waiters(
        state: &Arc<Mutex<DeviceState>>,
        route_waiters: &RouteWaiters,
        packet: meshtastic::protobufs::FromRadio,
    ) -> Result<()> {
        process_from_radio_packet(
            packet,
            state.clone(),
            Arc::new(Mutex::new(HashMap::new())),
            route_waiters.clone(),
            Arc::new(Mutex::new(None)),
        )
        .await
    }

    /// The firmware answers on the traceroute port with a bare RouteDiscovery naming our
    /// packet id. Each path runs endpoint to endpoint, SNR is in quarter-dB, and a hop that
    /// did not record itself is listed as the broadcast address with SNR `i8::MIN`.
    #[tokio::test]
    async fn traceroute_reply_resolves_the_waiter_with_both_paths() -> Result<()> {
        let state = Arc::new(Mutex::new(DeviceState::new()));
        state.lock().await.nodes.insert(
            0xa0cce0c0,
            NodeInfo {
                id: "a0cce0c0".to_string(),
                num: 0xa0cce0c0,
                user: User {
                    id: "!a0cce0c0".to_string(),
                    long_name: "Relay".to_string(),
                    short_name: "RL".to_string(),
                    hw_model: None,
                },
                last_heard: None,
                last_heard_iso: None,
                snr: None,
                rssi: None,
            },
        );
        let route_waiters: RouteWaiters = Arc::new(Mutex::new(HashMap::new()));
        let rx = route_waiter(&route_waiters, 42).await;

        let reply = meshtastic::protobufs::RouteDiscovery {
            route: vec![0xa0cce0c0, u32::MAX],
            snr_towards: vec![24, i32::from(i8::MIN), 41],
            route_back: vec![0xa0cce0c0],
            snr_back: vec![10, 43],
        };
        feed_with_route_waiters(
            &state,
            &route_waiters,
            mesh_packet(
                0x7e9bb193,
                3,
                meshtastic::protobufs::PortNum::TracerouteApp,
                42,
                reply.encode_to_vec(),
            ),
        )
        .await?;

        let route = outcome(rx).await?.map_err(anyhow::Error::msg)?;
        let summary = |hops: &[crate::mesh::RouteHop]| {
            hops.iter()
                .map(|h| (h.hop_number, h.node_id, h.node_name.clone(), h.snr))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            summary(&route.towards),
            vec![
                (0, 0x5c15c784, "Unknown".to_string(), None),
                (1, 0xa0cce0c0, "Relay".to_string(), Some(6.0)),
                (2, u32::MAX, "Unknown".to_string(), None),
                (3, 0x7e9bb193, "Unknown".to_string(), Some(10.25)),
            ]
        );
        assert_eq!(
            summary(&route.back.context("the way back was recorded")?),
            vec![
                (0, 0x7e9bb193, "Unknown".to_string(), None),
                (1, 0xa0cce0c0, "Relay".to_string(), Some(2.5)),
                (2, 0x5c15c784, "Unknown".to_string(), Some(10.75)),
            ]
        );
        Ok(())
    }

    /// A direct neighbour answers with no hops in between; both endpoints still make a path.
    #[test]
    fn a_direct_neighbour_is_a_two_node_route() -> Result<()> {
        let reply = meshtastic::protobufs::RouteDiscovery {
            snr_towards: vec![25],
            snr_back: vec![39],
            ..Default::default()
        };
        let route = traceroute_result(&reply, 0x5c15c784, 0x7e9bb193, 3, &HashMap::new());
        let pairs = |hops: &[crate::mesh::RouteHop]| {
            hops.iter().map(|h| (h.node_id, h.snr)).collect::<Vec<_>>()
        };
        assert_eq!(
            pairs(&route.towards),
            vec![(0x5c15c784, None), (0x7e9bb193, Some(6.25))]
        );
        assert_eq!(
            pairs(&route.back.context("the way back was recorded")?),
            vec![(0x7e9bb193, None), (0x5c15c784, Some(9.75))]
        );
        Ok(())
    }

    /// Without a hop_start the firmware cannot fill in hops that did not record themselves,
    /// so the way back may be silently short; like the reference client, leave it out.
    #[test]
    fn no_hop_start_means_no_way_back() -> Result<()> {
        let reply = meshtastic::protobufs::RouteDiscovery {
            snr_towards: vec![25],
            snr_back: vec![39],
            ..Default::default()
        };
        let route = traceroute_result(&reply, 0x5c15c784, 0x7e9bb193, 0, &HashMap::new());
        assert!(route.back.is_none());
        Ok(())
    }

    /// A routing ACK (error NONE) for the request is not a failure: the reply may still be
    /// on its way. A real routing error ends the wait with its reason.
    #[tokio::test]
    async fn a_routing_ack_does_not_end_a_traceroute_but_an_error_does() -> Result<()> {
        let state = Arc::new(Mutex::new(DeviceState::new()));
        let route_waiters: RouteWaiters = Arc::new(Mutex::new(HashMap::new()));
        let rx = route_waiter(&route_waiters, 42).await;

        feed_with_route_waiters(
            &state,
            &route_waiters,
            routing_error(42, meshtastic::protobufs::routing::Error::None),
        )
        .await?;
        assert!(
            route_waiters.lock().await.contains_key(&42),
            "an ACK must leave the traceroute waiting"
        );

        feed_with_route_waiters(
            &state,
            &route_waiters,
            routing_error(42, meshtastic::protobufs::routing::Error::NoResponse),
        )
        .await?;
        assert_eq!(outcome(rx).await?.err().as_deref(), Some("NO_RESPONSE"));
        Ok(())
    }

    /// The radio refuses a second traceroute within 30 s with a ClientNotification naming
    /// the request, not a routing error. That must end the wait with the radio's reason,
    /// not a timeout that blames the destination.
    #[tokio::test]
    async fn a_refused_traceroute_ends_with_the_radios_reason() -> Result<()> {
        let state = Arc::new(Mutex::new(DeviceState::new()));
        let route_waiters: RouteWaiters = Arc::new(Mutex::new(HashMap::new()));
        let rx = route_waiter(&route_waiters, 42).await;

        feed_with_route_waiters(
            &state,
            &route_waiters,
            meshtastic::protobufs::FromRadio {
                id: 0,
                payload_variant: Some(
                    meshtastic::protobufs::from_radio::PayloadVariant::ClientNotification(
                        meshtastic::protobufs::ClientNotification {
                            reply_id: Some(42),
                            message: "TraceRoute can only be sent once every 30 seconds"
                                .to_string(),
                            ..Default::default()
                        },
                    ),
                ),
            },
        )
        .await?;
        assert_eq!(
            outcome(rx).await?.err().as_deref(),
            Some("TraceRoute can only be sent once every 30 seconds")
        );
        Ok(())
    }
}
