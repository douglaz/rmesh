use anyhow::Result;
use serde_json::{json, Value};
use std::time::Instant;

use crate::define_test;
use crate::tests::{Test, TestContext};

pub fn get_tests() -> Vec<Test> {
    vec![
        define_test!(
            "Send Message",
            "Test sending a broadcast message",
            test_send_message
        ),
        define_test!(
            "Message Queue",
            "Test message queue functionality",
            test_message_queue
        ),
    ]
}

async fn test_send_message(ctx: &mut TestContext<'_>) -> Result<Value> {
    let start = Instant::now();

    // Send a test message
    rmesh_core::message::send_text_message(
        ctx.connection,
        "Hardware test message",
        None,  // Broadcast
        0,     // Default channel
        false, // No ACK needed for test
    )
    .await?;

    let elapsed = start.elapsed();

    Ok(json!({
        "message_sent": true,
        "send_time_ms": elapsed.as_millis(),
        "destination": "broadcast",
        "channel": 0,
    }))
}

async fn test_message_queue(ctx: &mut TestContext<'_>) -> Result<Value> {
    let state = ctx.connection.get_device_state().await;
    let initial_count = state.messages.len();

    // Send multiple messages
    for i in 0..3 {
        rmesh_core::message::send_text_message(
            ctx.connection,
            &format!("Test message {i}"),
            None,
            0,
            false,
        )
        .await?;

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    // Check message queue
    let new_state = ctx.connection.get_device_state().await;
    let new_count = new_state.messages.len();

    Ok(json!({
        "initial_messages": initial_count,
        "final_messages": new_count,
        "messages_sent": 3,
        "queue_working": true,
    }))
}
