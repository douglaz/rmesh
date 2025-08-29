use crate::cli::MessageCommands;
use crate::output::{print_output, OutputFormat};
use crate::utils::{print_info, print_success};
use anyhow::Result;
use colored::*;
use rmesh_core::ConnectionManager;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct SentMessage {
    pub text: String,
    pub destination: String,
    pub channel: u32,
    pub acknowledged: Option<bool>,
}

pub async fn handle_message(
    mut connection: ConnectionManager,
    subcommand: MessageCommands,
    format: OutputFormat,
) -> Result<()> {
    match subcommand {
        MessageCommands::Send {
            text,
            dest,
            channel,
            ack,
        } => {
            // Use the core library function
            rmesh_core::message::send_text_message(&mut connection, &text, dest, channel, ack)
                .await?;

            let sent_msg = SentMessage {
                text: text.clone(),
                destination: dest
                    .map(|d| format!("{d:08x}"))
                    .unwrap_or_else(|| "Broadcast".to_string()),
                channel,
                acknowledged: if ack { Some(false) } else { None },
            };

            match format {
                OutputFormat::Json => print_output(&sent_msg, format),
                OutputFormat::Table => {
                    print_success(&format!(
                        "Message sent to {destination} on channel {channel}",
                        destination = sent_msg.destination
                    ));
                    if ack {
                        println!(
                            "{message}",
                            message = "Waiting for acknowledgment...".yellow()
                        );
                    }
                }
            }
        }

        MessageCommands::Recv { from, count } => {
            print_info("Receiving messages...");

            // Get packet receiver
            let mut receiver = connection.take_packet_receiver()?;

            // Use the core library function
            let messages = rmesh_core::message::receive_messages(
                &mut receiver,
                from,
                if count == 0 { None } else { Some(count) },
                30, // 30 second timeout
            )
            .await?;

            if messages.is_empty() {
                print_info("No messages received");
            } else {
                match format {
                    OutputFormat::Json => print_output(&messages, format),
                    OutputFormat::Table => {
                        for msg in messages {
                            println!(
                                "{from} [{channel}]: {text}",
                                from = msg.from.blue().bold(),
                                channel = msg.channel,
                                text = msg.text
                            );
                            if let (Some(snr), Some(rssi)) = (msg.snr, msg.rssi) {
                                println!(
                                    "  {label} SNR: {snr:.1} dB, RSSI: {rssi} dBm",
                                    label = "Signal:".dimmed()
                                );
                            }
                        }
                    }
                }
            }
        }

        MessageCommands::Monitor { from } => {
            print_info("Monitoring messages... Press Ctrl+C to stop");

            // Get packet receiver
            let mut receiver = connection.take_packet_receiver()?;

            // Use the core library function
            rmesh_core::message::monitor_messages(&mut receiver, from, |msg| {
                match format {
                    OutputFormat::Json => {
                        if let Ok(json) = serde_json::to_string(&msg) {
                            println!("{json}");
                        }
                    }
                    OutputFormat::Table => {
                        println!("{} [{}]: {}", msg.from.blue().bold(), msg.channel, msg.text);
                        if let (Some(snr), Some(rssi)) = (msg.snr, msg.rssi) {
                            println!(
                                "  {} SNR: {:.1} dB, RSSI: {} dBm",
                                "Signal:".dimmed(),
                                snr,
                                rssi
                            );
                        }
                    }
                }
                Ok(())
            })
            .await?;
        }
    }

    Ok(())
}
