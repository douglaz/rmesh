use colored::*;

pub fn print_error(message: &str) {
    eprintln!("{} {}", "Error:".red().bold(), message);
}

pub fn print_success(message: &str) {
    eprintln!("{} {}", "✓".green().bold(), message);
}

pub fn print_warning(message: &str) {
    eprintln!("{} {}", "⚠".yellow().bold(), message);
}

pub fn print_info(message: &str) {
    eprintln!("{} {}", "ℹ".blue().bold(), message);
}
