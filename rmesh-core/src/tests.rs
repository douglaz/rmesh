#[cfg(test)]
mod state_tests {
    use crate::state::{DeviceConfig, DeviceMetrics, PositionConfig, TelemetryData};
    use crate::state::{DeviceState, MyNodeInfo, NodeInfo, Position, TextMessage, User};
    use anyhow::{Context, Result};

    #[test]
    fn test_device_state_creation() -> Result<()> {
        let state = DeviceState::new();
        assert!(state.nodes.is_empty());
        assert!(state.channels.is_empty());
        assert!(state.my_node_info.is_none());
        assert!(state.positions.is_empty());
        assert!(state.messages.is_empty());
        Ok(())
    }

    #[test]
    fn test_node_update() -> Result<()> {
        let mut state = DeviceState::new();
        let node = NodeInfo {
            id: "test123".to_string(),
            num: 0x12345678,
            user: User {
                id: "test".to_string(),
                long_name: "Test User".to_string(),
                short_name: "TU".to_string(),
                hw_model: Some("T-Beam".to_string()),
            },
            last_heard: Some(1234567890),
            last_heard_iso: chrono::DateTime::from_timestamp(1234567890, 0)
                .map(|dt| dt.to_rfc3339()),
            snr: Some(5.5),
            rssi: Some(-70),
        };

        state.update_node(0x12345678, node.clone());
        assert_eq!(state.nodes.len(), 1);

        let stored_node = state.nodes.get(&0x12345678).context("Node not found")?;
        assert_eq!(stored_node.id, "test123");
        Ok(())
    }

    #[test]
    fn test_position_update() -> Result<()> {
        let mut state = DeviceState::new();
        let position = Position {
            node_id: "test123".to_string(),
            node_num: 0x12345678,
            latitude: 37.7749,
            longitude: -122.4194,
            altitude: Some(100),
            time: Some("2024-01-01T00:00:00Z".to_string()),
            last_updated: 1234567890,
        };

        state.update_position(0x12345678, position.clone());
        assert_eq!(state.positions.len(), 1);

        let stored_position = state
            .positions
            .get(&0x12345678)
            .context("Position not found")?;
        assert_eq!(stored_position.latitude, 37.7749);
        Ok(())
    }

    #[test]
    fn test_message_add() -> Result<()> {
        let mut state = DeviceState::new();
        let message = TextMessage {
            from: "sender123".to_string(),
            from_node: 0x11111111,
            to: "receiver456".to_string(),
            to_node: 0x22222222,
            channel: 0,
            text: "Hello, mesh!".to_string(),
            time: 1234567890,
            snr: Some(5.0),
            rssi: Some(-80),
            acknowledged: false,
        };

        state.add_message(message.clone());
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].text, "Hello, mesh!");
        Ok(())
    }

    #[test]
    fn test_my_node_info() -> Result<()> {
        let mut state = DeviceState::new();
        let my_info = MyNodeInfo {
            node_num: 0x12345678,
            node_id: "12345678".to_string(),
            reboot_count: 5,
            min_app_version: 20300,
            device_id: "abcdef123456".to_string(),
        };

        state.set_my_node_info(my_info.clone());
        assert!(state.my_node_info.is_some());

        let stored_info = state.my_node_info.context("My node info not found")?;
        assert_eq!(stored_info.node_num, 0x12345678);
        Ok(())
    }

    #[test]
    fn test_get_node_by_id() -> Result<()> {
        let mut state = DeviceState::new();
        let node = NodeInfo {
            id: "test123".to_string(),
            num: 0x12345678,
            user: User {
                id: "test".to_string(),
                long_name: "Test User".to_string(),
                short_name: "TU".to_string(),
                hw_model: None,
            },
            last_heard: None,
            last_heard_iso: None,
            snr: None,
            rssi: None,
        };

        state.update_node(0x12345678, node.clone());
        let found = state.get_node_by_id("test123");
        assert!(found.is_some());

        let found_node = found.context("Node not found by ID")?;
        assert_eq!(found_node.user.long_name, "Test User");

        let not_found = state.get_node_by_id("nonexistent");
        assert!(not_found.is_none());
        Ok(())
    }

    #[test]
    fn test_telemetry_update() -> Result<()> {
        let mut state = DeviceState::new();
        let telemetry = TelemetryData {
            node_num: 0x12345678,
            time: 1234567890,
            device_metrics: Some(DeviceMetrics {
                battery_level: Some(75),
                voltage: Some(3.8),
                channel_utilization: Some(10.5),
                air_util_tx: Some(5.2),
                uptime_seconds: Some(3600),
            }),
            environment_metrics: None,
            air_quality_metrics: None,
        };

        state.update_telemetry(0x12345678, telemetry.clone());
        assert_eq!(state.telemetry.len(), 1);

        let stored_telemetry = state
            .telemetry
            .get(&0x12345678)
            .context("Telemetry not found")?;
        let device_metrics = stored_telemetry
            .device_metrics
            .as_ref()
            .context("Device metrics not found")?;
        assert_eq!(device_metrics.battery_level, Some(75));
        Ok(())
    }

    #[test]
    fn test_config_storage() -> Result<()> {
        let mut state = DeviceState::new();

        let device_config = DeviceConfig {
            role: "Router".to_string(),
            button_gpio: 12,
            buzzer_gpio: 13,
            rebroadcast_mode: "All".to_string(),
            node_info_broadcast_secs: 900,
            tzdef: Some("PST8PDT".to_string()),
            disable_triple_click: false,
        };

        state.device_config = Some(device_config);
        assert!(state.device_config.is_some());

        let config = state
            .device_config
            .as_ref()
            .context("Device config not found")?;
        assert_eq!(config.role, "Router");

        let position_config = PositionConfig {
            position_broadcast_secs: 300,
            position_broadcast_smart_enabled: true,
            fixed_position: false,
            gps_enabled: true,
            gps_mode: "Enabled".to_string(),
        };

        state.position_config = Some(position_config);
        assert!(state.position_config.is_some());

        let pos_config = state
            .position_config
            .as_ref()
            .context("Position config not found")?;
        assert!(pos_config.gps_enabled);
        Ok(())
    }
}

