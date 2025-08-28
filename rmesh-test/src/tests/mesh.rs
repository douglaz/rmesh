use anyhow::Result;
use serde_json::{json, Value};

use crate::define_test;
use crate::tests::{Test, TestContext};

pub fn get_tests() -> Vec<Test> {
    vec![
        define_test!(
            "Node Discovery",
            "Test discovering nodes in the mesh",
            test_node_discovery
        ),
        define_test!(
            "Neighbor Detection",
            "Find direct mesh neighbors",
            test_neighbor_detection
        ),
        define_test!(
            "Network Stats",
            "Calculate network statistics",
            test_network_stats
        ),
    ]
}

async fn test_node_discovery(ctx: &mut TestContext<'_>) -> Result<Value> {
    let nodes = rmesh_core::mesh::get_nodes(ctx.connection).await?;

    Ok(json!({
        "nodes_discovered": nodes.len(),
        "nodes": nodes.iter().map(|n| json!({
            "id": n.id,
            "name": n.user.long_name,
            "snr": n.snr,
        })).collect::<Vec<_>>(),
    }))
}

async fn test_neighbor_detection(ctx: &mut TestContext<'_>) -> Result<Value> {
    let neighbors = rmesh_core::mesh::get_neighbors(ctx.connection).await?;

    Ok(json!({
        "neighbor_count": neighbors.len(),
        "neighbors": neighbors.iter().map(|n| json!({
            "id": n.id,
            "name": n.user.long_name,
            "snr": n.snr,
            "rssi": n.rssi,
        })).collect::<Vec<_>>(),
    }))
}

async fn test_network_stats(ctx: &mut TestContext<'_>) -> Result<Value> {
    let stats = rmesh_core::mesh::get_network_stats(ctx.connection).await?;

    Ok(json!({
        "total_nodes": stats.total_nodes,
        "active_nodes": stats.active_nodes,
        "neighbors": stats.neighbors,
        "average_snr": stats.average_snr,
        "average_rssi": stats.average_rssi,
        "mesh_health": stats.mesh_health,
    }))
}
