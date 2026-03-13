//! Gap detection and historical block backfill from Stacks RPC.
//!
//! On startup the indexer compares its last processed block against the chain
//! tip (via `GET /v2/info`). If blocks were missed while the indexer was
//! offline, they are fetched sequentially via `GET /v3/blocks/{height}` and
//! processed through the normal match-and-store pipeline.

use stacks_indexer_core::config::IndexerConfig;
use stacks_indexer_core::matcher::EventMatcher;
use stacks_indexer_core::types::BlockPayload;

use crate::Database;

/// Detect and fill any gap between our last block and the chain tip.
///
/// Returns the number of blocks backfilled. Stops on the first fetch error
/// rather than panicking, so the indexer can still start in live-listen mode.
pub async fn backfill_gaps(
    config: &IndexerConfig,
    db: &Database,
    matcher: &EventMatcher,
) -> anyhow::Result<u64> {
    let rpc_url = config.rpc_url.clone().unwrap_or_else(|| match config.network.as_str() {
        "mainnet" => "https://stacks-node-api.mainnet.stacks.co".to_string(),
        "testnet" => "https://stacks-node-api.testnet.stacks.co".to_string(),
        _ => "http://localhost:20443".to_string(),
    });

    let (last_height, _) = db.get_last_processed_block()?;
    let chain_tip = get_chain_tip(&rpc_url).await?;

    if last_height >= chain_tip {
        tracing::info!("no gap detected (at height {last_height})");
        return Ok(0);
    }

    let gap = chain_tip - last_height;
    tracing::info!("gap detected: {gap} blocks behind (local: {last_height}, tip: {chain_tip})");

    let mut filled = 0u64;
    for height in (last_height + 1)..=chain_tip {
        match fetch_block(&rpc_url, height).await {
            Ok(block) => {
                let matched = matcher.match_events(&block);
                db.apply_block(&block, &matched)?;
                filled += 1;
                if filled % 100 == 0 {
                    tracing::info!("backfilled {filled} / {gap} blocks");
                }
            }
            Err(e) => {
                tracing::warn!("failed to fetch block {height}: {e}");
                break;
            }
        }
    }

    tracing::info!("backfill complete: {filled} blocks filled");
    Ok(filled)
}

/// Query the Stacks node for the current chain tip height.
async fn get_chain_tip(rpc_url: &str) -> anyhow::Result<u64> {
    let resp: serde_json::Value = reqwest::get(format!("{rpc_url}/v2/info"))
        .await?
        .json()
        .await?;
    resp["stacks_tip_height"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("missing stacks_tip_height in /v2/info"))
}

/// Fetch a single block by height from the Stacks RPC.
async fn fetch_block(rpc_url: &str, height: u64) -> anyhow::Result<BlockPayload> {
    let resp = reqwest::get(format!("{rpc_url}/v3/blocks/{height}")).await?;
    if !resp.status().is_success() {
        anyhow::bail!("RPC returned {} for block {height}", resp.status());
    }
    Ok(resp.json().await?)
}
