use anyhow::Result;
use comfy_table::Cell;
use serde::Serialize;

use crate::cli::{InfoCommands, TelemetryType};
use crate::output::{OutputFormat, create_table, print_output};
use rmesh_core::ConnectionManager;

/// Format uptime seconds into a human-readable string
fn format_uptime(seconds: u32) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h {minutes}m {secs}s")
    } else if minutes > 0 {
        format!("{minutes}m {secs}s")
    } else {
        format!("{secs}s")
    }
}

#[derive(Debug, Serialize)]
struct RadioInfo {
    pub firmware_version: String,
    pub hardware_model: String,
    pub region: String,
    pub node_id: String,
    pub node_num: u32,
    pub has_gps: bool,
    pub num_channels: usize,
}

pub async fn handle_info(
    mut connection: ConnectionManager,
    subcommand: InfoCommands,
    format: OutputFormat,
) -> Result<()> {
    match subcommand {
        InfoCommands::Radio => {
            // Get actual device information from the device state
            let state = connection.get_device_state().await;

            // Extract firmware version from min_app_version
            let firmware_version = if let Some(my_info) = &state.my_node_info {
                let major = my_info.min_app_version / 10000;
                let minor = (my_info.min_app_version % 10000) / 100;
                let patch = my_info.min_app_version % 100;
                format!("{major}.{minor}.{patch}")
            } else {
                "Unknown".to_string()
            };

            // Get hardware model from nodes (typically the local node has this info)
            let hardware_model = if let Some(my_info) = &state.my_node_info {
                state
                    .nodes
                    .get(&my_info.node_num)
                    .and_then(|node| node.user.hw_model.clone())
                    .unwrap_or_else(|| "Unknown".to_string())
            } else {
                "Unknown".to_string()
            };

            // Get region from LoRa config
            let region = state
                .lora_config
                .as_ref()
                .map(|cfg| cfg.region.clone())
                .unwrap_or_else(|| "Unknown".to_string());

            // Get node ID and number from my_node_info
            let (node_id, node_num) = if let Some(my_info) = &state.my_node_info {
                (my_info.node_id.clone(), my_info.node_num)
            } else {
                ("Unknown".to_string(), 0)
            };

            // Check GPS status from position config
            let has_gps = state
                .position_config
                .as_ref()
                .map(|cfg| cfg.gps_enabled)
                .unwrap_or_default();

            // Count actual channels
            let num_channels = state.channels.len();

            let radio_info = RadioInfo {
                firmware_version,
                hardware_model,
                region,
                node_id,
                node_num,
                has_gps,
                num_channels,
            };

            match format {
                OutputFormat::Json => print_output(&radio_info, format),
                OutputFormat::Table => {
                    let mut table = create_table();
                    table.set_header(vec![Cell::new("Property"), Cell::new("Value")]);
                    table.add_row(vec![
                        Cell::new("Firmware Version"),
                        Cell::new(&radio_info.firmware_version),
                    ]);
                    table.add_row(vec![
                        Cell::new("Hardware Model"),
                        Cell::new(&radio_info.hardware_model),
                    ]);
                    table.add_row(vec![Cell::new("Region"), Cell::new(&radio_info.region)]);
                    table.add_row(vec![Cell::new("Node ID"), Cell::new(&radio_info.node_id)]);
                    table.add_row(vec![
                        Cell::new("Node Number"),
                        Cell::new(radio_info.node_num),
                    ]);
                    table.add_row(vec![Cell::new("Has GPS"), Cell::new(radio_info.has_gps)]);
                    table.add_row(vec![
                        Cell::new("Num Channels"),
                        Cell::new(radio_info.num_channels),
                    ]);
                    println!("{table}");
                }
            }
        }

        InfoCommands::Nodes => {
            // Use the core library function
            let nodes = rmesh_core::mesh::get_nodes(&connection).await?;

            match format {
                OutputFormat::Json => {
                    // Always output JSON, even if empty (will be [])
                    print_output(&nodes, format);
                }
                OutputFormat::Table => {
                    if nodes.is_empty() {
                        println!("No nodes found in the mesh network");
                        return Ok(());
                    }
                    let mut table = create_table();
                    table.set_header(vec![
                        Cell::new("ID"),
                        Cell::new("Number"),
                        Cell::new("User"),
                        Cell::new("SNR"),
                        Cell::new("Last Heard"),
                    ]);

                    for node in nodes {
                        table.add_row(vec![
                            Cell::new(&node.id),
                            Cell::new(node.num),
                            Cell::new(&node.user.long_name),
                            Cell::new(
                                node.snr
                                    .map(|s| format!("{snr:.1}", snr = s))
                                    .unwrap_or_else(|| "N/A".to_string()),
                            ),
                            Cell::new(
                                node.last_heard
                                    .and_then(|timestamp| {
                                        chrono::DateTime::from_timestamp(timestamp as i64, 0)
                                            .map(|dt| dt.to_rfc3339())
                                    })
                                    .unwrap_or_else(|| "Never".to_string()),
                            ),
                        ]);
                    }

                    println!("{table}");
                }
            }
        }

        InfoCommands::Channels => {
            // Use the core library function
            let channels = rmesh_core::channel::list_channels(&connection).await?;

            if channels.is_empty() {
                println!("No channels configured");
                return Ok(());
            }

            match format {
                OutputFormat::Json => print_output(&channels, format),
                OutputFormat::Table => {
                    let mut table = create_table();
                    table.set_header(vec![
                        Cell::new("Index"),
                        Cell::new("Name"),
                        Cell::new("Role"),
                        Cell::new("Encrypted"),
                    ]);

                    for channel in channels {
                        table.add_row(vec![
                            Cell::new(channel.index),
                            Cell::new(&channel.name),
                            Cell::new(&channel.role),
                            Cell::new(if channel.has_psk { "Yes" } else { "No" }),
                        ]);
                    }

                    println!("{table}");
                }
            }
        }

        InfoCommands::Metrics { wait, request } => {
            // First, send telemetry request if requested
            if request {
                eprintln!("Requesting telemetry from device...");
                rmesh_core::telemetry::request_device_telemetry(&mut connection).await?;
            }

            // Then collect telemetry based on wait flag
            let metrics = if let Some(wait_seconds) = wait {
                // Wait for telemetry broadcasts/responses
                if request {
                    eprintln!("Waiting {wait_seconds} seconds for telemetry response...");
                } else {
                    eprintln!("Waiting {wait_seconds} seconds for telemetry broadcasts...");
                }
                rmesh_core::telemetry::collect_telemetry(&mut connection, wait_seconds).await?
            } else if request {
                // Just requested telemetry, wait default 10 seconds for response
                eprintln!("Waiting for telemetry response...");
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                let state = connection.get_device_state().await;
                let local_node_num = state.my_node_info.as_ref().map(|i| i.node_num);
                local_node_num.and_then(|num| {
                    state
                        .telemetry
                        .get(&num)
                        .and_then(|t| t.device_metrics.clone())
                })
            } else {
                // No flags: Get current telemetry data from device state
                let state = connection.get_device_state().await;
                let local_node_num = state.my_node_info.as_ref().map(|i| i.node_num);
                local_node_num.and_then(|num| {
                    state
                        .telemetry
                        .get(&num)
                        .and_then(|t| t.device_metrics.clone())
                })
            };

            match format {
                OutputFormat::Json => {
                    // Output device metrics or null
                    print_output(&metrics, format);
                }
                OutputFormat::Table => {
                    // Get device state for context
                    let state = connection.get_device_state().await;
                    let local_node_num = match &state.my_node_info {
                        Some(info) => info.node_num,
                        None => {
                            println!("No local node information available");
                            return Ok(());
                        }
                    };

                    // Get node info for additional context
                    let node_info = state.nodes.get(&local_node_num);
                    let hw_model = node_info
                        .and_then(|n| n.user.hw_model.as_ref())
                        .map(|s| s.as_str())
                        .unwrap_or("Unknown");

                    let mut table = create_table();
                    table.set_header(vec![Cell::new("Property"), Cell::new("Value")]);

                    // Add node context
                    table.add_row(vec![
                        Cell::new("Node ID"),
                        Cell::new(format!("{:08x}", local_node_num)),
                    ]);
                    table.add_row(vec![Cell::new("Hardware"), Cell::new(hw_model)]);

                    if let Some(m) = metrics {
                        // Battery level
                        table.add_row(vec![
                            Cell::new("Battery Level"),
                            Cell::new(
                                m.battery_level
                                    .map(|b| format!("{b}%"))
                                    .unwrap_or_else(|| "N/A".to_string()),
                            ),
                        ]);

                        // Voltage
                        table.add_row(vec![
                            Cell::new("Voltage"),
                            Cell::new(
                                m.voltage
                                    .map(|v| format!("{v:.2}V"))
                                    .unwrap_or_else(|| "N/A".to_string()),
                            ),
                        ]);

                        // Channel utilization
                        table.add_row(vec![
                            Cell::new("Channel Util"),
                            Cell::new(
                                m.channel_utilization
                                    .map(|u| format!("{u:.1}%"))
                                    .unwrap_or_else(|| "N/A".to_string()),
                            ),
                        ]);

                        // Air utilization TX
                        table.add_row(vec![
                            Cell::new("Air Util TX"),
                            Cell::new(
                                m.air_util_tx
                                    .map(|u| format!("{u:.1}%"))
                                    .unwrap_or_else(|| "N/A".to_string()),
                            ),
                        ]);

                        // Uptime
                        table.add_row(vec![
                            Cell::new("Uptime"),
                            Cell::new(
                                m.uptime_seconds
                                    .map(format_uptime)
                                    .unwrap_or_else(|| "N/A".to_string()),
                            ),
                        ]);
                    } else {
                        table.add_row(vec![
                            Cell::new("Status"),
                            Cell::new("No metrics data available"),
                        ]);
                    }

                    println!("{table}");
                }
            }
        }

        InfoCommands::Position { wait, request_all } => {
            // First, send position requests if requested
            if request_all {
                eprintln!("Requesting positions from all nodes...");
                rmesh_core::position::send_position_requests(&mut connection).await?;
            }

            // Then collect positions based on wait flag
            let positions = if let Some(wait_seconds) = wait {
                // Wait for position broadcasts/responses
                if request_all {
                    eprintln!(
                        "Waiting {wait_seconds} seconds for position responses and broadcasts..."
                    );
                } else {
                    eprintln!("Waiting {wait_seconds} seconds for position broadcasts...");
                }
                rmesh_core::position::collect_positions(&mut connection, wait_seconds).await?
            } else if request_all {
                // Just requested positions, wait default 10 seconds for responses
                eprintln!("Waiting for position responses...");
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                let state = connection.get_device_state().await;
                state.positions
            } else {
                // No flags: Get current position data from device state
                let state = connection.get_device_state().await;
                state.positions
            };

            match format {
                OutputFormat::Json => {
                    // Always output JSON, even if empty (will be {})
                    print_output(&positions, format);
                }
                OutputFormat::Table => {
                    if positions.is_empty() {
                        println!("No position data available");
                        return Ok(());
                    }

                    let mut table = create_table();
                    table.set_header(vec![
                        Cell::new("Node ID"),
                        Cell::new("Latitude"),
                        Cell::new("Longitude"),
                        Cell::new("Altitude"),
                        Cell::new("Time"),
                    ]);

                    for (node_num, position) in positions {
                        table.add_row(vec![
                            Cell::new(format!("{num:08x}", num = node_num)),
                            Cell::new(format!("{lat:.6}", lat = position.latitude)),
                            Cell::new(format!("{lon:.6}", lon = position.longitude)),
                            Cell::new(
                                position
                                    .altitude
                                    .map(|a| a.to_string())
                                    .unwrap_or_else(|| "N/A".to_string()),
                            ),
                            Cell::new(position.time.as_deref().unwrap_or("N/A")),
                        ]);
                    }

                    println!("{table}");
                }
            }
        }

        InfoCommands::Telemetry => {
            // Get telemetry data from device state
            let state = connection.get_device_state().await;

            match format {
                OutputFormat::Json => {
                    // Always output JSON, even if empty (will be {})
                    print_output(&state.telemetry, format);
                }
                OutputFormat::Table => {
                    if state.telemetry.is_empty() {
                        println!("No telemetry data available");
                        return Ok(());
                    }
                    let mut table = create_table();
                    table.set_header(vec![
                        Cell::new("Node ID"),
                        Cell::new("Type"),
                        Cell::new("Battery"),
                        Cell::new("Voltage"),
                        Cell::new("Temperature"),
                        Cell::new("Humidity"),
                    ]);

                    for (node_num, telemetry) in state.telemetry {
                        let mut battery = "N/A".to_string();
                        let mut voltage = "N/A".to_string();
                        let mut temp = "N/A".to_string();
                        let mut humidity = "N/A".to_string();
                        let mut data_type = "None".to_string();

                        if let Some(device) = &telemetry.device_metrics {
                            data_type = "Device".to_string();
                            battery = device
                                .battery_level
                                .map(|b| format!("{b}%"))
                                .unwrap_or_else(|| "N/A".to_string());
                            voltage = device
                                .voltage
                                .map(|v| format!("{voltage:.2}V", voltage = v))
                                .unwrap_or_else(|| "N/A".to_string());
                        }

                        if let Some(env) = &telemetry.environment_metrics {
                            data_type = if data_type == "None" {
                                "Environment".to_string()
                            } else {
                                format!("{data_type}, Environment")
                            };
                            temp = env
                                .temperature
                                .map(|t| format!("{temp:.1}°C", temp = t))
                                .unwrap_or_else(|| "N/A".to_string());
                            humidity = env
                                .relative_humidity
                                .map(|h| format!("{humidity:.1}%", humidity = h))
                                .unwrap_or_else(|| "N/A".to_string());
                        }

                        table.add_row(vec![
                            Cell::new(format!("{num:08x}", num = node_num)),
                            Cell::new(data_type),
                            Cell::new(battery),
                            Cell::new(voltage),
                            Cell::new(temp),
                            Cell::new(humidity),
                        ]);
                    }

                    println!("{table}");
                }
            }
        }
    }

    Ok(())
}

pub async fn handle_telemetry(
    _connection: ConnectionManager,
    telemetry_type: TelemetryType,
    _dest: Option<u32>,
    _format: OutputFormat,
) -> Result<()> {
    match telemetry_type {
        TelemetryType::Device => {
            println!("Device telemetry not yet implemented");
        }
        TelemetryType::Environment => {
            println!("Environment telemetry not yet implemented");
        }
    }

    Ok(())
}
