//! Event matching engine.
//!
//! Given an [`IndexerConfig`] and a [`BlockPayload`], the [`EventMatcher`]
//! filters the block's events down to only those that match configured
//! contracts and event types. Each match produces a [`MatchedEvent`] ready
//! for storage.

use crate::config::IndexerConfig;
use crate::types::{BlockPayload, EventEnvelope, RawEvent, StacksEvent};

/// An event that matched a configured source rule.
#[derive(Debug, Clone)]
pub struct MatchedEvent {
    /// Target SQLite table for this event.
    pub table: String,
    /// Configured event name.
    pub event_name: String,
    /// Contract identifier from the config rule that matched.
    pub contract: String,
    /// The parsed event with metadata.
    pub envelope: EventEnvelope,
    /// The original raw JSON (preserved for storage).
    pub raw_json: serde_json::Value,
}

/// Filters block events against configured sources.
///
/// Build once from config, then call [`match_events`](Self::match_events) for
/// every incoming block.
pub struct EventMatcher {
    rules: Vec<MatchRule>,
}

#[derive(Debug, Clone)]
struct MatchRule {
    contract: String,
    event_type: String,
    table: String,
    event_name: String,
}

impl EventMatcher {
    /// Create a matcher from the indexer configuration.
    pub fn from_config(config: &IndexerConfig) -> Self {
        let rules = config
            .sources
            .iter()
            .flat_map(|source| {
                source.events.iter().map(move |ev| MatchRule {
                    contract: source.contract.clone(),
                    event_type: ev.event_type.clone(),
                    table: ev.table.clone(),
                    event_name: ev.name.clone(),
                })
            })
            .collect();

        Self { rules }
    }

    /// Filter a block's events, returning only those matching configured rules.
    ///
    /// Uncommitted events (from aborted transactions) are always skipped.
    pub fn match_events(&self, block: &BlockPayload) -> Vec<MatchedEvent> {
        let mut matched = Vec::new();

        for raw_json in &block.events {
            let raw: RawEvent = match serde_json::from_value(raw_json.clone()) {
                Ok(e) => e,
                Err(err) => {
                    tracing::warn!("failed to parse raw event: {err}");
                    continue;
                }
            };

            let envelope = match raw.parse() {
                Ok(e) => e,
                Err(err) => {
                    tracing::warn!("failed to parse event type '{}': {err}", raw.event_type);
                    continue;
                }
            };

            if !envelope.committed {
                continue;
            }

            for rule in &self.rules {
                if self.matches_rule(rule, &envelope.event, &raw.event_type) {
                    matched.push(MatchedEvent {
                        table: rule.table.clone(),
                        event_name: rule.event_name.clone(),
                        contract: rule.contract.clone(),
                        envelope: envelope.clone(),
                        raw_json: raw_json.clone(),
                    });
                }
            }
        }

        matched
    }

