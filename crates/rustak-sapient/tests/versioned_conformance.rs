use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use rustak_limits::Limits;
use rustak_sapient::{SapientCodec, SapientSessionBuffers};
use serde::Deserialize;
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Deserialize)]
struct FixtureDocument {
    fixtures: Vec<VersionedFixture>,
}

#[derive(Debug, Deserialize)]
struct VersionedFixture {
    schema_version: String,
    codec_payload_utf8: String,
    inbound_session_sequence_utf8: Vec<String>,
    outbound_session_sequence_utf8: Vec<String>,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn fixture_path() -> PathBuf {
    repo_root()
        .join("tests")
        .join("sapient_conformance")
        .join("fixtures")
        .join("versioned_codec_session.json")
}

fn load_fixtures() -> FixtureDocument {
    let bytes = fs::read(fixture_path()).expect("SAPIENT conformance fixture should exist");
    serde_json::from_slice(&bytes).expect("SAPIENT conformance fixture should parse")
}

fn test_limits(
    max_frame_bytes: usize,
    max_queue_messages: usize,
    max_queue_bytes: usize,
) -> Limits {
    Limits {
        max_frame_bytes,
        max_xml_scan_bytes: max_frame_bytes,
        max_protobuf_bytes: max_frame_bytes,
        max_queue_messages,
        max_queue_bytes,
        max_detail_elements: 8,
    }
}

async fn encode_frame(codec: &SapientCodec, payload: &[u8]) -> Vec<u8> {
    let capacity = payload.len().saturating_add(16).max(64);
    let (mut writer, mut reader) = duplex(capacity);
    codec
        .write_message(&mut writer, payload)
        .await
        .expect("fixture payload should encode");

    let mut framed = vec![0_u8; payload.len().saturating_add(4)];
    reader
        .read_exact(&mut framed)
        .await
        .expect("encoded frame should be readable");
    framed
}

async fn decode_frame(codec: &SapientCodec, framed: &[u8]) -> Vec<u8> {
    let capacity = framed.len().saturating_add(16).max(64);
    let (mut writer, mut reader) = duplex(capacity);
    writer
        .write_all(framed)
        .await
        .expect("fixture frame should be writable");
    drop(writer);
    codec
        .read_message(&mut reader)
        .await
        .expect("fixture frame should decode")
}

#[test]
fn fixture_versions_match_supported_schema_set() {
    let fixture_doc = load_fixtures();
    let versions = fixture_doc
        .fixtures
        .iter()
        .map(|fixture| fixture.schema_version.as_str())
        .collect::<BTreeSet<_>>();

    let expected = ["bsi_flex_335_v2_0", "bsi_flex_335_v2_1"]
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(versions, expected);
}

#[tokio::test]
async fn codec_round_trip_is_stable_across_versioned_fixtures() {
    let fixture_doc = load_fixtures();
    let codec = SapientCodec::from_limits(&test_limits(512, 16, 4096));

    for fixture in fixture_doc.fixtures {
        let payload = fixture.codec_payload_utf8.into_bytes();
        let framed = encode_frame(&codec, &payload).await;

        let expected_prefix = u32::try_from(payload.len())
            .expect("fixture payload must fit u32 frame prefix")
            .to_be_bytes();
        assert_eq!(&framed[..4], expected_prefix.as_slice());
        assert_eq!(&framed[4..], payload.as_slice());

        let decoded = decode_frame(&codec, &framed).await;
        assert_eq!(decoded, payload);
    }
}

#[test]
fn session_fifo_and_accounting_are_stable_across_versions() {
    let fixture_doc = load_fixtures();
    let limits = test_limits(512, 32, 4096);

    for fixture in fixture_doc.fixtures {
        let inbound_expected = fixture
            .inbound_session_sequence_utf8
            .iter()
            .map(|value| value.clone().into_bytes())
            .collect::<Vec<_>>();
        let outbound_expected = fixture
            .outbound_session_sequence_utf8
            .iter()
            .map(|value| value.clone().into_bytes())
            .collect::<Vec<_>>();

        let inbound_total_bytes = inbound_expected.iter().map(Vec::len).sum::<usize>();
        let outbound_total_bytes = outbound_expected.iter().map(Vec::len).sum::<usize>();

        let mut session = SapientSessionBuffers::from_limits(&limits);
        for payload in inbound_expected.iter().cloned() {
            session
                .push_inbound(payload)
                .expect("fixture inbound payload should fit");
        }
        for payload in outbound_expected.iter().cloned() {
            session
                .push_outbound(payload)
                .expect("fixture outbound payload should fit");
        }

        assert_eq!(session.inbound_len(), inbound_expected.len());
        assert_eq!(session.outbound_len(), outbound_expected.len());
        assert_eq!(session.inbound_bytes(), inbound_total_bytes);
        assert_eq!(session.outbound_bytes(), outbound_total_bytes);

        let drained_inbound = std::iter::from_fn(|| session.pop_inbound()).collect::<Vec<_>>();
        let drained_outbound = std::iter::from_fn(|| session.pop_outbound()).collect::<Vec<_>>();

        assert_eq!(drained_inbound, inbound_expected);
        assert_eq!(drained_outbound, outbound_expected);
        assert_eq!(session.inbound_len(), 0);
        assert_eq!(session.outbound_len(), 0);
        assert_eq!(session.inbound_bytes(), 0);
        assert_eq!(session.outbound_bytes(), 0);
    }
}

#[test]
fn reconnect_order_preserves_unsent_outbound_frames() {
    let mut session = SapientSessionBuffers::from_limits(&test_limits(256, 8, 2048));

    let pre_disconnect = [
        b"SAPIENT|v2|outbound|seq=1".to_vec(),
        b"SAPIENT|v2|outbound|seq=2".to_vec(),
        b"SAPIENT|v2|outbound|seq=3".to_vec(),
    ];
    for payload in pre_disconnect.iter().cloned() {
        session
            .push_outbound(payload)
            .expect("outbound payload should fit");
    }

    assert_eq!(
        session.pop_outbound(),
        Some(pre_disconnect[0].clone()),
        "first frame should be sent before reconnect boundary",
    );

    let post_reconnect = b"SAPIENT|v2|outbound|seq=4".to_vec();
    session
        .push_outbound(post_reconnect.clone())
        .expect("post reconnect payload should fit");

    let replayed = std::iter::from_fn(|| session.pop_outbound()).collect::<Vec<_>>();
    assert_eq!(
        replayed,
        vec![
            pre_disconnect[1].clone(),
            pre_disconnect[2].clone(),
            post_reconnect,
        ],
    );
}

#[test]
fn reconnect_order_preserves_buffered_inbound_frames() {
    let mut session = SapientSessionBuffers::from_limits(&test_limits(256, 8, 2048));

    let pre_disconnect = [
        b"SAPIENT|v2|inbound|seq=10".to_vec(),
        b"SAPIENT|v2|inbound|seq=11".to_vec(),
        b"SAPIENT|v2|inbound|seq=12".to_vec(),
    ];
    for payload in pre_disconnect.iter().cloned() {
        session
            .push_inbound(payload)
            .expect("inbound payload should fit");
    }

    assert_eq!(
        session.pop_inbound(),
        Some(pre_disconnect[0].clone()),
        "first frame should be consumed before reconnect boundary",
    );

    let post_reconnect = b"SAPIENT|v2|inbound|seq=13".to_vec();
    session
        .push_inbound(post_reconnect.clone())
        .expect("post reconnect inbound payload should fit");

    let replayed = std::iter::from_fn(|| session.pop_inbound()).collect::<Vec<_>>();
    assert_eq!(
        replayed,
        vec![
            pre_disconnect[1].clone(),
            pre_disconnect[2].clone(),
            post_reconnect,
        ],
    );
}
