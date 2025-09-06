use crate::connection::ConnectionManager;
use anyhow::{Result, bail, ensure};
use meshtastic::{Message, protobufs};
use serde_json::json;

/// Get a configuration value by key
pub async fn get_config_value(
    connection: &mut ConnectionManager,
    key: &str,
) -> Result<serde_json::Value> {
    // Ensure we have a session key for admin operations
    connection.ensure_session_key().await?;

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

    // Send config request
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

    // Wait a moment for the response to be processed
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

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
    // Ensure we have a session key for admin operations
    connection.ensure_session_key().await?;

    // Get the session key
    let session_key = connection.get_session_key().await.unwrap_or_default();

    let api = connection.get_api()?;

    let parts: Vec<&str> = key.split('.').collect();
    ensure!(
        parts.len() == 2,
        "Invalid config key format. Use format: category.field (e.g., lora.region)"
    );

    let category = parts[0];
    let field = parts[1];

    // Create admin message for config change
    let admin_msg = match category {
        "lora" => {
            match field {
                "region" => {
                    // Parse region enum
                    let region = parse_region(value)?;
                    let config = protobufs::config::LoRaConfig {
                        region: region as i32,
                        ..Default::default()
                    };
                    protobufs::AdminMessage {
                        payload_variant: Some(protobufs::admin_message::PayloadVariant::SetConfig(
                            protobufs::Config {
                                payload_variant: Some(protobufs::config::PayloadVariant::Lora(
                                    config,
                                )),
                            },
                        )),
                        session_passkey: session_key.clone(),
                    }
                }
                _ => bail!("Unknown lora field: {field}"),
            }
        }
        "device" => {
            match field {
                "role" => {
                    // Parse role enum
                    let role = parse_role(value)?;
                    let config = protobufs::config::DeviceConfig {
                        role: role as i32,
                        ..Default::default()
                    };
                    protobufs::AdminMessage {
                        payload_variant: Some(protobufs::admin_message::PayloadVariant::SetConfig(
                            protobufs::Config {
                                payload_variant: Some(protobufs::config::PayloadVariant::Device(
                                    config,
                                )),
                            },
                        )),
                        session_passkey: session_key.clone(),
                    }
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

/// List all configuration settings
pub async fn list_config(connection: &ConnectionManager) -> Result<serde_json::Value> {
    // Get the current device state which includes all config
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

    let region = match value.to_uppercase().as_str() {
        "US" | "US915" => RegionCode::Us,
        "EU" | "EU433" => RegionCode::Eu433,
        "EU868" | "EU_868" => RegionCode::Eu868,
        "CN" => RegionCode::Cn,
        "JP" => RegionCode::Jp,
        "ANZ" => RegionCode::Anz,
        "KR" => RegionCode::Kr,
        "TW" => RegionCode::Tw,
        "RU" => RegionCode::Ru,
        "IN" => RegionCode::In,
        "NZ865" | "NZ_865" => RegionCode::Nz865,
        "TH" => RegionCode::Th,
        "UA433" | "UA_433" => RegionCode::Ua433,
        "UA868" | "UA_868" => RegionCode::Ua868,
        "MY_433" => RegionCode::My433,
        "MY_919" => RegionCode::My919,
        "SG_923" => RegionCode::Sg923,
        "LORA_24" => RegionCode::Lora24,
        _ => bail!("Unknown region: {value}"),
    };

    Ok(region)
}

fn parse_role(value: &str) -> Result<protobufs::config::device_config::Role> {
    use protobufs::config::device_config::Role;

    let role = match value.to_uppercase().as_str() {
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
