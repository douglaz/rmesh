mod report;
mod runner;
mod tests;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use colored::*;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
    Markdown,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Args {
    /// Serial port or TCP address (e.g., /dev/ttyUSB0 or 192.168.1.100:4403)
    #[arg(short, long)]
    port: Option<String>,

    /// Auto-detect connected device
    #[arg(short, long, conflicts_with = "port")]
    auto_detect: bool,

    /// Test categories to run (comma-separated: connection,device,messaging,etc.)
    #[arg(short, long, value_delimiter = ',')]
    tests: Option<Vec<String>>,

    /// Output format
    #[arg(short = 'f', long, default_value = "human")]
    format: OutputFormat,

    /// Output file path
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Connection timeout in seconds
    #[arg(long, default_value = "30")]
    timeout: u64,

    /// Non-interactive mode (disables progress bars, suitable for nohup/background execution)
    #[arg(long)]
    non_interactive: bool,

    /// Quiet mode (suppress non-critical errors like packet sync issues)
    #[arg(short = 'q', long)]
    quiet: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Setup logging
    let filter = if args.quiet {
        // In quiet mode, suppress errors from meshtastic stream_buffer
        EnvFilter::new("warn,meshtastic::connections::stream_buffer=off")
    } else if args.verbose {
        EnvFilter::new("debug")
    } else {
        // Normal mode: show info but suppress repetitive packet sync errors
        EnvFilter::new("info,meshtastic::connections::stream_buffer=warn")
    };

    // Check if we're connected to a TTY
    let is_tty = atty::is(atty::Stream::Stdout);
    let non_interactive = args.non_interactive || !is_tty;

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .init();

    // Print header
    println!(
        "{separator}",
        separator = "╔════════════════════════════════════════════════════════╗".bold()
    );
    println!(
        "{title}",
        title = "║          Meshtastic Hardware Test Suite v0.1.0        ║"
            .bold()
            .cyan()
    );
    println!(
        "{separator}",
        separator = "╚════════════════════════════════════════════════════════╝".bold()
    );
    println!();

    // Determine port
    let port = if let Some(p) = args.port {
        p
    } else if args.auto_detect {
        auto_detect_device().await?
    } else {
        // Try common ports
        let common_ports = vec![
            "/dev/ttyACM0",
            "/dev/ttyUSB0",
            "/dev/ttyUSB1",
            "/dev/tty.usbserial",
            "/dev/tty.usbmodem",
        ];

        let mut found_port = String::new();
        for port in common_ports {
            if std::path::Path::new(port).exists() {
                eprintln!(
                    "{arrow} Found device at {port}",
                    arrow = "→".green(),
                    port = port.bold()
                );
                found_port = port.to_string();
                break;
            }
        }

        anyhow::ensure!(
            !found_port.is_empty(),
            "No device found. Please specify --port or use --auto-detect"
        );
        found_port
    };

    // Create test runner
    let mut runner = runner::TestRunner::new(port.clone(), args.verbose, non_interactive).await?;

    // Run tests
    let report = if let Some(test_list) = args.tests {
        runner.run_specific_tests(test_list).await?
    } else {
        runner.run_all_tests().await?
    };

    // Output results
    match args.format {
        OutputFormat::Human => {
            report.print_summary();
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&report)?;
            if let Some(output_path) = args.output {
                std::fs::write(output_path, json)?;
            } else {
                println!("{json}");
            }
        }
        OutputFormat::Markdown => {
            let markdown = generate_markdown_report(&report);
            if let Some(output_path) = args.output {
                std::fs::write(output_path, markdown)?;
            } else {
                println!("{markdown}");
            }
        }
    }

    // Exit with appropriate code
    if report.tests_failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}

