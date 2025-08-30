use crate::cli::AdminCommands;
use crate::output::OutputFormat;
use crate::utils::{print_error, print_success, print_warning};
use anyhow::{Result, bail};
use colored::*;
use rmesh_core::{ConnectionManager, device};

pub async fn handle_admin(
    mut connection: ConnectionManager,
    subcommand: AdminCommands,
    _format: OutputFormat,
) -> Result<()> {
    match subcommand {
        AdminCommands::Reboot { confirm } => {
            if !confirm {
                print_warning("Reboot requires confirmation. Use --confirm to proceed.");
                bail!("Operation cancelled");
            }

            print_warning("Sending reboot command to device...");
            device::reboot_device(&mut connection, Some(5)).await?;
            print_success("Reboot command sent. Device will restart in 5 seconds.");
        }

        AdminCommands::FactoryReset { confirm } => {
            if !confirm {
                print_error("FACTORY RESET WILL ERASE ALL SETTINGS!");
                println!(
                    "{message}",
                    message = "This operation cannot be undone.".red().bold()
                );
                print_warning("Use --confirm to proceed with factory reset.");
                bail!("Operation cancelled");
            }

            print_warning("Sending factory reset command...");
            device::factory_reset_device(&mut connection).await?;
            print_success("Factory reset command sent. Device will reset to defaults.");
        }

        AdminCommands::Shutdown { confirm } => {
            if !confirm {
                print_warning("Shutdown requires confirmation. Use --confirm to proceed.");
                bail!("Operation cancelled");
            }

            print_warning("Sending shutdown command to device...");
            device::shutdown_device(&mut connection, Some(5)).await?;
            print_success("Shutdown command sent. Device will power off in 5 seconds.");
        }
    }

    Ok(())
}
