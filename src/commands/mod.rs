mod info;
mod message;
mod config;
mod channel;
mod position;
mod mesh;
mod admin;

use anyhow::Result;
use crate::cli::{Cli, Commands};
use crate::connection::ConnectionManager;
use crate::output::OutputFormat;

pub async fn handle_command(cli: Cli) -> Result<()> {
    // Determine output format
    let output_format = if cli.json {
        OutputFormat::Json
    } else {
        OutputFormat::Table
    };
    
    // Establish connection
    let mut connection = ConnectionManager::new(
        cli.port.clone(),
        cli.ble.clone(),
        cli.timeout_duration(),
    ).await?;
    
    // Connect to the device
    connection.connect().await?;
    
    // Handle the specific command
    match cli.command {
        Commands::Info { subcommand } => {
            info::handle_info(connection, subcommand, output_format).await
        }
        Commands::Message { subcommand } => {
            message::handle_message(connection, subcommand, output_format).await
        }
        Commands::Config { subcommand } => {
            config::handle_config(connection, subcommand, output_format).await
        }
        Commands::Channel { subcommand } => {
            channel::handle_channel(connection, subcommand, output_format).await
        }
        Commands::Position { subcommand } => {
            position::handle_position(connection, subcommand, output_format).await
        }
        Commands::Mesh { subcommand } => {
            mesh::handle_mesh(connection, subcommand, output_format).await
        }
        Commands::Telemetry { telemetry_type, dest } => {
            // Handle telemetry command
            info::handle_telemetry(connection, telemetry_type, dest, output_format).await
        }
        Commands::Admin { subcommand } => {
            admin::handle_admin(connection, subcommand, output_format).await
        }
    }
}