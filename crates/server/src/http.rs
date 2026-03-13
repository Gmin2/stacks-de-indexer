//! Axum HTTP server — event listener and API endpoints.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use tokio::sync::broadcast;

use stacks_indexer_core::config::IndexerConfig;
use stacks_indexer_core::matcher::EventMatcher;
use stacks_indexer_core::types::{BlockPayload, BurnBlockPayload, DroppedTxPayload, MempoolTxPayload};
use stacks_indexer_storage::Database;

use crate::metrics::Metrics;

/// Shared application state accessible by all handlers.
struct AppState {
    db: Arc<Database>,
    matcher: EventMatcher,
    metrics: Metrics,
    config: IndexerConfig,
    event_tx: broadcast::Sender<serde_json::Value>,
    dev_mode: bool,
}

/// Start both the event listener and API servers, blocking until shutdown.
pub async fn run(config: IndexerConfig, dev_mode: bool) -> anyhow::Result<()> {
    let db = Arc::new(Database::open(&config)?);
    let matcher = EventMatcher::from_config(&config);
    let metrics = Metrics::new();
    let (event_tx, _) = broadcast::channel(1024);

    let (last_height, last_hash) = db.get_last_processed_block()?;
    tracing::info!(
        "indexer '{}' starting on {} (last block: #{} {})",
        config.name,
        config.network,
        last_height,
        if last_hash.is_empty() { "(genesis)" } else { &last_hash }
    );

    let schema = crate::graphql::build_schema(&config, db.clone(), event_tx.clone())?;

    let state = Arc::new(AppState {
        db,
        matcher,
        metrics,
        config: config.clone(),
        event_tx,
        dev_mode,
    });

    // Event listener (receives POSTs from stacks-core)
    let event_router = Router::new()
        .route("/new_block", post(handle_new_block))
        .route("/new_burn_block", post(handle_new_burn_block))
        .route("/new_mempool_tx", post(handle_new_mempool_tx))
        .route("/drop_mempool_tx", post(handle_drop_mempool_tx))
        .route("/new_microblocks", post(handle_new_microblocks))
        .route("/attachments/new", post(handle_attachments))
        .layer(axum::extract::DefaultBodyLimit::max(512 * 1024 * 1024)) // 512MB for genesis
        .with_state(state.clone());

    // API server (health, metrics, GraphQL)
    let api_router = Router::new()
        .route("/health", get(handle_health))
        .route("/metrics", get(handle_metrics))
        .route("/graphql", get(graphql_playground).post(graphql_handler))
        .route("/graphql/ws", get(graphql_ws))
        .layer(tower_http::cors::CorsLayer::permissive())
        .layer(axum::Extension(schema))
        .with_state(state.clone());

    let ev_port = config.server.event_listener_port;
    let api_port = config.server.api_port;

    tracing::info!("event listener on port {ev_port}");
    tracing::info!("API server on port {api_port}");
    if dev_mode {
        tracing::info!("GraphQL playground at http://localhost:{api_port}/graphql");
    }

    let ev_server = axum::serve(
        tokio::net::TcpListener::bind(format!("[::]:{ev_port}")).await?,
        event_router,
    );
    let api_server = axum::serve(
        tokio::net::TcpListener::bind(format!("[::]:{api_port}")).await?,
        api_router,
    );

    tokio::select! {
        r = ev_server => { r?; }
        r = api_server => { r?; }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("shutting down...");
        }
    }

    Ok(())
}

// Event handlers

async fn handle_new_block(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<BlockPayload>,
) -> impl IntoResponse {
    tracing::info!(
        "block #{} ({}) — {} txs, {} events (queued)",
        payload.block_height,
        truncate(&payload.block_hash, 10),
        payload.transactions.len(),
        payload.events.len(),
    );

    // Respond 200 immediately so stacks-core doesn't timeout,
    // then process the block in the background.
    tokio::task::spawn_blocking(move || {
        process_block(&state, payload);
    });

    StatusCode::OK
}

