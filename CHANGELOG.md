# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2024-08-28

### Added - Initial Release

#### Core Features
- Complete Rust implementation of Meshtastic CLI
- Workspace structure with `meshtastic-cli-core` library and `meshtastic-cli` binary
- Static musl binary support for maximum portability
- NixOS flake for reproducible builds

#### Connection Support
- Serial port connection (auto-detection and manual specification)
- TCP/IP connection for network-attached devices
- Bluetooth LE support (compile-time feature flag)
- Connection timeout handling
- ACK tracking system with configurable timeouts

#### Commands Implemented
- **info** - Display radio information, nodes, channels, metrics, position, and telemetry
- **message** - Send (with ACK support), receive, and monitor messages
- **config** - Get, set, and list device configuration
- **channel** - List, add, delete, and configure channels
- **position** - Get, set, and track node positions
- **mesh** - Display topology, trace routes, list neighbors
- **telemetry** - Request device and environment telemetry
- **admin** - Reboot, factory reset, and shutdown device

#### Output Formats
- JSON output for all commands (scripting support)
- Formatted table output with Unicode borders
- Colored terminal output for better readability
- Progress bars for long-running operations

#### Testing & Tools
- **hardware-test** binary for comprehensive device testing
- Non-interactive mode for background/CI execution
- Wrapper script for safe nohup execution
- Unit tests for core library functionality

#### Developer Experience
- Comprehensive README with usage examples
- Development environment via Nix flakes
- Support for both debug and release builds
- Cargo workspace for clean separation of concerns

### Known Issues
- Bluetooth support requires compile-time feature flag
- Some TODO items remain for advanced routing features:
  - Hop count calculation from routing info
  - Complete telemetry request implementation
  - Route information storage for traceroute

### Technical Details
- Built with Tokio async runtime
- Uses meshtastic-rust library for protocol implementation
- Protocol Buffers for message serialization
- Cross-platform serial communication via serialport-rs