async fn auto_detect_device() -> Result<String> {
    eprintln!(
        "{arrow} Auto-detecting Meshtastic device...",
        arrow = "→".cyan()
    );

    // Check common serial port locations used by Meshtastic devices
    // First check /dev/serial/by-id for most reliable identification
    if let Ok(entries) = std::fs::read_dir("/dev/serial/by-id") {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                let lower_name = name.to_lowercase();
                // Check for Meshtastic-related identifiers
                if lower_name.contains("meshtastic") ||
                   lower_name.contains("esp32") ||
                   lower_name.contains("heltec") ||
                   lower_name.contains("lilygo") ||
                   lower_name.contains("tbeam") ||
                   lower_name.contains("t-beam") ||
                   lower_name.contains("rak") ||
                   lower_name.contains("wisblock") ||
                   lower_name.contains("cp210") ||  // CP2102/CP2104 USB-Serial
                   lower_name.contains("ch340") ||  // CH340 USB-Serial
                   lower_name.contains("ch9102")
                {
                    // CH9102 USB-Serial
                    if let Ok(path) = entry.path().canonicalize() {
                        eprintln!(
                            "{check} Found device: {name} -> {path}",
                            check = "✓".green(),
                            name = name.bold(),
                            path = path.display()
                        );
                        return Ok(path.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    // Check common device paths directly
    let common_ports = vec![
        "/dev/ttyACM0", // Most common for modern ESP32-S3 devices
        "/dev/ttyACM1",
        "/dev/ttyUSB0", // Common for CP2102/CH340 based devices
        "/dev/ttyUSB1",
        "/dev/tty.usbserial",      // macOS
        "/dev/tty.usbmodem",       // macOS
        "/dev/tty.SLAB_USBtoUART", // macOS Silicon Labs
    ];

    for port in common_ports {
        if std::path::Path::new(port).exists() {
            // Try to verify it's actually accessible as a serial port
            // We'll just check if the path exists and is a character device
            if let Ok(metadata) = std::fs::metadata(port) {
                use std::os::unix::fs::FileTypeExt;
                if metadata.file_type().is_char_device() {
                    eprintln!(
                        "{check} Found device at {port}",
                        check = "✓".green(),
                        port = port.bold()
                    );
                    return Ok(port.to_string());
                }
            }
        }
    }

    // Also check numbered variants
    for base in &[
        "/dev/ttyACM",
        "/dev/ttyUSB",
        "/dev/tty.usbserial-",
        "/dev/tty.usbmodem",
    ] {
        for i in 0..10 {
            let port = format!("{base}{i}");
            if std::path::Path::new(&port).exists()
                && let Ok(metadata) = std::fs::metadata(&port)
            {
                use std::os::unix::fs::FileTypeExt;
                if metadata.file_type().is_char_device() {
                    eprintln!(
                        "{check} Found device at {port}",
                        check = "✓".green(),
                        port = port.bold()
                    );
                    return Ok(port);
                }
            }
        }
    }

    anyhow::bail!("No Meshtastic device detected. Please connect a device or specify --port")
}

fn generate_markdown_report(report: &report::TestReport) -> String {
    let mut md = String::new();

    md.push_str("# Meshtastic Hardware Test Report\n\n");
    md.push_str(&format!("**Test ID:** {id}\n", id = report.test_id));
    md.push_str(&format!(
        "**Date:** {timestamp}\n",
        timestamp = report.timestamp
    ));
    md.push_str(&format!(
        "**Device:** {port}\n\n",
        port = report.device_info.port
    ));

    md.push_str("## Summary\n\n");
    md.push_str(&format!(
        "- **Total Tests:** {total}\n",
        total = report.tests_run
    ));
    md.push_str(&format!(
        "- **Passed:** {passed} ({percentage:.1}%)\n",
        passed = report.tests_passed,
        percentage = report.tests_passed as f64 / report.tests_run as f64 * 100.0
    ));
    md.push_str(&format!(
        "- **Failed:** {failed} ({percentage:.1}%)\n",
        failed = report.tests_failed,
        percentage = report.tests_failed as f64 / report.tests_run as f64 * 100.0
    ));
    md.push('\n');

    md.push_str("## Test Results\n\n");
    md.push_str("| Category | Test | Result | Duration | Details |\n");
    md.push_str("|----------|------|--------|----------|----------|\n");

    for result in &report.test_results {
        let status = if result.passed {
            "✅ Pass"
        } else {
            "❌ Fail"
        };
        let details = if let Some(err) = &result.error {
            err.clone()
        } else {
            "OK".to_string()
        };

        md.push_str(&format!(
            "| {category} | {name} | {status} | {duration}ms | {details} |\n",
            category = result.category,
            name = result.name,
            status = status,
            duration = result.duration_ms,
            details = details
        ));
    }

    md.push_str("\n## Recommendations\n\n");
    for rec in &report.recommendations {
        md.push_str(&format!("- {rec}\n"));
    }

    md
}
