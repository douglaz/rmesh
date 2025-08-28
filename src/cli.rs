use clap::{Parser, Subcommand};
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "rmesh")]
#[command(author, version, about, long_about = None)]
#[command(arg_required_else_help = true)]
pub struct Cli {
    /// Serial port or TCP address (e.g., /dev/ttyUSB0 or 192.168.1.100:4403)
    #[arg(short, long, global = true)]
    pub port: Option<String>,

    /// Bluetooth device name or MAC address
    #[arg(short = 'b', long, global = true)]
    pub ble: Option<String>,

    /// Output in JSON format
    #[arg(short = 'j', long, global = true)]
    pub json: bool,

    /// Connection timeout in seconds
    #[arg(short = 't', long, global = true, default_value = "30")]
    pub timeout: u64,

    /// Enable debug logging
    #[arg(short = 'd', long, global = true)]
    pub debug: bool,

    /// Enable verbose logging
    #[arg(short = 'v', long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Display radio information
    Info {
        #[command(subcommand)]
        subcommand: InfoCommands,
    },
    
    /// Send and receive messages
    Message {
        #[command(subcommand)]
        subcommand: MessageCommands,
    },
    
    /// Device configuration management
    Config {
        #[command(subcommand)]
        subcommand: ConfigCommands,
    },
    
    /// Channel management
    Channel {
        #[command(subcommand)]
        subcommand: ChannelCommands,
    },
    
    /// Location/position management
    Position {
        #[command(subcommand)]
        subcommand: PositionCommands,
    },
    
    /// Mesh network analysis
    Mesh {
        #[command(subcommand)]
        subcommand: MeshCommands,
    },
    
    /// Device telemetry
    Telemetry {
        /// Type of telemetry to request (battery, environment, device)
        #[arg(short = 't', long)]
        telemetry_type: Option<String>,
        
        /// Destination node ID
        #[arg(short = 'd', long)]
        dest: Option<u32>,
    },
    
    /// Administrative commands
    Admin {
        #[command(subcommand)]
        subcommand: AdminCommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum InfoCommands {
    /// Display radio information
    Radio,
    /// Display channel configuration
    Channels,
    /// Display node list
    Nodes,
    /// Display position information
    Position,
    /// Display device metrics
    Metrics,
    /// Display telemetry data
    Telemetry,
}

#[derive(Subcommand, Debug)]
pub enum MessageCommands {
    /// Send a text message
    Send {
        /// Message text to send
        #[arg(short = 'm', long)]
        text: String,
        
        /// Destination node ID (broadcast if not specified)
        #[arg(short = 'd', long)]
        dest: Option<u32>,
        
        /// Channel index
        #[arg(short = 'c', long, default_value = "0")]
        channel: u32,
        
        /// Wait for acknowledgment
        #[arg(short = 'a', long)]
        ack: bool,
    },
    
    /// Receive messages
    Recv {
        /// Filter by sender node ID
        #[arg(short = 'f', long)]
        from: Option<u32>,
        
        /// Maximum messages to receive (0 for unlimited)
        #[arg(short = 'n', long, default_value = "0")]
        count: usize,
    },
    
    /// Monitor messages in real-time
    Monitor {
        /// Filter by sender node ID
        #[arg(short = 'f', long)]
        from: Option<u32>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Get configuration value
    Get {
        /// Configuration key (e.g., lora.region)
        #[arg(short = 'k', long)]
        key: String,
    },
    
    /// Set configuration value
    Set {
        /// Configuration key (e.g., lora.region)
        #[arg(short = 'k', long)]
        key: String,
        
        /// Configuration value
        #[arg(short = 'v', long)]
        value: String,
    },
    
    /// List all configuration values
    List,
}

#[derive(Subcommand, Debug)]
pub enum ChannelCommands {
    /// List all channels
    List,
    
    /// Add a new channel
    Add {
        /// Channel name
        #[arg(short = 'n', long)]
        name: String,
        
        /// Pre-shared key (PSK)
        #[arg(short = 'p', long)]
        psk: Option<String>,
    },
    
    /// Delete a channel
    Delete {
        /// Channel index
        #[arg(short = 'i', long)]
        index: u32,
    },
    
    /// Configure a channel
    Set {
        /// Channel index
        #[arg(short = 'i', long)]
        index: u32,
        
        /// Channel name
        #[arg(short = 'n', long)]
        name: Option<String>,
        
        /// Pre-shared key (PSK)
        #[arg(short = 'p', long)]
        psk: Option<String>,
        
        /// Uplink enabled
        #[arg(short = 'u', long)]
        uplink: Option<bool>,
        
        /// Downlink enabled
        #[arg(short = 'd', long)]
        downlink: Option<bool>,
    },
}

#[derive(Subcommand, Debug)]
pub enum PositionCommands {
    /// Get current position
    Get {
        /// Node ID (local if not specified)
        #[arg(short = 'n', long)]
        node: Option<u32>,
    },
    
    /// Set position
    Set {
        /// Latitude in decimal degrees
        #[arg(long)]
        lat: f64,
        
        /// Longitude in decimal degrees
        #[arg(long)]
        lon: f64,
        
        /// Altitude in meters
        #[arg(long)]
        alt: Option<i32>,
    },
    
    /// Track node positions
    Track {
        /// Node IDs to track (all if not specified)
        #[arg(short = 'n', long)]
        nodes: Vec<u32>,
    },
}

#[derive(Subcommand, Debug)]
pub enum MeshCommands {
    /// Display network topology
    Topology,
    
    /// Trace route to destination
    Traceroute {
        /// Destination node ID
        #[arg(short = 'd', long)]
        dest: u32,
    },
    
    /// List neighboring nodes
    Neighbors,
}

#[derive(Subcommand, Debug)]
pub enum AdminCommands {
    /// Reboot the device
    Reboot {
        /// Confirm the action
        #[arg(short = 'y', long)]
        confirm: bool,
    },
    
    /// Factory reset the device
    FactoryReset {
        /// Confirm the action
        #[arg(short = 'y', long)]
        confirm: bool,
    },
    
    /// Shutdown the device
    Shutdown {
        /// Confirm the action
        #[arg(short = 'y', long)]
        confirm: bool,
    },
}

impl Cli {
    pub fn timeout_duration(&self) -> Duration {
        Duration::from_secs(self.timeout)
    }
}