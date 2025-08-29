use crate::connection::ConnectionManager;
use anyhow::Result;
use meshtastic::{Message, protobufs};
use serde::Serialize;

/// List all channels configured on the device
pub async fn list_channels(connection: &ConnectionManager) -> Result<Vec<ChannelInfo>> {
    // Get cached channels from device state
    let state = connection.get_device_state().await;

    // Convert from internal ChannelInfo to public ChannelInfo
    let channels: Vec<ChannelInfo> = state
        .channels
        .into_iter()
        .map(|ch| ChannelInfo {
            index: ch.index,
            name: ch.name,
            role: ch.role,
            has_psk: ch.has_psk,
        })
        .collect();

    Ok(channels)
}

/// Add a new channel
pub async fn add_channel(
    connection: &mut ConnectionManager,
    name: &str,
    psk: Option<&str>,
) -> Result<()> {
    let api = connection.get_api()?;

    // Create channel settings
    let mut settings = protobufs::ChannelSettings {
        name: name.to_string(),
        ..Default::default()
    };

    // Set pre-shared key if provided
    if let Some(key) = psk {
        settings.psk = key.as_bytes().to_vec();
    }

    // Create admin message for channel add
    let admin_msg = protobufs::AdminMessage {
        payload_variant: Some(protobufs::admin_message::PayloadVariant::SetChannel(
            protobufs::Channel {
                index: 0, // Will be assigned by device
                settings: Some(settings),
                role: protobufs::channel::Role::Primary as i32,
            },
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
        to: 0,
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

/// Delete a channel
pub async fn delete_channel(connection: &mut ConnectionManager, index: u32) -> Result<()> {
    let api = connection.get_api()?;

    // Create admin message for channel delete
    let admin_msg = protobufs::AdminMessage {
        payload_variant: Some(protobufs::admin_message::PayloadVariant::RemoveByNodenum(
            index,
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
        to: 0,
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

/// Set channel configuration
pub async fn set_channel(
    connection: &mut ConnectionManager,
    index: u32,
    name: Option<&str>,
    psk: Option<&str>,
) -> Result<()> {
    let api = connection.get_api()?;

    // Create channel settings
    let mut settings = protobufs::ChannelSettings::default();

    if let Some(n) = name {
        settings.name = n.to_string();
    }

    if let Some(key) = psk {
        settings.psk = key.as_bytes().to_vec();
    }

    // Create admin message for channel set
    let admin_msg = protobufs::AdminMessage {
        payload_variant: Some(protobufs::admin_message::PayloadVariant::SetChannel(
            protobufs::Channel {
                index: index as i32,
                settings: Some(settings),
                role: protobufs::channel::Role::Primary as i32,
            },
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
        to: 0,
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

#[derive(Debug, Clone, Serialize)]
pub struct ChannelInfo {
    pub index: u32,
    pub name: String,
    pub role: String,
    pub has_psk: bool,
}
