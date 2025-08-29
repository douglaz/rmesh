use crate::connection::ConnectionManager;
use crate::state::Position;
use anyhow::Result;
use meshtastic::packet::{PacketDestination, PacketReceiver};
use meshtastic::protobufs;
use meshtastic::Message as ProstMessage;
use tokio::time::{timeout, Duration};
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
            .unwrap_or_default()
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

    debug!(
        "Position set to {}, {}, alt: {:?}",
        latitude, longitude, altitude
    );
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

    // Ignore timeout error - it's expected
    let _ = result;

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
        _packet: protobufs::FromRadio,
    ) -> Result<(), std::convert::Infallible> {
        Ok(())
    }

    fn handle_mesh_packet(
        &mut self,
        _packet: protobufs::MeshPacket,
    ) -> Result<(), std::convert::Infallible> {
        Ok(())
    }

    fn source_node_id(&self) -> NodeId {
        0u32.into() // Default node ID for simple router
    }
}