#[cfg(test)]
mod mesh_tests {
    use crate::mesh::{MeshHealth, MeshNode, NetworkStats, RouteHop};
    use anyhow::Result;

    #[test]
    fn test_network_stats_creation() -> Result<()> {
        let stats = NetworkStats {
            total_nodes: 10,
            active_nodes: 8,
            neighbors: 3,
            average_snr: Some(5.5),
            average_rssi: Some(-75),
            mesh_health: MeshHealth::Good,
        };

        assert_eq!(stats.total_nodes, 10);
        assert_eq!(stats.active_nodes, 8);
        assert_eq!(stats.mesh_health, MeshHealth::Good);
        Ok(())
    }

    #[test]
    fn test_mesh_health_enum() -> Result<()> {
        // Test from_metrics logic
        assert_eq!(MeshHealth::from_metrics(0, None), MeshHealth::Isolated);
        assert_eq!(MeshHealth::from_metrics(1, None), MeshHealth::Weak);
        assert_eq!(MeshHealth::from_metrics(2, None), MeshHealth::Fair);
        assert_eq!(MeshHealth::from_metrics(2, Some(-1.0)), MeshHealth::Fair);
        assert_eq!(MeshHealth::from_metrics(2, Some(0.5)), MeshHealth::Good);
        assert_eq!(MeshHealth::from_metrics(3, Some(2.0)), MeshHealth::Good);
        assert_eq!(
            MeshHealth::from_metrics(5, Some(6.0)),
            MeshHealth::Excellent
        );

        // Test Display trait from strum
        assert_eq!(MeshHealth::Isolated.to_string(), "Isolated");
        assert_eq!(MeshHealth::Weak.to_string(), "Weak");
        assert_eq!(MeshHealth::Fair.to_string(), "Fair");
        assert_eq!(MeshHealth::Good.to_string(), "Good");
        assert_eq!(MeshHealth::Excellent.to_string(), "Excellent");

        Ok(())
    }

    #[test]
    fn test_mesh_node_creation() -> Result<()> {
        let node = MeshNode {
            id: "test123".to_string(),
            num: 0x12345678,
            name: "Test Node".to_string(),
            snr: Some(5.5),
            rssi: Some(-70),
            last_heard: Some(1234567890),
            hops_away: Some(2),
        };

        assert_eq!(node.id, "test123");
        assert_eq!(node.hops_away, Some(2));
        assert_eq!(node.name, "Test Node");
        Ok(())
    }

    #[test]
    fn test_route_hop_creation() -> Result<()> {
        let hop = RouteHop {
            node_id: 0x12345678,
            node_name: "Hop Node".to_string(),
            hop_number: 1,
            snr: Some(5.5),
            rssi: Some(-70),
        };

        assert_eq!(hop.node_id, 0x12345678);
        assert_eq!(hop.snr, Some(5.5));
        Ok(())
    }
}
