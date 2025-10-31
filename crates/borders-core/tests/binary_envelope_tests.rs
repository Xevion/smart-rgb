//! Tests for binary envelope encoding and decoding

use assert2::assert;
use borders_core::ui::protocol::{BinaryMessageType, decode_binary_envelope, encode_binary_message};
use rstest::rstest;

#[test]
fn test_binary_message_type_constants() {
    // Verify that the enum values match the expected constants
    assert!(BinaryMessageType::Init as u8 == 0);
    assert!(BinaryMessageType::Delta as u8 == 1);
}

#[test]
fn test_binary_message_type_from_u8() {
    assert!(BinaryMessageType::from_u8(0) == Some(BinaryMessageType::Init));
    assert!(BinaryMessageType::from_u8(1) == Some(BinaryMessageType::Delta));
    assert!(BinaryMessageType::from_u8(2).is_none());
    assert!(BinaryMessageType::from_u8(255).is_none());
}

/// Test encoding binary messages for different message types
///
/// Verifies the structure: [type:1 byte][payload:N bytes]
#[rstest]
#[case::init(BinaryMessageType::Init, vec![1, 2, 3, 4, 5], 0)]
#[case::delta(BinaryMessageType::Delta, vec![10, 20, 30], 1)]
fn test_encode_binary_message(#[case] msg_type: BinaryMessageType, #[case] payload: Vec<u8>, #[case] expected_type_byte: u8) {
    let encoded = encode_binary_message(msg_type, payload.clone());

    // Check structure: [type:1][payload:N]
    assert!(encoded.len() == 1 + payload.len());
    assert!(encoded[0] == expected_type_byte);
    assert!(&encoded[1..] == &payload[..]);
}

/// Test encode-decode roundtrip for different message types
///
/// Verifies that messages can be encoded and then decoded back to their original form
#[rstest]
#[case::init(BinaryMessageType::Init, vec![42, 43, 44, 45])]
#[case::delta(BinaryMessageType::Delta, vec![100, 101, 102])]
fn test_encode_decode_roundtrip(#[case] msg_type: BinaryMessageType, #[case] original_payload: Vec<u8>) {
    let encoded = encode_binary_message(msg_type, original_payload.clone());

    let decoded = decode_binary_envelope(&encoded);
    assert!(decoded.is_some());

    let (decoded_type, decoded_payload) = decoded.unwrap();
    assert!(decoded_type == msg_type);
    assert!(decoded_payload == &original_payload[..]);
}

#[test]
fn test_decode_empty_data() {
    let empty: &[u8] = &[];
    assert!(decode_binary_envelope(empty).is_none());
}

#[test]
fn test_decode_invalid_type() {
    let invalid = vec![99, 1, 2, 3]; // type = 99 (invalid)
    assert!(decode_binary_envelope(&invalid).is_none());
}

#[test]
fn test_decode_type_only() {
    // Valid type with no payload
    let init_only = vec![0];
    let decoded = decode_binary_envelope(&init_only);
    assert!(decoded.is_some());

    let (msg_type, payload) = decoded.unwrap();
    assert!(msg_type == BinaryMessageType::Init);
    assert!(payload.is_empty());
}

#[test]
fn test_encode_large_payload() {
    // Test with a large payload to ensure no issues with size
    let large_payload = vec![42; 100_000];
    let encoded = encode_binary_message(BinaryMessageType::Init, large_payload.clone());

    assert!(encoded.len() == 1 + large_payload.len());
    assert!(encoded[0] == 0);

    let decoded = decode_binary_envelope(&encoded);
    assert!(decoded.is_some());

    let (msg_type, payload) = decoded.unwrap();
    assert!(msg_type == BinaryMessageType::Init);
    assert!(payload.len() == large_payload.len());
    assert!(payload == &large_payload[..]);
}
