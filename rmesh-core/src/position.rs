use crate::connection::ConnectionManager;
use crate::state::Position;
use anyhow::{Context, Result};
use meshtastic::Message;
use meshtastic::packet::{PacketDestination, PacketReceiver};
use meshtastic::protobufs;
use meshtastic::types::EncodedMeshPacketData;
use tokio::time::{Duration, timeout};
use tracing::debug;

/// Get position for a specific node
pub async fn get_position(
    connection: &ConnectionManager,
    node_num: Option<u32>,
) -> Result<Option<Position>> {
    let state = connection.get_device_state().await;

    // If node_num is specified, get that node's position
    if let Some(num) = node_num {
        Ok(state.positions.get(&num).cloned())
    } else {
        // Otherwise get our node's position
        if let Some(my_info) = &state.my_node_info {
            Ok(state.positions.get(&my_info.node_num).cloned())
        } else {
            Ok(None)
        }
    }
}

/// Request position from a specific node
pub async fn request_position(
    connection: &mut ConnectionManager,
    node_num: u32,
    timeout_secs: u64,
) -> Result<Option<Position>> {
    // First check if we already have recent position data for this node
    {
        let state = connection.get_device_state().await;
        if let Some(existing_pos) = state.positions.get(&node_num) {
            // If we have position data less than 60 seconds old, return it
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            if current_time - existing_pos.last_updated < 60 {
                debug!("Returning cached position for node {node_num:08x}");
                return Ok(Some(existing_pos.clone()));
            }
        }
    }

    // Create an empty position packet to request position
    let position = protobufs::Position::default();

    // Create a simple packet router
    let mut packet_router = SimplePacketRouter;

    // Get API and send position request with wantResponse flag
    let api = connection.get_api()?;

    // Encode position to bytes
    let byte_data: EncodedMeshPacketData = position.encode_to_vec().into();

    // Send mesh packet directly with want_response set to true
    api.send_mesh_packet(
        &mut packet_router,
        byte_data,
        protobufs::PortNum::PositionApp,
        PacketDestination::Node(node_num.into()),
        0.into(), // primary channel
        false,    // want_ack
        true,     // want_response - THIS IS THE KEY!
        false,    // echo_response
        None,     // reply_id
        None,     // emoji
    )
    .await?;

    debug!("Sent position request to node {node_num:08x} with wantResponse=true");

    // Wait for the response to be processed by the background task
    // We'll poll the device state for updates
    let start_time = std::time::Instant::now();
    let timeout_duration = Duration::from_secs(timeout_secs);

    while start_time.elapsed() < timeout_duration {
        // Check if we've received an update
        {
            let state = connection.get_device_state().await;
            if let Some(pos) = state.positions.get(&node_num) {
                // Check if this position is newer than when we started
                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                if pos.last_updated > (current_time - timeout_secs) {
                    debug!("Received position response from node {node_num:08x}");
                    return Ok(Some(pos.clone()));
                }
            }
        }

        // Wait a bit before checking again
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    debug!("Position request timeout after {timeout_secs} seconds");
    Ok(None)
}

/// Set the position of the connected device
pub async fn set_position(
    connection: &mut ConnectionManager,
    latitude: f64,
    longitude: f64,
    altitude: Option<i32>,
) -> Result<()> {
    let api = connection.get_api()?;

    // Create position protobuf
    let position = protobufs::Position {
        latitude_i: Some((latitude * 1e7) as i32),
        longitude_i: Some((longitude * 1e7) as i32),
        altitude,
        time: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .context("Failed to get system time")?
            .as_secs() as u32,
        ..Default::default()
    };

    // Create a simple packet router
    let mut packet_router = SimplePacketRouter;

    // Send position update
    api.send_position(
        &mut packet_router,
        position,
        PacketDestination::Broadcast,
        true,     // want_ack
        0.into(), // primary channel
    )
    .await?;

    debug!("Position set to {latitude}, {longitude}, alt: {altitude:?}");
    Ok(())
}

/// Track positions from multiple nodes
pub async fn track_positions(
    receiver: &mut PacketReceiver,
    node_filter: Vec<u32>,
    timeout_secs: u64,
) -> Result<Vec<Position>> {
    let mut positions = Vec::new();
    let timeout_duration = Duration::from_secs(timeout_secs);

    // Track positions until timeout
    let result = timeout(timeout_duration, async {
        while let Some(packet) = receiver.recv().await {
            if let Some(pos) = process_packet_for_position(packet, &node_filter) {
                positions.push(pos);
            }
        }
    })
    .await;

    // Handle timeout result
    match result {
        Ok(_) => debug!("Position tracking completed before timeout"),
        Err(_) => debug!("Position tracking timeout after {timeout_secs} seconds"),
    }

    Ok(positions)
}

fn process_packet_for_position(
    from_radio: protobufs::FromRadio,
    node_filter: &[u32],
) -> Option<Position> {
    // Check if this is a mesh packet
    let mesh_packet = match from_radio.payload_variant? {
        protobufs::from_radio::PayloadVariant::Packet(p) => p,
        _ => return None,
    };

    // Apply node filter if not empty
    if !node_filter.is_empty() && !node_filter.contains(&mesh_packet.from) {
        return None;
    }

    // Check if it's a decoded packet
    let data = match mesh_packet.payload_variant? {
        protobufs::mesh_packet::PayloadVariant::Decoded(d) => d,
        _ => return None,
    };

    // Check if it's a position packet
    if data.portnum() != protobufs::PortNum::PositionApp {
        return None;
    }

    // Decode position protobuf
    let position_proto = protobufs::Position::decode(data.payload.as_slice()).ok()?;

    // Convert to our Position type
    let (lat, lon) = (position_proto.latitude_i?, position_proto.longitude_i?);

    Some(Position {
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
    })
}

// Simple packet router that ignores all packets
struct SimplePacketRouter;

// NodeId is exported in meshtastic::types when tokio feature is enabled
use meshtastic::types::NodeId;

impl meshtastic::packet::PacketRouter<(), std::convert::Infallible> for SimplePacketRouter {
    fn handle_packet_from_radio(
        &mut self,
        packet: protobufs::FromRadio,
    ) -> Result<(), std::convert::Infallible> {
        if let Some(variant) = &packet.payload_variant {
            debug!(
                "SimplePacketRouter: Ignoring FromRadio packet (variant: {variant:?})",
                variant = std::mem::discriminant(variant)
            );
        } else {
            debug!("SimplePacketRouter: Ignoring empty FromRadio packet");
        }
        Ok(())
    }

    fn handle_mesh_packet(
        &mut self,
        packet: protobufs::MeshPacket,
    ) -> Result<(), std::convert::Infallible> {
        let portnum = packet.payload_variant.as_ref().and_then(|p| match p {
            protobufs::mesh_packet::PayloadVariant::Decoded(d) => Some(d.portnum()),
            _ => None,
        });

        debug!(
            "SimplePacketRouter: Ignoring MeshPacket (from: {from:08x}, to: {to:08x}, portnum: {portnum:?})",
            from = packet.from,
            to = packet.to
        );
        Ok(())
    }

    fn source_node_id(&self) -> NodeId {
        0u32.into() // Default node ID for simple router
    }
}
