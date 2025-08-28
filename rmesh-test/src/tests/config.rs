use anyhow::Result;
use serde_json::{json, Value};

use crate::define_test;
use crate::tests::{Test, TestContext};

pub fn get_tests() -> Vec<Test> {
    vec![
        define_test!(
            "Read Config",
            "Test reading device configuration",
            test_read_config
        ),
        define_test!(
            "Config Categories",
            "Verify config categories are accessible",
            test_config_categories
        ),
    ]
}

async fn test_read_config(ctx: &mut TestContext<'_>) -> Result<Value> {
    // Try to read a basic config value
    let config_value =
        rmesh_core::config::get_config_value(ctx.connection, "lora.region").await?;

    Ok(json!({
        "config_readable": true,
        "test_key": "lora.region",
        "value": config_value,
    }))
}

async fn test_config_categories(ctx: &mut TestContext<'_>) -> Result<Value> {
    let state = ctx.connection.get_device_state().await;

    let mut available_configs = vec![];

    if state.device_config.is_some() {
        available_configs.push("device");
    }
    if state.position_config.is_some() {
        available_configs.push("position");
    }
    if state.lora_config.is_some() {
        available_configs.push("lora");
    }
    if state.bluetooth_config.is_some() {
        available_configs.push("bluetooth");
    }

    Ok(json!({
        "available_categories": available_configs,
        "total_categories": available_configs.len(),
    }))
}
