//! Core types and utilities for the Stacks native indexer.
//!
//! This crate provides the foundational building blocks for indexing the Stacks
//! blockchain without depending on `chainhook-sdk`. It directly consumes the
//! HTTP event observer protocol that `stacks-core` exposes via `[[events_observer]]`.
//!
//! # Modules
//!
//! - [`types`] — Serde structs matching stacks-core's JSON payloads (`/new_block`,
//!   `/new_burn_block`, etc.) and all 11 event types.
//! - [`clarity`] — A standalone decoder for Clarity consensus-serialized values
//!   (the `raw_value` hex strings in event payloads).
//! - [`config`] — YAML configuration schema: which contracts, events, and tables
//!   the indexer should track.
//! - [`matcher`] — Filters incoming block events against the configured sources,
//!   producing [`matcher::MatchedEvent`] values ready for storage.

pub mod clarity;
pub mod config;
pub mod matcher;
pub mod types;
