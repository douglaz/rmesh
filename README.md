# rmesh

A comprehensive, high-performance command-line interface for Meshtastic devices written in Rust.

## Features

- 🚀 **Fast** - Native Rust implementation for superior performance
- 📦 **Portable** - Static musl binaries that work everywhere
- 🔌 **Multiple Connections** - Serial, TCP/IP, and Bluetooth LE support
- 📊 **Flexible Output** - JSON for scripting, formatted tables for humans
- 🛠️ **Full-Featured** - Complete command set matching the Python CLI
- 🔒 **Secure** - Memory-safe Rust implementation
- 🎨 **Modern UX** - Colored output, progress bars, clear error messages

## Installation

### Pre-built Binaries

Download the latest release for your platform from the [releases page](https://github.com/yourusername/rmesh/releases).

```bash
# Linux (static binary, works on all distributions)
curl -L https://github.com/yourusername/rmesh/releases/latest/download/rmesh-linux-x86_64.tar.gz | tar xz
sudo mv rmesh /usr/local/bin/

# macOS (Intel)
curl -L https://github.com/yourusername/rmesh/releases/latest/download/rmesh-macos-x86_64.tar.gz | tar xz
sudo mv rmesh /usr/local/bin/

# macOS (Apple Silicon)
curl -L https://github.com/yourusername/rmesh/releases/latest/download/rmesh-macos-aarch64.tar.gz | tar xz
sudo mv rmesh /usr/local/bin/

# Windows
# Download rmesh-windows-x86_64.zip and extract to your PATH
```

### Build from Source

#### Using Nix (Recommended)

```bash
# Enter development environment
nix develop

# Build static binary
cargo build --release --target x86_64-unknown-linux-musl

# Or use Nix to build
nix build
```

#### Using Cargo

```bash
# Clone the repository
git clone https://github.com/yourusername/rmesh.git
cd rmesh

# Build release version
cargo build --release

# Install to system
cargo install --path .
```

## Usage

### Connection Options

The CLI supports multiple connection methods:

```bash
# Serial port (auto-detect)
rmesh info radio

# Specific serial port
rmesh --port /dev/ttyUSB0 info radio

# TCP/IP connection
rmesh --port 192.168.1.100:4403 info radio

# Bluetooth LE (requires bluetooth feature)
rmesh --ble "Meshtastic Device" info radio
```

### Global Options

- `--port <PORT>` - Serial port or TCP address
- `--ble <NAME/MAC>` - Bluetooth device name or MAC address
- `--json` - Output in JSON format
- `--timeout <SECONDS>` - Connection timeout (default: 30)
- `--debug` - Enable debug logging
- `--verbose` - Enable verbose logging

## Commands

### Device Information

```bash
# Display radio information
rmesh --port /dev/ttyUSB0 info radio

# List all nodes in the mesh
rmesh --port /dev/ttyUSB0 info nodes

# Show channel configuration
rmesh --port /dev/ttyUSB0 info channels

# Display device metrics
rmesh --port /dev/ttyUSB0 info metrics
```

### Messaging

```bash
# Send broadcast message
rmesh --port /dev/ttyUSB0 message send --text "Hello mesh!"

# Send direct message to specific node
rmesh --port /dev/ttyUSB0 message send --text "Hello" --dest 0x12345678

# Send with acknowledgment
rmesh --port /dev/ttyUSB0 message send --text "Important" --dest 0x12345678 --ack

# Receive messages (listen for 10 messages)
rmesh --port /dev/ttyUSB0 message recv --count 10

# Monitor messages in real-time
rmesh --port /dev/ttyUSB0 message monitor

# Filter messages by sender
rmesh --port /dev/ttyUSB0 message monitor --from 0x12345678
```

### Configuration

```bash
# Get configuration value
rmesh --port /dev/ttyUSB0 config get --key lora.region

# Set configuration value
rmesh --port /dev/ttyUSB0 config set --key lora.region --value US

# List all configuration
rmesh --port /dev/ttyUSB0 config list
```

### Channel Management

```bash
# List channels
rmesh --port /dev/ttyUSB0 channel list

# Add new channel
rmesh --port /dev/ttyUSB0 channel add --name "MyChannel" --psk "secretkey"

# Delete channel
rmesh --port /dev/ttyUSB0 channel delete --index 1

# Configure channel
rmesh --port /dev/ttyUSB0 channel set --index 0 --name "Primary" --psk "newsecret"
```

### Position/Location

```bash
# Get current position
rmesh --port /dev/ttyUSB0 position get

# Set position manually
rmesh --port /dev/ttyUSB0 position set --lat 37.7749 --lon -122.4194 --alt 100

# Track node positions
rmesh --port /dev/ttyUSB0 position track --nodes 0x12345678 0x87654321
```

### Mesh Network Analysis

```bash
# Display network topology
rmesh --port /dev/ttyUSB0 mesh topology

# Trace route to node
rmesh --port /dev/ttyUSB0 mesh traceroute --dest 0x12345678

# List neighboring nodes
rmesh --port /dev/ttyUSB0 mesh neighbors
```

### Administrative Commands

```bash
# Reboot device (requires confirmation)
rmesh --port /dev/ttyUSB0 admin reboot --confirm

# Factory reset (CAUTION: erases all settings)
rmesh --port /dev/ttyUSB0 admin factory-reset --confirm

# Shutdown device
rmesh --port /dev/ttyUSB0 admin shutdown --confirm
```

### JSON Output

All commands support JSON output for scripting:

```bash
# Get node list as JSON
rmesh --port /dev/ttyUSB0 --json info nodes

# Pipe to jq for processing
rmesh --port /dev/ttyUSB0 --json info nodes | jq '.[] | select(.snr > 10)'

# Save output to file
rmesh --port /dev/ttyUSB0 --json info radio > radio_info.json
```

## Examples

### Monitor and Log Messages

```bash
# Monitor messages and save to file with timestamp
rmesh --port /dev/ttyUSB0 --json message monitor | \
  while read line; do 
    echo "$(date -Iseconds) $line" >> messages.log
  done
```

### Send Periodic Status Updates

```bash
#!/bin/bash
while true; do
  rmesh --port /dev/ttyUSB0 message send \
    --text "Status OK at $(date +%H:%M)"
  sleep 3600  # Every hour
done
```

### Track Node Positions to CSV

```bash
rmesh --port /dev/ttyUSB0 --json position track | \
  jq -r '[.node_id, .latitude, .longitude, .altitude] | @csv' >> positions.csv
```

## Development

### Prerequisites

- Rust 1.70+ (for building)
- NixOS/Nix (optional, for development environment)
- USB drivers for your Meshtastic device

### Building

```bash
# Clone repository
git clone https://github.com/yourusername/rmesh.git
cd rmesh

# Enter Nix development environment (optional)
nix develop

# Build debug version
cargo build

# Run tests
cargo test

# Run with debug output
cargo run -- --debug --port /dev/ttyUSB0 info radio
```

### Static Binary Build

```bash
# Build static musl binary
cargo build --release --target x86_64-unknown-linux-musl

# Strip binary to reduce size
strip target/x86_64-unknown-linux-musl/release/rmesh
```

## Project Structure

The project is organized as a Rust workspace with two main crates:

- **`rmesh-core`**: Core library containing all business logic for Meshtastic operations
  - Connection management (serial, TCP, Bluetooth)
  - Device operations (reboot, factory reset, etc.)
  - Message handling
  - Configuration management
  - Position tracking
  - Can be used as a standalone library in other projects

- **`rmesh`**: Command-line interface crate
  - CLI argument parsing with Clap
  - Output formatting (JSON, tables, colored output)
  - User interaction and display
  - Thin wrapper around the core library

## Architecture

The workspace is built on top of the `meshtastic-rust` library and uses:

- **Tokio** - Async runtime for concurrent operations
- **Clap** - Command-line argument parsing
- **Serde** - JSON serialization/deserialization
- **Colored** - Terminal color output
- **Comfy-table** - Formatted table output

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Run `cargo fmt` and `cargo clippy`
6. Submit a pull request

## License

This project is dual-licensed under MIT OR Apache-2.0.

## Acknowledgments

- The Meshtastic project team for the protocol and device firmware
- The `meshtastic-rust` library maintainers
- The Rust community for excellent tooling and libraries

## Support

- Report issues on [GitHub](https://github.com/yourusername/rmesh/issues)
- Join the Meshtastic community on [Discord](https://discord.gg/meshtastic)
- Check the [Meshtastic documentation](https://meshtastic.org/docs)

## Comparison with Other CLIs

| Feature | rmesh (Rust) | Python CLI | Go CLI |
|---------|------------------------|------------|---------|
| Performance | ⚡ Fastest | 🐢 Slower | 🏃 Fast |
| Binary Size | ~5MB static | Requires Python | ~10MB |
| Platform Support | All (static) | All (needs Python) | All |
| Features | Full | Full | Partial |
| JSON Output | ✅ | ✅ | ✅ |
| Bluetooth | ✅ (optional) | ✅ | ❌ |
| Active Development | ✅ | ✅ | 🔶 |