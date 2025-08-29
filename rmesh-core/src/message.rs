use crate::connection::ConnectionManager;
use anyhow::Result;
use meshtastic::packet::{PacketDestination, PacketReceiver};
use meshtastic::protobufs;
use serde::Serialize;
use tokio::time::{Duration, timeout};
use tracing::debug;

/// Send a text message to the mesh network
pub async fn send_text_message(
    connection: &mut ConnectionManager,
    text: &str,
    destination: Option<u32>,
    channel: u32,
    want_ack: bool,
) -> Result<()> {
    let api = connection.get_api()?;

    // Determine destination
    let dest = match destination {
        Some(node_num) => PacketDestination::Node(node_num.into()),
        None => PacketDestination::Broadcast,
    };

    // Create a simple packet router that ignores packets
    let mut packet_router = SimplePacketRouter;

    // Send the text message
    api.send_text(
        &mut packet_router,
        text.to_string(),
        dest,
        want_ack,
        channel.into(),
    )
    .await?;

    debug!("Text message sent to {dest:?} on channel {channel}");
    Ok(())
}

/// Receive messages from the mesh network
pub async fn receive_messages(
    receiver: &mut PacketReceiver,
    from_node: Option<u32>,
    count: Option<usize>,
    timeout_secs: u64,
) -> Result<Vec<ReceivedMessage>> {
    let mut messages = Vec::new();
    let timeout_duration = Duration::from_secs(timeout_secs);
    let target_count = count.unwrap_or(usize::MAX);

    // Receive messages until timeout or count reached
    let result = timeout(timeout_duration, async {
        while messages.len() < target_count {
            if let Some(packet) = receiver.recv().await {
                if let Some(msg) = process_packet_for_message(packet, from_node) {
                    messages.push(msg);
                }
            } else {
                break; // Channel closed
            }
        }
    })
    .await;

    // Ignore timeout error - it's expected
    let _ = result;

    Ok(messages)
}

/// Monitor messages in real-time
pub async fn monitor_messages<F>(
    receiver: &mut PacketReceiver,
    from_node: Option<u32>,
    mut callback: F,
) -> Result<()>
where
    F: FnMut(ReceivedMessage) -> Result<()>,
{
    while let Some(packet) = receiver.recv().await {
        if let Some(msg) = process_packet_for_message(packet, from_node) {
            callback(msg)?;
        }
    }

    Ok(())
}

fn process_packet_for_message(
    from_radio: protobufs::FromRadio,
    from_node_filter: Option<u32>,
) -> Option<ReceivedMessage> {
    // Check if this is a mesh packet
    let mesh_packet = match from_radio.payload_variant? {
        protobufs::from_radio::PayloadVariant::Packet(p) => p,
        _ => return None,
    };

    // Apply from_node filter if specified
    if let Some(filter) = from_node_filter
        && mesh_packet.from != filter
    {
        return None;
    }

    // Check if it's a decoded packet
    let data = match mesh_packet.payload_variant? {
        protobufs::mesh_packet::PayloadVariant::Decoded(d) => d,
        _ => return None,
    };

    // Check if it's a text message
    if data.portnum() != protobufs::PortNum::TextMessageApp {
        return None;
    }

    // Parse text from payload
    let text = String::from_utf8_lossy(&data.payload).to_string();

    Some(ReceivedMessage {
        from: format!("{:08x}", mesh_packet.from),
        from_node: mesh_packet.from,
        to: format!("{:08x}", mesh_packet.to),
        to_node: mesh_packet.to,
        channel: mesh_packet.channel,
        text,
        snr: Some(mesh_packet.rx_snr),
        rssi: Some(mesh_packet.rx_rssi),
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct ReceivedMessage {
    pub from: String,
    pub from_node: u32,
    pub to: String,
    pub to_node: u32,
    pub channel: u32,
    pub text: String,
    pub snr: Option<f32>,
    pub rssi: Option<i32>,
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
