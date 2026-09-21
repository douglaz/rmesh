#[cfg(test)]
mod state_tests {
    use crate::state::{
        ChannelInfo, DeviceState, LoraConfig, MyNodeInfo, NodeInfo, Position, TextMessage, User,
    };
    use crate::state::{DeviceConfig, DeviceMetrics, PositionConfig, TelemetryData};
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

    /// Builds a state whose local node reports `hw_model` UNSET and whose NodeDB also holds
    /// a neighbour with a real board — the shape that made both call sites wrong.
    fn state_with_unset_local_hw_model() -> DeviceState {
        let mut state = DeviceState::new();
        state.set_my_node_info(MyNodeInfo {
            node_num: 0x608c440e,
            node_id: "608c440e".to_string(),
            reboot_count: 0,
            min_app_version: 30200,
            device_id: "abc".to_string(),
        });
        let node = |num: u32, hw: &str| NodeInfo {
            id: format!("{num:08x}"),
            num,
            user: User {
                id: format!("!{num:08x}"),
                long_name: "n".to_string(),
                short_name: "n".to_string(),
                hw_model: Some(hw.to_string()),
            },
            last_heard: None,
            last_heard_iso: None,
            snr: None,
            rssi: None,
        };
        state.update_node(0x608c440e, node(0x608c440e, "Unset"));
        state.update_node(0x16cfa1c8, node(0x16cfa1c8, "StationG2"));
        state
    }

    /// A board that leaves hw_model UNSET in its own NodeDB entry reports the real value
    /// only in DeviceMetadata, so metadata has to win.
    #[test]
    fn test_hardware_model_prefers_metadata_over_unset_nodedb() -> Result<()> {
        let mut state = state_with_unset_local_hw_model();
        state.metadata = Some(meshtastic::protobufs::DeviceMetadata {
            hw_model: meshtastic::protobufs::HardwareModel::TrackerT1000E as i32,
            ..Default::default()
        });

        assert_eq!(
            state.hardware_model().as_deref(),
            Some("TrackerT1000E"),
            "metadata must override an UNSET NodeDB entry"
        );
        Ok(())
    }

    /// A board newer than the vendored protobufs must still report its id: prost's
    /// generated accessor maps an unrecognised value back to UNSET, which would make a real
    /// radio look like it does not know its own hardware.
    ///
    /// This is what happened to the Seeed Solar Node (95) while the protobufs were pinned
    /// at v2.5.23. Regenerating them made 95 known, so the id below is asserted to be
    /// unassigned rather than hardcoded as "some board we don't have" — otherwise a later
    /// regen silently turns this into a test of the known-board path.
    #[test]
    fn test_hardware_model_surfaces_a_board_newer_than_the_protobufs() -> Result<()> {
        const UNASSIGNED_HW_MODEL: i32 = 222;
        assert!(
            meshtastic::protobufs::HardwareModel::try_from(UNASSIGNED_HW_MODEL).is_err(),
            "hw_model {UNASSIGNED_HW_MODEL} is now a real board — pick another unassigned id"
        );

        let mut state = state_with_unset_local_hw_model();
        state.metadata = Some(meshtastic::protobufs::DeviceMetadata {
            hw_model: UNASSIGNED_HW_MODEL,
            ..Default::default()
        });

        assert_eq!(
            state.hardware_model().as_deref(),
            Some("Unknown(222)"),
            "a board the protobufs do not know must still report its id"
        );
        Ok(())
    }

    /// Without metadata there is nothing to report for this device — and in particular the
    /// neighbour's board must never be substituted for it.
    ///
    /// The local node is deliberately absent from the NodeDB rather than present-but-UNSET:
    /// with both present, a scan-any-node implementation only returns the wrong board on
    /// the HashMap orderings that happen to visit the neighbour first, so the test would
    /// pass against the very bug it exists to catch. Leaving exactly one entry makes it
    /// deterministic.
    #[test]
    fn test_hardware_model_never_reports_another_node() -> Result<()> {
        let mut state = state_with_unset_local_hw_model();
        state.nodes.remove(&0x608c440e);

        assert_eq!(
            state.hardware_model(),
            None,
            "a missing local entry must not fall through to another node's hardware"
        );
        Ok(())
    }

    /// A board that does populate its own NodeDB entry still works when metadata is absent.
    #[test]
    fn test_hardware_model_falls_back_to_local_nodedb() -> Result<()> {
        let mut state = state_with_unset_local_hw_model();
        state.update_node(
            0x608c440e,
            NodeInfo {
                id: "608c440e".to_string(),
                num: 0x608c440e,
                user: User {
                    id: "!608c440e".to_string(),
                    long_name: "n".to_string(),
                    short_name: "n".to_string(),
                    hw_model: Some("TrackerT1000E".to_string()),
                },
                last_heard: None,
                last_heard_iso: None,
                snr: None,
                rssi: None,
            },
        );

        assert_eq!(state.hardware_model().as_deref(), Some("TrackerT1000E"));
        Ok(())
    }

