use crate::cli::ChannelCommands;
use crate::output::{OutputFormat, print_output};
use crate::utils::{print_error, print_info, print_success};
use anyhow::Result;
use rmesh_core::ConnectionManager;

pub async fn handle_channel(
    mut connection: ConnectionManager,
    subcommand: ChannelCommands,
    format: OutputFormat,
) -> Result<()> {
    match subcommand {
        ChannelCommands::List => {
            // List all channels
            let channels = rmesh_core::channel::list_channels(&connection).await?;

            match format {
                OutputFormat::Json => print_output(&channels, format),
                OutputFormat::Table => {
                    if channels.is_empty() {
                        print_info("No channels configured");
                    } else {
                        use comfy_table::{Cell, Table};
                        let mut table = Table::new();
                        table.set_header(vec![
                            Cell::new("Index"),
                            Cell::new("Name"),
                            Cell::new("Role"),
                            Cell::new("PSK"),
                        ]);

                        for channel in channels {
                            table.add_row(vec![
                                Cell::new(channel.index.to_string()),
                                Cell::new(&channel.name),
                                Cell::new(&channel.role),
                                Cell::new(if channel.has_psk { "Yes" } else { "No" }),
                            ]);
                        }

                        println!("{table}");
                    }
                }
            }
        }

        ChannelCommands::Add { name, psk } => {
            print_info(&format!("Adding channel '{name}'..."));

            // Add the channel
            rmesh_core::channel::add_channel(&mut connection, &name, psk.as_deref()).await?;

            print_success(&format!("Channel '{name}' added successfully"));

            // Wait a moment for the channel to be processed
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            // List channels to show the new one
            let channels = rmesh_core::channel::list_channels(&connection).await?;
            match format {
                OutputFormat::Json => print_output(&channels, format),
                OutputFormat::Table => {
                    print_info("Current channels:");
                    for channel in channels {
                        println!("  [{}] {} ({})", channel.index, channel.name, channel.role);
                    }
                }
            }
        }

        ChannelCommands::Delete { index } => {
            if index == 0 {
                print_error("Cannot delete primary channel (index 0)");
                return Ok(());
            }

            print_info(&format!("Deleting channel at index {index}..."));

            // Delete the channel
            rmesh_core::channel::delete_channel(&mut connection, index).await?;

            print_success(&format!("Channel at index {index} deleted"));
        }

        ChannelCommands::Set {
            index,
            name,
            psk,
            uplink,
            downlink,
        } => {
            print_info(&format!("Configuring channel at index {index}..."));

            // For now, we'll use the simpler set_channel that doesn't support uplink/downlink
            // TODO: Update rmesh_core::channel::set_channel to support uplink/downlink
            if uplink.is_some() || downlink.is_some() {
                print_info("Note: Uplink/downlink settings not yet supported");
            }

            // Set the channel configuration
            rmesh_core::channel::set_channel(
                &mut connection,
                index,
                name.as_deref(),
                psk.as_deref(),
            )
            .await?;

            print_success(&format!("Channel {index} updated successfully"));
        }
    }

    Ok(())
}
