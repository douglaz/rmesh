use anyhow::{Context, Result, bail};
use meshtastic::Message as ProstMessage;
use meshtastic::api::state::Configured;
use meshtastic::api::{ConnectedStreamApi, StreamApi};
use meshtastic::packet::{PacketReceiver, PacketRouter};
use meshtastic::utils;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, oneshot};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use crate::state::{
    AirQualityMetrics, BluetoothConfig, ChannelInfo, DeviceConfig, DeviceMetrics, DeviceState,
    DisplayConfig, EnvironmentMetrics, LoraConfig, MyNodeInfo, NetworkConfig, NodeInfo, Position,
    PositionConfig, PowerConfig, TelemetryData, TextMessage, User,
};

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

pub struct ConnectionManager {
    port: Option<String>,
    ble: Option<String>,
    #[allow(dead_code)] // Will be used for connection timeouts in the future
    timeout: Duration,
    api: Option<ConnectedStreamApi<Configured>>,
    packet_receiver: Option<PacketReceiver>,
    device_state: Arc<Mutex<DeviceState>>,
    packet_processor: Option<JoinHandle<()>>,
    ack_waiters: Arc<Mutex<HashMap<u32, oneshot::Sender<bool>>>>,
    route_waiters: Arc<Mutex<HashMap<u32, oneshot::Sender<Vec<crate::mesh::RouteHop>>>>>,
}

