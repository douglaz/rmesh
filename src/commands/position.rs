use anyhow::Result;
use serde::Serialize;
use crate::cli::PositionCommands;
use crate::connection::ConnectionManager;
use crate::output::{print_output, OutputFormat, create_table};
use crate::utils::{print_success, print_info, print_warning};
use meshtastic::packet::PacketDestination;
use meshtastic::{protobufs, Message};
use colored::*;
use comfy_table::Cell;
use tokio::time::{timeout, Duration};

#[derive(Debug, Serialize)]
struct Position {
    node_id: String,
    node_num: u32,
    latitude: f64,
    longitude: f64,
    altitude: Option<i32>,
    time: Option<String>,
}

pub async fn handle_position(
    mut connection: ConnectionManager,
    subcommand: PositionCommands,
    format: OutputFormat,
) -> Result<()> {
    match subcommand {
        PositionCommands::Get { node } => {
            let api = connection.api()?;
            let nodes = api.get_nodes();
            let metadata = api.get_metadata();
            
            // Determine which node to get position for
            let target_node = node.unwrap_or(metadata.my_node_num);
            
            if let Some(node_info) = nodes.get(&target_node) {
                if let Some(pos) = &node_info.position {
                    let position = Position {
                        node_id: format!("{:08x}", target_node),
                        node_num: target_node,
                        latitude: pos.latitude_i as f64 / 1e7,
                        longitude: pos.longitude_i as f64 / 1e7,
                        altitude: if pos.altitude != 0 { Some(pos.altitude) } else { None },
                        time: pos.time.map(|t| {
                            chrono::DateTime::from_timestamp(t as i64, 0)
                                .map(|dt| dt.to_rfc3339())
                                .unwrap_or_else(|| format!("{} seconds ago", t))
                        }),
                    };
                    
                    match format {
                        OutputFormat::Json => print_output(position, format)?,
                        OutputFormat::Table => {
                            let mut table = create_table();
                            table.set_header(vec!["Property", "Value"]);
                            
                            table.add_row(vec![
                                Cell::new("Node ID"),
                                Cell::new(position.node_id),
                            ]);
                            table.add_row(vec![
                                Cell::new("Latitude"),
                                Cell::new(format!("{:.6}", position.latitude)),
                            ]);
                            table.add_row(vec![
                                Cell::new("Longitude"),
                                Cell::new(format!("{:.6}", position.longitude)),
                            ]);
                            if let Some(alt) = position.altitude {
                                table.add_row(vec![
                                    Cell::new("Altitude"),
                                    Cell::new(format!("{} m", alt)),
                                ]);
                            }
                            if let Some(time) = position.time {
                                table.add_row(vec![
                                    Cell::new("Last Update"),
                                    Cell::new(time),
                                ]);
                            }
                            
                            println!("{}", "Position Information".bold());
                            println!("{}", table);
                        }
                    }
                } else {
                    print_warning(&format!("No position data available for node {:08x}", target_node));
                }
            } else {
                print_warning(&format!("Node {:08x} not found", target_node));
            }
        }
        
        PositionCommands::Set { lat, lon, alt } => {
            let api = connection.api_mut()?;
            
            print_info(&format!("Setting position: lat={:.6}, lon={:.6}, alt={:?}", lat, lon, alt));
            
            // Create position message
            let position = protobufs::Position {
                latitude_i: (lat * 1e7) as i32,
                longitude_i: (lon * 1e7) as i32,
                altitude: alt.unwrap_or(0),
                time: chrono::Utc::now().timestamp() as u32,
                ..Default::default()
            };
            
            // Send position
            api.send_position(
                position,
                PacketDestination::Broadcast,
                true, // want_ack
                0, // channel
            ).await?;
            
            print_success("Position update sent successfully");
        }
        
        PositionCommands::Track { nodes } => {
            let mut packet_receiver = connection.take_packet_receiver()?;
            
            if nodes.is_empty() {
                print_info("Tracking position updates from all nodes... (Press Ctrl+C to stop)");
            } else {
                print_info(&format!(
                    "Tracking position updates from nodes: {} (Press Ctrl+C to stop)",
                    nodes.iter().map(|n| format!("{:08x}", n)).collect::<Vec<_>>().join(", ")
                ));
            }
            
            loop {
                if let Ok(Some(packet)) = timeout(
                    Duration::from_secs(1),
                    packet_receiver.recv()
                ).await {
                    if let Some(payload) = packet.payload_variant {
                        if let protobufs::from_radio::PayloadVariant::Packet(mesh_packet) = payload {
                            // Filter by nodes if specified
                            if !nodes.is_empty() && !nodes.contains(&mesh_packet.from) {
                                continue;
                            }
                            
                            if let Some(protobufs::mesh_packet::PayloadVariant::Decoded(data)) = 
                                mesh_packet.payload_variant {
                                
                                if data.portnum() == protobufs::PortNum::PositionApp {
                                    if let Ok(position) = protobufs::Position::decode(data.payload.as_slice()) {
                                        let lat = position.latitude_i as f64 / 1e7;
                                        let lon = position.longitude_i as f64 / 1e7;
                                        let timestamp = chrono::Local::now().format("%H:%M:%S");
                                        
                                        match format {
                                            OutputFormat::Json => {
                                                let pos = Position {
                                                    node_id: format!("{:08x}", mesh_packet.from),
                                                    node_num: mesh_packet.from,
                                                    latitude: lat,
                                                    longitude: lon,
                                                    altitude: if position.altitude != 0 { 
                                                        Some(position.altitude) 
                                                    } else { 
                                                        None 
                                                    },
                                                    time: Some(timestamp.to_string()),
                                                };
                                                print_output(pos, format)?;
                                            }
                                            OutputFormat::Table => {
                                                println!("{} {} {} Position: {:.6}, {:.6}{}",
                                                    format!("[{}]", timestamp).dimmed(),
                                                    "[POS]".yellow().bold(),
                                                    format!("From {:08x}", mesh_packet.from).cyan(),
                                                    lat, lon,
                                                    if position.altitude != 0 {
                                                        format!(", {} m", position.altitude)
                                                    } else {
                                                        String::new()
                                                    }
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    Ok(())
}