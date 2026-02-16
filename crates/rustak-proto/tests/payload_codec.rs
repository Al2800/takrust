use std::fs;
use std::path::PathBuf;

use rustak_proto::{decode_v1_payload, encode_v1_payload, ProtoError};

fn fixture_path(file_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("protocol_conformance")
        .join("fixtures")
        .join(file_name)
}

#[test]
fn v1_payload_round_trip_is_deterministic_for_fixture() {
    let cot_message =
        fs::read(fixture_path("proto_v1_cot_message.xml")).expect("fixture should be readable");

    let encoded_once = encode_v1_payload(&cot_message).expect("encoding should succeed");
    let encoded_twice = encode_v1_payload(&cot_message).expect("encoding should succeed");
    assert_eq!(
        encoded_once, encoded_twice,
        "encoding should be deterministic for canonical fixture payload"
    );

    let decoded = decode_v1_payload(&encoded_once).expect("decoding should succeed");
    assert_eq!(decoded, cot_message);
}

#[test]
fn empty_payloads_are_rejected() {
    let empty = Vec::new();
    let encode_error = encode_v1_payload(&empty).expect_err("empty payload must fail");
    assert!(matches!(encode_error, ProtoError::EmptyCotMessage));

    let decode_error = decode_v1_payload(&[0x0A, 0x00]).expect_err("empty encoded payload fails");
    assert!(matches!(decode_error, ProtoError::EmptyCotMessage));
}