impl ConnectionManager {
    pub async fn new(port: Option<String>, ble: Option<String>, timeout: Duration) -> Result<Self> {
        Ok(Self {
            port,
            ble,
            timeout,
            api: None,
            packet_receiver: None,
            device_state: Arc::new(Mutex::new(DeviceState::new())),
            packet_processor: None,
            ack_waiters: Arc::new(Mutex::new(HashMap::new())),
            route_waiters: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn connect(&mut self) -> Result<()> {
        info!("Establishing connection to Meshtastic device...");

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
                info!("Connecting via TCP to {}", port);
                let stream = utils::stream::build_tcp_stream(port.clone())
                    .await
                    .context("Failed to connect via TCP")?;
                stream_api.connect(stream).await
            } else {
                // Serial connection
                info!("Connecting via serial port {}", port);
                let stream = utils::stream::build_serial_stream(
                    port.clone(),
                    None, // Use default baud rate
                    None, // Use default DTR
                    None, // Use default RTS
                )
                .context("Failed to connect via serial")?;
                stream_api.connect(stream).await
            }
        } else {
            // Auto-detect serial port
            info!("Auto-detecting serial port...");
            let ports =
                utils::stream::available_serial_ports().context("Failed to list serial ports")?;

            if ports.is_empty() {
                bail!("No serial ports found. Please specify --port or --ble");
            }

            let port_name = ports[0].clone();
            info!("Using auto-detected port: {}", port_name);

            let stream = utils::stream::build_serial_stream(
                port_name, None, // Use default baud rate
                None, // Use default DTR
                None, // Use default RTS
            )
            .context("Failed to connect to auto-detected serial port")?;
            stream_api.connect(stream).await
        };

        // Configure the connection
        info!("Configuring connection...");
        let config_id = utils::generate_rand_id();
        let configured_api = connected_api
            .configure(config_id)
            .await
            .context("Failed to configure connection")?;

        // Store the configured API
        self.api = Some(configured_api);

        // Start packet processing
        self.start_packet_processing(packet_receiver).await;

        info!("Connection established and configured successfully");
        Ok(())
    }

    async fn start_packet_processing(&mut self, mut receiver: PacketReceiver) {
        let device_state = self.device_state.clone();
        let ack_waiters = self.ack_waiters.clone();
        let route_waiters = self.route_waiters.clone();

        // Spawn a background task to process packets
        let handle = tokio::spawn(async move {
            info!("Starting packet processing loop");

            while let Some(packet) = receiver.recv().await {
                if let Err(e) = process_from_radio_packet(
                    packet,
                    device_state.clone(),
                    ack_waiters.clone(),
                    route_waiters.clone(),
                )
                .await
                {
                    warn!("Error processing packet: {}", e);
                }
            }

            info!("Packet processing loop ended");
        });

        self.packet_processor = Some(handle);

        // Give the processor a moment to start receiving initial packets
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    pub fn is_connected(&self) -> bool {
        self.api.is_some()
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(processor) = self.packet_processor.take() {
            processor.abort();
        }

        if let Some(api) = self.api.take() {
            api.disconnect().await?;
        }

        Ok(())
    }

    pub fn get_api(&mut self) -> Result<&mut ConnectedStreamApi<Configured>> {
        self.api.as_mut().context("Not connected")
    }

    pub async fn get_device_state(&self) -> DeviceState {
        self.device_state.lock().await.clone()
    }

    pub fn get_device_state_ref(&self) -> Arc<Mutex<DeviceState>> {
        self.device_state.clone()
    }

    pub fn take_packet_receiver(&mut self) -> Result<PacketReceiver> {
        self.packet_receiver
            .take()
            .context("Packet receiver already taken or not connected")
    }

    pub async fn send_traceroute(
        &mut self,
        destination: u32,
        timeout_secs: u64,
    ) -> Result<Vec<crate::mesh::RouteHop>> {
        // Generate a unique request ID for tracking
        let request_id = rand::random::<u32>();

        // Create a oneshot channel for route response
        let (tx, rx) = oneshot::channel();

        // Register the route waiter
        {
            let mut waiters = self.route_waiters.lock().await;
            waiters.insert(request_id, tx);
        }

        // Create the RouteDiscovery packet
        let route_discovery = meshtastic::protobufs::RouteDiscovery {
            route: Vec::new(),
            route_back: Vec::new(),
            snr_back: Vec::new(),
            snr_towards: Vec::new(),
        };

        let routing_packet = meshtastic::protobufs::Routing {
            variant: Some(meshtastic::protobufs::routing::Variant::RouteRequest(
                route_discovery,
            )),
        };

        let payload = routing_packet.encode_to_vec();

        // Create mesh packet for traceroute
        let mesh_packet = meshtastic::protobufs::MeshPacket {
            payload_variant: Some(meshtastic::protobufs::mesh_packet::PayloadVariant::Decoded(
                meshtastic::protobufs::Data {
                    portnum: meshtastic::protobufs::PortNum::TracerouteApp as i32,
                    payload,
                    want_response: true,
                    dest: 0,
                    source: 0,
                    request_id,
                    reply_id: 0,
                    emoji: 0,
                    bitfield: Some(0),
                },
            )),
            from: 0,
            to: destination,
            id: request_id,
            rx_time: 0,
            rx_snr: 0.0,
            hop_limit: 7,
            want_ack: false,
            priority: meshtastic::protobufs::mesh_packet::Priority::Reliable as i32,
            rx_rssi: 0,
            via_mqtt: false,
            hop_start: 7,
            ..Default::default()
        };

        // Send the traceroute packet
        let api = self.get_api()?;
        api.send_to_radio_packet(Some(
            meshtastic::protobufs::to_radio::PayloadVariant::Packet(mesh_packet),
        ))
        .await?;

        debug!(
            "Sent traceroute to {:08x} with request ID {}",
            destination, request_id
        );

        // Wait for route response with timeout
        let timeout = tokio::time::timeout(Duration::from_secs(timeout_secs), rx).await;

        // Clean up the waiter if timeout occurred
        if timeout.is_err() {
            let mut waiters = self.route_waiters.lock().await;
            waiters.remove(&request_id);
            debug!("Traceroute timeout for request {}", request_id);
            return Ok(Vec::new());
        }

        // Return the route hops
        Ok(timeout.unwrap().unwrap_or_else(|_| Vec::new()))
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

        debug!("Sent message with ID {} and ACK request", packet_id);

        // Wait for ACK with timeout
        let timeout = tokio::time::timeout(Duration::from_secs(timeout_secs), rx).await;

        // Clean up the waiter if timeout occurred
        if timeout.is_err() {
            let mut waiters = self.ack_waiters.lock().await;
            waiters.remove(&packet_id);
            debug!("ACK timeout for packet {}", packet_id);
            return Ok(false);
        }

        // Return whether ACK was received
        Ok(timeout.unwrap().unwrap_or(false))
    }
}

async fn process_from_radio_packet(
    from_radio: meshtastic::protobufs::FromRadio,
    device_state: Arc<Mutex<DeviceState>>,
    ack_waiters: Arc<Mutex<HashMap<u32, oneshot::Sender<bool>>>>,
    route_waiters: Arc<Mutex<HashMap<u32, oneshot::Sender<Vec<crate::mesh::RouteHop>>>>>,
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
                node_id: format!("{:08x}", my_info.my_node_num),
                reboot_count: my_info.reboot_count,
                min_app_version: my_info.min_app_version,
                device_id: hex::encode(my_info.device_id),
            });
            debug!("Updated my node info");
        }

        meshtastic::protobufs::from_radio::PayloadVariant::NodeInfo(node_info) => {
            let mut state = device_state.lock().await;
            let user = node_info.user.clone().unwrap_or_default();
            state.update_node(
                node_info.num,
                NodeInfo {
                    id: format!("{:08x}", node_info.num),
                    num: node_info.num,
                    user: User {
                        id: user.id.clone(),
                        long_name: user.long_name.clone(),
                        short_name: user.short_name.clone(),
                        hw_model: Some(format!("{:?}", user.hw_model())),
                    },
                    last_heard: Some(node_info.last_heard as u64),
                    snr: Some(node_info.snr),
                    rssi: Some(0), // NodeInfo doesn't have RSSI
                },
            );
            debug!("Updated node info for {}", node_info.num);
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
                role: format!("{:?}", channel.role()),
                has_psk: channel
                    .settings
                    .as_ref()
                    .map(|s| !s.psk.is_empty())
                    .unwrap_or(false),
                settings: channel.settings,
            });
            debug!("Updated channel {}", channel.index);
        }

        meshtastic::protobufs::from_radio::PayloadVariant::Packet(mesh_packet) => {
            process_mesh_packet(mesh_packet, device_state, ack_waiters, route_waiters).await?;
        }

        _ => {
            // Other packet types not yet handled
        }
    }

    Ok(())
}

