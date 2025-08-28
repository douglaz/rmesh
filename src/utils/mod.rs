use colored::*;

pub fn print_error(message: &str) {
    eprintln!("{} {}", "Error:".red().bold(), message);
}

pub fn print_success(message: &str) {
    println!("{} {}", "✓".green().bold(), message);
}

pub fn print_warning(message: &str) {
    println!("{} {}", "⚠".yellow().bold(), message);
}

pub fn print_info(message: &str) {
    println!("{} {}", "ℹ".blue().bold(), message);
}