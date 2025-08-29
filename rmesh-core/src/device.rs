use crate::connection::ConnectionManager;
use anyhow::Result;
use meshtastic::{Message, protobufs};

/// Reboot the connected Meshtastic device
///
/// # Arguments
/// * `connection` - Active connection to the device
/// * `delay_seconds` - Seconds to wait before rebooting (default: 5)
pub async fn reboot_device(
    connection: &mut ConnectionManager,
    delay_seconds: Option<i32>,
) -> Result<()> {
    let api = connection.get_api()?;
    let delay = delay_seconds.unwrap_or(5);

    // Create admin message for reboot
    let admin_msg = protobufs::AdminMessage {
        payload_variant: Some(protobufs::admin_message::PayloadVariant::RebootSeconds(
            delay,
        )),
        session_passkey: Vec::new(),
    };

    // Create mesh packet
    let mesh_packet = protobufs::MeshPacket {
        payload_variant: Some(protobufs::mesh_packet::PayloadVariant::Decoded(
            protobufs::Data {
                portnum: protobufs::PortNum::AdminApp as i32,
                payload: admin_msg.encode_to_vec(),
                ..Default::default()
            },
        )),
        from: 0,
        to: 0, // Local destination
        id: 0,
        rx_time: 0,
        rx_snr: 0.0,
        hop_limit: 0,
        want_ack: false,
        priority: protobufs::mesh_packet::Priority::Default as i32,
        rx_rssi: 0,
        via_mqtt: false,
        hop_start: 0,
        ..Default::default()
    };

    // Send as ToRadio packet
    api.send_to_radio_packet(Some(protobufs::to_radio::PayloadVariant::Packet(
        mesh_packet,
    )))
    .await?;

    Ok(())
}

/// Factory reset the connected Meshtastic device
///
/// # Warning
/// This will erase all device settings and cannot be undone!
pub async fn factory_reset_device(connection: &mut ConnectionManager) -> Result<()> {
    let api = connection.get_api()?;

    // Create admin message for factory reset
    let admin_msg = protobufs::AdminMessage {
        payload_variant: Some(protobufs::admin_message::PayloadVariant::FactoryResetDevice(1)),
        session_passkey: Vec::new(),
    };

    // Create mesh packet
    let mesh_packet = protobufs::MeshPacket {
        payload_variant: Some(protobufs::mesh_packet::PayloadVariant::Decoded(
            protobufs::Data {
                portnum: protobufs::PortNum::AdminApp as i32,
                payload: admin_msg.encode_to_vec(),
                ..Default::default()
            },
        )),
        from: 0,
        to: 0, // Local destination
        id: 0,
        rx_time: 0,
        rx_snr: 0.0,
        hop_limit: 0,
        want_ack: false,
        priority: protobufs::mesh_packet::Priority::Default as i32,
        rx_rssi: 0,
        via_mqtt: false,
        hop_start: 0,
        ..Default::default()
    };

    // Send as ToRadio packet
    api.send_to_radio_packet(Some(protobufs::to_radio::PayloadVariant::Packet(
        mesh_packet,
    )))
    .await?;

    Ok(())
}

/// Shutdown the connected Meshtastic device
///
/// # Arguments
/// * `connection` - Active connection to the device
/// * `delay_seconds` - Seconds to wait before shutdown (default: 5)
pub async fn shutdown_device(
    connection: &mut ConnectionManager,
    delay_seconds: Option<i32>,
) -> Result<()> {
    let api = connection.get_api()?;
    let delay = delay_seconds.unwrap_or(5);

    // Create admin message for shutdown
    let admin_msg = protobufs::AdminMessage {
        payload_variant: Some(protobufs::admin_message::PayloadVariant::ShutdownSeconds(
            delay,
        )),
        session_passkey: Vec::new(),
    };

    // Create mesh packet
    let mesh_packet = protobufs::MeshPacket {
        payload_variant: Some(protobufs::mesh_packet::PayloadVariant::Decoded(
            protobufs::Data {
                portnum: protobufs::PortNum::AdminApp as i32,
                payload: admin_msg.encode_to_vec(),
                ..Default::default()
            },
        )),
        from: 0,
        to: 0, // Local destination
        id: 0,
        rx_time: 0,
        rx_snr: 0.0,
        hop_limit: 0,
        want_ack: false,
        priority: protobufs::mesh_packet::Priority::Default as i32,
        rx_rssi: 0,
        via_mqtt: false,
        hop_start: 0,
        ..Default::default()
    };

    // Send as ToRadio packet
    api.send_to_radio_packet(Some(protobufs::to_radio::PayloadVariant::Packet(
        mesh_packet,
    )))
    .await?;

    Ok(())
}
