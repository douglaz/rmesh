use anyhow::Result;
use colored::*;
use serde::Serialize;
use std::time::Duration;
use tokio::time::timeout;

use crate::cli::MessageCommands;
use crate::connection::ConnectionManager;
use crate::output::{print_output, OutputFormat};
use crate::utils::{print_error, print_success, print_info};
use meshtastic::packet::PacketDestination;
use meshtastic::protobufs;

#[derive(Debug, Serialize)]
struct SentMessage {
    pub text: String,
    pub destination: String,
    pub channel: u32,
    pub acknowledged: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ReceivedMessage {
    pub from: String,
    pub from_id: u32,
    pub text: String,
    pub channel: u32,
    pub snr: Option<f32>,
    pub rssi: Option<i32>,
}

pub async fn handle_message(
    mut connection: ConnectionManager,
    subcommand: MessageCommands,
    format: OutputFormat,
) -> Result<()> {
    match subcommand {
        MessageCommands::Send { text, dest, channel, ack } => {
            // Send message with or without ACK
            let acknowledged = if ack {
                print_info("Sending message with acknowledgment request...");
                
                let ack_received = connection.send_text_with_ack(
                    text.clone(),
                    dest.unwrap_or(0xFFFFFFFF),
                    channel,
                    30, // 30 second timeout
                ).await?;
                
                if ack_received {
                    print_success("Message acknowledged by recipient");
                } else {
                    print_warning("Message sent but acknowledgment timed out");
                }
                Some(ack_received)
            } else {
                // Send without ACK
                let api = connection.get_api()?;
                // Create a simple no-op router
                struct NoOpRouter;
                impl meshtastic::packet::PacketRouter<(), std::io::Error> for NoOpRouter {
                    fn handle_packet_from_radio(&mut self, _packet: meshtastic::protobufs::FromRadio) -> std::result::Result<(), std::io::Error> {
                        Ok(())
                    }
                    fn handle_mesh_packet(&mut self, _packet: meshtastic::protobufs::MeshPacket) -> std::result::Result<(), std::io::Error> {
                        Ok(())
                    }
                    fn source_node_id(&self) -> meshtastic::types::NodeId {
                        0u32.into()
                    }
                }
                let mut router = NoOpRouter;
                
                api.send_text(
                    &mut router,
                    text.clone(),
                    match dest {
                        Some(d) => meshtastic::packet::PacketDestination::Node(d.into()),
                        None => meshtastic::packet::PacketDestination::Broadcast,
                    },
                    false,  // want_ack
                    (channel as u32).into(),
                ).await?;
                print_success("Message sent");
                None
            };
            
            let sent_msg = SentMessage {
                text: text.clone(),
                destination: match dest {
                    Some(d) => format!("Node {d:08x}"),
                    None => "Broadcast".to_string(),
                },
                channel,
                acknowledged,
            };
            
            match format {
                OutputFormat::Json => print_output(sent_msg, format)?,
                OutputFormat::Table => {
                    // Already printed status above
                }
            }
        }
        
        MessageCommands::Recv { from, count } => {
            let mut packet_receiver = connection.take_packet_receiver()?;
            let mut received_count = 0;
            let mut messages = Vec::new();
            
            print_info(&format!("Listening for messages{}...", 
                if let Some(f) = from { 
                    format!(" from node {:08x}", f) 
                } else { 
                    String::new() 
                }
            ));
            
            while count == 0 || received_count < count {
                if let Ok(Some(packet)) = timeout(
                    Duration::from_secs(1),
                    packet_receiver.recv()
                ).await {
                    if let Some(payload) = packet.payload_variant {
                        if let protobufs::from_radio::PayloadVariant::Packet(mesh_packet) = payload {
                            // Check if it's a text message
                            if let Some(protobufs::mesh_packet::PayloadVariant::Decoded(data)) = 
                                mesh_packet.payload_variant {
                                
                                // Filter by sender if specified
                                if let Some(filter_from) = from {
                                    if mesh_packet.from != filter_from {
                                        continue;
                                    }
                                }
                                
                                if data.portnum() == protobufs::PortNum::TextMessageApp {
                                    let text = String::from_utf8_lossy(&data.payload);
                                    let msg = ReceivedMessage {
                                        from: format!("{:08x}", mesh_packet.from),
                                        from_id: mesh_packet.from,
                                        text: text.to_string(),
                                        channel: mesh_packet.channel as u32,
                                        snr: mesh_packet.rx_snr,
                                        rssi: mesh_packet.rx_rssi,
                                    };
                                    
                                    match format {
                                        OutputFormat::Json => {
                                            messages.push(msg);
                                        }
                                        OutputFormat::Table => {
                                            println!("{} {} {}: {}",
                                                "[MSG]".green().bold(),
                                                format!("From {:08x}", mesh_packet.from).cyan(),
                                                format!("Ch{}", mesh_packet.channel).yellow(),
                                                text
                                            );
                                            if let Some(snr) = mesh_packet.rx_snr {
                                                println!("      {} SNR: {:.1} dB", 
                                                    "└".dimmed(), 
                                                    snr
                                                );
                                            }
                                        }
                                    }
                                    
                                    received_count += 1;
                                }
                            }
                        }
                    }
                }
            }
            
            if format == OutputFormat::Json {
                print_output(messages, format)?;
            }
        }
        
        MessageCommands::Monitor { from } => {
            let mut packet_receiver = connection.take_packet_receiver()?;
            
            print_info(&format!("Monitoring messages{}... (Press Ctrl+C to stop)", 
                if let Some(f) = from { 
                    format!(" from node {:08x}", f) 
                } else { 
                    String::new() 
                }
            ));
            
            loop {
                if let Some(packet) = packet_receiver.recv().await {
                    if let Some(payload) = packet.payload_variant {
                        if let protobufs::from_radio::PayloadVariant::Packet(mesh_packet) = payload {
                            // Filter by sender if specified
                            if let Some(filter_from) = from {
                                if mesh_packet.from != filter_from {
                                    continue;
                                }
                            }
                            
                            if let Some(protobufs::mesh_packet::PayloadVariant::Decoded(data)) = 
                                mesh_packet.payload_variant {
                                
                                if data.portnum() == protobufs::PortNum::TextMessageApp {
                                    let text = String::from_utf8_lossy(&data.payload);
                                    
                                    match format {
                                        OutputFormat::Json => {
                                            let msg = ReceivedMessage {
                                                from: format!("{:08x}", mesh_packet.from),
                                                from_id: mesh_packet.from,
                                                text: text.to_string(),
                                                channel: mesh_packet.channel as u32,
                                                snr: mesh_packet.rx_snr,
                                                rssi: mesh_packet.rx_rssi,
                                            };
                                            print_output(msg, format)?;
                                        }
                                        OutputFormat::Table => {
                                            let timestamp = chrono::Local::now()
                                                .format("%H:%M:%S");
                                            
                                            println!("{} {} {} {}: {}",
                                                format!("[{}]", timestamp).dimmed(),
                                                "[MSG]".green().bold(),
                                                format!("From {:08x}", mesh_packet.from).cyan(),
                                                format!("Ch{}", mesh_packet.channel).yellow(),
                                                text
                                            );
                                            
                                            if let Some(snr) = mesh_packet.rx_snr {
                                                println!("             {} SNR: {:.1} dB, RSSI: {} dBm", 
                                                    "└".dimmed(), 
                                                    snr,
                                                    mesh_packet.rx_rssi.unwrap_or(0)
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