use std::fs;
use std::path::PathBuf;

use rustak_wire::{decode_payload_for_format, encode_payload_for_format, WireFormat};

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
fn xml_and_tak_v1_payload_paths_round_trip_fixture() {
    let fixture =
        fs::read(fixture_path("proto_v1_cot_message.xml")).expect("fixture should be readable");

    let xml_encoded = encode_payload_for_format(&fixture, WireFormat::Xml).expect("xml encode");
    let xml_decoded = decode_payload_for_format(&xml_encoded, WireFormat::Xml).expect("xml decode");
    assert_eq!(
        xml_decoded, fixture,
        "xml payload path should be passthrough"
    );

    let tak_encoded =
        encode_payload_for_format(&fixture, WireFormat::TakProtocolV1).expect("tak encode");
    let tak_decoded =
        decode_payload_for_format(&tak_encoded, WireFormat::TakProtocolV1).expect("tak decode");
    assert_eq!(
        tak_decoded, fixture,
        "tak path should preserve payload semantics"
    );

    assert_ne!(
        tak_encoded, fixture,
        "tak v1 payload bytes should be protobuf-encoded, not raw xml bytes"
    );
}
