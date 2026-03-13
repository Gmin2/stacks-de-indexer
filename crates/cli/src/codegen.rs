//! Rust code generation from YAML config.

use std::path::Path;

use stacks_indexer_core::config::IndexerConfig;

/// Generate Rust struct definitions from the indexer config.
///
/// Each configured event becomes a struct with system fields (`_block_height`,
/// `_tx_id`, etc.) plus event-specific fields derived from the event type.
pub fn generate(config: &IndexerConfig, output: &Path) -> anyhow::Result<()> {
    let mut code = String::new();

    code.push_str("//! Auto-generated types from stacks-indexer YAML config.\n");
    code.push_str("//! Regenerate with: `stacks-indexer codegen`\n\n");
    code.push_str("use serde::{Deserialize, Serialize};\n\n");

    for source in &config.sources {
        code.push_str(&format!("// Contract: {}\n\n", source.contract));

        for event_cfg in &source.events {
            let struct_name = to_pascal_case(&event_cfg.name);

            code.push_str(&format!(
                "/// Event `{}` — stored in table `{}`.\n",
                event_cfg.name, event_cfg.table
            ));
            code.push_str("#[derive(Debug, Clone, Serialize, Deserialize)]\n");
            code.push_str(&format!("pub struct {struct_name} {{\n"));
            code.push_str("    pub _block_height: u64,\n");
            code.push_str("    pub _block_hash: String,\n");
            code.push_str("    pub _tx_id: String,\n");
            code.push_str("    pub _event_index: u64,\n");
            code.push_str("    pub _timestamp: u64,\n");

            match event_cfg.event_type.as_str() {
                "stx_transfer" => {
                    code.push_str("    pub sender: String,\n");
                    code.push_str("    pub recipient: String,\n");
                    code.push_str("    pub amount: String,\n");
                    code.push_str("    pub memo: Option<String>,\n");
                }
                "stx_mint" => {
                    code.push_str("    pub recipient: String,\n");
                    code.push_str("    pub amount: String,\n");
                }
                "stx_burn" => {
                    code.push_str("    pub sender: String,\n");
                    code.push_str("    pub amount: String,\n");
                }
                "stx_lock" => {
                    code.push_str("    pub locked_amount: String,\n");
                    code.push_str("    pub unlock_height: String,\n");
                    code.push_str("    pub locked_address: String,\n");
                }
                "ft_transfer" | "ft_mint" | "ft_burn" => {
                    code.push_str("    pub asset_identifier: String,\n");
                    if event_cfg.event_type == "ft_burn" || event_cfg.event_type == "ft_transfer" {
                        code.push_str("    pub sender: String,\n");
                    }
                    if event_cfg.event_type == "ft_mint" || event_cfg.event_type == "ft_transfer" {
                        code.push_str("    pub recipient: String,\n");
                    }
                    code.push_str("    pub amount: String,\n");
                }
                "nft_transfer" | "nft_mint" | "nft_burn" => {
                    code.push_str("    pub asset_identifier: String,\n");
                    if event_cfg.event_type != "nft_mint" {
                        code.push_str("    pub sender: String,\n");
                    }
                    if event_cfg.event_type != "nft_burn" {
                        code.push_str("    pub recipient: String,\n");
                    }
                    code.push_str("    pub value: Option<serde_json::Value>,\n");
                }
                "print_event" => {
                    code.push_str("    pub contract_identifier: String,\n");
                    code.push_str("    pub topic: String,\n");
                    code.push_str("    pub data: serde_json::Value,\n");
                }
                _ => {
                    code.push_str("    pub data: serde_json::Value,\n");
                }
            }

            code.push_str("}\n\n");
        }
    }

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, &code)?;
    tracing::info!("generated types at {}", output.display());
    Ok(())
}

fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut c = part.chars();
            match c.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect()
}
