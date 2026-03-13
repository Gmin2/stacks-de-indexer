//! Byte-level Clarity value decoder.

use sha2::{Digest, Sha256};
use thiserror::Error;

/// Errors that can occur when decoding a consensus-serialized Clarity value.
#[derive(Debug, Error)]
pub enum ClarityDecodeError {
    /// The input ended before the value was fully read.
    #[error("unexpected end of input at byte offset {0}")]
    UnexpectedEof(usize),
    /// An unrecognized type prefix byte was encountered.
    #[error("unknown type prefix 0x{0:02x} at byte offset {1}")]
    UnknownTypePrefix(u8, usize),
    /// The hex string could not be parsed.
    #[error("invalid hex string: {0}")]
    InvalidHex(String),
    /// A string value contained invalid UTF-8.
    #[error("invalid UTF-8 in Clarity string value")]
    InvalidUtf8,
}

/// Decode a Clarity consensus-serialized value from a hex string into JSON.
///
/// Accepts hex strings with or without the `0x` prefix.
///
/// # Examples
///
/// ```
/// use stacks_indexer_core::clarity::decode_clarity_value;
///
/// // Decode a boolean `true` (type tag 0x03)
/// let val = decode_clarity_value("03").unwrap();
/// assert_eq!(val, serde_json::Value::Bool(true));
///
/// // Decode with 0x prefix
/// let val = decode_clarity_value("0x03").unwrap();
/// assert_eq!(val, serde_json::Value::Bool(true));
/// ```
///
/// # Errors
///
/// Returns [`ClarityDecodeError`] if the hex is malformed or the bytes do not
/// represent a valid Clarity value.
pub fn decode_clarity_value(hex_str: &str) -> Result<serde_json::Value, ClarityDecodeError> {
    let hex_str = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    let bytes = hex::decode(hex_str).map_err(|e| ClarityDecodeError::InvalidHex(e.to_string()))?;
    let mut cursor = 0;
    decode_value(&bytes, &mut cursor)
}

// Internal helpers

fn read_byte(data: &[u8], cursor: &mut usize) -> Result<u8, ClarityDecodeError> {
    if *cursor >= data.len() {
        return Err(ClarityDecodeError::UnexpectedEof(*cursor));
    }
    let b = data[*cursor];
    *cursor += 1;
    Ok(b)
}

fn read_bytes<'a>(
    data: &'a [u8],
    cursor: &mut usize,
    n: usize,
) -> Result<&'a [u8], ClarityDecodeError> {
    if *cursor + n > data.len() {
        return Err(ClarityDecodeError::UnexpectedEof(*cursor));
    }
    let slice = &data[*cursor..*cursor + n];
    *cursor += n;
    Ok(slice)
}

