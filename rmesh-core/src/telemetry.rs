use crate::connection::ConnectionManager;
use crate::state::DeviceMetrics;
use anyhow::Result;
use meshtastic::Message;
use meshtastic::packet::PacketDestination;
use meshtastic::protobufs;
use meshtastic::types::EncodedMeshPacketData;
use serde::Serialize;
use tokio::time::{Duration, sleep};
use tracing::{debug, info};

/// Request telemetry from the local device
pub async fn request_device_telemetry(connection: &mut ConnectionManager) -> Result<()> {
    // Get local node number
    let state = connection.get_device_state().await;
    let local_node_num = match &state.my_node_info {
        Some(info) => info.node_num,
        None => {
            debug!("No local node information available");
            return Ok(());
        }
    };

    // Create an empty telemetry packet to request telemetry
    let telemetry = protobufs::Telemetry::default();

    // Create a simple packet router
    let mut packet_router = SimplePacketRouter;

    // Get API and send telemetry request with wantResponse flag
    let api = connection.get_api()?;

    // Encode telemetry to bytes
    let byte_data: EncodedMeshPacketData = telemetry.encode_to_vec().into();

    // Send mesh packet to self with want_response set to true
    api.send_mesh_packet(
        &mut packet_router,
        byte_data,
        protobufs::PortNum::TelemetryApp,
        PacketDestination::Node(local_node_num.into()),
        0.into(), // primary channel
        false,    // want_ack
        true,     // want_response - request telemetry
        false,    // echo_response
        None,     // reply_id
        None,     // emoji
    )
    .await?;

    info!("Sent telemetry request to local device");
    Ok(())
}

/// Collect telemetry data for a specified duration
pub async fn collect_telemetry(
    connection: &mut ConnectionManager,
    wait_seconds: u64,
) -> Result<Option<DeviceMetrics>> {
    info!("Collecting telemetry broadcasts for {wait_seconds} seconds...");

    // Get local node number
    let state = connection.get_device_state().await;
    let local_node_num = match &state.my_node_info {
        Some(info) => info.node_num,
        None => {
            debug!("No local node information available");
            return Ok(None);
        }
    };

    // Record initial state
    let initial_metrics = state
        .telemetry
        .get(&local_node_num)
        .and_then(|t| t.device_metrics.clone());
    let initial_time = initial_metrics.as_ref().map(|_| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    });

    // Poll for new telemetry during the wait period
    let start_time = std::time::Instant::now();
    let timeout_duration = Duration::from_secs(wait_seconds);

    while start_time.elapsed() < timeout_duration {
        // Get current state
        let state = connection.get_device_state().await;

        // Check for new or updated telemetry
        if let Some(telemetry) = state.telemetry.get(&local_node_num) {
            if let Some(metrics) = &telemetry.device_metrics {
                // Check if this is newer than what we started with
                if initial_time.is_none() || telemetry.time > initial_time.unwrap() {
                    debug!("Received telemetry update from local device");
                    return Ok(Some(metrics.clone()));
                }
            }
        }

        // Wait a bit before checking again
        sleep(Duration::from_millis(250)).await;
    }

    // Return whatever we have (could be initial metrics or nothing)
    let final_state = connection.get_device_state().await;
    Ok(final_state
        .telemetry
        .get(&local_node_num)
        .and_then(|t| t.device_metrics.clone()))
}

/// Request telemetry from a node (legacy function, kept for compatibility)
pub async fn request_telemetry(
    _connection: &mut ConnectionManager,
    _telemetry_type: TelemetryType,
    _node_id: Option<u32>,
) -> Result<()> {
    // TODO: Implement telemetry request for specific types
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub enum TelemetryType {
    Battery,
    Environment,
    Device,
}

// Simple packet router that ignores all packets
struct SimplePacketRouter;

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
