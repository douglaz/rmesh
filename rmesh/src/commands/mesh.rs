use crate::cli::MeshCommands;
use crate::output::{OutputFormat, create_table, print_output};
use crate::utils::print_info;
use anyhow::Result;
use colored::*;
use comfy_table::Cell;
use rmesh_core::ConnectionManager;

pub async fn handle_mesh(
    mut connection: ConnectionManager,
    subcommand: MeshCommands,
    format: OutputFormat,
) -> Result<()> {
    match subcommand {
        MeshCommands::Topology => {
            print_info("Analyzing mesh network topology...");

            // Get topology from core library
            let topology = rmesh_core::mesh::get_topology(&connection).await?;

            match format {
                OutputFormat::Json => print_output(&topology, format),
                OutputFormat::Table => {
                    // Print network summary
                    if let Some(my_node) = topology.get("my_node") {
                        println!("\n{title}", title = "My Node:".bold().cyan());
                        if let Some(node_obj) = my_node.as_object() {
                            if let Some(id) = node_obj.get("node_id") {
                                println!("  ID: {id}", id = id.as_str().unwrap_or("unknown"));
                            }
                            if let Some(num) = node_obj.get("node_num") {
                                println!("  Number: {num:08x}", num = num.as_u64().unwrap_or(0));
                            }
                        }
                    }

                    // Print nodes table
                    if let Some(nodes) = topology.get("nodes").and_then(|n| n.as_array()) {
                        println!(
                            "\n{}",
                            format!("Network Nodes ({total} total):", total = nodes.len())
                                .bold()
                                .green()
                        );

                        let mut table = create_table();
                        table.set_header(vec![
                            Cell::new("Node ID"),
                            Cell::new("Name"),
                            Cell::new("SNR (dB)"),
                            Cell::new("RSSI (dBm)"),
                            Cell::new("Last Heard"),
                        ]);

                        for node in nodes {
                            if let Some(obj) = node.as_object() {
                                table.add_row(vec![
                                    Cell::new(
                                        obj.get("id").and_then(|v| v.as_str()).unwrap_or("unknown"),
                                    ),
                                    Cell::new(
                                        obj.get("name")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("unknown"),
                                    ),
                                    Cell::new(
                                        obj.get("snr")
                                            .and_then(|v| v.as_f64())
                                            .map(|s| format!("{s:.1}"))
                                            .unwrap_or_else(|| "N/A".to_string()),
                                    ),
                                    Cell::new(
                                        obj.get("rssi")
                                            .and_then(|v| v.as_i64())
                                            .map(|r| r.to_string())
                                            .unwrap_or_else(|| "N/A".to_string()),
                                    ),
                                    Cell::new(
                                        obj.get("last_heard")
                                            .and_then(|v| v.as_u64())
                                            .map(|h| {
                                                let now = std::time::SystemTime::now()
                                                    .duration_since(std::time::UNIX_EPOCH)
                                                    .unwrap()
                                                    .as_secs();
                                                let ago = now.saturating_sub(h);
                                                if ago < 60 {
                                                    format!("{ago}s ago")
                                                } else if ago < 3600 {
                                                    format!("{minutes}m ago", minutes = ago / 60)
                                                } else {
                                                    format!("{hours}h ago", hours = ago / 3600)
                                                }
                                            })
                                            .unwrap_or_else(|| "Never".to_string()),
                                    ),
                                ]);
                            }
                        }

                        println!("{table}");
                    }

                    // Print network edges if available
                    if let Some(edges) = topology.get("edges").and_then(|e| e.as_array()) {
                        if !edges.is_empty() {
                            println!(
                                "\n{}",
                                format!("Direct Connections ({count}):", count = edges.len())
                                    .bold()
                                    .blue()
                            );
                            for edge in edges {
                                if let Some(obj) = edge.as_object() {
                                    let from = obj
                                        .get("from")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown");
                                    let to =
                                        obj.get("to").and_then(|v| v.as_str()).unwrap_or("unknown");
                                    let snr = obj.get("snr").and_then(|v| v.as_f64());
                                    let rssi = obj.get("rssi").and_then(|v| v.as_i64());

                                    print!("  {} → {}", from.yellow(), to.yellow());
                                    if let Some(s) = snr {
                                        print!(" (SNR: {:.1} dB", s);
                                        if let Some(r) = rssi {
                                            print!(", RSSI: {} dBm", r);
                                        }
                                        print!(")");
                                    }
                                    println!();
                                }
                            }
                        }
                    }
                }
            }
        }

        MeshCommands::Traceroute { dest } => {
            print_info(&format!("Performing traceroute to node {dest:08x}..."));

            // Perform traceroute
            let hops = rmesh_core::mesh::traceroute(&mut connection, dest).await?;

            if hops.is_empty() {
                println!(
                    "{}",
                    "No route found or traceroute not yet fully implemented".yellow()
                );
                return Ok(());
            }

            match format {
                OutputFormat::Json => print_output(&hops, format),
                OutputFormat::Table => {
                    println!(
                        "\n{title}",
                        title = format!("Traceroute to {dest:08x}:").bold().green()
                    );

                    let mut table = create_table();
                    table.set_header(vec![
                        Cell::new("Hop"),
                        Cell::new("Node ID"),
                        Cell::new("Name"),
                        Cell::new("SNR"),
                        Cell::new("RSSI"),
                    ]);

                    for hop in hops {
                        table.add_row(vec![
                            Cell::new(hop.hop_number),
                            Cell::new(format!("{node_id:08x}", node_id = hop.node_id)),
                            Cell::new(&hop.node_name),
                            Cell::new(
                                hop.snr
                                    .map(|s| format!("{s:.1} dB"))
                                    .unwrap_or_else(|| "N/A".to_string()),
                            ),
                            Cell::new(
                                hop.rssi
                                    .map(|r| format!("{} dBm", r))
                                    .unwrap_or_else(|| "N/A".to_string()),
                            ),
                        ]);
                    }

                    println!("{table}");
                }
            }
        }

        MeshCommands::Neighbors => {
            print_info("Finding direct mesh neighbors...");

            // Get neighbors
            let neighbors = rmesh_core::mesh::get_neighbors(&connection).await?;

            if neighbors.is_empty() {
                println!("{message}", message = "No direct neighbors found".yellow());
                return Ok(());
            }

            match format {
                OutputFormat::Json => print_output(&neighbors, format),
                OutputFormat::Table => {
                    println!(
                        "\n{}",
                        format!("Direct Neighbors ({} found):", neighbors.len())
                            .bold()
                            .green()
                    );

                    let mut table = create_table();
                    table.set_header(vec![
                        Cell::new("Node ID"),
                        Cell::new("Name"),
                        Cell::new("SNR (dB)"),
                        Cell::new("RSSI (dBm)"),
                        Cell::new("Last Heard"),
                    ]);

                    for neighbor in neighbors {
                        table.add_row(vec![
                            Cell::new(&neighbor.id),
                            Cell::new(&neighbor.user.long_name),
                            Cell::new(
                                neighbor
                                    .snr
                                    .map(|s| format!("{:.1}", s))
                                    .unwrap_or_else(|| "N/A".to_string()),
                            ),
                            Cell::new(
                                neighbor
                                    .rssi
                                    .map(|r| r.to_string())
                                    .unwrap_or_else(|| "N/A".to_string()),
                            ),
                            Cell::new(
                                neighbor
                                    .last_heard
                                    .map(|h| {
                                        let now = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap()
                                            .as_secs();
                                        let ago = now.saturating_sub(h);
                                        if ago < 60 {
                                            format!("{}s ago", ago)
                                        } else if ago < 3600 {
                                            format!("{}m ago", ago / 60)
                                        } else {
                                            format!("{}h ago", ago / 3600)
                                        }
                                    })
                                    .unwrap_or_else(|| "Never".to_string()),
                            ),
                        ]);
                    }

                    println!("{table}");

                    // Calculate and show network stats
                    if let Ok(stats) = rmesh_core::mesh::get_network_stats(&connection).await {
                        println!("\n{title}", title = "Network Statistics:".bold().cyan());
                        println!("  Total Nodes: {total}", total = stats.total_nodes);
                        println!("  Active Nodes: {active}", active = stats.active_nodes);
                        println!(
                            "  Direct Neighbors: {neighbors}",
                            neighbors = stats.neighbors
                        );
                        if let Some(snr) = stats.average_snr {
                            println!("  Average SNR: {snr:.1} dB");
                        }
                        if let Some(rssi) = stats.average_rssi {
                            println!("  Average RSSI: {rssi} dBm");
                        }
                        println!(
                            "  Mesh Health: {health}",
                            health = match stats.mesh_health.as_str() {
                                "Excellent" => stats.mesh_health.green(),
                                "Good" => stats.mesh_health.green(),
                                "Fair" => stats.mesh_health.yellow(),
                                "Weak" => stats.mesh_health.red(),
                                "Isolated" => stats.mesh_health.red().bold(),
                                _ => stats.mesh_health.normal(),
                            }
                        );
                    }
                }
            }
        }
    }

    Ok(())
}
