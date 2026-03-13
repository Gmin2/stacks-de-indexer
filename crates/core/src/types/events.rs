//! Strongly-typed Stacks event parsing.
//!
//! Each block payload from stacks-core contains a flat `events` array. Every
//! element has a `type` discriminator and a nested object keyed by that same
//! type name. This module provides [`RawEvent`] for initial deserialization
//! and [`StacksEvent`] as the parsed, strongly-typed enum.
//!
//! # Supported event types
//!
//! | Discriminator            | Variant                          |
//! |--------------------------|----------------------------------|
//! | `stx_transfer_event`     | [`StacksEvent::StxTransfer`]     |
//! | `stx_mint_event`         | [`StacksEvent::StxMint`]         |
//! | `stx_burn_event`         | [`StacksEvent::StxBurn`]         |
//! | `stx_lock_event`         | [`StacksEvent::StxLock`]         |
//! | `ft_transfer_event`      | [`StacksEvent::FtTransfer`]      |
//! | `ft_mint_event`          | [`StacksEvent::FtMint`]          |
//! | `ft_burn_event`          | [`StacksEvent::FtBurn`]          |
//! | `nft_transfer_event`     | [`StacksEvent::NftTransfer`]     |
//! | `nft_mint_event`         | [`StacksEvent::NftMint`]         |
//! | `nft_burn_event`         | [`StacksEvent::NftBurn`]         |
//! | `contract_event`         | [`StacksEvent::ContractEvent`]   |

use serde::{Deserialize, Serialize};

// Raw event (from JSON)

/// A raw event as it appears in the block payload's `events` array.
///
/// The `type` field determines which nested key holds the actual data. Use
/// [`RawEvent::parse`] to convert into a strongly-typed [`EventEnvelope`].
#[derive(Debug, Clone, Deserialize)]
pub struct RawEvent {
    /// Transaction that emitted this event.
    pub txid: String,
    /// Position of this event within the block's event list.
    pub event_index: u64,
    /// Whether the emitting transaction was committed (not rolled back).
    pub committed: bool,
    /// Event type discriminator (e.g. `"stx_transfer_event"`).
    #[serde(rename = "type")]
    pub event_type: String,
    /// Remaining fields — the nested event data lives under a key matching
    /// [`event_type`](Self::event_type).
    #[serde(flatten)]
    pub data: serde_json::Map<String, serde_json::Value>,
}

// Parsed event enum

/// A strongly-typed Stacks event, one of 11 possible variants.
#[derive(Debug, Clone)]
pub enum StacksEvent {
    /// Native STX token transfer.
    StxTransfer(StxTransferEvent),
    /// Native STX token mint (e.g. coinbase, unlock).
    StxMint(StxMintEvent),
    /// Native STX token burn.
    StxBurn(StxBurnEvent),
    /// STX locked for stacking.
    StxLock(StxLockEvent),
    /// Fungible token (SIP-010) transfer.
    FtTransfer(FtTransferEvent),
    /// Fungible token mint.
    FtMint(FtMintEvent),
    /// Fungible token burn.
    FtBurn(FtBurnEvent),
    /// Non-fungible token (SIP-009) transfer.
    NftTransfer(NftTransferEvent),
    /// Non-fungible token mint.
    NftMint(NftMintEvent),
    /// Non-fungible token burn.
    NftBurn(NftBurnEvent),
    /// Custom contract event — emitted by Clarity `(print ...)` expressions.
    ContractEvent(ContractEventData),
}

/// Metadata wrapper around a parsed [`StacksEvent`].
#[derive(Debug, Clone)]
pub struct EventEnvelope {
    /// Transaction that emitted this event.
    pub txid: String,
    /// Position within the block's event list.
    pub event_index: u64,
    /// Whether the transaction was committed.
    pub committed: bool,
    /// The parsed event payload.
    pub event: StacksEvent,
}

// Individual event structs

/// Native STX transfer between two principals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StxTransferEvent {
    /// Sender principal (c32-encoded address).
    pub sender: String,
    /// Recipient principal.
    pub recipient: String,
    /// Amount in micro-STX (string to preserve u128 precision).
    pub amount: String,
    /// Optional memo (hex-encoded).
    #[serde(default)]
    pub memo: Option<String>,
}

/// STX minted (coinbase, PoX unlock, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StxMintEvent {
    /// Recipient of the minted STX.
    pub recipient: String,
    /// Amount in micro-STX.
    pub amount: String,
}

/// STX burned.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StxBurnEvent {
    /// Address that burned the STX.
    pub sender: String,
    /// Amount in micro-STX.
    pub amount: String,
}

/// STX locked for stacking via PoX.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StxLockEvent {
    /// Amount locked in micro-STX.
    pub locked_amount: String,
    /// Block height at which the STX unlock.
    pub unlock_height: String,
    /// Address whose STX are locked.
    pub locked_address: String,
    /// The PoX contract that performed the lock.
    #[serde(default)]
    pub contract_identifier: Option<String>,
}

