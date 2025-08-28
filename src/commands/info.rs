use anyhow::Result;
use serde::Serialize;
use comfy_table::Cell;
use colored::*;

use crate::cli::InfoCommands;
use crate::connection::ConnectionManager;
use crate::output::{create_table, print_output, OutputFormat};
use crate::utils::print_success;

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

#[derive(Debug, Serialize)]
struct NodeInfo {
    pub id: String,
    pub num: u32,
    pub user: String,
    pub snr: Option<f32>,
    pub last_heard: Option<String>,
}

pub async fn handle_info(
    mut connection: ConnectionManager,
    subcommand: InfoCommands,
    format: OutputFormat,
) -> Result<()> {
    let api = connection.api()?;
    
    match subcommand {
        InfoCommands::Radio => {
            // Get device metadata
            let metadata = api.get_metadata();
            let config = api.get_config();
            let channels = api.get_channels();
            
            let radio_info = RadioInfo {
                firmware_version: metadata.firmware_version.clone(),
                hardware_model: format!("{:?}", metadata.hw_model),
                region: format!("{:?}", config.lora.region()),
                node_id: format!("{:08x}", metadata.my_node_num),
                node_num: metadata.my_node_num,
                has_gps: metadata.has_gps,
                num_channels: channels.len(),
            };
            
            match format {
                OutputFormat::Json => print_output(radio_info, format)?,
                OutputFormat::Table => {
                    let mut table = create_table();
                    table.set_header(vec!["Property", "Value"]);
                    
                    table.add_row(vec![
                        Cell::new("Firmware Version"),
                        Cell::new(radio_info.firmware_version),
                    ]);
                    table.add_row(vec![
                        Cell::new("Hardware Model"),
                        Cell::new(radio_info.hardware_model),
                    ]);
                    table.add_row(vec![
                        Cell::new("Region"),
                        Cell::new(radio_info.region),
                    ]);
                    table.add_row(vec![
                        Cell::new("Node ID"),
                        Cell::new(radio_info.node_id),
                    ]);
                    table.add_row(vec![
                        Cell::new("Node Number"),
                        Cell::new(radio_info.node_num),
                    ]);
                    table.add_row(vec![
                        Cell::new("GPS"),
                        Cell::new(if radio_info.has_gps { "Yes" } else { "No" }),
                    ]);
                    table.add_row(vec![
                        Cell::new("Channels"),
                        Cell::new(radio_info.num_channels),
                    ]);
                    
                    println!("{}", "Radio Information".bold());
                    println!("{}", table);
                }
            }
        }
        
        InfoCommands::Nodes => {
            let nodes = api.get_nodes();
            let mut node_list = Vec::new();
            
            for (id, node_info) in nodes.iter() {
                node_list.push(NodeInfo {
                    id: format!("{:08x}", id),
                    num: *id,
                    user: node_info.user.long_name.clone(),
                    snr: node_info.snr,
                    last_heard: node_info.last_heard.map(|t| {
                        format!("{} seconds ago", t)
                    }),
                });
            }
            
            match format {
                OutputFormat::Json => print_output(node_list, format)?,
                OutputFormat::Table => {
                    let mut table = create_table();
                    table.set_header(vec!["Node ID", "Number", "User", "SNR", "Last Heard"]);
                    
                    for node in node_list {
                        table.add_row(vec![
                            Cell::new(node.id),
                            Cell::new(node.num),
                            Cell::new(node.user),
                            Cell::new(node.snr.map_or("-".to_string(), |s| format!("{:.1}", s))),
                            Cell::new(node.last_heard.unwrap_or_else(|| "Never".to_string())),
                        ]);
                    }
                    
                    println!("{}", "Node List".bold());
                    println!("{}", table);
                }
            }
        }
        
        InfoCommands::Channels => {
            let channels = api.get_channels();
            
            match format {
                OutputFormat::Json => print_output(channels, format)?,
                OutputFormat::Table => {
                    let mut table = create_table();
                    table.set_header(vec!["Index", "Name", "Role", "PSK Set"]);
                    
                    for (index, channel) in channels.iter().enumerate() {
                        let settings = &channel.settings;
                        table.add_row(vec![
                            Cell::new(index),
                            Cell::new(settings.name.as_ref().unwrap_or(&"Unnamed".to_string())),
                            Cell::new(format!("{:?}", channel.role())),
                            Cell::new(if settings.psk.as_ref().map_or(false, |p| !p.is_empty()) { 
                                "Yes" 
                            } else { 
                                "No" 
                            }),
                        ]);
                    }
                    
                    println!("{}", "Channel Configuration".bold());
                    println!("{}", table);
                }
            }
        }
        
        _ => {
            print_success("Feature not yet implemented");
        }
    }
    
    Ok(())
}

pub async fn handle_telemetry(
    _connection: ConnectionManager,
    telemetry_type: Option<String>,
    dest: Option<u32>,
    format: OutputFormat,
) -> Result<()> {
    let telemetry_info = format!("Requesting telemetry type: {:?} from node: {:?}", 
                                telemetry_type, dest);
    
    match format {
        OutputFormat::Json => {
            let data = serde_json::json!({
                "telemetry_type": telemetry_type,
                "destination": dest,
                "status": "not_implemented"
            });
            print_output(data, format)?;
        }
        OutputFormat::Table => {
            print_success(&telemetry_info);
            print_success("Telemetry feature not yet implemented");
        }
    }
    
    Ok(())
}