use crate::connection::ConnectionManager;
use crate::state::NodeInfo;
use anyhow::Result;
use serde::Serialize;
use serde_json::json;
use tracing::debug;

/// Represents a node in the mesh network
#[derive(Debug, Clone, Serialize)]
pub struct MeshNode {
    pub id: String,
    pub num: u32,
    pub name: String,
    pub snr: Option<f32>,
    pub rssi: Option<i32>,
    pub last_heard: Option<u64>,
    pub hops_away: Option<u32>,
}

/// Represents the mesh network topology
#[derive(Debug, Clone, Serialize)]
pub struct MeshTopology {
    pub nodes: Vec<MeshNode>,
    pub edges: Vec<MeshEdge>,
    pub total_nodes: usize,
    pub my_node_id: String,
}

/// Represents an edge/connection between two nodes
#[derive(Debug, Clone, Serialize)]
pub struct MeshEdge {
    pub from: String,
    pub to: String,
    pub snr: Option<f32>,
    pub rssi: Option<i32>,
}

/// Traceroute result
#[derive(Debug, Clone, Serialize)]
pub struct TracerouteResult {
    pub destination: String,
    pub hops: Vec<RouteHop>,
    pub total_time_ms: u64,
    pub success: bool,
}

/// Single hop in a traceroute
#[derive(Debug, Clone, Serialize)]
pub struct RouteHop {
    pub node_id: u32,
    pub node_name: String,
    pub hop_number: u32,
    pub snr: Option<f32>,
    pub rssi: Option<i32>,
}

/// Get the current mesh network topology
pub async fn get_topology(connection: &ConnectionManager) -> Result<serde_json::Value> {
    let state = connection.get_device_state().await;

    // Build node list from cached state
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // Add our node
    let my_node_id = if let Some(my_info) = &state.my_node_info {
        my_info.node_id.clone()
    } else {
        "unknown".to_string()
    };

    // Add all known nodes
    for (node_num, node_info) in &state.nodes {
        nodes.push(MeshNode {
            id: node_info.id.clone(),
            num: *node_num,
            name: node_info.user.long_name.clone(),
            snr: node_info.snr,
            rssi: node_info.rssi,
            last_heard: node_info.last_heard,
            hops_away: None, // TODO: Calculate from routing info
        });

        // If we have SNR/RSSI, there's likely a direct connection
        if node_info.snr.is_some() || node_info.rssi.is_some() {
            edges.push(MeshEdge {
                from: my_node_id.clone(),
                to: node_info.id.clone(),
                snr: node_info.snr,
                rssi: node_info.rssi,
            });
        }
    }

    Ok(json!({
        "nodes": nodes,
        "edges": edges,
        "total_nodes": nodes.len(),
        "my_node": state.my_node_info,
    }))
}

/// Perform a traceroute to a specific node
pub async fn traceroute(
    connection: &mut ConnectionManager,
    destination: u32,
) -> Result<Vec<RouteHop>> {
    // Use the ConnectionManager's traceroute method which handles response waiting
    let hops = connection.send_traceroute(destination, 10).await?;

    if hops.is_empty() {
        debug!("No route found to destination {:08x}", destination);
    } else {
        debug!(
            "Found route to {:08x} with {} hops",
            destination,
            hops.len()
        );
    }

    Ok(hops)
}

/// List neighboring nodes (direct connections)
pub async fn get_neighbors(connection: &ConnectionManager) -> Result<Vec<NodeInfo>> {
    let state = connection.get_device_state().await;

    // Filter nodes that have recent SNR/RSSI values (indicating direct connection)
    let neighbors: Vec<NodeInfo> = state
        .nodes
        .values()
        .filter(|node| {
            // Consider it a neighbor if we have signal strength info and heard recently
            (node.snr.is_some() || node.rssi.is_some())
                && node
                    .last_heard
                    .map(|h| {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        // Use saturating subtraction to avoid overflow if timestamp is in the future
                        now.saturating_sub(h) < 3600 // Heard within last hour
                    })
                    .unwrap_or(false)
        })
        .cloned()
        .collect();

    Ok(neighbors)
}

/// Get list of all nodes in the mesh
pub async fn get_nodes(connection: &ConnectionManager) -> Result<Vec<NodeInfo>> {
    let state = connection.get_device_state().await;
    Ok(state.nodes.values().cloned().collect())
}

/// Calculate network statistics
#[derive(Debug, Clone, Serialize)]
pub struct NetworkStats {
    pub total_nodes: usize,
    pub active_nodes: usize,
    pub neighbors: usize,
    pub average_snr: Option<f32>,
    pub average_rssi: Option<i32>,
    pub mesh_health: String,
}

pub async fn get_network_stats(connection: &ConnectionManager) -> Result<NetworkStats> {
    let state = connection.get_device_state().await;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let total_nodes = state.nodes.len();

    // Active nodes (heard in last hour)
    let active_nodes = state
        .nodes
        .values()
        .filter(|n| {
            n.last_heard
                .map(|h| now.saturating_sub(h) < 3600)
                .unwrap_or(false)
        })
        .count();

    // Direct neighbors
    let neighbors = state
        .nodes
        .values()
        .filter(|n| n.snr.is_some() || n.rssi.is_some())
        .count();

    // Calculate average SNR
    let snr_values: Vec<f32> = state.nodes.values().filter_map(|n| n.snr).collect();
    let average_snr = if !snr_values.is_empty() {
        Some(snr_values.iter().sum::<f32>() / snr_values.len() as f32)
    } else {
        None
    };

    // Calculate average RSSI
    let rssi_values: Vec<i32> = state.nodes.values().filter_map(|n| n.rssi).collect();
    let average_rssi = if !rssi_values.is_empty() {
        Some(rssi_values.iter().sum::<i32>() / rssi_values.len() as i32)
    } else {
        None
    };

    // Determine mesh health based on metrics
    let mesh_health = if neighbors == 0 {
        "Isolated"
    } else if neighbors == 1 {
        "Weak"
    } else if average_snr.map(|s| s > 5.0).unwrap_or(false) {
        "Excellent"
    } else if average_snr.map(|s| s > 0.0).unwrap_or(false) {
        "Good"
    } else {
        "Fair"
    }
    .to_string();

    Ok(NetworkStats {
        total_nodes,
        active_nodes,
        neighbors,
        average_snr,
        average_rssi,
        mesh_health,
    })
}

/// Request node information from remote nodes
pub async fn request_node_info(
    _connection: &mut ConnectionManager,
    node_num: Option<u32>,
) -> Result<()> {
    // Note: Node info request requires specific admin message variant
    // that may not be available in current protobuf version
    // For now, we rely on passive node discovery from received packets

    debug!(
        "Node info request for {} - passive discovery only",
        node_num
            .map(|n| format!("{:08x}", n))
            .unwrap_or_else(|| "all nodes".to_string())
    );

    Ok(())
}
