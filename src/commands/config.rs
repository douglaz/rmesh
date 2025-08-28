use anyhow::{bail, Result};
use serde::Serialize;
use serde_json::json;
use crate::cli::ConfigCommands;
use crate::connection::ConnectionManager;
use crate::output::{print_output, OutputFormat, create_table};
use crate::utils::{print_success, print_info, print_error};
use meshtastic::{protobufs, Message};
use colored::*;
use comfy_table::Cell;

#[derive(Debug, Serialize)]
struct ConfigValue {
    key: String,
    value: serde_json::Value,
}

pub async fn handle_config(
    mut connection: ConnectionManager,
    subcommand: ConfigCommands,
    format: OutputFormat,
) -> Result<()> {
    match subcommand {
        ConfigCommands::Get { key } => {
            let api = connection.api()?;
            let config = api.get_config();
            
            // Parse the key (e.g., "lora.region" -> category: lora, field: region)
            let parts: Vec<&str> = key.split('.').collect();
            if parts.len() != 2 {
                bail!("Invalid config key format. Use format: category.field (e.g., lora.region)");
            }
            
            let category = parts[0];
            let field = parts[1];
            
            let value = match category {
                "device" => extract_device_config(&config.device, field),
                "position" => extract_position_config(&config.position, field),
                "power" => extract_power_config(&config.power, field),
                "network" => extract_network_config(&config.network, field),
                "display" => extract_display_config(&config.display, field),
                "lora" => extract_lora_config(&config.lora, field),
                "bluetooth" => extract_bluetooth_config(&config.bluetooth, field),
                _ => bail!("Unknown config category: {}", category),
            };
            
            let config_value = ConfigValue {
                key: key.clone(),
                value,
            };
            
            match format {
                OutputFormat::Json => print_output(config_value, format)?,
                OutputFormat::Table => {
                    println!("{}: {}", config_value.key.cyan(), config_value.value);
                }
            }
        }
        
        ConfigCommands::Set { key, value } => {
            let api = connection.api_mut()?;
            
            // Parse the key
            let parts: Vec<&str> = key.split('.').collect();
            if parts.len() != 2 {
                bail!("Invalid config key format. Use format: category.field (e.g., lora.region)");
            }
            
            let category = parts[0];
            let field = parts[1];
            
            print_info(&format!("Setting {} = {}", key, value));
            
            // Create config update based on category
            let config = match category {
                "lora" => {
                    let mut lora_config = protobufs::config::LoRaConfig::default();
                    match field {
                        "region" => {
                            // Parse region enum
                            let region = match value.to_uppercase().as_str() {
                                "US" | "UNSET" => protobufs::config::lo_ra_config::RegionCode::Unset,
                                "US_902_928" => protobufs::config::lo_ra_config::RegionCode::Us,
                                "EU_433" => protobufs::config::lo_ra_config::RegionCode::Eu433,
                                "EU_868" => protobufs::config::lo_ra_config::RegionCode::Eu868,
                                "CN" => protobufs::config::lo_ra_config::RegionCode::Cn,
                                "JP" => protobufs::config::lo_ra_config::RegionCode::Jp,
                                "ANZ" => protobufs::config::lo_ra_config::RegionCode::Anz,
                                "KR" => protobufs::config::lo_ra_config::RegionCode::Kr,
                                "TW" => protobufs::config::lo_ra_config::RegionCode::Tw,
                                "RU" => protobufs::config::lo_ra_config::RegionCode::Ru,
                                "IN" => protobufs::config::lo_ra_config::RegionCode::In,
                                "NZ_865" => protobufs::config::lo_ra_config::RegionCode::Nz865,
                                "TH" => protobufs::config::lo_ra_config::RegionCode::Th,
                                _ => bail!("Unknown region: {}", value),
                            };
                            lora_config.set_region(region);
                        }
                        "hop_limit" => {
                            lora_config.hop_limit = value.parse::<u32>()
                                .map_err(|_| anyhow::anyhow!("hop_limit must be a number"))?;
                        }
                        "tx_power" => {
                            lora_config.tx_power = value.parse::<i32>()
                                .map_err(|_| anyhow::anyhow!("tx_power must be a number"))?;
                        }
                        _ => bail!("Unknown lora config field: {}", field),
                    }
                    protobufs::Config {
                        payload_variant: Some(protobufs::config::PayloadVariant::Lora(lora_config)),
                    }
                }
                "device" => {
                    let mut device_config = protobufs::config::DeviceConfig::default();
                    match field {
                        "role" => {
                            let role = match value.to_uppercase().as_str() {
                                "CLIENT" => protobufs::config::device_config::Role::Client,
                                "CLIENT_MUTE" => protobufs::config::device_config::Role::ClientMute,
                                "ROUTER" => protobufs::config::device_config::Role::Router,
                                "ROUTER_CLIENT" => protobufs::config::device_config::Role::RouterClient,
                                _ => bail!("Unknown device role: {}", value),
                            };
                            device_config.set_role(role);
                        }
                        _ => bail!("Unknown device config field: {}", field),
                    }
                    protobufs::Config {
                        payload_variant: Some(protobufs::config::PayloadVariant::Device(device_config)),
                    }
                }
                _ => bail!("Config updates for '{}' category not yet implemented", category),
            };
            
            // Send config update
            api.update_config(config).await?;
            
            print_success(&format!("Configuration updated: {} = {}", key, value));
        }
        
        ConfigCommands::List => {
            let api = connection.api()?;
            let config = api.get_config();
            
            match format {
                OutputFormat::Json => print_output(config, format)?,
                OutputFormat::Table => {
                    let mut table = create_table();
                    table.set_header(vec!["Category", "Field", "Value"]);
                    
                    // Device config
                    add_config_rows(&mut table, "device", &[
                        ("role", format!("{:?}", config.device.role())),
                        ("rebroadcast_mode", format!("{:?}", config.device.rebroadcast_mode())),
                    ]);
                    
                    // LoRa config  
                    add_config_rows(&mut table, "lora", &[
                        ("region", format!("{:?}", config.lora.region())),
                        ("hop_limit", config.lora.hop_limit.to_string()),
                        ("tx_power", config.lora.tx_power.to_string()),
                        ("channel_num", config.lora.channel_num.to_string()),
                    ]);
                    
                    // Position config
                    add_config_rows(&mut table, "position", &[
                        ("broadcast_interval", config.position.position_broadcast_secs.to_string()),
                        ("gps_enabled", (config.position.gps_enabled != 0).to_string()),
                    ]);
                    
                    // Power config
                    add_config_rows(&mut table, "power", &[
                        ("is_power_saving", config.power.is_power_saving.to_string()),
                    ]);
                    
                    println!("{}", "Device Configuration".bold());
                    println!("{}", table);
                }
            }
        }
    }
    
    Ok(())
}

