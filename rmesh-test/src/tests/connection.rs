use anyhow::Result;
use serde_json::{Value, json};
use std::time::Instant;

use crate::define_test;
use crate::tests::{Test, TestContext};

pub fn get_tests() -> Vec<Test> {
    vec![
        define_test!(
            "Connection Stability",
            "Test if connection remains stable over time",
            test_connection_stability
        ),
        define_test!(
            "Packet Round Trip",
            "Test sending and receiving packets",
            test_packet_round_trip
        ),
        define_test!(
            "Response Time",
            "Measure average response time",
            test_response_time
        ),
    ]
}

async fn test_connection_stability(ctx: &mut TestContext<'_>) -> Result<Value> {
    let start = Instant::now();
    let test_duration = std::time::Duration::from_secs(5);
    let mut successful_pings = 0;
    let mut failed_pings = 0;

    while start.elapsed() < test_duration {
        // Try to get device state (this sends/receives packets)
        match tokio::time::timeout(
            std::time::Duration::from_secs(2),
            ctx.connection.get_device_state(),
        )
        .await
        {
            Ok(_state) => successful_pings += 1,
            Err(_) => failed_pings += 1,
        }

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    let total_pings = successful_pings + failed_pings;
    let success_rate = if total_pings > 0 {
        (successful_pings as f64 / total_pings as f64) * 100.0
    } else {
        0.0
    };

    anyhow::ensure!(
        success_rate >= 80.0,
        "Connection unstable: {:.1}% success rate",
        success_rate
    );

    Ok(json!({
        "successful_pings": successful_pings,
        "failed_pings": failed_pings,
        "success_rate": format!("{rate:.1}%", rate = success_rate),
        "test_duration_ms": start.elapsed().as_millis(),
    }))
}

async fn test_packet_round_trip(ctx: &mut TestContext<'_>) -> Result<Value> {
    // Test basic packet exchange by getting device info
    let start = Instant::now();
    let state = ctx.connection.get_device_state().await;
    let elapsed = start.elapsed();

    // Check if we have basic device info
    anyhow::ensure!(
        state.my_node_info.is_some() || !state.nodes.is_empty(),
        "No device information received"
    );

    Ok(json!({
        "round_trip_ms": elapsed.as_millis(),
        "has_node_info": state.my_node_info.is_some(),
        "nodes_discovered": state.nodes.len(),
    }))
}

async fn test_response_time(ctx: &mut TestContext<'_>) -> Result<Value> {
    let mut response_times = Vec::new();
    let num_samples = 10;

    for _ in 0..num_samples {
        let start = Instant::now();
        let _ = ctx.connection.get_device_state().await;
        response_times.push(start.elapsed().as_millis() as u64);

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    let avg_response_time = response_times.iter().sum::<u64>() / response_times.len() as u64;
    let min_response_time = *response_times.iter().min().unwrap_or(&0);
    let max_response_time = *response_times.iter().max().unwrap_or(&0);

    anyhow::ensure!(
        avg_response_time <= 1000,
        "Response time too high: {}ms average",
        avg_response_time
    );

    Ok(json!({
        "average_ms": avg_response_time,
        "min_ms": min_response_time,
        "max_ms": max_response_time,
        "samples": num_samples,
    }))
}