/// SIP-010 fungible token transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FtTransferEvent {
    /// Fully qualified asset identifier (e.g. `SP1.token::my-ft`).
    pub asset_identifier: String,
    /// Sender principal.
    pub sender: String,
    /// Recipient principal.
    pub recipient: String,
    /// Amount transferred (string to preserve precision).
    pub amount: String,
}

/// SIP-010 fungible token mint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FtMintEvent {
    /// Fully qualified asset identifier.
    pub asset_identifier: String,
    /// Recipient of the minted tokens.
    pub recipient: String,
    /// Amount minted.
    pub amount: String,
}

/// SIP-010 fungible token burn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FtBurnEvent {
    /// Fully qualified asset identifier.
    pub asset_identifier: String,
    /// Address that burned the tokens.
    pub sender: String,
    /// Amount burned.
    pub amount: String,
}

/// SIP-009 non-fungible token transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftTransferEvent {
    /// Fully qualified asset identifier (e.g. `SP1.nft::my-nft`).
    pub asset_identifier: String,
    /// Previous owner.
    pub sender: String,
    /// New owner.
    pub recipient: String,
    /// Token identifier as parsed Clarity JSON.
    pub value: Option<serde_json::Value>,
    /// Token identifier as hex-encoded consensus bytes.
    pub raw_value: Option<String>,
}

/// SIP-009 non-fungible token mint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftMintEvent {
    /// Fully qualified asset identifier.
    pub asset_identifier: String,
    /// Recipient of the minted NFT.
    pub recipient: String,
    /// Token identifier as parsed Clarity JSON.
    pub value: Option<serde_json::Value>,
    /// Token identifier as hex-encoded consensus bytes.
    pub raw_value: Option<String>,
}

/// SIP-009 non-fungible token burn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftBurnEvent {
    /// Fully qualified asset identifier.
    pub asset_identifier: String,
    /// Address that burned the NFT.
    pub sender: String,
    /// Token identifier as parsed Clarity JSON.
    pub value: Option<serde_json::Value>,
    /// Token identifier as hex-encoded consensus bytes.
    pub raw_value: Option<String>,
}

/// Custom contract event emitted by a Clarity `(print ...)` expression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractEventData {
    /// Fully qualified contract identifier (e.g. `SP1.my-contract`).
    pub contract_identifier: String,
    /// Topic string (typically `"print"`).
    pub topic: String,
    /// The printed value as parsed Clarity JSON.
    pub value: Option<serde_json::Value>,
    /// The printed value as hex-encoded consensus bytes.
    pub raw_value: Option<String>,
}

// Parsing

impl RawEvent {
    /// Parse into a strongly-typed [`EventEnvelope`].
    ///
    /// # Errors
    ///
    /// Returns `Err` if the `type` field is unknown or the nested payload
    /// cannot be deserialized into the expected struct.
    pub fn parse(&self) -> Result<EventEnvelope, String> {
        let inner_data = self
            .data
            .get(&self.event_type)
            .ok_or_else(|| format!("missing nested key '{}' in event", self.event_type))?;

        let event = match self.event_type.as_str() {
            "stx_transfer_event" => StacksEvent::StxTransfer(deser(inner_data)?),
            "stx_mint_event" => StacksEvent::StxMint(deser(inner_data)?),
            "stx_burn_event" => StacksEvent::StxBurn(deser(inner_data)?),
            "stx_lock_event" => StacksEvent::StxLock(deser(inner_data)?),
            "ft_transfer_event" => StacksEvent::FtTransfer(deser(inner_data)?),
            "ft_mint_event" => StacksEvent::FtMint(deser(inner_data)?),
            "ft_burn_event" => StacksEvent::FtBurn(deser(inner_data)?),
            "nft_transfer_event" => StacksEvent::NftTransfer(deser(inner_data)?),
            "nft_mint_event" => StacksEvent::NftMint(deser(inner_data)?),
            "nft_burn_event" => StacksEvent::NftBurn(deser(inner_data)?),
            "contract_event" => StacksEvent::ContractEvent(deser(inner_data)?),
            other => return Err(format!("unknown event type: {other}")),
        };

        Ok(EventEnvelope {
            txid: self.txid.clone(),
            event_index: self.event_index,
            committed: self.committed,
            event,
        })
    }
}

/// Deserialize a `serde_json::Value` into `T`, mapping errors to `String`.
fn deser<T: serde::de::DeserializeOwned>(val: &serde_json::Value) -> Result<T, String> {
    serde_json::from_value(val.clone()).map_err(|e| e.to_string())
}

// StacksEvent helpers

