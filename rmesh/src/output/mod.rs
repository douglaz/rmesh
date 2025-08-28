use comfy_table::Table;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    Json,
    Table,
}

pub fn print_output<T: Serialize>(data: T, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            if let Ok(json) = serde_json::to_string_pretty(&data) {
                println!("{}", json);
            }
        }
        OutputFormat::Table => {
            // Default table output - override in specific implementations
            if let Ok(json) = serde_json::to_string_pretty(&data) {
                println!("{}", json);
            }
        }
    }
}

pub fn create_table() -> Table {
    let mut table = Table::new();
    table
        .load_preset(comfy_table::presets::UTF8_FULL)
        .apply_modifier(comfy_table::modifiers::UTF8_ROUND_CORNERS);
    table
}