fn add_config_rows(table: &mut comfy_table::Table, category: &str, fields: &[(&str, String)]) {
    for (field, value) in fields {
        table.add_row(vec![
            Cell::new(category),
            Cell::new(field),
            Cell::new(value),
        ]);
    }
}

fn extract_device_config(config: &protobufs::config::DeviceConfig, field: &str) -> serde_json::Value {
    match field {
        "role" => json!(format!("{:?}", config.role())),
        "rebroadcast_mode" => json!(format!("{:?}", config.rebroadcast_mode())),
        _ => json!(null),
    }
}

fn extract_position_config(config: &protobufs::config::PositionConfig, field: &str) -> serde_json::Value {
    match field {
        "broadcast_interval" => json!(config.position_broadcast_secs),
        "gps_enabled" => json!(config.gps_enabled != 0),
        _ => json!(null),
    }
}

fn extract_power_config(config: &protobufs::config::PowerConfig, field: &str) -> serde_json::Value {
    match field {
        "is_power_saving" => json!(config.is_power_saving),
        "on_battery_shutdown_after_secs" => json!(config.on_battery_shutdown_after_secs),
        _ => json!(null),
    }
}

fn extract_network_config(config: &protobufs::config::NetworkConfig, field: &str) -> serde_json::Value {
    match field {
        "wifi_enabled" => json!(config.wifi_enabled),
        "wifi_ssid" => json!(config.wifi_ssid),
        _ => json!(null),
    }
}

fn extract_display_config(config: &protobufs::config::DisplayConfig, field: &str) -> serde_json::Value {
    match field {
        "screen_on_secs" => json!(config.screen_on_secs),
        _ => json!(null),
    }
}

fn extract_lora_config(config: &protobufs::config::LoRaConfig, field: &str) -> serde_json::Value {
    match field {
        "region" => json!(format!("{:?}", config.region())),
        "hop_limit" => json!(config.hop_limit),
        "tx_power" => json!(config.tx_power),
        "channel_num" => json!(config.channel_num),
        _ => json!(null),
    }
}

fn extract_bluetooth_config(config: &protobufs::config::BluetoothConfig, field: &str) -> serde_json::Value {
    match field {
        "enabled" => json!(config.enabled),
        "mode" => json!(format!("{:?}", config.mode())),
        _ => json!(null),
    }
}