    /// Check whether a single event matches a single rule.
    fn matches_rule(&self, rule: &MatchRule, event: &StacksEvent, raw_type: &str) -> bool {
        // Map config event type names to stacks-core's discriminator strings
        let expected = match rule.event_type.as_str() {
            "print_event" => "contract_event",
            "stx_transfer" => "stx_transfer_event",
            "stx_mint" => "stx_mint_event",
            "stx_burn" => "stx_burn_event",
            "stx_lock" => "stx_lock_event",
            "ft_transfer" => "ft_transfer_event",
            "ft_mint" => "ft_mint_event",
            "ft_burn" => "ft_burn_event",
            "nft_transfer" => "nft_transfer_event",
            "nft_mint" => "nft_mint_event",
            "nft_burn" => "nft_burn_event",
            _ => return false,
        };

        if raw_type != expected {
            return false;
        }

        // Wildcard contract matches everything
        if rule.contract == "*" {
            return true;
        }

        // Contract-specific matching
        match event {
            StacksEvent::ContractEvent(e) => e.contract_identifier == rule.contract,
            StacksEvent::FtTransfer(e) => e.asset_identifier.starts_with(&rule.contract),
            StacksEvent::FtMint(e) => e.asset_identifier.starts_with(&rule.contract),
            StacksEvent::FtBurn(e) => e.asset_identifier.starts_with(&rule.contract),
            StacksEvent::NftTransfer(e) => e.asset_identifier.starts_with(&rule.contract),
            StacksEvent::NftMint(e) => e.asset_identifier.starts_with(&rule.contract),
            StacksEvent::NftBurn(e) => e.asset_identifier.starts_with(&rule.contract),
            // STX native events don't have a contract — they match any rule with the right type
            StacksEvent::StxTransfer(_)
            | StacksEvent::StxMint(_)
            | StacksEvent::StxBurn(_)
            | StacksEvent::StxLock(_) => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;

    fn test_config() -> IndexerConfig {
        parse_config(
            r#"
name: test
network: devnet
server: { event_listener_port: 20445, api_port: 4000 }
storage: { path: ":memory:" }
sources:
  - contract: "SP1.vault"
    events:
      - { name: vault_created, type: print_event, table: vaults }
      - { name: stx_moves, type: stx_transfer, table: stx_transfers }
"#,
        )
        .unwrap()
    }

    fn empty_block(events: Vec<serde_json::Value>) -> BlockPayload {
        BlockPayload {
            block_hash: "0x1".into(),
            block_height: 100,
            block_time: Some(0),
            burn_block_hash: "0x2".into(),
            burn_block_height: 800,
            miner_txid: None,
            burn_block_time: None,
            index_block_hash: "0x3".into(),
            parent_block_hash: "0x0".into(),
            parent_index_block_hash: "0x0".into(),
            parent_microblock: None,
            parent_microblock_sequence: None,
            consensus_hash: None,
            tenure_height: None,
            transactions: vec![],
            events,
            parent_burn_block_hash: None,
            parent_burn_block_height: None,
            parent_burn_block_timestamp: None,
            anchored_cost: None,
            confirmed_microblocks_cost: None,
            signer_bitvec: None,
            reward_set: None,
            cycle_number: None,
            signer_signature_hash: None,
            miner_signature: None,
            signer_signature: vec![],
            matured_miner_rewards: vec![],
            pox_v1_unlock_height: None,
            pox_v2_unlock_height: None,
            pox_v3_unlock_height: None,
        }
    }

    #[test]
    fn matches_contract_event_and_stx_transfer() {
        let matcher = EventMatcher::from_config(&test_config());
        let block = empty_block(vec![
            // Should match: contract_event from SP1.vault
            serde_json::json!({
                "txid":"0xtx1","event_index":0,"committed":true,
                "type":"contract_event",
                "contract_event":{"contract_identifier":"SP1.vault","topic":"print","value":null}
            }),
            // Should NOT match: different contract
            serde_json::json!({
                "txid":"0xtx2","event_index":1,"committed":true,
                "type":"contract_event",
                "contract_event":{"contract_identifier":"SP2.other","topic":"print","value":null}
            }),
            // Should match: stx_transfer
            serde_json::json!({
                "txid":"0xtx3","event_index":2,"committed":true,
                "type":"stx_transfer_event",
                "stx_transfer_event":{"sender":"SP1A","recipient":"SP1B","amount":"1000"}
            }),
            // Should NOT match: uncommitted
            serde_json::json!({
                "txid":"0xtx4","event_index":3,"committed":false,
                "type":"stx_transfer_event",
                "stx_transfer_event":{"sender":"S","recipient":"R","amount":"1"}
            }),
        ]);

        let matched = matcher.match_events(&block);
        assert_eq!(matched.len(), 2);
        assert_eq!(matched[0].table, "vaults");
        assert_eq!(matched[1].table, "stx_transfers");
    }
}