impl StacksEvent {
    /// Short name for this event variant (e.g. `"stx_transfer"`).
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::StxTransfer(_) => "stx_transfer",
            Self::StxMint(_) => "stx_mint",
            Self::StxBurn(_) => "stx_burn",
            Self::StxLock(_) => "stx_lock",
            Self::FtTransfer(_) => "ft_transfer",
            Self::FtMint(_) => "ft_mint",
            Self::FtBurn(_) => "ft_burn",
            Self::NftTransfer(_) => "nft_transfer",
            Self::NftMint(_) => "nft_mint",
            Self::NftBurn(_) => "nft_burn",
            Self::ContractEvent(_) => "contract_event",
        }
    }

    /// Serialize the event payload to a [`serde_json::Value`] for storage.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::StxTransfer(e) => serde_json::to_value(e).unwrap(),
            Self::StxMint(e) => serde_json::to_value(e).unwrap(),
            Self::StxBurn(e) => serde_json::to_value(e).unwrap(),
            Self::StxLock(e) => serde_json::to_value(e).unwrap(),
            Self::FtTransfer(e) => serde_json::to_value(e).unwrap(),
            Self::FtMint(e) => serde_json::to_value(e).unwrap(),
            Self::FtBurn(e) => serde_json::to_value(e).unwrap(),
            Self::NftTransfer(e) => serde_json::to_value(e).unwrap(),
            Self::NftMint(e) => serde_json::to_value(e).unwrap(),
            Self::NftBurn(e) => serde_json::to_value(e).unwrap(),
            Self::ContractEvent(e) => serde_json::to_value(e).unwrap(),
        }
    }

    /// Returns the contract identifier associated with this event, if any.
    pub fn contract_identifier(&self) -> Option<&str> {
        match self {
            Self::ContractEvent(e) => Some(&e.contract_identifier),
            Self::StxLock(e) => e.contract_identifier.as_deref(),
            Self::FtTransfer(e) => asset_contract(&e.asset_identifier),
            Self::FtMint(e) => asset_contract(&e.asset_identifier),
            Self::FtBurn(e) => asset_contract(&e.asset_identifier),
            Self::NftTransfer(e) => asset_contract(&e.asset_identifier),
            Self::NftMint(e) => asset_contract(&e.asset_identifier),
            Self::NftBurn(e) => asset_contract(&e.asset_identifier),
            _ => None,
        }
    }
}

/// Extract the contract portion from a fully qualified asset identifier
/// like `SP1.contract::asset-name`.
fn asset_contract(asset_id: &str) -> Option<&str> {
    asset_id.split("::").next()
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;

    fn make_raw(event_type: &str, data: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "txid": "0xtx1",
            "event_index": 0,
            "committed": true,
            "type": event_type,
            event_type: data,
        })
    }

    #[test]
    fn parse_stx_transfer() {
        let json = make_raw("stx_transfer_event", serde_json::json!({
            "sender": "SP1A", "recipient": "SP1B", "amount": "1000000", "memo": "0x"
        }));
        let raw: RawEvent = serde_json::from_value(json).unwrap();
        let env = raw.parse().unwrap();
        assert!(matches!(env.event, StacksEvent::StxTransfer(_)));
        assert_eq!(env.event.type_name(), "stx_transfer");
    }

    #[test]
    fn parse_contract_event() {
        let json = make_raw("contract_event", serde_json::json!({
            "contract_identifier": "SP1.vault",
            "topic": "print",
            "value": {"vault-id": 1}
        }));
        let raw: RawEvent = serde_json::from_value(json).unwrap();
        let env = raw.parse().unwrap();
        assert_eq!(env.event.contract_identifier(), Some("SP1.vault"));
    }

    #[test]
    fn parse_all_eleven_event_types() {
        let cases = vec![
            ("stx_transfer_event", serde_json::json!({"sender":"S","recipient":"R","amount":"1"})),
            ("stx_mint_event", serde_json::json!({"recipient":"R","amount":"1"})),
            ("stx_burn_event", serde_json::json!({"sender":"S","amount":"1"})),
            ("stx_lock_event", serde_json::json!({"locked_amount":"1","unlock_height":"5","locked_address":"S"})),
            ("ft_transfer_event", serde_json::json!({"asset_identifier":"SP1.t::t","sender":"S","recipient":"R","amount":"1"})),
            ("ft_mint_event", serde_json::json!({"asset_identifier":"SP1.t::t","recipient":"R","amount":"1"})),
            ("ft_burn_event", serde_json::json!({"asset_identifier":"SP1.t::t","sender":"S","amount":"1"})),
            ("nft_transfer_event", serde_json::json!({"asset_identifier":"SP1.n::n","sender":"S","recipient":"R"})),
            ("nft_mint_event", serde_json::json!({"asset_identifier":"SP1.n::n","recipient":"R"})),
            ("nft_burn_event", serde_json::json!({"asset_identifier":"SP1.n::n","sender":"S"})),
            ("contract_event", serde_json::json!({"contract_identifier":"SP1.c","topic":"print"})),
        ];
        for (event_type, data) in cases {
            let json = make_raw(event_type, data);
            let raw: RawEvent = serde_json::from_value(json).unwrap();
            assert!(raw.parse().is_ok(), "failed to parse {event_type}");
        }
    }
}
