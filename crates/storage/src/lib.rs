//! SQLite storage engine for the Stacks native indexer.
//!
//! This crate manages all persistent state: event data, indexer progress, and
//! the reorg journal. It uses SQLite in WAL mode for concurrent reads during
//! writes and atomic rollback on chain reorganizations.
//!
//! # Architecture
//!
//! - **System tables** — `_indexer_state` tracks the chain tip; `_reorg_journal`
//!   records every insert so blocks can be rolled back atomically.
//! - **Event tables** — auto-generated from YAML config. Each table has system
//!   columns (`_id`, `_block_height`, `_block_hash`, `_tx_id`, `_event_index`,
//!   `_timestamp`, `_event_type`) plus a `_raw_data` JSONB column holding the
//!   full event payload.
//! - **Reorg handling** — on fork detection, the journal is replayed in reverse
//!   to delete all rows from the invalidated fork.
//!
//! # Modules
//!
//! - [`db`] — Core [`Database`] type with open, apply_block, rollback.
//! - [`reorg`] — Fork detection using `parent_index_block_hash`.
//! - [`backfill`] — Gap detection and block fetching from Stacks RPC on restart.

pub mod backfill;
pub mod db;
pub mod reorg;

pub use db::Database;
