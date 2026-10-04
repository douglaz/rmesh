mod admin;
mod channel;
mod config;
mod info;
mod mesh;
mod message;
mod position;

use crate::cli::{Cli, Commands};
use crate::output::OutputFormat;
use anyhow::Result;
use rmesh_core::ConnectionManager;

pub async fn handle_command(cli: Cli) -> Result<()> {
    // Determine output format
    let output_format = if cli.json {
        OutputFormat::Json
    } else {
        OutputFormat::Table
    };

    // Establish connection
    let mut connection =
        ConnectionManager::new(cli.port.clone(), cli.ble.clone(), cli.timeout_duration()).await?;

    // Connect and handle the specific command. Connecting is inside the block so Ctrl+C
    // during the configuration dump also releases the radio.
    let command = async {
        connection.connect().await?;
        match cli.command {
            Commands::Info { subcommand } => {
                info::handle_info(&mut connection, subcommand, output_format).await
            }
            Commands::Message { subcommand } => {
                message::handle_message(&mut connection, subcommand, output_format).await
            }
            Commands::Config { subcommand } => {
                config::handle_config(&mut connection, subcommand, output_format).await
            }
            Commands::Channel { subcommand } => {
                channel::handle_channel(&mut connection, subcommand, output_format).await
            }
            Commands::Position { subcommand } => {
                position::handle_position(&mut connection, subcommand, output_format).await
            }
            Commands::Mesh { subcommand } => {
                mesh::handle_mesh(&mut connection, subcommand, output_format).await
            }
            Commands::Telemetry {
                telemetry_type,
                dest,
            } => {
                // Handle telemetry command
                info::handle_telemetry(&mut connection, telemetry_type, dest, output_format).await
            }
            Commands::Admin { subcommand } => {
                admin::handle_admin(&mut connection, subcommand, output_format).await
            }
        }
    };
    // Let Ctrl+C end the command rather than the process, so the radio is still released
    // below.
    let (result, interrupted) = tokio::select! {
        result = command => (result, false),
        _ = tokio::signal::ctrl_c() => (Ok(()), true),
    };

    // Release the radio even when the command failed: until it hears a disconnect, the
    // firmware keeps Bluetooth off.
    if let Err(e) = connection.disconnect().await {
        tracing::debug!("Failed to disconnect cleanly: {e}");
    }

    // Keep the exit status a signal-killed process would have had, so a script cannot
    // mistake an interrupted `config set` for one that completed.
    if interrupted {
        std::process::exit(130);
    }

    result
}
