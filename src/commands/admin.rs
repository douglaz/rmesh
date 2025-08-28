use anyhow::{bail, Result};
use crate::cli::AdminCommands;
use crate::connection::ConnectionManager;
use crate::output::OutputFormat;
use crate::utils::{print_success, print_error, print_warning};
use colored::*;
use meshtastic::packet::PacketDestination;
use meshtastic::protobufs;
use meshtastic::Message;

pub async fn handle_admin(
    mut connection: ConnectionManager,
    subcommand: AdminCommands,
    _format: OutputFormat,
) -> Result<()> {
    let api = connection.api_mut()?;
    
    match subcommand {
        AdminCommands::Reboot { confirm } => {
            if !confirm {
                print_warning("Reboot requires confirmation. Use --confirm to proceed.");
                bail!("Operation cancelled");
            }
            
            print_warning("Sending reboot command to device...");
            
            // Create admin message for reboot (5 second delay)
            let admin_msg = protobufs::AdminMessage {
                payload_variant: Some(protobufs::admin_message::PayloadVariant::RebootSeconds(5)),
                session_passkey: Vec::new(),
            };
            
            // Send the admin message
            let packet_data = admin_msg.encode_to_vec();
            api.send_mesh_packet(
                packet_data.into(),
                protobufs::PortNum::AdminApp,
                PacketDestination::Local,
                0, // channel
                true, // want_ack
                false, // want_response
                true, // echo to mesh
                None, // reply_id
                None, // emoji
            ).await?;
            
            print_success("Reboot command sent. Device will restart in 5 seconds.");
        }
        
        AdminCommands::FactoryReset { confirm } => {
            if !confirm {
                print_error("FACTORY RESET WILL ERASE ALL SETTINGS!");
                println!("{}", "This operation cannot be undone.".red().bold());
                print_warning("Use --confirm to proceed with factory reset.");
                bail!("Operation cancelled");
            }
            
            print_warning("Sending factory reset command...");
            
            // Create admin message for factory reset
            let admin_msg = protobufs::AdminMessage {
                payload_variant: Some(protobufs::admin_message::PayloadVariant::FactoryResetDevice(1)),
                session_passkey: Vec::new(),
            };
            
            // Send the admin message
            let packet_data = admin_msg.encode_to_vec();
            api.send_mesh_packet(
                packet_data.into(),
                protobufs::PortNum::AdminApp,
                PacketDestination::Local,
                0, // channel
                true, // want_ack
                false, // want_response
                true, // echo to mesh
                None, // reply_id
                None, // emoji
            ).await?;
            
            print_success("Factory reset command sent. Device will reset to defaults.");
        }
        
        AdminCommands::Shutdown { confirm } => {
            if !confirm {
                print_warning("Shutdown requires confirmation. Use --confirm to proceed.");
                bail!("Operation cancelled");
            }
            
            print_warning("Sending shutdown command to device...");
            
            // Create admin message for shutdown (5 second delay)
            let admin_msg = protobufs::AdminMessage {
                payload_variant: Some(protobufs::admin_message::PayloadVariant::ShutdownSeconds(5)),
                session_passkey: Vec::new(),
            };
            
            // Send the admin message
            let packet_data = admin_msg.encode_to_vec();
            api.send_mesh_packet(
                packet_data.into(),
                protobufs::PortNum::AdminApp,
                PacketDestination::Local,
                0, // channel
                true, // want_ack
                false, // want_response
                true, // echo to mesh
                None, // reply_id
                None, // emoji
            ).await?;
            
            print_success("Shutdown command sent. Device will power off in 5 seconds.");
        }
    }
    
    Ok(())
}