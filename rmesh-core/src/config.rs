use crate::connection::ConnectionManager;
use anyhow::{Context, Result, bail, ensure};
use meshtastic::{Message, protobufs};
use serde_json::json;
use tracing::{debug, warn};

/// Get a configuration value by key
pub async fn get_config_value(
    connection: &mut ConnectionManager,
    key: &str,
) -> Result<serde_json::Value> {
    // Try to get a session key, but continue even if it fails
    // Some devices may not require authentication
    if let Err(e) = connection.ensure_session_key().await {
        debug!("Failed to get session key (may not be required): {e}");
    }

    // Parse the key
    let parts: Vec<&str> = key.split('.').collect();
    ensure!(
        parts.len() == 2,
        "Invalid config key format. Use format: category.field (e.g., lora.region)"
    );

    let category = parts[0];
    let field = parts[1];

    // Get the session key
    let session_key = connection.get_session_key().await.unwrap_or_default();

    // Drop the cached copy first. Otherwise a reply that arrives late is indistinguishable
    // from one that never came, and the caller reads the previous value believing it is
    // current — which made `config set` verification report failure on a successful write.
    connection
        .get_device_state_ref()
        .lock()
        .await
        .invalidate_config(category);

    // Send config request
    // Admin messages are addressed to the radio itself, not 0.
    let local_node = connection.local_node_num().await?;

    let api = connection.get_api()?;

    // Create the appropriate config request based on category
    let config_type = match category {
        "device" => protobufs::admin_message::ConfigType::DeviceConfig,
        "position" => protobufs::admin_message::ConfigType::PositionConfig,
        "power" => protobufs::admin_message::ConfigType::PowerConfig,
        "network" => protobufs::admin_message::ConfigType::NetworkConfig,
        "display" => protobufs::admin_message::ConfigType::DisplayConfig,
        "lora" => protobufs::admin_message::ConfigType::LoraConfig,
        "bluetooth" => protobufs::admin_message::ConfigType::BluetoothConfig,
        _ => bail!("Unknown config category: {category}"),
    };

    // Create admin message for config request with session key
    let admin_msg = protobufs::AdminMessage {
        payload_variant: Some(protobufs::admin_message::PayloadVariant::GetConfigRequest(
            config_type as i32,
        )),
        session_passkey: session_key,
    };

    // Create mesh packet
    let mesh_packet = protobufs::MeshPacket {
        payload_variant: Some(protobufs::mesh_packet::PayloadVariant::Decoded(
            protobufs::Data {
                portnum: protobufs::PortNum::AdminApp as i32,
                payload: admin_msg.encode_to_vec(),
                // Without this the radio never replies, so the wait below can only ever
                // time out. It was invisible while a cached value was returned instead.
                want_response: true,
                ..Default::default()
            },
        )),
        from: 0,
        to: local_node,
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

    // Wait for the reply rather than guessing at a delay. A fixed sleep is not a signal:
    // on a slow serial or BLE link the response lands after it, and the caller then reads
    // stale state as though the radio had answered.
    let deadline = connection.timeout();
    let start = std::time::Instant::now();
    let mut answered = false;
    while start.elapsed() < deadline {
        if connection.get_device_state().await.has_config(category) {
            answered = true;
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    }
    // Falling through would return {"value": null} with a success exit code, which is
    // indistinguishable from a real value for a field that is not nullable.
    ensure!(
        answered,
        "Timed out after {deadline:?} waiting for the {category} config. \
         Admin reads usually need an authorised admin key for this radio."
    );

    // Get the cached config from device state
    let state = connection.get_device_state().await;

    // Extract the requested field from the appropriate config
    let value = match category {
        "device" => {
            if let Some(config) = &state.device_config {
                match field {
                    "role" => json!(config.role),
                    "button_gpio" => json!(config.button_gpio),
                    "buzzer_gpio" => json!(config.buzzer_gpio),
                    "rebroadcast_mode" => json!(config.rebroadcast_mode),
                    "node_info_broadcast_secs" => json!(config.node_info_broadcast_secs),
                    "tzdef" => json!(config.tzdef),
                    "disable_triple_click" => json!(config.disable_triple_click),
                    _ => bail!("Unknown device config field: {field}"),
                }
            } else {
                json!(null)
            }
        }
        "position" => {
            if let Some(config) = &state.position_config {
                match field {
                    "position_broadcast_secs" => json!(config.position_broadcast_secs),
                    "position_broadcast_smart_enabled" => {
                        json!(config.position_broadcast_smart_enabled)
                    }
                    "fixed_position" => json!(config.fixed_position),
                    "gps_enabled" => json!(config.gps_enabled),
                    "gps_mode" => json!(config.gps_mode),
                    _ => bail!("Unknown position config field: {field}"),
                }
            } else {
                json!(null)
            }
        }
        "display" => {
            if let Some(config) = &state.display_config {
                match field {
                    "screen_on_secs" => json!(config.screen_on_secs),
                    "gps_format" => json!(config.gps_format),
                    "auto_screen_carousel_secs" => json!(config.auto_screen_carousel_secs),
                    "compass_north_top" => json!(config.compass_north_top),
                    "compass_orientation" => json!(config.compass_orientation),
                    "flip_screen" => json!(config.flip_screen),
                    "units" => json!(config.units),
                    "displaymode" => json!(config.displaymode),
                    "heading_bold" => json!(config.heading_bold),
                    "wake_on_tap_or_motion" => json!(config.wake_on_tap_or_motion),
                    _ => bail!("Unknown display config field: {field}"),
                }
            } else {
                json!(null)
            }
        }
        "lora" => {
            if let Some(config) = &state.lora_config {
                match field {
                    "use_preset" => json!(config.use_preset),
                    "modem_preset" => json!(config.modem_preset),
                    "bandwidth" => json!(config.bandwidth),
                    "spread_factor" => json!(config.spread_factor),
                    "coding_rate" => json!(config.coding_rate),
                    "frequency_offset" => json!(config.frequency_offset),
                    "region" => json!(config.region),
                    "hop_limit" => json!(config.hop_limit),
                    "tx_enabled" => json!(config.tx_enabled),
                    "tx_power" => json!(config.tx_power),
                    "channel_num" => json!(config.channel_num),
                    "ignore_mqtt" => json!(config.ignore_mqtt),
                    _ => bail!("Unknown lora config field: {field}"),
                }
            } else {
                json!(null)
            }
        }
        _ => json!(null),
    };

    Ok(json!({
        "key": key,
        "value": value
    }))
}

/// Set a configuration value by key
pub async fn set_config_value(
    connection: &mut ConnectionManager,
    key: &str,
    value: &str,
) -> Result<()> {
    // Try to get a session key, but continue even if it fails
    // Some devices may not require authentication
    if let Err(e) = connection.ensure_session_key().await {
        debug!("Failed to get session key (may not be required): {e}");
    }

    // Get the session key
    let session_key = connection.get_session_key().await.unwrap_or_default();

    // SetConfig replaces the whole sub-message, so the payload has to start from what the
    // radio currently has. Building it from Default would silently blank every field the
    // caller did not name — tx_power, hop_limit, channel_num and the rest.
    let state = connection.get_device_state().await;

    // Admin messages are addressed to the radio itself, not 0.
    let local_node = connection.local_node_num().await?;

    let api = connection.get_api()?;

    let parts: Vec<&str> = key.split('.').collect();
    ensure!(
        parts.len() == 2,
        "Invalid config key format. Use format: category.field (e.g., lora.region)"
    );

    let category = parts[0];
    let field = parts[1];

    // Create admin message for config change
    let (admin_msg, expected) = match category {
        "lora" => {
            match field {
                "region" => {
                    // Parse region enum
                    let region = parse_region(value)?;
                    let mut config = state.raw_lora_config.clone().context(
                        "No LoRa config received from the device, so changing one field \
                         would blank the rest. Reconnect and retry.",
                    )?;
                    config.region = region as i32;
                    let msg = protobufs::AdminMessage {
                        payload_variant: Some(protobufs::admin_message::PayloadVariant::SetConfig(
                            protobufs::Config {
                                payload_variant: Some(protobufs::config::PayloadVariant::Lora(
                                    config,
                                )),
                            },
                        )),
                        session_passkey: session_key.clone(),
                    };
                    (msg, ExpectedConfig::Region(region as i32))
                }
                _ => bail!("Unknown lora field: {field}"),
            }
        }
        "device" => {
            match field {
                "role" => {
                    // Parse role enum
                    let role = parse_role(value)?;
                    let mut config = state.raw_device_config.clone().context(
                        "No device config received from the device, so changing one field \
                         would blank the rest. Reconnect and retry.",
                    )?;
                    config.role = role as i32;
                    let msg = protobufs::AdminMessage {
                        payload_variant: Some(protobufs::admin_message::PayloadVariant::SetConfig(
                            protobufs::Config {
                                payload_variant: Some(protobufs::config::PayloadVariant::Device(
                                    config,
                                )),
                            },
                        )),
                        session_passkey: session_key.clone(),
                    };
                    (msg, ExpectedConfig::Role(role as i32))
                }
                _ => bail!("Unknown device field: {field}"),
            }
        }
        _ => bail!("Config category '{category}' not yet implemented"),
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
        to: local_node,
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

    // Read the setting back. Sending the packet only means the bytes left the host: the
    // radio rejects admin writes it will not authorise and says nothing the write path
    // sees, so reporting success here would be reporting that we typed, not that anything
    // changed.
    verify_config_value(connection, category, field, &expected).await
}

/// What a `set` should be able to read back afterwards.
enum ExpectedConfig {
    Region(i32),
    Role(i32),
}

/// Re-read the config from the radio and confirm the write landed.
async fn verify_config_value(
    connection: &mut ConnectionManager,
    category: &str,
    field: &str,
    expected: &ExpectedConfig,
) -> Result<()> {
    // Ask for the sub-message again and let the reply be processed.
    let key = format!("{category}.{field}");
    let _ = get_config_value(connection, &key).await?;

    let state = connection.get_device_state().await;
    let actual = match expected {
        ExpectedConfig::Region(_) => state.raw_lora_config.as_ref().map(|c| c.region),
        ExpectedConfig::Role(_) => state.raw_device_config.as_ref().map(|c| c.role),
    };
    let wanted = match expected {
        ExpectedConfig::Region(v) | ExpectedConfig::Role(v) => *v,
    };

    let actual = actual
        .context("Device sent no configuration back, so the change could not be confirmed")?;
    ensure!(
        actual == wanted,
        "Device did not apply {key}: it still reports {actual} rather than {wanted}. \
         Admin writes usually need an authorised admin key for this radio."
    );

    Ok(())
}

/// List all configuration settings
pub async fn list_config(connection: &mut ConnectionManager) -> Result<serde_json::Value> {
    // Try to get a session key, but continue even if it fails
    // Some devices may not require authentication
    if let Err(e) = connection.ensure_session_key().await {
        debug!("Failed to get session key (may not be required): {e}");
    }

    // Get the session key
    let session_key = connection.get_session_key().await.unwrap_or_default();

    // Admin messages are addressed to the radio itself, not 0.
    let local_node = connection.local_node_num().await?;

    let api = connection.get_api()?;

    // Request all config types to get fresh data
    let config_types = [
        protobufs::admin_message::ConfigType::DeviceConfig,
        protobufs::admin_message::ConfigType::PositionConfig,
        protobufs::admin_message::ConfigType::PowerConfig,
        protobufs::admin_message::ConfigType::NetworkConfig,
        protobufs::admin_message::ConfigType::DisplayConfig,
        protobufs::admin_message::ConfigType::LoraConfig,
        protobufs::admin_message::ConfigType::BluetoothConfig,
    ];

    for config_type in config_types {
        // Create admin message for config request with session key
        let admin_msg = protobufs::AdminMessage {
            payload_variant: Some(protobufs::admin_message::PayloadVariant::GetConfigRequest(
                config_type as i32,
            )),
            session_passkey: session_key.clone(),
        };

        // Create mesh packet
        let mesh_packet = protobufs::MeshPacket {
            payload_variant: Some(protobufs::mesh_packet::PayloadVariant::Decoded(
                protobufs::Data {
                    portnum: protobufs::PortNum::AdminApp as i32,
                    payload: admin_msg.encode_to_vec(),
                    want_response: true,
                    ..Default::default()
                },
            )),
            to: local_node,
            ..Default::default()
        };

        // Send config request
        api.send_to_radio_packet(Some(protobufs::to_radio::PayloadVariant::Packet(
            mesh_packet,
        )))
        .await?;

        // Small delay between requests
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Wait for all responses to be processed
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Get the updated device state which includes all config
    let state = connection.get_device_state().await;

    // Build complete configuration from cached state
    let mut config = json!({});

    // Add device config if available
    if let Some(device_cfg) = &state.device_config {
        config["device"] = json!({
            "role": device_cfg.role,
            "button_gpio": device_cfg.button_gpio,
            "buzzer_gpio": device_cfg.buzzer_gpio,
            "rebroadcast_mode": device_cfg.rebroadcast_mode,
            "node_info_broadcast_secs": device_cfg.node_info_broadcast_secs,
            "tzdef": device_cfg.tzdef,
            "disable_triple_click": device_cfg.disable_triple_click,
        });
    }

    // Add position config if available
    if let Some(pos_cfg) = &state.position_config {
        config["position"] = json!({
            "position_broadcast_secs": pos_cfg.position_broadcast_secs,
            "position_broadcast_smart_enabled": pos_cfg.position_broadcast_smart_enabled,
            "fixed_position": pos_cfg.fixed_position,
            "gps_enabled": pos_cfg.gps_enabled,
            "gps_mode": pos_cfg.gps_mode,
        });
    }

    // Add power config if available
    if let Some(power_cfg) = &state.power_config {
        config["power"] = json!({
            "is_power_saving": power_cfg.is_power_saving,
            "on_battery_shutdown_after_secs": power_cfg.on_battery_shutdown_after_secs,
            "adc_multiplier_override": power_cfg.adc_multiplier_override,
            "wait_bluetooth_secs": power_cfg.wait_bluetooth_secs,
            "sds_secs": power_cfg.sds_secs,
            "ls_secs": power_cfg.ls_secs,
            "min_wake_secs": power_cfg.min_wake_secs,
        });
    }

    // Add network config if available
    if let Some(net_cfg) = &state.network_config {
        config["network"] = json!({
            "wifi_enabled": net_cfg.wifi_enabled,
            "wifi_ssid": net_cfg.wifi_ssid,
            "wifi_psk": net_cfg.wifi_psk,
            "ntp_server": net_cfg.ntp_server,
            "eth_enabled": net_cfg.eth_enabled,
            "ipv4_config": net_cfg.ipv4_config,
        });
    }

    // Add display config if available
    if let Some(display_cfg) = &state.display_config {
        config["display"] = json!({
            "screen_on_secs": display_cfg.screen_on_secs,
            "gps_format": display_cfg.gps_format,
            "auto_screen_carousel_secs": display_cfg.auto_screen_carousel_secs,
            "compass_north_top": display_cfg.compass_north_top,
            "compass_orientation": display_cfg.compass_orientation,
            "flip_screen": display_cfg.flip_screen,
            "units": display_cfg.units,
            "displaymode": display_cfg.displaymode,
            "heading_bold": display_cfg.heading_bold,
            "wake_on_tap_or_motion": display_cfg.wake_on_tap_or_motion,
        });
    }

    // Add LoRa config if available
    if let Some(lora_cfg) = &state.lora_config {
        config["lora"] = json!({
            "use_preset": lora_cfg.use_preset,
            "modem_preset": lora_cfg.modem_preset,
            "bandwidth": lora_cfg.bandwidth,
            "spread_factor": lora_cfg.spread_factor,
            "coding_rate": lora_cfg.coding_rate,
            "frequency_offset": lora_cfg.frequency_offset,
            "region": lora_cfg.region,
            "hop_limit": lora_cfg.hop_limit,
            "tx_enabled": lora_cfg.tx_enabled,
            "tx_power": lora_cfg.tx_power,
            "channel_num": lora_cfg.channel_num,
            "ignore_mqtt": lora_cfg.ignore_mqtt,
        });
    }

    // Add Bluetooth config if available
    if let Some(bt_cfg) = &state.bluetooth_config {
        config["bluetooth"] = json!({
            "enabled": bt_cfg.enabled,
            "mode": bt_cfg.mode,
            "fixed_pin": bt_cfg.fixed_pin,
            "device_logging_enabled": bt_cfg.device_logging_enabled,
        });
    }

    // Return the complete configuration
    if config.as_object().is_none_or(|o| o.is_empty()) {
        Ok(json!({
            "status": "no_config",
            "message": "No configuration data available. Device may not be fully synchronized.",
            "hint": "Try running 'rmesh device refresh' to fetch latest configuration"
        }))
    } else {
        Ok(config)
    }
}

fn parse_region(value: &str) -> Result<protobufs::config::lo_ra_config::RegionCode> {
    use protobufs::config::lo_ra_config::RegionCode;

    let upper = value.to_uppercase();

    // Canonical protobuf names first, so every region the protobufs know is accepted
    // without listing it here — including whatever the next regen adds.
    if let Some(region) = RegionCode::from_str_name(&upper) {
        // from_str_name accepts "UNSET", which the old hand-written table did not. Setting
        // it parks the radio in a region where it will not transmit, so it needs to be an
        // explicit refusal rather than something a typo can reach.
        ensure!(
            region != RegionCode::Unset,
            "Refusing to set region UNSET: the radio will not transmit. \
             Pass a real region such as US, EU_868 or ANZ."
        );
        return Ok(region);
    }

    // Spellings rmesh accepted before it emitted the canonical names. Kept so existing
    // scripts keep working; the underscored forms are already covered above.
    let region = match upper.as_str() {
        "US915" => RegionCode::Us,
        "EU" | "EU433" => RegionCode::Eu433,
        "EU868" => RegionCode::Eu868,
        "NZ865" => RegionCode::Nz865,
        "UA433" => RegionCode::Ua433,
        "UA868" => RegionCode::Ua868,
        "MY433" => RegionCode::My433,
        "MY919" => RegionCode::My919,
        "SG923" => RegionCode::Sg923,
        "PH433" => RegionCode::Ph433,
        "PH868" => RegionCode::Ph868,
        "PH915" => RegionCode::Ph915,
        "LORA24" => RegionCode::Lora24,
        _ => bail!("Unknown region: {value}"),
    };

    Ok(region)
}

/// ROUTER_CLIENT (deprecated upstream in v2.3.15) and REPEATER (v2.7.11) are still accepted:
/// radios in the field continue to report them, and refusing to name a role the user's device
/// is actually running would make it unreadable rather than discouraged.
#[allow(deprecated)]
fn parse_role(value: &str) -> Result<protobufs::config::device_config::Role> {
    use protobufs::config::device_config::Role;

    let upper = value.to_uppercase();

    // Canonical protobuf names first, for the same reason as parse_region: the hand-written
    // table below had already fallen behind, rejecting ROUTER_LATE and CLIENT_BASE even
    // though rmesh reports them when a peer uses them.
    if let Some(role) = Role::from_str_name(&upper) {
        if matches!(role, Role::Repeater | Role::RouterClient) {
            warn!(
                "Role {upper} is deprecated upstream and harms public meshes; \
                 prefer ROUTER or CLIENT_BASE"
            );
        }
        return Ok(role);
    }

    let role = match upper.as_str() {
        "CLIENT" => Role::Client,
        "CLIENT_MUTE" => Role::ClientMute,
        "ROUTER" => Role::Router,
        "ROUTER_CLIENT" => Role::RouterClient,
        "REPEATER" => Role::Repeater,
        "TRACKER" => Role::Tracker,
        "SENSOR" => Role::Sensor,
        "TAK" => Role::Tak,
        "CLIENT_HIDDEN" => Role::ClientHidden,
        "LOST_AND_FOUND" => Role::LostAndFound,
        "TAK_TRACKER" => Role::TakTracker,
        _ => bail!("Unknown role: {value}"),
    };

    Ok(role)
}

#[cfg(test)]
mod parse_tests {
    use super::*;
    use protobufs::config::device_config::Role;
    use protobufs::config::lo_ra_config::RegionCode;

    /// UNSET is a real region code meaning "will not transmit". `from_str_name` accepts the
    /// string, so without an explicit refusal a typo can silence the radio.
    #[test]
    fn region_unset_is_refused() {
        for spelling in ["unset", "UNSET", "Unset"] {
            assert!(
                parse_region(spelling).is_err(),
                "{spelling} must not be settable"
            );
        }
    }

    #[test]
    fn region_accepts_canonical_and_legacy_spellings() {
        assert_eq!(parse_region("US").unwrap(), RegionCode::Us);
        // canonical, as emitted by `config get`
        assert_eq!(parse_region("EU_433").unwrap(), RegionCode::Eu433);
        assert_eq!(parse_region("LORA_24").unwrap(), RegionCode::Lora24);
        // spellings rmesh accepted before it emitted canonical names
        assert_eq!(parse_region("eu433").unwrap(), RegionCode::Eu433);
        assert_eq!(parse_region("LORA24").unwrap(), RegionCode::Lora24);
        assert!(parse_region("NOT_A_REGION").is_err());
    }

    /// The hand-written table fell behind the protobufs: rmesh reported these roles when a
    /// peer used them but refused to set them.
    #[test]
    fn role_accepts_roles_newer_than_the_hand_written_table() {
        assert_eq!(parse_role("ROUTER_LATE").unwrap(), Role::RouterLate);
        assert_eq!(parse_role("CLIENT_BASE").unwrap(), Role::ClientBase);
    }

    #[test]
    fn role_accepts_existing_and_deprecated_names() {
        assert_eq!(parse_role("client").unwrap(), Role::Client);
        assert_eq!(parse_role("CLIENT_MUTE").unwrap(), Role::ClientMute);
        // deprecated upstream, still settable so a device can be moved off them
        #[allow(deprecated)]
        {
            assert_eq!(parse_role("REPEATER").unwrap(), Role::Repeater);
        }
        assert!(parse_role("NOT_A_ROLE").is_err());
    }
}
