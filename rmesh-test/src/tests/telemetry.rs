use anyhow::Result;
use serde_json::{json, Value};

use crate::define_test;
use crate::tests::{Test, TestContext};

pub fn get_tests() -> Vec<Test> {
    vec![
        define_test!(
            "Device Telemetry",
            "Test device telemetry data",
            test_device_telemetry
        ),
        define_test!(
            "Telemetry Updates",
            "Check telemetry update frequency",
            test_telemetry_updates
        ),
    ]
}

async fn test_device_telemetry(ctx: &mut TestContext<'_>) -> Result<Value> {
    let state = ctx.connection.get_device_state().await;

    let has_telemetry = !state.telemetry.is_empty();
    let telemetry_count = state.telemetry.len();

    // Get first telemetry data if available
    let sample = state.telemetry.values().next();

    Ok(json!({
        "has_telemetry": has_telemetry,
        "telemetry_count": telemetry_count,
        "sample": sample.map(|t| json!({
            "node_num": t.node_num,
            "has_device_metrics": t.device_metrics.is_some(),
            "has_environment_metrics": t.environment_metrics.is_some(),
        })),
    }))
}

async fn test_telemetry_updates(ctx: &mut TestContext<'_>) -> Result<Value> {
    let initial_state = ctx.connection.get_device_state().await;
    let initial_count = initial_state.telemetry.len();

    // Wait for potential updates
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let final_state = ctx.connection.get_device_state().await;
    let final_count = final_state.telemetry.len();

    Ok(json!({
        "initial_telemetry_count": initial_count,
        "final_telemetry_count": final_count,
        "updates_received": final_count > initial_count,
    }))
}
