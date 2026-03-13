//! Run a minimal indexer against a local Clarinet devnet.
//!
//! Loads config from `examples/devnet-explorer.yaml` and starts listening
//! for block events from stacks-core. When a block arrives, it matches
//! events against the configured sources and stores them in SQLite.
//!
//! Prerequisites:
//!   1. Add to your Clarinet `settings/Devnet.toml`:
//!      [[devnet.events_observer]]
//!      url = "http://host.docker.internal:20445"
//!   2. Start devnet: `clarinet devnet start`
//!   3. Run this: `cargo run --example local_devnet`
//!
//! Run with:
//!   cargo run --example local_devnet

use std::path::Path;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};

use stacks_indexer_core::config::load_config;
use stacks_indexer_core::matcher::EventMatcher;
use stacks_indexer_core::types::{BlockPayload, BurnBlockPayload};
use stacks_indexer_storage::Database;

struct AppState {
    db: Database,
    matcher: EventMatcher,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let config = load_config(Path::new("examples/devnet-explorer.yaml"))?;
    let db = Database::open(&config)?;
    let matcher = EventMatcher::from_config(&config);
    let state = Arc::new(AppState { db, matcher });

    let app = Router::new()
        .route("/new_block", post(handle_block))
        .route("/new_burn_block", post(handle_burn_block))
        .route("/new_mempool_tx", post(handle_mempool))
        .route("/drop_mempool_tx", post(handle_drop))
        .route("/new_microblocks", post(handle_microblocks))
        .route("/attachments/new", post(handle_attachments))
        .with_state(state);

    let port = config.server.event_listener_port;
    tracing::info!("devnet explorer listening on port {port}");
    tracing::info!("waiting for blocks from Clarinet devnet...");

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn handle_block(
    State(state): State<Arc<AppState>>,
    Json(block): Json<BlockPayload>,
) -> impl IntoResponse {
    println!(
        "block #{} ({}) - {} txs, {} events",
        block.block_height, block.block_hash,
        block.transactions.len(), block.events.len(),
    );

    for tx in &block.transactions {
        let status = if tx.status == "success" { "ok" } else { "fail" };
        println!("  tx {} [{}] {}", &tx.txid[..20.min(tx.txid.len())], tx.tx_index, status);
    }

    let matched = state.matcher.match_events(&block);
    if !matched.is_empty() {
        println!("  matched {} events:", matched.len());
        for m in &matched {
            println!("    {} -> table:{}", m.event_name, m.table);
        }
    }

    if let Err(e) = state.db.apply_block(&block, &matched) {
        tracing::error!("storage error: {e}");
    }

    StatusCode::OK
}

async fn handle_burn_block(
    State(_): State<Arc<AppState>>,
    Json(bb): Json<BurnBlockPayload>,
) -> impl IntoResponse {
    println!("burn block #{}", bb.burn_block_height);
    StatusCode::OK
}

async fn handle_mempool(
    State(_): State<Arc<AppState>>,
    Json(txs): Json<Vec<String>>,
) -> impl IntoResponse {
    if !txs.is_empty() {
        println!("{} mempool txs", txs.len());
    }
    StatusCode::OK
}

async fn handle_drop(
    State(_): State<Arc<AppState>>,
    Json(_): Json<serde_json::Value>,
) -> impl IntoResponse {
    StatusCode::OK
}

async fn handle_microblocks(
    State(_): State<Arc<AppState>>,
    Json(_): Json<serde_json::Value>,
) -> impl IntoResponse {
    StatusCode::OK
}

async fn handle_attachments(
    State(_): State<Arc<AppState>>,
    Json(_): Json<serde_json::Value>,
) -> impl IntoResponse {
    StatusCode::OK
}
