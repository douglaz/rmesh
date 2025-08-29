use anyhow::Result;
use chrono::Utc;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use rmesh_core::ConnectionManager;
use std::time::{Duration, Instant};

use crate::report::{TestReport, TestResult};
use crate::tests::{TestCategory, TestContext};

pub struct TestRunner {
    connection: ConnectionManager,
    report: TestReport,
    verbose: bool,
    non_interactive: bool,
    categories: Vec<TestCategory>,
    progress: Option<ProgressBar>,
}

impl TestRunner {
    pub async fn new(port: String, verbose: bool, non_interactive: bool) -> Result<Self> {
        eprintln!(
            "{arrow} Connecting to device on {port}...",
            arrow = "→".cyan(),
            port = port.bold()
        );

        let mut connection = ConnectionManager::new(
            Some(port.clone()),
            None, // No BLE support in test
            Duration::from_secs(30),
        )
        .await?;

        connection.connect().await?;

        eprintln!("{check} Connected successfully!", check = "✓".green());

        Ok(Self {
            connection,
            report: TestReport::new(port),
            verbose,
            non_interactive,
            categories: vec![
                TestCategory::Connection,
                TestCategory::Device,
                TestCategory::Messaging,
                TestCategory::Configuration,
                TestCategory::Channels,
                TestCategory::Position,
                TestCategory::Mesh,
                TestCategory::Telemetry,
            ],
            progress: None,
        })
    }

    pub async fn run_all_tests(&mut self) -> Result<TestReport> {
        let start_time = Instant::now();

        eprintln!(
            "\n{message}",
            message = "Starting hardware tests...".bold().cyan()
        );

        // Setup progress bar only if in interactive mode
        if !self.non_interactive {
            let total_tests = self.estimate_total_tests();
            self.progress = Some(ProgressBar::new(total_tests as u64));
            if let Some(pb) = &self.progress {
                pb.set_style(
                    ProgressStyle::default_bar()
                        .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                        .unwrap_or_else(|_| ProgressStyle::default_bar())
                        .progress_chars("#>-"),
                );
            }
        } else if self.verbose {
            let total_tests = self.estimate_total_tests();
            eprintln!(
                "{arrow} Running {total} tests across {categories} categories",
                arrow = "→".blue(),
                total = total_tests,
                categories = self.categories.len()
            );
        }

        // Run tests for each category
        for category in self.categories.clone() {
            self.run_category_tests(category).await?;
        }

        // Finalize report
        self.report.duration_ms = start_time.elapsed().as_millis() as u64;
        self.report.calculate_stats();

        if let Some(pb) = &self.progress {
            pb.finish_and_clear();
        }

        Ok(self.report.clone())
    }

    async fn run_category_tests(&mut self, category: TestCategory) -> Result<()> {
        let category_name = format!("{category:?}");

        if self.verbose || self.non_interactive {
            eprintln!(
                "\n{arrow} Running {category} tests...",
                arrow = "→".blue(),
                category = category_name.bold()
            );
        }

        let tests = category.get_tests();

        for test in tests {
            let test_start = Instant::now();

            // Update progress bar only if in interactive mode
            if let Some(pb) = &self.progress {
                pb.set_message(format!("{category_name}: {name}", name = test.name));
            } else if self.non_interactive && self.verbose {
                eprintln!(
                    "  {arrow} Testing: {name}",
                    arrow = "→".cyan(),
                    name = test.name
                );
            }

            let mut context = TestContext::new(&mut self.connection, self.verbose);
            let (passed, details, error) = match (test.run_fn)(&mut context).await {
                Ok(details) => (true, details, None),
                Err(e) => {
                    let error_msg = format!("{e:?}");
                    (
                        false,
                        serde_json::json!({"error": &error_msg}),
                        Some(error_msg),
                    )
                }
            };

            let result = TestResult {
                name: test.name.to_string(),
                category: category_name.clone(),
                passed,
                duration_ms: test_start.elapsed().as_millis() as u64,
                error,
                details,
                timestamp: Utc::now(),
            };

            // Always show results in non-interactive mode or when verbose
            if self.verbose || self.non_interactive {
                if result.passed {
                    eprintln!(
                        "  {check} {name} ({duration}ms)",
                        check = "✓".green(),
                        name = test.name,
                        duration = result.duration_ms
                    );
                } else {
                    eprintln!(
                        "  {cross} {name} - {error} ({duration}ms)",
                        cross = "✗".red(),
                        name = test.name,
                        error = result
                            .error
                            .as_ref()
                            .unwrap_or(&"Unknown error".to_string())
                            .red(),
                        duration = result.duration_ms
                    );
                }
            }

            self.report.add_test_result(result);

            // Update progress bar only if it exists
            if let Some(pb) = &self.progress {
                pb.inc(1);
            }
        }

        Ok(())
    }

    fn estimate_total_tests(&self) -> usize {
        self.categories.iter().map(|c| c.get_tests().len()).sum()
    }

    pub async fn run_specific_tests(&mut self, categories: Vec<String>) -> Result<TestReport> {
        self.categories = categories
            .iter()
            .filter_map(|name| TestCategory::from_str(name))
            .collect();

        if self.categories.is_empty() {
            anyhow::bail!("No valid test categories specified");
        }

        self.run_all_tests().await
    }
}
