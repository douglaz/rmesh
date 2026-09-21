use crate::connection::ConnectionManager;
use anyhow::{Result, bail};
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
    // NOT IMPLEMENTED, deliberately. The previous body wrote a Channel with index 0 and
    // role PRIMARY. Channel indices are explicit — the radio does not allocate them — so
    // every "add" overwrote the primary channel, replacing its name and PSK and dropping
    // the radio off its mesh. That was inert only while admin packets were addressed to
    // node 0 and discarded; addressing them correctly turns it into data loss.
    //
    // Doing it properly means allocating a free index and writing role SECONDARY. That is
    // not shipped here because it cannot be exercised without mutating a radio's real
    // channels, and an untested write of this shape is exactly what caused the problem.
    let _ = (&connection, name, psk);
    bail!(
        "`channel add` is not implemented. The request it used to send would overwrite \
         the primary channel and its PSK. Use the Meshtastic app, or `meshtastic --ch-add`."
    )
}

/// Delete a channel
pub async fn delete_channel(connection: &mut ConnectionManager, index: u32) -> Result<()> {
    // NOT IMPLEMENTED, deliberately. The previous body sent RemoveByNodenum(index), which
    // acts on the NodeDB rather than on channels: the channel stayed configured and a node
    // whose number happened to equal the index was removed instead. Deleting a channel
    // means SetChannel for that index with role DISABLED.
    let _ = (&connection, index);
    bail!(
        "`channel delete` is not implemented. The request it used to send removes a NODE \
         with that number, not the channel. Use the Meshtastic app, or `meshtastic --ch-del`."
    )
}

/// Set channel configuration
pub async fn set_channel(
    connection: &mut ConnectionManager,
    index: u32,
    name: Option<&str>,
    psk: Option<&str>,
) -> Result<()> {
    // NOT IMPLEMENTED, deliberately. The previous body rebuilt the channel from defaults
    // and hard-coded role PRIMARY, so changing only --name sent an empty PSK and default
    // uplink/downlink: a rename erased the channel's credentials. A correct version reads
    // the cached channel and modifies only the named fields.
    let _ = (&connection, index, name, psk);
    bail!(
        "`channel set` is not implemented. The request it used to send replaces the whole \
         channel from defaults, erasing its PSK. Use the Meshtastic app, or `meshtastic --ch-set`."
    )
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelInfo {
    pub index: u32,
    pub name: String,
    pub role: String,
    pub has_psk: bool,
}
