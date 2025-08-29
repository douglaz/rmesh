use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::define_test;
use crate::tests::{Test, TestContext};

pub fn get_tests() -> Vec<Test> {
    vec![
        define_test!(
            "List Channels",
            "Test listing available channels",
            test_list_channels
        ),
        define_test!(
            "Primary Channel",
            "Verify primary channel configuration",
            test_primary_channel
        ),
    ]
}

async fn test_list_channels(ctx: &mut TestContext<'_>) -> Result<Value> {
    let channels = rmesh_core::channel::list_channels(ctx.connection).await?;

    anyhow::ensure!(!channels.is_empty(), "No channels configured");

    Ok(json!({
        "channel_count": channels.len(),
        "channels": channels.iter().map(|c| json!({
            "index": c.index,
            "name": c.name,
            "role": c.role,
            "encrypted": c.has_psk,
        })).collect::<Vec<_>>(),
    }))
}

async fn test_primary_channel(ctx: &mut TestContext<'_>) -> Result<Value> {
    let channels = rmesh_core::channel::list_channels(ctx.connection).await?;

    let primary = channels
        .iter()
        .find(|c| c.role == "Primary")
        .context("No primary channel found")?;

    Ok(json!({
        "has_primary": true,
        "primary_index": primary.index,
        "primary_name": &primary.name,
        "primary_encrypted": primary.has_psk,
    }))
}