async fn process_mesh_packet(
    mesh_packet: meshtastic::protobufs::MeshPacket,
    device_state: Arc<Mutex<DeviceState>>,
    ack_waiters: Arc<Mutex<HashMap<u32, oneshot::Sender<bool>>>>,
    route_waiters: Arc<Mutex<HashMap<u32, oneshot::Sender<Vec<crate::mesh::RouteHop>>>>>,
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
                from: format!("{:08x}", mesh_packet.from),
                from_node: mesh_packet.from,
                to: format!("{:08x}", mesh_packet.to),
                to_node: mesh_packet.to,
                channel: mesh_packet.channel,
                text,
                time: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                snr: Some(mesh_packet.rx_snr),
                rssi: Some(mesh_packet.rx_rssi),
                acknowledged: false,
            });
            debug!("Received text message from {:08x}", mesh_packet.from);
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
                            node_id: format!("{:08x}", mesh_packet.from),
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
                                .unwrap()
                                .as_secs(),
                        },
                    );
                    debug!("Updated position for {:08x}", mesh_packet.from);
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
                        _ => {
                            // Other telemetry types not yet handled
                        }
                    }
                }

                state.update_telemetry(mesh_packet.from, telemetry_data);
                debug!("Updated telemetry for {:08x}", mesh_packet.from);
            }
        }

        meshtastic::protobufs::PortNum::AdminApp => {
            if let Ok(admin_msg) =
                meshtastic::protobufs::AdminMessage::decode(packet_data.payload.as_slice())
                && let Some(
                    meshtastic::protobufs::admin_message::PayloadVariant::GetConfigResponse(config),
                ) = admin_msg.payload_variant
            {
                process_config_response(config, device_state).await?;
            }
        }

        meshtastic::protobufs::PortNum::RoutingApp => {
            // Handle routing packets (including ACKs and route replies)
            if let Ok(routing) =
                meshtastic::protobufs::Routing::decode(packet_data.payload.as_slice())
                && let Some(variant) = routing.variant
            {
                match variant {
                    meshtastic::protobufs::routing::Variant::RouteReply(route) => {
                        debug!("Received route reply with {} hops", route.route.len());

                        // Check if this is a response to a traceroute request
                        if packet_data.request_id != 0 {
                            let mut waiters = route_waiters.lock().await;
                            if let Some(sender) = waiters.remove(&packet_data.request_id) {
                                // Convert route to RouteHop structure
                                let mut hops = Vec::new();
                                for (idx, node_num) in route.route.iter().enumerate() {
                                    // Look up node info from state
                                    let state = device_state.lock().await;
                                    let node_name = state
                                        .nodes
                                        .get(node_num)
                                        .map(|n| n.user.long_name.clone())
                                        .unwrap_or_else(|| format!("Unknown ({:08x})", node_num));

                                    hops.push(crate::mesh::RouteHop {
                                        node_id: *node_num,
                                        node_name,
                                        hop_number: idx as u32,
                                        snr: None,  // Route replies don't include SNR
                                        rssi: None, // Route replies don't include RSSI
                                    });
                                }

                                let _ = sender.send(hops);
                                debug!("Sent route reply for request {}", packet_data.request_id);
                            }
                        }
                    }
                    meshtastic::protobufs::routing::Variant::ErrorReason(reason) => {
                        debug!("Routing error: {:?}", reason);
                        // If this is an error for a traceroute request, send empty result
                        if packet_data.request_id != 0 {
                            let mut waiters = route_waiters.lock().await;
                            if let Some(sender) = waiters.remove(&packet_data.request_id) {
                                let _ = sender.send(Vec::new());
                                debug!(
                                    "Route request {} failed: {:?}",
                                    packet_data.request_id, reason
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }

            // Check if this is an ACK by looking at the request_id
            if packet_data.request_id != 0 {
                let mut waiters = ack_waiters.lock().await;
                if let Some(sender) = waiters.remove(&packet_data.request_id) {
                    let _ = sender.send(true);
                    debug!("Received ACK for packet {}", packet_data.request_id);
                }
            }
        }

        _ => {
            // Other port types not yet handled
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
                let _ = sender.send(true);
                debug!("Received implicit ACK for packet {}", data.request_id);
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
                state.device_config = Some(DeviceConfig {
                    role: format!("{:?}", device_config.role()),
                    button_gpio: device_config.button_gpio,
                    buzzer_gpio: device_config.buzzer_gpio,
                    rebroadcast_mode: format!("{:?}", device_config.rebroadcast_mode()),
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
                    gps_mode: format!("{:?}", position_config.gps_mode()),
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
                    ipv4_config: if network_config.ipv4_config.is_some() {
                        Some(format!("{:?}", network_config.ipv4_config.unwrap()))
                    } else {
                        None
                    },
                });
                debug!("Updated network config");
            }
            meshtastic::protobufs::config::PayloadVariant::Display(display_config) => {
                state.display_config = Some(DisplayConfig {
                    screen_on_secs: display_config.screen_on_secs,
                    gps_format: format!("{:?}", display_config.gps_format()),
                    auto_screen_carousel_secs: display_config.auto_screen_carousel_secs,
                    compass_north_top: display_config.compass_north_top,
                    flip_screen: display_config.flip_screen,
                    units: format!("{:?}", display_config.units()),
                    displaymode: format!("{:?}", display_config.displaymode()),
                    heading_bold: display_config.heading_bold,
                    wake_on_tap_or_motion: display_config.wake_on_tap_or_motion,
                });
                debug!("Updated display config");
            }
            meshtastic::protobufs::config::PayloadVariant::Lora(lora_config) => {
                state.lora_config = Some(LoraConfig {
                    use_preset: lora_config.use_preset,
                    modem_preset: format!("{:?}", lora_config.modem_preset()),
                    bandwidth: lora_config.bandwidth,
                    spread_factor: lora_config.spread_factor,
                    coding_rate: lora_config.coding_rate,
                    frequency_offset: lora_config.frequency_offset,
                    region: format!("{:?}", lora_config.region()),
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
                    mode: format!("{:?}", bluetooth_config.mode()),
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