/// Synchronous block processing — runs on a blocking thread pool.
fn process_block(state: &AppState, payload: BlockPayload) {
    let start = Instant::now();

    // Reorg check
    match stacks_indexer_storage::reorg::detect_reorg(&state.db, &payload) {
        Ok(true) => {
            state.metrics.reorgs_detected_total.inc();
            if let Err(e) = stacks_indexer_storage::reorg::handle_reorg(&state.db, &payload) {
                tracing::error!("reorg handling failed: {e}");
                return;
            }
        }
        Err(e) => {
            tracing::error!("reorg detection failed: {e}");
            return;
        }
        _ => {}
    }

    // Match events
    let matched = state.matcher.match_events(&payload);
    if !matched.is_empty() {
        let summary: HashMap<&str, u32> =
            matched.iter().fold(HashMap::new(), |mut acc, e| {
                *acc.entry(e.table.as_str()).or_default() += 1;
                acc
            });
        let parts: Vec<String> = summary.iter().map(|(t, c)| format!("{c} {t}")).collect();
        tracing::info!("  matched {} events: {}", matched.len(), parts.join(", "));
    }

    // Store
    if let Err(e) = state.db.apply_block(&payload, &matched) {
        tracing::error!("failed to store block: {e}");
        return;
    }

    // Metrics
    let elapsed = start.elapsed();
    state.metrics.last_block_height.set(payload.block_height as f64);
    state.metrics.blocks_processed_total.inc();
    state.metrics.block_processing_duration.observe(elapsed.as_secs_f64());
    for e in &matched {
        state.metrics.events_matched_total.with_label_values(&[&e.table]).inc();
    }

    tracing::info!(
        "  block #{} processed in {:.2}s",
        payload.block_height,
        elapsed.as_secs_f64(),
    );

    // Broadcast for subscriptions
    let _ = state.event_tx.send(serde_json::json!({
        "_type": "new_block",
        "block_height": payload.block_height,
        "block_hash": payload.block_hash,
    }));
    for e in &matched {
        let _ = state.event_tx.send(serde_json::json!({
            "_table": e.table,
            "_event_name": e.event_name,
            "block_height": payload.block_height,
            "data": e.envelope.event.to_json(),
        }));
    }

    // Periodic maintenance
    if payload.block_height % 100 == 0 {
        let _ = state.db.prune_journal(100);
        if let Ok(meta) = std::fs::metadata(&state.config.storage.path) {
            state.metrics.storage_size_bytes.set(meta.len() as f64);
        }
    }
}

async fn handle_new_burn_block(
    State(_state): State<Arc<AppState>>,
    Json(payload): Json<BurnBlockPayload>,
) -> impl IntoResponse {
    tracing::debug!(
        "burn block #{} ({})",
        payload.burn_block_height,
        truncate(&payload.burn_block_hash, 10),
    );
    StatusCode::OK
}

async fn handle_new_mempool_tx(
    State(_state): State<Arc<AppState>>,
    Json(payload): Json<MempoolTxPayload>,
) -> impl IntoResponse {
    tracing::debug!("mempool: {} new txs", payload.len());
    StatusCode::OK
}

async fn handle_drop_mempool_tx(
    State(_state): State<Arc<AppState>>,
    Json(payload): Json<DroppedTxPayload>,
) -> impl IntoResponse {
    tracing::debug!("mempool drop: {} txs ({})", payload.dropped_txids.len(), payload.reason);
    StatusCode::OK
}

async fn handle_new_microblocks(
    State(_state): State<Arc<AppState>>,
    Json(_payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    tracing::debug!("microblocks received (pre-Nakamoto, ignored)");
    StatusCode::OK
}

async fn handle_attachments(
    State(_state): State<Arc<AppState>>,
    Json(_payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    tracing::debug!("attachments received");
    StatusCode::OK
}

// API handlers

async fn handle_health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let (height, _) = state.db.get_last_processed_block().unwrap_or((0, String::new()));
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "network": state.config.network,
        "last_block_height": height,
    }))
}

async fn handle_metrics(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        state.metrics.render(),
    )
}

async fn graphql_playground(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    if state.dev_mode {
        axum::response::Html(async_graphql::http::playground_source(
            async_graphql::http::GraphQLPlaygroundConfig::new("/graphql")
                .subscription_endpoint("/graphql/ws"),
        ))
        .into_response()
    } else {
        (StatusCode::NOT_FOUND, "playground only available in dev mode").into_response()
    }
}

async fn graphql_handler(
    schema: axum::Extension<async_graphql::dynamic::Schema>,
    req: async_graphql_axum::GraphQLRequest,
) -> impl IntoResponse {
    async_graphql_axum::GraphQLResponse::from(schema.execute(req.into_inner()).await)
}

async fn graphql_ws(
    schema: axum::Extension<async_graphql::dynamic::Schema>,
    protocol: async_graphql_axum::GraphQLProtocol,
    ws: axum::extract::WebSocketUpgrade,
) -> impl IntoResponse {
    let schema = schema.0.clone();
    ws.protocols(async_graphql::http::ALL_WEBSOCKET_PROTOCOLS)
        .on_upgrade(move |stream| {
            let ws = async_graphql_axum::GraphQLWebSocket::new(stream, schema, protocol);
            async move { let _ = ws.serve().await; }
        })
}

// Utilities

fn truncate(s: &str, max: usize) -> &str {
    &s[..s.len().min(max)]
}
