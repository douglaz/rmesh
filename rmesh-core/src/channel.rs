use crate::connection::ConnectionManager;
use anyhow::{Context, Result, bail, ensure};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use meshtastic::{Message, protobufs};
use serde::Serialize;
use tracing::debug;

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
    // Channel indices are explicit: the radio does not allocate them. Writing index 0 with
    // role PRIMARY, as this used to, overwrites the primary channel and its PSK.
    let index = free_channel_index(connection).await?;

    let mut settings = protobufs::ChannelSettings {
        name: name.to_string(),
        ..Default::default()
    };
    if let Some(key) = psk {
        settings.psk = decode_psk(key)?;
    }

    send_channel(
        connection,
        protobufs::Channel {
            index: index as i32,
            settings: Some(settings),
            // Only index 0 may be PRIMARY; everything else has to be SECONDARY or the
            // radio rejects the set.
            role: protobufs::channel::Role::Secondary as i32,
        },
    )
    .await?;

    debug!("Added channel {name} at index {index}");
    Ok(())
}

/// Delete a channel
pub async fn delete_channel(connection: &mut ConnectionManager, index: u32) -> Result<()> {
    // Not RemoveByNodenum, which this used to send: that operates on the NodeDB and would
    // remove whichever node happened to have this number, leaving the channel in place.
    ensure!(
        index != 0,
        "Refusing to delete channel 0: it is the primary channel and the radio needs it. \
         Use `channel set` to change it instead."
    );
    let existing = cached_channel(connection, index).await?;
    ensure!(
        existing.role != "Disabled",
        "Channel {index} is already disabled"
    );

    send_channel(
        connection,
        protobufs::Channel {
            index: index as i32,
            settings: Some(protobufs::ChannelSettings::default()),
            role: protobufs::channel::Role::Disabled as i32,
        },
    )
    .await?;

    debug!("Disabled channel {index}");
    Ok(())
}

/// Set channel configuration
pub async fn set_channel(
    connection: &mut ConnectionManager,
    index: u32,
    name: Option<&str>,
    psk: Option<&str>,
) -> Result<()> {
    // SetChannel replaces the whole channel, so start from what the radio has. Rebuilding
    // from defaults, as this used to, erased the PSK whenever only --name was given.
    let existing = cached_channel(connection, index).await?;
    let mut settings = existing
        .settings
        .clone()
        .context("Radio reported no settings for this channel; reconnect and retry")?;

    if let Some(new_name) = name {
        settings.name = new_name.to_string();
    }
    if let Some(key) = psk {
        settings.psk = decode_psk(key)?;
    }

    // Preserve the role the radio reported rather than forcing PRIMARY.
    let role = match existing.role.as_str() {
        "Primary" => protobufs::channel::Role::Primary,
        "Secondary" => protobufs::channel::Role::Secondary,
        other => bail!("Refusing to modify channel {index}: it is {other}"),
    };

    send_channel(
        connection,
        protobufs::Channel {
            index: index as i32,
            settings: Some(settings),
            role: role as i32,
        },
    )
    .await?;

    debug!("Updated channel {index}");
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelInfo {
    pub index: u32,
    pub name: String,
    pub role: String,
    pub has_psk: bool,
}

/// The channel slots the radio reported, refusing if the dump never arrived — writing a
/// channel without knowing the current layout is how the primary channel gets clobbered.
async fn cached_channels(connection: &ConnectionManager) -> Result<Vec<crate::state::ChannelInfo>> {
    let channels = connection.get_device_state().await.channels;
    ensure!(
        !channels.is_empty(),
        "Radio sent no channel list; reconnect and retry"
    );
    Ok(channels)
}

async fn cached_channel(
    connection: &ConnectionManager,
    index: u32,
) -> Result<crate::state::ChannelInfo> {
    cached_channels(connection)
        .await?
        .into_iter()
        .find(|c| c.index == index)
        .with_context(|| format!("Radio has no channel {index}"))
}

/// The lowest slot the radio is not using.
async fn free_channel_index(connection: &ConnectionManager) -> Result<u32> {
    let channels = cached_channels(connection).await?;
    channels
        .iter()
        .filter(|c| c.role == "Disabled")
        .map(|c| c.index)
        .min()
        .context("Every channel slot is in use; delete one first")
}

/// Decode a PSK the way the reference client does: base64, or `none`/empty to clear it.
fn decode_psk(value: &str) -> Result<Vec<u8>> {
    if value.is_empty() || value.eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    // A bare byte string was accepted before and silently produced an invalid key.
    BASE64
        .decode(value)
        .with_context(|| format!("PSK must be base64 (or `none` to clear): {value}"))
}

/// Send one SetChannel admin message to the attached radio.
async fn send_channel(
    connection: &mut ConnectionManager,
    channel: protobufs::Channel,
) -> Result<()> {
    if let Err(e) = connection.ensure_session_key().await {
        debug!("Failed to get session key (may not be required): {e}");
    }
    let session_key = connection.get_session_key().await.unwrap_or_default();
    let local_node = connection.local_node_num().await?;

    let admin_msg = protobufs::AdminMessage {
        payload_variant: Some(protobufs::admin_message::PayloadVariant::SetChannel(
            channel,
        )),
        session_passkey: session_key,
    };

    let api = connection.get_api()?;
    let mesh_packet = protobufs::MeshPacket {
        payload_variant: Some(protobufs::mesh_packet::PayloadVariant::Decoded(
            protobufs::Data {
                portnum: protobufs::PortNum::AdminApp as i32,
                payload: admin_msg.encode_to_vec(),
                ..Default::default()
            },
        )),
        to: local_node,
        priority: protobufs::mesh_packet::Priority::Default as i32,
        ..Default::default()
    };

    api.send_to_radio_packet(Some(protobufs::to_radio::PayloadVariant::Packet(
        mesh_packet,
    )))
    .await?;
    Ok(())
}
