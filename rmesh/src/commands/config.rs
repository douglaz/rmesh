use crate::cli::ConfigCommands;
use crate::output::{OutputFormat, create_table, print_output};
use crate::utils::{print_info, print_success};
use anyhow::Result;
use colored::*;
use comfy_table::Cell;
use rmesh_core::ConnectionManager;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct ConfigValue {
    key: String,
    value: serde_json::Value,
}

pub async fn handle_config(
    mut connection: ConnectionManager,
    subcommand: ConfigCommands,
    format: OutputFormat,
) -> Result<()> {
    match subcommand {
        ConfigCommands::Get { key } => {
            // Use the core library function
            let value = rmesh_core::config::get_config_value(&mut connection, &key).await?;

            let config_value = ConfigValue {
                key: key.clone(),
                value,
            };

            match format {
                OutputFormat::Json => print_output(&config_value, format),
                OutputFormat::Table => {
                    let mut table = create_table();
                    table.set_header(vec![Cell::new("Key"), Cell::new("Value")]);
                    table.add_row(vec![
                        Cell::new(&config_value.key),
                        Cell::new(config_value.value.to_string()),
                    ]);
                    println!("{table}");
                }
            }

            print_info(&format!("Configuration value for '{key}' retrieved"));
        }

        ConfigCommands::Set { key, value } => {
            // Use the core library function
            rmesh_core::config::set_config_value(&mut connection, &key, &value).await?;

            print_success(&format!("Configuration '{key}' set to '{value}'"));
            println!(
                "{}",
                "Note: Some settings may require a device reboot to take effect".yellow()
            );
        }

        ConfigCommands::List => {
            // Use the core library function
            let config = rmesh_core::config::list_config(&connection).await?;

            match format {
                OutputFormat::Json => print_output(&config, format),
                OutputFormat::Table => {
                    println!(
                        "{}",
                        "Full configuration listing not yet implemented".yellow()
                    );
                    println!(
                        "Available categories: device, position, power, network, display, lora, bluetooth"
                    );
                }
            }
        }
    }

    Ok(())
}