fn read_u32_be(data: &[u8], cursor: &mut usize) -> Result<u32, ClarityDecodeError> {
    let bytes = read_bytes(data, cursor, 4)?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_i128_be(data: &[u8], cursor: &mut usize) -> Result<i128, ClarityDecodeError> {
    let bytes = read_bytes(data, cursor, 16)?;
    let mut arr = [0u8; 16];
    arr.copy_from_slice(bytes);
    Ok(i128::from_be_bytes(arr))
}

fn read_u128_be(data: &[u8], cursor: &mut usize) -> Result<u128, ClarityDecodeError> {
    let bytes = read_bytes(data, cursor, 16)?;
    let mut arr = [0u8; 16];
    arr.copy_from_slice(bytes);
    Ok(u128::from_be_bytes(arr))
}

// Recursive value decoder

fn decode_value(data: &[u8], cursor: &mut usize) -> Result<serde_json::Value, ClarityDecodeError> {
    let offset = *cursor;
    let tag = read_byte(data, cursor)?;

    match tag {
        // int (i128)
        0x00 => {
            let v = read_i128_be(data, cursor)?;
            Ok(serde_json::json!({"type": "int", "value": v.to_string()}))
        }
        // uint (u128)
        0x01 => {
            let v = read_u128_be(data, cursor)?;
            Ok(serde_json::json!({"type": "uint", "value": v.to_string()}))
        }
        // buffer
        0x02 => {
            let len = read_u32_be(data, cursor)? as usize;
            let bytes = read_bytes(data, cursor, len)?;
            Ok(serde_json::json!({"type": "buff", "value": format!("0x{}", hex::encode(bytes))}))
        }
        // bool true
        0x03 => Ok(serde_json::Value::Bool(true)),
        // bool false
        0x04 => Ok(serde_json::Value::Bool(false)),
        // standard principal
        0x05 => {
            let version = read_byte(data, cursor)?;
            let hash = read_bytes(data, cursor, 20)?;
            Ok(serde_json::Value::String(encode_stacks_address(
                version, hash,
            )))
        }
        // contract principal
        0x06 => {
            let version = read_byte(data, cursor)?;
            let hash = read_bytes(data, cursor, 20)?;
            let addr = encode_stacks_address(version, hash);
            let name_len = read_byte(data, cursor)? as usize;
            let name_bytes = read_bytes(data, cursor, name_len)?;
            let name =
                std::str::from_utf8(name_bytes).map_err(|_| ClarityDecodeError::InvalidUtf8)?;
            Ok(serde_json::Value::String(format!("{addr}.{name}")))
        }
        // response ok
        0x07 => {
            let inner = decode_value(data, cursor)?;
            Ok(serde_json::json!({"type": "ok", "value": inner}))
        }
        // response err
        0x08 => {
            let inner = decode_value(data, cursor)?;
            Ok(serde_json::json!({"type": "err", "value": inner}))
        }
        // optional none
        0x09 => Ok(serde_json::Value::Null),
        // optional some — unwrap to inner value
        0x0a => decode_value(data, cursor),
        // list
        0x0b => {
            let len = read_u32_be(data, cursor)? as usize;
            let mut items = Vec::with_capacity(len);
            for _ in 0..len {
                items.push(decode_value(data, cursor)?);
            }
            Ok(serde_json::Value::Array(items))
        }
        // tuple
        0x0c => {
            let len = read_u32_be(data, cursor)? as usize;
            let mut map = serde_json::Map::with_capacity(len);
            for _ in 0..len {
                let name_len = read_byte(data, cursor)? as usize;
                let name_bytes = read_bytes(data, cursor, name_len)?;
                let name = std::str::from_utf8(name_bytes)
                    .map_err(|_| ClarityDecodeError::InvalidUtf8)?
                    .to_string();
                let value = decode_value(data, cursor)?;
                map.insert(name, value);
            }
            Ok(serde_json::Value::Object(map))
        }
        // string-ascii
        0x0d => {
            let len = read_u32_be(data, cursor)? as usize;
            let bytes = read_bytes(data, cursor, len)?;
            let s = std::str::from_utf8(bytes).map_err(|_| ClarityDecodeError::InvalidUtf8)?;
            Ok(serde_json::Value::String(s.to_string()))
        }
        // string-utf8
        0x0e => {
            let byte_len = read_u32_be(data, cursor)? as usize;
            let bytes = read_bytes(data, cursor, byte_len)?;
            let s = std::str::from_utf8(bytes).map_err(|_| ClarityDecodeError::InvalidUtf8)?;
            Ok(serde_json::Value::String(s.to_string()))
        }
        _ => Err(ClarityDecodeError::UnknownTypePrefix(tag, offset)),
    }
}

// c32check address encoding

const C32_ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Encode a Stacks address from a version byte and 20-byte hash160.
///
/// Uses c32check encoding: `S` + version-char + c32(hash160 + checksum).
fn encode_stacks_address(version: u8, hash160: &[u8]) -> String {
    let c32_version = C32_ALPHABET[version as usize % 32] as char;

    let mut check_data = vec![version];
    check_data.extend_from_slice(hash160);
    let checksum = double_sha256_checksum(&check_data);

    let mut payload = Vec::with_capacity(24);
    payload.extend_from_slice(hash160);
    payload.extend_from_slice(&checksum[0..4]);

    format!("S{}{}", c32_version, c32_encode(&payload))
}

fn c32_encode(data: &[u8]) -> String {
    if data.is_empty() {
        return String::new();
    }

    let mut result = Vec::new();
    let mut num = data.to_vec();

    while !is_zero(&num) {
        let (quotient, remainder) = divmod32(&num);
        result.push(C32_ALPHABET[remainder as usize] as char);
        num = quotient;
    }

    // Preserve leading zeros
    for &byte in data.iter() {
        if byte == 0 {
            result.push(C32_ALPHABET[0] as char);
        } else {
            break;
        }
    }

    result.reverse();
    result.into_iter().collect()
}

fn is_zero(data: &[u8]) -> bool {
    data.iter().all(|&b| b == 0)
}

fn divmod32(data: &[u8]) -> (Vec<u8>, u8) {
    let mut result = Vec::with_capacity(data.len());
    let mut remainder: u16 = 0;

    for &byte in data {
        let acc = (remainder << 8) | byte as u16;
        result.push((acc / 32) as u8);
        remainder = acc % 32;
    }

    // Strip leading zeros from quotient
    while result.first() == Some(&0) {
        result.remove(0);
    }

    (result, remainder as u8)
}

/// Compute a double-SHA-256 checksum: `SHA-256(SHA-256(data))`.
///
/// The first 4 bytes of the result are appended to the address payload
/// before c32 encoding, matching the real c32check specification.
fn double_sha256_checksum(data: &[u8]) -> [u8; 32] {
    let first = Sha256::digest(data);
    let second = Sha256::digest(first);
    second.into()
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_int() {
        // i128(42): type 0x00 + 16 big-endian bytes
        let val = decode_clarity_value("000000000000000000000000000000002a").unwrap();
        assert_eq!(val, serde_json::json!({"type": "int", "value": "42"}));
    }

    #[test]
    fn decode_negative_int() {
        // i128(-1): all 0xFF
        let val = decode_clarity_value("00ffffffffffffffffffffffffffffffff").unwrap();
        assert_eq!(val, serde_json::json!({"type": "int", "value": "-1"}));
    }

    #[test]
    fn decode_uint() {
        // u128(1000) = 0x3E8 in 16 bytes
        let val = decode_clarity_value("01000000000000000000000000000003e8").unwrap();
        assert_eq!(val, serde_json::json!({"type": "uint", "value": "1000"}));
    }

    #[test]
    fn decode_buffer() {
        let val = decode_clarity_value("0200000004deadbeef").unwrap();
        assert_eq!(
            val,
            serde_json::json!({"type": "buff", "value": "0xdeadbeef"})
        );
    }

    #[test]
    fn decode_booleans() {
        assert_eq!(decode_clarity_value("03").unwrap(), true);
        assert_eq!(decode_clarity_value("04").unwrap(), false);
    }

    #[test]
    fn decode_none() {
        assert!(decode_clarity_value("09").unwrap().is_null());
    }

    #[test]
    fn decode_some_uint() {
        // some(uint(5)): 0x0a + 0x01 + u128(5)
        let val = decode_clarity_value("0a0100000000000000000000000000000005").unwrap();
        assert_eq!(val, serde_json::json!({"type": "uint", "value": "5"}));
    }

    #[test]
    fn decode_ok_response() {
        // ok(true): 0x07 0x03
        let val = decode_clarity_value("0703").unwrap();
        assert_eq!(val, serde_json::json!({"type": "ok", "value": true}));
    }

    #[test]
    fn decode_err_response() {
        // err(false): 0x08 0x04
        let val = decode_clarity_value("0804").unwrap();
        assert_eq!(val, serde_json::json!({"type": "err", "value": false}));
    }

    #[test]
    fn decode_list() {
        // list(true, false): 0x0b + len(2) + 0x03 + 0x04
        let val = decode_clarity_value("0b000000020304").unwrap();
        assert_eq!(val, serde_json::json!([true, false]));
    }

    #[test]
    fn decode_empty_list() {
        let val = decode_clarity_value("0b00000000").unwrap();
        assert_eq!(val, serde_json::json!([]));
    }

    #[test]
    fn decode_tuple() {
        // tuple { id: uint(1) }
        let val = decode_clarity_value("0c000000010269640100000000000000000000000000000001").unwrap();
        let obj = val.as_object().unwrap();
        assert_eq!(
            obj.get("id").unwrap(),
            &serde_json::json!({"type": "uint", "value": "1"})
        );
    }

    #[test]
    fn decode_string_ascii() {
        // "hello" (5 ASCII bytes)
        let val = decode_clarity_value("0d0000000568656c6c6f").unwrap();
        assert_eq!(val, "hello");
    }

    #[test]
    fn decode_string_utf8() {
        let val = decode_clarity_value("0e0000000568656c6c6f").unwrap();
        assert_eq!(val, "hello");
    }

    #[test]
    fn decode_with_0x_prefix() {
        let val = decode_clarity_value("0x03").unwrap();
        assert!(val.as_bool().unwrap());
    }

    #[test]
    fn decode_standard_principal() {
        // version=22 (0x16), hash160=a46ff88886c2ef9762d970b4d2c63678835bd39d
        // Known correct c32check address: SP2J6ZY48GV1EZ5V2V5RB9MP66SW86PYKKNRV9EJ7
        let val = decode_clarity_value("0516a46ff88886c2ef9762d970b4d2c63678835bd39d").unwrap();
        assert_eq!(val.as_str().unwrap(), "SP2J6ZY48GV1EZ5V2V5RB9MP66SW86PYKKNRV9EJ7");
    }

    #[test]
    fn decode_contract_principal() {
        // Same principal + contract name "my-token"
        let val = decode_clarity_value(
            "0616a46ff88886c2ef9762d970b4d2c63678835bd39d086d792d746f6b656e",
        )
        .unwrap();
        assert_eq!(val.as_str().unwrap(), "SP2J6ZY48GV1EZ5V2V5RB9MP66SW86PYKKNRV9EJ7.my-token");
    }

    #[test]
    fn decode_nested_tuple_with_list() {
        // tuple { items: list[uint(1), uint(2)] }
        let hex = concat!(
            "0c", "00000001",                          // tuple, 1 field
            "05", "6974656d73",                        // "items" (5 bytes)
            "0b", "00000002",                          // list of 2
            "01", "00000000000000000000000000000001",   // uint(1)
            "01", "00000000000000000000000000000002",   // uint(2)
        );
        let val = decode_clarity_value(hex).unwrap();
        let items = val.as_object().unwrap().get("items").unwrap().as_array().unwrap();
        assert_eq!(items.len(), 2);
    }
}
