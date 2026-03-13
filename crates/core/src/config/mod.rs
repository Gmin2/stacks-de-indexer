//! YAML configuration loading and validation.
//!
//! The indexer is configured with a YAML file that declares which contracts
//! and event types to index. Each configured event maps to a SQLite table.
//!
//! # Example configuration
//!
//! ```yaml
//! name: "my-indexer"
//! network: mainnet
//! server:
//!   event_listener_port: 20445
//!   api_port: 4000
//! storage:
//!   path: "./data/indexer.db"
//! sources:
//!   - contract: "SP2C2YFP12AJZB1.arkadiko-vault"
//!     start_block: 34239
//!     events:
//!       - name: vault_created
//!         type: print_event
//!         table: vaults
//!         indexes: ["owner"]
//! ```

mod schema;

pub use schema::*;

use std::path::Path;

/// Load and validate an indexer configuration from a YAML file.
///
/// # Errors
///
/// Returns an error if the file cannot be read, the YAML is malformed,
/// or validation fails (invalid network, duplicate tables, etc.).
#[track_caller]
pub fn load_config(path: &Path) -> anyhow::Result<IndexerConfig> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("failed to read config '{}': {}", path.display(), e))?;
    let config: IndexerConfig = serde_yaml::from_str(&contents)?;
    validate(&config)?;
    Ok(config)
}

/// Parse and validate config from a YAML string (useful for tests).
pub fn parse_config(yaml: &str) -> anyhow::Result<IndexerConfig> {
    let config: IndexerConfig = serde_yaml::from_str(yaml)?;
    validate(&config)?;
    Ok(config)
}

/// Validate business rules that serde cannot enforce.
fn validate(config: &IndexerConfig) -> anyhow::Result<()> {
    if config.name.is_empty() {
        anyhow::bail!("config 'name' must not be empty");
    }

    match config.network.as_str() {
        "mainnet" | "testnet" | "devnet" => {}
        other => anyhow::bail!("invalid network '{other}': must be mainnet, testnet, or devnet"),
    }

    let mut tables = std::collections::HashSet::new();
    for source in &config.sources {
        if source.contract != "*" && !source.contract.contains('.') {
            anyhow::bail!(
                "invalid contract identifier '{}': expected '*' or <principal>.<contract-name>",
                source.contract
            );
        }

        for event in &source.events {
            if !tables.insert(&event.table) {
                anyhow::bail!("duplicate table name '{}'", event.table);
            }

            match event.event_type.as_str() {
                "print_event" | "stx_transfer" | "stx_mint" | "stx_burn" | "stx_lock"
                | "ft_transfer" | "ft_mint" | "ft_burn" | "nft_transfer" | "nft_mint"
                | "nft_burn" => {}
                other => anyhow::bail!(
                    "invalid event type '{other}' for '{}': expected one of print_event, \
                     stx_transfer, stx_mint, stx_burn, stx_lock, ft_transfer, ft_mint, \
                     ft_burn, nft_transfer, nft_mint, nft_burn",
                    event.name
                ),
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_config() {
        let yaml = r#"
name: "test-indexer"
network: devnet
server:
  event_listener_port: 20445
  api_port: 4000
storage:
  path: "./data/test.db"
sources:
  - contract: "SP2C2YFP12AJZB1.arkadiko-vault"
    start_block: 34239
    events:
      - name: vault_created
        type: print_event
        table: vaults
"#;
        assert!(parse_config(yaml).is_ok());
    }

    #[test]
    fn rejects_invalid_network() {
        let yaml = r#"
name: "test"
network: invalid
server: { event_listener_port: 20445, api_port: 4000 }
storage: { path: "test.db" }
sources: []
"#;
        assert!(parse_config(yaml).is_err());
    }

    #[test]
    fn rejects_duplicate_table() {
        let yaml = r#"
name: "test"
network: devnet
server: { event_listener_port: 20445, api_port: 4000 }
storage: { path: "test.db" }
sources:
  - contract: "SP1.contract-a"
    events:
      - { name: e1, type: print_event, table: dup }
      - { name: e2, type: stx_transfer, table: dup }
"#;
        assert!(parse_config(yaml).is_err());
    }

    #[test]
    fn rejects_invalid_contract_id() {
        let yaml = r#"
name: "test"
network: devnet
server: { event_listener_port: 20445, api_port: 4000 }
storage: { path: "test.db" }
sources:
  - contract: "no-dot-here"
    events:
      - { name: e, type: print_event, table: t }
"#;
        assert!(parse_config(yaml).is_err());
    }
}
