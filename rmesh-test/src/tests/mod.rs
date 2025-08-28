pub mod channels;
pub mod config;
pub mod connection;
pub mod device;
pub mod mesh;
pub mod messaging;
pub mod position;
pub mod telemetry;

use anyhow::Result;
use rmesh_core::ConnectionManager;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

/// Test context passed to all test functions
pub struct TestContext<'a> {
    pub connection: &'a mut ConnectionManager,
    #[allow(dead_code)]
    pub verbose: bool,
}

impl<'a> TestContext<'a> {
    pub fn new(connection: &'a mut ConnectionManager, verbose: bool) -> Self {
        Self {
            connection,
            verbose,
        }
    }
}

/// A single test definition
pub struct Test {
    pub name: &'static str,
    #[allow(dead_code)]
    pub description: &'static str,
    pub run_fn: Box<
        dyn for<'a> Fn(&'a mut TestContext<'_>) -> Pin<Box<dyn Future<Output = Result<Value>> + 'a>>
            + Send
            + Sync,
    >,
}

/// Test categories
#[derive(Debug, Clone, Copy)]
pub enum TestCategory {
    Connection,
    Device,
    Messaging,
    Configuration,
    Channels,
    Position,
    Mesh,
    Telemetry,
}

impl TestCategory {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "connection" => Some(Self::Connection),
            "device" => Some(Self::Device),
            "messaging" | "message" => Some(Self::Messaging),
            "configuration" | "config" => Some(Self::Configuration),
            "channels" | "channel" => Some(Self::Channels),
            "position" | "gps" => Some(Self::Position),
            "mesh" | "network" => Some(Self::Mesh),
            "telemetry" => Some(Self::Telemetry),
            _ => None,
        }
    }

    pub fn get_tests(&self) -> Vec<Test> {
        match self {
            Self::Connection => connection::get_tests(),
            Self::Device => device::get_tests(),
            Self::Messaging => messaging::get_tests(),
            Self::Configuration => config::get_tests(),
            Self::Channels => channels::get_tests(),
            Self::Position => position::get_tests(),
            Self::Mesh => mesh::get_tests(),
            Self::Telemetry => telemetry::get_tests(),
        }
    }
}

/// Helper macro for defining tests
#[macro_export]
macro_rules! define_test {
    ($name:expr, $desc:expr, $func:expr) => {
        Test {
            name: $name,
            description: $desc,
            run_fn: Box::new(move |ctx| Box::pin($func(ctx))),
        }
    };
}
