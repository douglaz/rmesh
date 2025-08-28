use anyhow::Result;
use comfy_table::Cell;
use serde::Serialize;

use crate::cli::{InfoCommands, TelemetryType};
use crate::output::{create_table, print_output, OutputFormat};
use rmesh_core::ConnectionManager;

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
    connection: ConnectionManager,
    subcommand: InfoCommands,
    format: OutputFormat,
) -> Result<()> {
    match subcommand {
        InfoCommands::Radio => {
            // Note: In a real implementation, we would get this from device packets
            let radio_info = RadioInfo {
                firmware_version: "2.0.0".to_string(),
                hardware_model: "TBEAM".to_string(),
                region: "US".to_string(),
                node_id: "00000000".to_string(),
                node_num: 0,
                has_gps: true,
                num_channels: 1,
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
                    println!("{}", table);
                }
            }
        }

        InfoCommands::Nodes => {
            // Use the core library function
            let nodes = rmesh_core::mesh::get_nodes(&connection).await?;

            if nodes.is_empty() {
                println!("No nodes found in the mesh network");
                return Ok(());
            }

            match format {
                OutputFormat::Json => print_output(&nodes, format),
                OutputFormat::Table => {
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
                                    .map(|s| format!("{:.1}", s))
                                    .unwrap_or_else(|| "N/A".to_string()),
                            ),
                            Cell::new(
                                node.last_heard
                                    .map(|h| h.to_string())
                                    .unwrap_or_else(|| "Never".to_string()),
                            ),
                        ]);
                    }

                    println!("{}", table);
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

                    println!("{}", table);
                }
            }
        }

        InfoCommands::Metrics => {
            println!("Device metrics not yet implemented");
        }

        InfoCommands::Position => {
            // Get position data from device state
            let state = connection.get_device_state().await;

            if state.positions.is_empty() {
                println!("No position data available");
                return Ok(());
            }

            match format {
                OutputFormat::Json => print_output(&state.positions, format),
                OutputFormat::Table => {
                    let mut table = create_table();
                    table.set_header(vec![
                        Cell::new("Node ID"),
                        Cell::new("Latitude"),
                        Cell::new("Longitude"),
                        Cell::new("Altitude"),
                        Cell::new("Time"),
                    ]);

                    for (node_num, position) in state.positions {
                        table.add_row(vec![
                            Cell::new(format!("{:08x}", node_num)),
                            Cell::new(format!("{:.6}", position.latitude)),
                            Cell::new(format!("{:.6}", position.longitude)),
                            Cell::new(
                                position
                                    .altitude
                                    .map(|a| a.to_string())
                                    .unwrap_or_else(|| "N/A".to_string()),
                            ),
                            Cell::new(position.time.as_deref().unwrap_or("N/A")),
                        ]);
                    }

                    println!("{}", table);
                }
            }
        }

        InfoCommands::Telemetry => {
            // Get telemetry data from device state
            let state = connection.get_device_state().await;

            if state.telemetry.is_empty() {
                println!("No telemetry data available");
                return Ok(());
            }

            match format {
                OutputFormat::Json => print_output(&state.telemetry, format),
                OutputFormat::Table => {
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
                                .map(|b| format!("{}%", b))
                                .unwrap_or_else(|| "N/A".to_string());
                            voltage = device
                                .voltage
                                .map(|v| format!("{:.2}V", v))
                                .unwrap_or_else(|| "N/A".to_string());
                        }

                        if let Some(env) = &telemetry.environment_metrics {
                            data_type = if data_type == "None" {
                                "Environment".to_string()
                            } else {
                                format!("{}, Environment", data_type)
                            };
                            temp = env
                                .temperature
                                .map(|t| format!("{:.1}°C", t))
                                .unwrap_or_else(|| "N/A".to_string());
                            humidity = env
                                .relative_humidity
                                .map(|h| format!("{:.1}%", h))
                                .unwrap_or_else(|| "N/A".to_string());
                        }

                        table.add_row(vec![
                            Cell::new(format!("{:08x}", node_num)),
                            Cell::new(data_type),
                            Cell::new(battery),
                            Cell::new(voltage),
                            Cell::new(temp),
                            Cell::new(humidity),
                        ]);
                    }

                    println!("{}", table);
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
