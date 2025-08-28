use serde::Serialize;
use comfy_table::Table;

#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Json,
    Table,
}

pub fn print_output<T: Serialize>(data: T, format: OutputFormat) -> anyhow::Result<()> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
        OutputFormat::Table => {
            // Default table output - override in specific implementations
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
    }
    Ok(())
}

pub fn create_table() -> Table {
    let mut table = Table::new();
    table.load_preset(comfy_table::presets::UTF8_FULL)
         .apply_modifier(comfy_table::modifiers::UTF8_ROUND_CORNERS);
    table
}