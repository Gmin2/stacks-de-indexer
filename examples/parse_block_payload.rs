//! Parse a stacks-core block payload and extract typed events.
//!
//! When stacks-core POSTs to `/new_block`, it sends a JSON payload containing
//! the block header, transactions, and a flat list of events. This example
//! shows how to deserialize that payload and classify each event by type.
//!
//! Run with:
//!   cargo run --example parse_block_payload

use stacks_indexer_core::types::{BlockPayload, RawEvent, StacksEvent};

fn main() {
    let block_json = serde_json::json!({
        "block_hash": "0x1a2b3c4d5e6f7890abcdef1234567890abcdef12",
        "block_height": 158392,
        "block_time": 1710000000u64,
        "burn_block_hash": "0x00000000000000000002a4c9b4e6f7890abcdef12",
        "burn_block_height": 832100,
        "index_block_hash": "0xidx_1a2b3c",
        "parent_block_hash": "0xparent_abc123",
        "parent_index_block_hash": "0xidx_parent",
        "transactions": [
            {
                "txid": "0xabc123def456",
                "tx_index": 0,
                "status": "success",
                "raw_tx": "0x00",
                "execution_cost": {
                    "read_count": 15,
                    "read_length": 2048,
                    "runtime": 5000,
                    "write_count": 3,
                    "write_length": 256
                }
            },
            {
                "txid": "0x789def012345",
                "tx_index": 1,
                "status": "abort_by_post_condition",
                "raw_tx": "0x00",
                "vm_error": "Post-condition check failed"
            }
        ],
        "events": [
            {
                "txid": "0xabc123def456",
                "event_index": 0,
                "committed": true,
                "type": "stx_transfer_event",
                "stx_transfer_event": {
                    "sender": "SP2J6ZY48GV1EZ5V2V5RB9MP66SW86PYKKNRV9EJ7",
                    "recipient": "SP1A2B3C4D5E6F7G8H9J0KLMNPQRST",
                    "amount": "5000000",
                    "memo": "0x"
                }
            },
            {
                "txid": "0xabc123def456",
                "event_index": 1,
                "committed": true,
                "type": "contract_event",
                "contract_event": {
                    "contract_identifier": "SP2C2YFP12AJZB1.arkadiko-vault",
                    "topic": "print",
                    "value": {
                        "event": "vault-created",
                        "vault-id": 42,
                        "owner": "SP2J6ZY48GV1EZ5V2V5RB9MP66SW86PYKKNRV9EJ7",
                        "collateral": 10000000
                    },
                    "raw_value": "0x0c00000004"
                }
            },
            {
                "txid": "0xabc123def456",
                "event_index": 2,
                "committed": true,
                "type": "ft_mint_event",
                "ft_mint_event": {
                    "asset_identifier": "SP2C2YFP12AJZB1.usda-token::usda",
                    "recipient": "SP2J6ZY48GV1EZ5V2V5RB9MP66SW86PYKKNRV9EJ7",
                    "amount": "1000000"
                }
            },
            {
                "txid": "0x789def012345",
                "event_index": 3,
                "committed": false,
                "type": "stx_transfer_event",
                "stx_transfer_event": {
                    "sender": "SP1INVALID",
                    "recipient": "SP2INVALID",
                    "amount": "999"
                }
            }
        ]
    });

    let block: BlockPayload = serde_json::from_value(block_json).unwrap();

    println!("Block #{} ({})", block.block_height, block.block_hash);
    println!("  burn block: #{}, txs: {}, events: {}",
        block.burn_block_height, block.transactions.len(), block.events.len());
    println!();

    for tx in &block.transactions {
        println!("tx [{}] {} - {}", tx.tx_index, tx.txid, tx.status);
        if let Some(err) = &tx.vm_error {
            println!("  vm error: {err}");
        }
    }
    println!();

    for raw_json in &block.events {
        let raw: RawEvent = serde_json::from_value(raw_json.clone()).unwrap();
        match raw.parse() {
            Ok(envelope) => {
                let status = if envelope.committed { "committed" } else { "rolled-back" };
                print!("[{}] {} ({}) ", envelope.event_index, envelope.event.type_name(), status);

                match &envelope.event {
                    StacksEvent::StxTransfer(e) => {
                        println!("{} -> {} ({} uSTX)", e.sender, e.recipient, e.amount);
                    }
                    StacksEvent::ContractEvent(e) => {
                        print!("contract: {}", e.contract_identifier);
                        if let Some(val) = &e.value {
                            println!(" data: {}", serde_json::to_string(val).unwrap());
                        } else {
                            println!();
                        }
                    }
                    StacksEvent::FtMint(e) => {
                        println!("{} minted {} to {}", e.asset_identifier, e.amount, e.recipient);
                    }
                    _ => println!(),
                }
            }
            Err(e) => println!("ERROR parsing event: {e}"),
        }
    }
}
