//! Core library for Meshtastic CLI operations
//!
//! This crate provides the business logic for interacting with Meshtastic devices,
//! including connection management, message handling, configuration, and more.

pub mod channel;
pub mod config;
pub mod connection;
pub mod device;
pub mod mesh;
pub mod message;
pub mod position;
pub mod state;
pub mod telemetry;

// Re-export commonly used types
pub use anyhow::Result;
pub use connection::ConnectionManager;

// Re-export meshtastic types for convenience
pub use meshtastic::packet::PacketDestination;
pub use meshtastic::{Message, protobufs};

#[cfg(test)]
mod tests;
