use colored::*;

pub fn print_error(message: &str) {
    eprintln!("{prefix} {message}", prefix = "Error:".red().bold());
}

pub fn print_success(message: &str) {
    eprintln!("{prefix} {message}", prefix = "✓".green().bold());
}

pub fn print_warning(message: &str) {
    eprintln!("{prefix} {message}", prefix = "⚠".yellow().bold());
}

pub fn print_info(message: &str) {
    eprintln!("{prefix} {message}", prefix = "ℹ".blue().bold());
}