    #[test]
    fn test_begin_config_dump_clears_previous_completion() -> Result<()> {
        let mut state = DeviceState::new();

        state.begin_config_dump(7);
        state.config_complete = true;

        state.metadata = Some(meshtastic::protobufs::DeviceMetadata {
            firmware_version: "2.6.11.60ec05e".to_string(),
            ..Default::default()
        });

        state.begin_config_dump(42);
        assert_eq!(state.want_config_id, Some(42));
        assert!(
            !state.config_complete,
            "a reconnect must wait for its own config dump"
        );
        assert!(
            state.metadata.is_none(),
            "a dump that omits metadata must report Unknown, not the earlier firmware"
        );
        Ok(())
    }

    /// The raw sub-messages feed the read-modify-write in `config set`. A leftover copy
    /// would let one radio's complete config be written to whichever radio is attached
    /// next — the two Solar Nodes here share a tty, so that sequence is routine.
    #[test]
    fn test_begin_config_dump_clears_raw_configs() -> Result<()> {
        let mut state = DeviceState::new();
        state.raw_lora_config = Some(meshtastic::protobufs::config::LoRaConfig {
            tx_power: 30,
            ..Default::default()
        });
        state.raw_device_config = Some(meshtastic::protobufs::config::DeviceConfig::default());

        state.begin_config_dump(1);

        assert!(
            state.raw_lora_config.is_none() && state.raw_device_config.is_none(),
            "a reconnect must not write a previous radio's config to the current one"
        );
        Ok(())
    }

    /// The class, not one field. Five separate review rounds each found a different
    /// `DeviceState` field surviving a reconnect, so this asserts that nothing does:
    /// populate every cache, begin a dump, and require the struct to equal a fresh one.
    /// A field added later fails here unless `begin_config_dump` accounts for it.
    #[test]
    fn test_begin_config_dump_discards_everything_from_the_previous_radio() -> Result<()> {
        let mut state = DeviceState::new();

        state.set_my_node_info(MyNodeInfo {
            node_num: 1,
            node_id: "1".to_string(),
            reboot_count: 0,
            min_app_version: 30200,
            device_id: "d".to_string(),
        });
        state.metadata = Some(meshtastic::protobufs::DeviceMetadata::default());
        state.raw_lora_config = Some(meshtastic::protobufs::config::LoRaConfig::default());
        state.raw_device_config = Some(meshtastic::protobufs::config::DeviceConfig::default());
        state.update_channel(ChannelInfo {
            index: 1,
            name: "c".to_string(),
            role: "Secondary".to_string(),
            has_psk: true,
            settings: None,
        });
        state.config_complete = true;
        state.device_config = Some(DeviceConfig {
            role: "CLIENT".to_string(),
            button_gpio: 0,
            buzzer_gpio: 0,
            rebroadcast_mode: "ALL".to_string(),
            node_info_broadcast_secs: 1,
            tzdef: None,
            disable_triple_click: false,
        });
        state.position_config = Some(PositionConfig {
            position_broadcast_secs: 1,
            position_broadcast_smart_enabled: false,
            fixed_position: false,
            gps_enabled: true,
            gps_mode: "ENABLED".to_string(),
        });

        state.begin_config_dump(7);

        // Everything except the id we are now waiting for.
        let mut expected = DeviceState::new();
        expected.want_config_id = Some(7);
        assert_eq!(
            format!("{state:?}"),
            format!("{expected:?}"),
            "a field survived begin_config_dump; it describes the previous radio"
        );
        Ok(())
    }

    /// The channel list drives slot allocation in `channel add` and is sent back verbatim
    /// by `channel set`. Carrying it across a reconnect would let one radio's PSK be
    /// written to another — the two Solar Nodes here share a tty, so that is routine.
    #[test]
    fn test_begin_config_dump_clears_channels() -> Result<()> {
        let mut state = DeviceState::new();
        state.update_channel(ChannelInfo {
            index: 1,
            name: "other-radio".to_string(),
            role: "Secondary".to_string(),
            has_psk: true,
            settings: None,
        });

        state.begin_config_dump(1);

        assert!(
            state.channels.is_empty(),
            "a reconnect must not offer the previous radio's channels"
        );
        Ok(())
    }

    /// `config get` invalidates before requesting so a late reply cannot be mistaken for a
    /// fresh one; the read path has to actually observe the sub-message going away.
    #[test]
    fn test_invalidate_config_clears_both_views() -> Result<()> {
        let mut state = DeviceState::new();
        state.raw_lora_config = Some(meshtastic::protobufs::config::LoRaConfig::default());
        state.lora_config = Some(LoraConfig {
            use_preset: true,
            modem_preset: "LONG_FAST".to_string(),
            bandwidth: 250,
            spread_factor: 11,
            coding_rate: 5,
            frequency_offset: 0.0,
            region: "ANZ".to_string(),
            hop_limit: 3,
            tx_enabled: true,
            tx_power: 30,
            channel_num: 0,
            ignore_mqtt: false,
        });
        assert!(state.has_config("lora"));

        state.invalidate_config("lora");

        assert!(!state.has_config("lora"), "parsed view must be cleared");
        assert!(
            state.raw_lora_config.is_none(),
            "raw view must be cleared too, or the write path still sees stale data"
        );
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
