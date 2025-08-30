use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Result of a single test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub category: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub error: Option<String>,
    pub details: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

/// Summary statistics for a test category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryStats {
    pub category: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub duration_ms: u64,
}

/// Device information collected during testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub port: String,
    pub firmware_version: Option<String>,
    pub hardware_model: Option<String>,
    pub node_id: Option<String>,
    pub node_num: Option<u32>,
    pub region: Option<String>,
    pub has_gps: Option<bool>,
    pub num_channels: Option<u32>,
}

/// Complete test report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestReport {
    pub test_id: String,
    pub timestamp: DateTime<Utc>,
    pub device_info: DeviceInfo,
    pub connection_quality: ConnectionQuality,
    pub tests_run: usize,
    pub tests_passed: usize,
    pub tests_failed: usize,
    pub tests_skipped: usize,
    pub duration_ms: u64,
    pub test_results: Vec<TestResult>,
    pub category_stats: Vec<CategoryStats>,
    pub recommendations: Vec<String>,
}

/// Connection quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionQuality {
    pub packet_errors: usize,
    pub successful_packets: usize,
    pub error_rate: f64,
    pub average_response_time_ms: Option<u64>,
    pub connection_stability: String, // "Excellent", "Good", "Fair", "Poor"
}

impl TestReport {
    pub fn new(device_port: String) -> Self {
        Self {
            test_id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            device_info: DeviceInfo {
                port: device_port,
                firmware_version: None,
                hardware_model: None,
                node_id: None,
                node_num: None,
                region: None,
                has_gps: None,
                num_channels: None,
            },
            connection_quality: ConnectionQuality {
                packet_errors: 0,
                successful_packets: 0,
                error_rate: 0.0,
                average_response_time_ms: None,
                connection_stability: "Unknown".to_string(),
            },
            tests_run: 0,
            tests_passed: 0,
            tests_failed: 0,
            tests_skipped: 0,
            duration_ms: 0,
            test_results: Vec::new(),
            category_stats: Vec::new(),
            recommendations: Vec::new(),
        }
    }

    pub fn add_test_result(&mut self, result: TestResult) {
        self.tests_run += 1;
        if result.passed {
            self.tests_passed += 1;
        } else {
            self.tests_failed += 1;
        }
        self.test_results.push(result);
    }

    pub fn calculate_stats(&mut self) {
        let mut category_map: std::collections::HashMap<String, CategoryStats> =
            std::collections::HashMap::new();

        for result in &self.test_results {
            let stat = category_map
                .entry(result.category.clone())
                .or_insert_with(|| CategoryStats {
                    category: result.category.clone(),
                    total: 0,
                    passed: 0,
                    failed: 0,
                    skipped: 0,
                    duration_ms: 0,
                });

            stat.total += 1;
            stat.duration_ms += result.duration_ms;

            if result.passed {
                stat.passed += 1;
            } else {
                stat.failed += 1;
            }
        }

        self.category_stats = category_map.into_values().collect();

        // Calculate connection quality
        if self.connection_quality.successful_packets > 0
            || self.connection_quality.packet_errors > 0
        {
            let total =
                self.connection_quality.successful_packets + self.connection_quality.packet_errors;
            self.connection_quality.error_rate =
                self.connection_quality.packet_errors as f64 / total as f64;

            self.connection_quality.connection_stability = match self.connection_quality.error_rate
            {
                r if r < 0.01 => "Excellent",
                r if r < 0.05 => "Good",
                r if r < 0.10 => "Fair",
                _ => "Poor",
            }
            .to_string();
        }

        // Generate recommendations
        self.generate_recommendations();
    }

    fn generate_recommendations(&mut self) {
        self.recommendations.clear();

        if self.connection_quality.error_rate > 0.05 {
            self.recommendations.push(
                "High packet error rate detected. Check USB cable and connections.".to_string(),
            );
        }

        if self.tests_failed > self.tests_passed {
            self.recommendations.push(
                "Majority of tests failed. Device may need firmware update or reset.".to_string(),
            );
        }

        // Check for specific category failures
        for stat in &self.category_stats {
            if stat.failed > stat.passed {
                self.recommendations.push(format!(
                    "{category} tests are failing. Focus on debugging this area.",
                    category = stat.category
                ));
            }
        }
    }

    pub fn print_summary(&self) {
        use colored::*;

        println!(
            "\n{separator}",
            separator = "═══════════════════════════════════════════════════════".bold()
        );
        println!(
            "{title}",
            title = "                   TEST REPORT SUMMARY                  "
                .bold()
                .cyan()
        );
        println!(
            "{separator}",
            separator = "═══════════════════════════════════════════════════════".bold()
        );

        println!("\n{section}", section = "Device Information:".bold());
        println!("  Port: {port}", port = self.device_info.port);
        if let Some(fw) = &self.device_info.firmware_version {
            println!("  Firmware: {fw}");
        }
        if let Some(hw) = &self.device_info.hardware_model {
            println!("  Hardware: {hw}");
        }

        println!("\n{section}", section = "Test Results:".bold());
        println!("  Total Tests: {total}", total = self.tests_run);
        println!(
            "  Passed: {passed} {percentage}",
            passed = self.tests_passed,
            percentage = format!(
                "({percent}%)",
                percent = self.tests_passed * 100 / self.tests_run.max(1)
            )
            .green()
        );
        println!(
            "  Failed: {failed} {percentage}",
            failed = self.tests_failed,
            percentage = if self.tests_failed > 0 {
                format!(
                    "({percent}%)",
                    percent = self.tests_failed * 100 / self.tests_run.max(1)
                )
                .red()
            } else {
                "".normal()
            }
        );

        println!("\n{section}", section = "Connection Quality:".bold());
        println!(
            "  Packet Success Rate: {rate:.1}%",
            rate = (1.0 - self.connection_quality.error_rate) * 100.0
        );
        println!(
            "  Connection Stability: {stability}",
            stability = match self.connection_quality.connection_stability.as_str() {
                "Excellent" => self.connection_quality.connection_stability.green(),
                "Good" => self.connection_quality.connection_stability.green(),
                "Fair" => self.connection_quality.connection_stability.yellow(),
                "Poor" => self.connection_quality.connection_stability.red(),
                _ => self.connection_quality.connection_stability.normal(),
            }
        );

        if !self.recommendations.is_empty() {
            println!("\n{section}", section = "Recommendations:".bold().yellow());
            for rec in &self.recommendations {
                println!("  • {rec}");
            }
        }

        println!(
            "\n{}",
            "═══════════════════════════════════════════════════════".bold()
        );
    }
}
