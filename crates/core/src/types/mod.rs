//! Stacks blockchain payload types.
//!
//! These structs mirror the JSON payloads that `stacks-core` HTTP POSTs to event
//! observers. The canonical source is `stacks-node/src/event_dispatcher/payloads.rs`
//! in the stacks-core repository.
//!
//! # Endpoints
//!
//! | Endpoint              | Rust type                |
//! |-----------------------|--------------------------|
//! | `POST /new_block`     | [`BlockPayload`]         |
//! | `POST /new_burn_block`| [`BurnBlockPayload`]     |
//! | `POST /new_mempool_tx`| [`MempoolTxPayload`]     |
//! | `POST /drop_mempool_tx`| [`DroppedTxPayload`]    |
//! | `POST /new_microblocks`| [`MicroblocksPayload`]  |

mod events;

pub use events::*;

use serde::Deserialize;

// Block payload

/// Full block payload from `POST /new_block`.
///
/// This is the primary data structure received from stacks-core after a new
/// Stacks block is processed. It contains the block header fields, a list of
/// transactions, and a flat list of events emitted by those transactions.
#[derive(Debug, Clone, Deserialize)]
pub struct BlockPayload {
    /// The Stacks block hash (hex with `0x` prefix).
    pub block_hash: String,
    /// The Stacks block height.
    pub block_height: u64,
    /// Unix timestamp of the block (may be `None` for very old blocks).
    pub block_time: Option<u64>,
    /// The Bitcoin block hash this Stacks block is anchored to.
    pub burn_block_hash: String,
    /// The Bitcoin block height this Stacks block is anchored to.
    pub burn_block_height: u64,
    /// Transaction ID of the miner's commit on the burn chain.
    pub miner_txid: Option<String>,
    /// Unix timestamp of the Bitcoin anchor block.
    pub burn_block_time: Option<u64>,
    /// Index block hash — unique identifier across forks.
    pub index_block_hash: String,
    /// Parent Stacks block hash.
    pub parent_block_hash: String,
    /// Parent index block hash — used for reorg detection.
    pub parent_index_block_hash: String,
    /// Parent microblock hash (pre-Nakamoto).
    #[serde(default)]
    pub parent_microblock: Option<String>,
    /// Parent microblock sequence number (pre-Nakamoto).
    #[serde(default)]
    pub parent_microblock_sequence: Option<u16>,
    /// Consensus hash for this block.
    pub consensus_hash: Option<String>,
    /// Tenure height (Nakamoto blocks).
    pub tenure_height: Option<u64>,
    /// Transactions included in this block.
    #[serde(default)]
    pub transactions: Vec<TransactionPayload>,
    /// Raw event objects — parsed further by [`events::RawEvent`].
    #[serde(default)]
    pub events: Vec<serde_json::Value>,
    /// Parent burn block hash (may differ from `burn_block_hash`).
    #[serde(default)]
    pub parent_burn_block_hash: Option<String>,
    /// Parent burn block height.
    #[serde(default)]
    pub parent_burn_block_height: Option<u64>,
    /// Parent burn block timestamp.
    #[serde(default)]
    pub parent_burn_block_timestamp: Option<u64>,
    /// Execution cost of the anchored block.
    pub anchored_cost: Option<ExecutionCost>,
    /// Execution cost of confirmed microblocks.
    pub confirmed_microblocks_cost: Option<ExecutionCost>,
    /// Signer bitvector (Nakamoto).
    pub signer_bitvec: Option<serde_json::Value>,
    /// Reward set snapshot (Nakamoto).
    pub reward_set: Option<serde_json::Value>,
    /// PoX cycle number.
    pub cycle_number: Option<u64>,
    /// Signer signature hash (Nakamoto).
    pub signer_signature_hash: Option<String>,
    /// Miner's signature (Nakamoto).
    pub miner_signature: Option<String>,
    /// Signer signatures (Nakamoto).
    #[serde(default)]
    pub signer_signature: Vec<serde_json::Value>,
    /// Matured miner rewards from earlier blocks.
    #[serde(default)]
    pub matured_miner_rewards: Vec<serde_json::Value>,
    /// PoX v1 unlock height.
    pub pox_v1_unlock_height: Option<u64>,
    /// PoX v2 unlock height.
    pub pox_v2_unlock_height: Option<u64>,
    /// PoX v3 unlock height.
    pub pox_v3_unlock_height: Option<u64>,
}

// Transaction payload

/// A transaction within a [`BlockPayload`].
#[derive(Debug, Clone, Deserialize)]
pub struct TransactionPayload {
    /// Transaction ID (hex with `0x` prefix).
    pub txid: String,
    /// Index of this transaction within the block.
    pub tx_index: u32,
    /// Execution status: `"success"`, `"abort_by_response"`, or `"abort_by_post_condition"`.
    pub status: String,
    /// The Clarity return value (serialized as JSON).
    pub raw_result: Option<serde_json::Value>,
    /// Hex-encoded serialized transaction.
    pub raw_tx: String,
    /// ABI of a newly deployed contract, if applicable.
    pub contract_interface: Option<serde_json::Value>,
    /// Burnchain operation data, if this transaction triggered one.
    pub burnchain_op: Option<serde_json::Value>,
    /// Resources consumed by this transaction.
    pub execution_cost: Option<ExecutionCost>,
    /// Microblock sequence (pre-Nakamoto).
    pub microblock_sequence: Option<u16>,
    /// Microblock hash (pre-Nakamoto).
    pub microblock_hash: Option<String>,
    /// Microblock parent hash (pre-Nakamoto).
    pub microblock_parent_hash: Option<String>,
    /// Human-readable VM error message, if the transaction failed.
    pub vm_error: Option<String>,
}

