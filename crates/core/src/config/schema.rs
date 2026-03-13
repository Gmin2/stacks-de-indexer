//! Configuration data structures.

use serde::Deserialize;

/// Top-level indexer configuration, typically loaded from `stacks-indexer.yaml`.
#[derive(Debug, Clone, Deserialize)]
pub struct IndexerConfig {
    /// Human-readable name for this indexer instance.
    pub name: String,
    /// Target network: `"mainnet"`, `"testnet"`, or `"devnet"`.
    pub network: String,
    /// Server port configuration.
    pub server: ServerConfig,
    /// Database storage configuration.
    pub storage: StorageConfig,
    /// Contract sources and events to index.
    #[serde(default)]
    pub sources: Vec<SourceConfig>,
    /// Optional Stacks RPC URL for backfill (defaults per network).
    #[serde(default)]
    pub rpc_url: Option<String>,
}

/// Server port configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// Port on which to receive stacks-core event POSTs (default: 20445).
    #[serde(default = "default_event_port")]
    pub event_listener_port: u16,
    /// Port for the GraphQL / REST API (default: 4000).
    #[serde(default = "default_api_port")]
    pub api_port: u16,
}

/// Database storage configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    /// Path to the SQLite database file.
    #[serde(default = "default_db_path")]
    pub path: String,
}

/// A contract source to index events from.
#[derive(Debug, Clone, Deserialize)]
pub struct SourceConfig {
    /// Fully qualified contract identifier (e.g. `SP2C2YFP12AJZB1.contract-name`).
    pub contract: String,
    /// Block height to start indexing from (for backfill).
    #[serde(default)]
    pub start_block: u64,
    /// Events to capture from this contract.
    #[serde(default)]
    pub events: Vec<EventConfig>,
}

/// An event to capture and store in a SQLite table.
#[derive(Debug, Clone, Deserialize)]
pub struct EventConfig {
    /// Human-readable event name.
    pub name: String,
    /// Event type to match: `print_event`, `stx_transfer`, `ft_mint`, etc.
    #[serde(rename = "type")]
    pub event_type: String,
    /// Target SQLite table name.
    pub table: String,
    /// JSON fields to create SQLite indexes on.
    #[serde(default)]
    pub indexes: Vec<String>,
}

fn default_event_port() -> u16 {
    20445
}

fn default_api_port() -> u16 {
    4000
}

fn default_db_path() -> String {
    "./data/indexer.db".to_string()
}
