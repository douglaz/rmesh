use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cached device state from received packets
#[derive(Debug, Clone, Default)]
pub struct DeviceState {
    pub nodes: HashMap<u32, NodeInfo>,
    pub channels: Vec<ChannelInfo>,
    pub config: HashMap<String, serde_json::Value>,
    pub my_node_info: Option<MyNodeInfo>,
    pub positions: HashMap<u32, Position>,
    pub messages: Vec<TextMessage>,
    pub device_config: Option<DeviceConfig>,
    pub position_config: Option<PositionConfig>,
    pub power_config: Option<PowerConfig>,
    pub network_config: Option<NetworkConfig>,
    pub display_config: Option<DisplayConfig>,
    pub lora_config: Option<LoraConfig>,
    pub bluetooth_config: Option<BluetoothConfig>,
    pub telemetry: HashMap<u32, TelemetryData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: String,
    pub num: u32,
    pub user: User,
    pub last_heard: Option<u64>,
    pub snr: Option<f32>,
    pub rssi: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub long_name: String,
    pub short_name: String,
    pub hw_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    pub index: u32,
    pub name: String,
    pub role: String,
    pub has_psk: bool,
    pub settings: Option<meshtastic::protobufs::ChannelSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyNodeInfo {
    pub node_num: u32,
    pub node_id: String,
    pub reboot_count: u32,
    pub min_app_version: u32,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub node_id: String,
    pub node_num: u32,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<i32>,
    pub time: Option<String>,
    pub last_updated: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextMessage {
    pub from: String,
    pub from_node: u32,
    pub to: String,
    pub to_node: u32,
    pub channel: u32,
    pub text: String,
    pub time: u64,
    pub snr: Option<f32>,
    pub rssi: Option<i32>,
    pub acknowledged: bool,
}

impl DeviceState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_node(&mut self, node_num: u32, node_info: NodeInfo) {
        self.nodes.insert(node_num, node_info);
    }

    pub fn update_position(&mut self, node_num: u32, position: Position) {
        self.positions.insert(node_num, position);
    }

    pub fn add_message(&mut self, message: TextMessage) {
        self.messages.push(message);
    }

    pub fn update_channel(&mut self, channel: ChannelInfo) {
        if let Some(existing) = self.channels.iter_mut().find(|c| c.index == channel.index) {
            *existing = channel;
        } else {
            self.channels.push(channel);
        }
    }

    pub fn set_my_node_info(&mut self, info: MyNodeInfo) {
        self.my_node_info = Some(info);
    }

    pub fn get_node_by_id(&self, node_id: &str) -> Option<&NodeInfo> {
        self.nodes.values().find(|n| n.id == node_id)
    }

    pub fn get_node_by_num(&self, node_num: u32) -> Option<&NodeInfo> {
        self.nodes.get(&node_num)
    }

    pub fn update_telemetry(&mut self, node_num: u32, telemetry: TelemetryData) {
        self.telemetry.insert(node_num, telemetry);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub role: String,
    pub button_gpio: u32,
    pub buzzer_gpio: u32,
    pub rebroadcast_mode: String,
    pub node_info_broadcast_secs: u32,
    pub tzdef: Option<String>,
    pub disable_triple_click: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionConfig {
    pub position_broadcast_secs: u32,
    pub position_broadcast_smart_enabled: bool,
    pub fixed_position: bool,
    pub gps_enabled: bool,
    pub gps_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerConfig {
    pub is_power_saving: bool,
    pub on_battery_shutdown_after_secs: u32,
    pub adc_multiplier_override: f32,
    pub wait_bluetooth_secs: u32,
    pub sds_secs: u32,
    pub ls_secs: u32,
    pub min_wake_secs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub wifi_enabled: bool,
    pub wifi_ssid: String,
    pub wifi_psk: String,
    pub ntp_server: String,
    pub eth_enabled: bool,
    pub ipv4_config: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    pub screen_on_secs: u32,
    pub gps_format: String,
    pub auto_screen_carousel_secs: u32,
    pub compass_north_top: bool,
    pub flip_screen: bool,
    pub units: String,
    pub displaymode: String,
    pub heading_bold: bool,
    pub wake_on_tap_or_motion: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoraConfig {
    pub use_preset: bool,
    pub modem_preset: String,
    pub bandwidth: u32,
    pub spread_factor: u32,
    pub coding_rate: u32,
    pub frequency_offset: f32,
    pub region: String,
    pub hop_limit: u32,
    pub tx_enabled: bool,
    pub tx_power: i32,
    pub channel_num: u32,
    pub ignore_mqtt: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluetoothConfig {
    pub enabled: bool,
    pub mode: String,
    pub fixed_pin: u32,
    pub device_logging_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryData {
    pub node_num: u32,
    pub time: u64,
    pub device_metrics: Option<DeviceMetrics>,
    pub environment_metrics: Option<EnvironmentMetrics>,
    pub air_quality_metrics: Option<AirQualityMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceMetrics {
    pub battery_level: Option<u32>,
    pub voltage: Option<f32>,
    pub channel_utilization: Option<f32>,
    pub air_util_tx: Option<f32>,
    pub uptime_seconds: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentMetrics {
    pub temperature: Option<f32>,
    pub relative_humidity: Option<f32>,
    pub barometric_pressure: Option<f32>,
    pub gas_resistance: Option<f32>,
    pub iaq: Option<u32>,
    pub distance: Option<f32>,
    pub lux: Option<f32>,
    pub white_lux: Option<f32>,
    pub ir_lux: Option<f32>,
    pub uv_lux: Option<f32>,
    pub wind_direction: Option<u32>,
    pub wind_speed: Option<f32>,
    pub weight: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirQualityMetrics {
    pub pm10_standard: Option<u32>,
    pub pm25_standard: Option<u32>,
    pub pm100_standard: Option<u32>,
    pub pm10_environmental: Option<u32>,
    pub pm25_environmental: Option<u32>,
    pub pm100_environmental: Option<u32>,
    pub particles_03um: Option<u32>,
    pub particles_05um: Option<u32>,
    pub particles_10um: Option<u32>,
    pub particles_25um: Option<u32>,
    pub particles_50um: Option<u32>,
    pub particles_100um: Option<u32>,
}