// Execution cost

/// Resource usage for a transaction or block.
#[derive(Debug, Clone, Deserialize)]
pub struct ExecutionCost {
    /// Number of reads performed.
    pub read_count: u64,
    /// Total bytes read.
    pub read_length: u64,
    /// Runtime cost units consumed.
    pub runtime: u64,
    /// Number of writes performed.
    pub write_count: u64,
    /// Total bytes written.
    pub write_length: u64,
}

// Burn block payload

/// Burn block payload from `POST /new_burn_block`.
///
/// Received when a new Bitcoin block is seen that anchors Stacks activity.
#[derive(Debug, Clone, Deserialize)]
pub struct BurnBlockPayload {
    /// Bitcoin block hash.
    pub burn_block_hash: String,
    /// Bitcoin block height.
    pub burn_block_height: u64,
    /// PoX reward recipients for this burn block.
    #[serde(default)]
    pub reward_recipients: Vec<RewardRecipient>,
    /// Addresses holding reward slots.
    #[serde(default)]
    pub reward_slot_holders: Vec<String>,
    /// Total BTC burned in this block.
    pub burn_amount: Option<u64>,
    /// Consensus hash.
    pub consensus_hash: Option<String>,
    /// Parent burn block hash.
    pub parent_burn_block_hash: Option<String>,
}

/// A PoX reward recipient in a [`BurnBlockPayload`].
#[derive(Debug, Clone, Deserialize)]
pub struct RewardRecipient {
    /// The recipient address (base58-encoded PoX address).
    pub recipient: String,
    /// Amount of BTC rewarded (in satoshis).
    pub amt: u64,
}

// Mempool payloads

/// Mempool transaction payload from `POST /new_mempool_tx`.
///
/// An array of hex-encoded raw transactions entering the mempool.
pub type MempoolTxPayload = Vec<String>;

/// Dropped transaction payload from `POST /drop_mempool_tx`.
#[derive(Debug, Clone, Deserialize)]
pub struct DroppedTxPayload {
    /// Transaction IDs that were dropped.
    pub dropped_txids: Vec<String>,
    /// Reason for dropping: `"ReplaceByFee"`, `"TooExpensive"`, `"StaleGarbageCollect"`.
    pub reason: String,
    /// If a replacement caused the drop, the new transaction ID.
    pub new_txid: Option<String>,
}

/// Microblock payload from `POST /new_microblocks` (pre-Nakamoto).
#[derive(Debug, Clone, Deserialize)]
pub struct MicroblocksPayload {
    /// Parent index block hash.
    pub parent_index_block_hash: String,
    /// Events from microblock transactions.
    #[serde(default)]
    pub events: Vec<serde_json::Value>,
    /// Transactions in these microblocks.
    #[serde(default)]
    pub transactions: Vec<TransactionPayload>,
    /// Burn block hash.
    pub burn_block_hash: Option<String>,
    /// Burn block height.
    pub burn_block_height: Option<u64>,
    /// Burn block timestamp.
    pub burn_block_timestamp: Option<u64>,
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_block_payload() {
        let json = serde_json::json!({
            "block_hash": "0xabc123",
            "block_height": 100,
            "block_time": 1_700_000_000u64,
            "burn_block_hash": "0xdef456",
            "burn_block_height": 800_000,
            "index_block_hash": "0xidx789",
            "parent_block_hash": "0xparent1",
            "parent_index_block_hash": "0xpidx1",
            "tenure_height": 50,
            "transactions": [{
                "txid": "0xtx1",
                "tx_index": 0,
                "status": "success",
                "raw_tx": "0x00",
                "raw_result": "0x0703",
                "execution_cost": {
                    "read_count": 10,
                    "read_length": 100,
                    "runtime": 1000,
                    "write_count": 5,
                    "write_length": 50
                }
            }],
            "events": [{
                "txid": "0xtx1",
                "event_index": 0,
                "committed": true,
                "type": "stx_transfer_event",
                "stx_transfer_event": {
                    "sender": "SP1A",
                    "recipient": "SP1B",
                    "amount": "1000000",
                    "memo": "0x"
                }
            }]
        });

        let block: BlockPayload = serde_json::from_value(json).unwrap();
        assert_eq!(block.block_height, 100);
        assert_eq!(block.transactions.len(), 1);
        assert_eq!(block.transactions[0].status, "success");
        assert_eq!(block.events.len(), 1);
    }

    #[test]
    fn deserialize_burn_block() {
        let json = serde_json::json!({
            "burn_block_hash": "0xburn1",
            "burn_block_height": 800_001,
            "reward_recipients": [{"recipient": "addr1", "amt": 5000}],
            "reward_slot_holders": ["addr1"],
            "burn_amount": 20_000
        });
        let bb: BurnBlockPayload = serde_json::from_value(json).unwrap();
        assert_eq!(bb.burn_block_height, 800_001);
        assert_eq!(bb.reward_recipients.len(), 1);
    }

    #[test]
    fn deserialize_dropped_tx() {
        let json = serde_json::json!({
            "dropped_txids": ["0xtx1", "0xtx2"],
            "reason": "ReplaceByFee",
            "new_txid": "0xtx3"
        });
        let d: DroppedTxPayload = serde_json::from_value(json).unwrap();
        assert_eq!(d.dropped_txids.len(), 2);
        assert_eq!(d.reason, "ReplaceByFee");
    }
}
