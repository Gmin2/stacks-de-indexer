//! Clarity value decoder for consensus-serialized hex strings.
//!
//! Stacks event payloads include Clarity values in two forms: a parsed `value`
//! field (JSON) and a `raw_value` field (hex-encoded consensus bytes). This
//! module decodes `raw_value` hex strings into [`serde_json::Value`].
//!
//! # Clarity type tags
//!
//! Each serialized value starts with a one-byte type prefix:
//!
//! | Byte   | Type              | JSON output                         |
//! |--------|-------------------|-------------------------------------|
//! | `0x00` | `int` (i128)      | `{"type":"int","value":"42"}`       |
//! | `0x01` | `uint` (u128)     | `{"type":"uint","value":"1000"}`    |
//! | `0x02` | `buff`            | `{"type":"buff","value":"0xdead"}`  |
//! | `0x03` | `true`            | `true`                              |
//! | `0x04` | `false`           | `false`                             |
//! | `0x05` | standard principal| `"SP2C2YFP12AJZB1..."`              |
//! | `0x06` | contract principal| `"SP2C2YFP12AJZB1.contract-name"`   |
//! | `0x07` | `(ok ...)`        | `{"type":"ok","value":...}`         |
//! | `0x08` | `(err ...)`       | `{"type":"err","value":...}`        |
//! | `0x09` | `none`            | `null`                              |
//! | `0x0a` | `(some ...)`      | the inner value                     |
//! | `0x0b` | `(list ...)`      | `[...]`                             |
//! | `0x0c` | `{...}` (tuple)   | `{"key":...}`                       |
//! | `0x0d` | string-ascii      | `"hello"`                           |
//! | `0x0e` | string-utf8       | `"hello"`                           |
//!
//! Reference: `clarity-types/src/types/serialization.rs` in stacks-core.

mod decoder;

pub use decoder::decode_clarity_value;
pub use decoder::ClarityDecodeError;
