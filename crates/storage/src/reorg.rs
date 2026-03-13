//! Chain reorganization detection and rollback.
//!
//! A reorg is detected when a new block's `parent_index_block_hash` does not
//! match the `index_block_hash` we have stored for the current tip. Under
//! Nakamoto consensus, reorgs are typically 1-2 blocks deep.

use crate::Database;
use stacks_indexer_core::types::BlockPayload;

/// Check whether a new block represents a chain reorganization.
///
/// Returns `true` if the block's parent does not match our stored tip.
/// Returns `false` if this is the first block or the chain is continuous.
pub fn detect_reorg(db: &Database, new_block: &BlockPayload) -> anyhow::Result<bool> {
    match db.get_last_index_block_hash()? {
        None => Ok(false),
        Some(tip_hash) => {
            let is_reorg = new_block.parent_index_block_hash != tip_hash;
            if is_reorg {
                tracing::warn!(
                    "reorg detected at height {}: parent {} != tip {}",
                    new_block.block_height,
                    new_block.parent_index_block_hash,
                    tip_hash
                );
            }
            Ok(is_reorg)
        }
    }
}

/// Handle a reorg by rolling back to the fork point.
///
/// Returns the block height from which re-processing should start.
pub fn handle_reorg(db: &Database, new_block: &BlockPayload) -> anyhow::Result<u64> {
    let (current_height, _) = db.get_last_processed_block()?;
    let target = new_block.block_height;

    tracing::info!(
        "rolling back from height {} to {} ({} blocks)",
        current_height,
        target,
        current_height.saturating_sub(target) + 1
    );

    let undone = db.rollback_from_height(target)?;
    tracing::info!("rolled back {undone} journal entries");

    Ok(target)
}
