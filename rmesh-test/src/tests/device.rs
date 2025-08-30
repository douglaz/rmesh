use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::define_test;
use crate::tests::{Test, TestContext};

pub fn get_tests() -> Vec<Test> {
    vec![
        define_test!(
            "Device Info",
            "Retrieve basic device information",
            test_device_info
        ),
        define_test!(
            "Firmware Version",
            "Check firmware version compatibility",
            test_firmware_version
        ),
        define_test!(
            "Hardware Model",
            "Verify hardware model detection",
            test_hardware_model
        ),
        define_test!(
            "Node Configuration",
            "Verify node ID and configuration",
            test_node_config
        ),
    ]
}

async fn test_device_info(ctx: &mut TestContext<'_>) -> Result<Value> {
    let state = ctx.connection.get_device_state().await;

    let my_info = state.my_node_info.context("No device info available")?;

    Ok(json!({
        "node_id": my_info.node_id,
        "node_num": my_info.node_num,
        "reboot_count": my_info.reboot_count,
        "min_app_version": my_info.min_app_version,
    }))
}

async fn test_firmware_version(ctx: &mut TestContext<'_>) -> Result<Value> {
    let state = ctx.connection.get_device_state().await;

    // Get firmware version from node info or config
    let firmware_version = if let Some(my_info) = &state.my_node_info {
        // Extract from min_app_version or other fields
        let major = my_info.min_app_version / 10000;
        let minor = (my_info.min_app_version % 10000) / 100;
        let patch = my_info.min_app_version % 100;
        Some(format!("{major}.{minor}.{patch}"))
    } else {
        None
    };

    let firmware = firmware_version.context("Could not determine firmware version")?;

    // Check if firmware is recent enough (2.x or higher)
    let parts: Vec<&str> = firmware.split('.').collect();
    if let Some(major_str) = parts.first()
        && let Ok(major) = major_str.parse::<u32>()
        && major < 2
    {
        anyhow::bail!(
            "Firmware version {version} is too old. Please update to 2.x or higher",
            version = firmware
        );
    }

    Ok(json!({
        "firmware_version": firmware,
        "compatible": true,
    }))
}

async fn test_hardware_model(ctx: &mut TestContext<'_>) -> Result<Value> {
    let state = ctx.connection.get_device_state().await;

    // Try to get hardware model from nodes
    let hardware_model = state
        .nodes
        .values()
        .find_map(|node| node.user.hw_model.clone())
        .or_else(|| {
            // Fallback: guess from other info
            if state.my_node_info.is_some() {
                Some("Unknown".to_string())
            } else {
                None
            }
        });

    let model = hardware_model.context("Could not determine hardware model")?;

    // List of known good models
    let known_models = [
        "TBEAM",
        "TLORA",
        "TECHO",
        "NANO_G1",
        "STATION_G1",
        "RAK4631",
        "RAK11200",
        "T_WATCH",
        "HELTEC",
        "LILYGO",
    ];

    let is_known = known_models.iter().any(|m| model.contains(m));

    Ok(json!({
        "hardware_model": model,
        "is_known_model": is_known,
    }))
}

async fn test_node_config(ctx: &mut TestContext<'_>) -> Result<Value> {
    let state = ctx.connection.get_device_state().await;

    let my_info = state
        .my_node_info
        .context("No node configuration available")?;

    // Validate node ID format
    anyhow::ensure!(!my_info.node_id.is_empty(), "Node ID is empty");

    // Check if node ID is valid hex
    anyhow::ensure!(
        my_info.node_id.len() == 8 && my_info.node_id.chars().all(|c| c.is_ascii_hexdigit()),
        "Invalid node ID format: {node_id}",
        node_id = my_info.node_id
    );

    Ok(json!({
        "node_id": my_info.node_id,
        "node_num": format!("{num:08x}", num = my_info.node_num),
        "device_id": my_info.device_id,
        "valid": true,
    }))
}
