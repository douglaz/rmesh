use crate::connection::ConnectionManager;
use anyhow::Result;
use serde::Serialize;

/// Request telemetry from a node
pub async fn request_telemetry(
    _connection: &mut ConnectionManager,
    _telemetry_type: TelemetryType,
    _node_id: Option<u32>,
) -> Result<()> {
    // TODO: Implement telemetry request
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub enum TelemetryType {
    Battery,
    Environment,
    Device,
}
