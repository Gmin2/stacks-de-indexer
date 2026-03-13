//! Decode Clarity values from consensus-serialized hex strings.
//!
//! stacks-core includes a `raw_value` hex field in every event payload.
//! This example shows how to decode those hex strings back into JSON
//! using the built-in Clarity decoder.
//!
//! Run with:
//!   cargo run --example decode_clarity_value

fn main() {
    let test_cases = [
        ("int(42)", "000000000000000000000000000000002a"),
        ("uint(1000)", "01000000000000000000000000000003e8"),
        ("true", "03"),
        ("false", "04"),
        ("none", "09"),
        ("ok(true)", "0703"),
        ("err(false)", "0804"),
        ("string-ascii 'hello'", "0d0000000568656c6c6f"),
        ("buffer 0xdeadbeef", "0200000004deadbeef"),
        ("list(true, false)", "0b000000020304"),
        ("tuple { id: uint(1) }", "0c000000010269640100000000000000000000000000000001"),
        ("some(uint(5))", "0a0100000000000000000000000000000005"),
    ];

    for (label, hex) in &test_cases {
        match stacks_indexer_core::clarity::decode_clarity_value(hex) {
            Ok(val) => println!("{label}: {}", serde_json::to_string(&val).unwrap()),
            Err(e) => println!("{label}: ERROR {e}"),
        }
    }
}
