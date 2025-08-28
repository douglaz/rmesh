use anyhow::Result;
use serde_json::{json, Value};

use crate::define_test;
use crate::tests::{Test, TestContext};

pub fn get_tests() -> Vec<Test> {
    vec![
        define_test!("GPS Status", "Check if GPS is available", test_gps_status),
        define_test!(
            "Position Data",
            "Test position data retrieval",
            test_position_data
        ),
    ]
}

async fn test_gps_status(ctx: &mut TestContext<'_>) -> Result<Value> {
    let state = ctx.connection.get_device_state().await;

    let has_position = !state.positions.is_empty();
    let position_count = state.positions.len();

    Ok(json!({
        "has_position_data": has_position,
        "position_count": position_count,
    }))
}

async fn test_position_data(ctx: &mut TestContext<'_>) -> Result<Value> {
    let position = rmesh_core::position::get_position(
        ctx.connection,
        None, // Get our own position
    )
    .await?;

    if let Some(pos) = position {
        Ok(json!({
            "has_position": true,
            "latitude": pos.latitude,
            "longitude": pos.longitude,
            "altitude": pos.altitude,
            "time": pos.time,
        }))
    } else {
        Ok(json!({
            "has_position": false,
            "note": "GPS may not be available or no fix yet",
        }))
    }
}
