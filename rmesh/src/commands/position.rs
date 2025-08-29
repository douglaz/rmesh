use crate::cli::PositionCommands;
use crate::output::{OutputFormat, create_table, print_output};
use crate::utils::{print_info, print_success, print_warning};
use anyhow::Result;
use colored::*;
use comfy_table::Cell;
use rmesh_core::ConnectionManager;

pub async fn handle_position(
    mut connection: ConnectionManager,
    subcommand: PositionCommands,
    format: OutputFormat,
) -> Result<()> {
    match subcommand {
        PositionCommands::Get { node } => {
            // Use the core library function
            let position = rmesh_core::position::get_position(&connection, node).await?;

            if let Some(pos) = position {
                match format {
                    OutputFormat::Json => print_output(&pos, format),
                    OutputFormat::Table => {
                        let mut table = create_table();
                        table.set_header(vec![Cell::new("Property"), Cell::new("Value")]);
                        table.add_row(vec![Cell::new("Node ID"), Cell::new(&pos.node_id)]);
                        table.add_row(vec![Cell::new("Node Number"), Cell::new(pos.node_num)]);
                        table.add_row(vec![
                            Cell::new("Latitude"),
                            Cell::new(format!("{:.6}", pos.latitude)),
                        ]);
                        table.add_row(vec![
                            Cell::new("Longitude"),
                            Cell::new(format!("{:.6}", pos.longitude)),
                        ]);
                        if let Some(alt) = pos.altitude {
                            table.add_row(vec![
                                Cell::new("Altitude"),
                                Cell::new(format!("{alt} m")),
                            ]);
                        }
                        if let Some(time) = &pos.time {
                            table.add_row(vec![Cell::new("Time"), Cell::new(time)]);
                        }
                        println!("{table}");
                    }
                }
            } else {
                print_warning("No position data available for this node");
            }
        }

        PositionCommands::Set { lat, lon, alt } => {
            // Use the core library function
            rmesh_core::position::set_position(&mut connection, lat, lon, alt).await?;

            print_success(&format!(
                "Position set to: {lat:.6}, {lon:.6}{altitude}",
                altitude = alt.map(|a| format!(" at {a} m")).unwrap_or_default()
            ));
        }

        PositionCommands::Track { nodes } => {
            print_info("Starting position tracking...");
            println!(
                "{message}",
                message = "Press Ctrl+C to stop tracking".yellow()
            );

            // Get packet receiver
            let mut receiver = connection.take_packet_receiver()?;

            // Use the core library function
            let positions = rmesh_core::position::track_positions(
                &mut receiver,
                nodes,
                60, // 60 second timeout
            )
            .await?;

            if positions.is_empty() {
                print_warning("No position updates received");
            } else {
                match format {
                    OutputFormat::Json => print_output(&positions, format),
                    OutputFormat::Table => {
                        let mut table = create_table();
                        table.set_header(vec![
                            Cell::new("Node ID"),
                            Cell::new("Latitude"),
                            Cell::new("Longitude"),
                            Cell::new("Altitude"),
                            Cell::new("Time"),
                        ]);

                        for pos in positions {
                            table.add_row(vec![
                                Cell::new(&pos.node_id),
                                Cell::new(format!("{:.6}", pos.latitude)),
                                Cell::new(format!("{:.6}", pos.longitude)),
                                Cell::new(
                                    pos.altitude
                                        .map(|a| format!("{a} m"))
                                        .unwrap_or_else(|| "N/A".to_string()),
                                ),
                                Cell::new(pos.time.unwrap_or_else(|| "Unknown".to_string())),
                            ]);
                        }

                        println!("{table}");
                    }
                }
            }
        }
    }

    Ok(())
}
