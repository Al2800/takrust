use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use interop_harness_tests::{deterministic_replay_digest, load_replay_fixture, ReplayObservation};
use rustak_bridge::{
    BehaviourMapping, BridgeConfig, CorrelationInput, Correlator, CorrelatorConfig, DedupDecision,
    Deduplicator, MappingSeverity, MappingTables, UidPolicy,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

fn fixture_path(file_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(file_name)
}

#[test]
fn bridge_replay_digest_is_stable_for_known_fixture() {
    let observations = load_replay_fixture(fixture_path("bridge_replay_fixture.json"))
        .expect("fixture should load");
    let digest = deterministic_replay_digest(&observations);

    assert_eq!(
        digest, "35df806c8b1fc1392df509ba828e0ba5e4737f0357a0bbf5cae830ae52722a41",
        "deterministic digest changed; verify intended semantic change before updating gate",
    );
}

#[test]
fn bridge_replay_digest_is_order_independent() {
    let mut observations = load_replay_fixture(fixture_path("bridge_replay_fixture.json"))
        .expect("fixture should load");
    let canonical_digest = deterministic_replay_digest(&observations);

    observations.reverse();
    let reversed_digest = deterministic_replay_digest(&observations);
    assert_eq!(reversed_digest, canonical_digest);
}

#[test]
fn bridge_replay_digest_detects_semantic_mutation() {
    let observations = load_replay_fixture(fixture_path("bridge_replay_fixture.json"))
        .expect("fixture should load");
    let baseline = deterministic_replay_digest(&observations);

    let mut mutated = observations.clone();
    mutated.push(ReplayObservation {
        stream_id: "alpha".to_string(),
        sequence: 99,
        timestamp_nanos: 1_700_000_999_999_999_999,
        uid: "uas-099".to_string(),
        cot_type: "a-f-G-U-C".to_string(),
        classification: "hostile".to_string(),
        behavior: "approach".to_string(),
        confidence: 0.99,
    });
    let mutated_digest = deterministic_replay_digest(&mutated);

    assert_ne!(mutated_digest, baseline);
}

#[test]
fn bridge_replay_rc_gate_end_to_end_is_deterministic_under_replay_and_reconnect() {
    let observations = load_replay_fixture(fixture_path("bridge_replay_fixture.json"))
        .expect("fixture should load");

    let baseline_projection = run_bridge_projection_pipeline(&observations);
    let baseline_digest = digest_projection(&baseline_projection);
    assert_eq!(
        baseline_digest, "9033e31c6581f3ea4e0b18bc538e0c646b39d125650319aa7f6d71f7a8606ff5",
        "release gate digest changed; verify intended bridge semantics before updating baseline",
    );

    let mut replay_and_reconnect_stream = observations.clone();
    replay_and_reconnect_stream.extend(observations.iter().rev().cloned());
    let replay_projection = run_bridge_projection_pipeline(&replay_and_reconnect_stream);
    let replay_digest = digest_projection(&replay_projection);
    assert_eq!(
        replay_digest, baseline_digest,
        "replay/reconnect ordering should not change deterministic bridge projection digest",
    );

    assert!(
        !baseline_projection.is_empty(),
        "bridge projection must produce at least one emission for RC gate evaluation"
    );
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct BridgeProjection {
    stream_id: String,
    sequence: u64,
    correlated_uid: String,
    cot_type: String,
    classification: String,
    time_nanos: u128,
    stale_nanos: u128,
}

fn run_bridge_projection_pipeline(observations: &[ReplayObservation]) -> Vec<BridgeProjection> {
    let bridge_config = BridgeConfig::default();
    let mapping_tables = release_mapping_tables();
    bridge_config
        .validate_with_mappings(&mapping_tables)
        .expect("strict startup mapping validation must pass for release gate");

    let mut correlator = Correlator::new(CorrelatorConfig {
        uid_policy: UidPolicy::StablePerObject,
        uid_prefix: "trk".to_owned(),
    })
    .expect("correlator config should be valid");
    let mut deduplicator =
        Deduplicator::new(bridge_config.dedup, bridge_config.limits.max_queue_messages)
            .expect("dedup config should be valid");
    let time_policy = bridge_config.build_time_policy();

    let mut projection = Vec::new();
    for observation in observations {
        let observed_time = unix_nanos_to_system_time(observation.timestamp_nanos);
        let correlated_uid = correlator
            .correlate(&CorrelationInput {
                node_id: observation.stream_id.clone(),
                object_id: Some(observation.uid.clone()),
                detection_id: Some(format!(
                    "{}:{}:{}",
                    observation.uid, observation.stream_id, observation.sequence
                )),
            })
            .expect("correlator input must be complete");

        let dedup_key = format!("{correlated_uid}:{}", observation.sequence);
        if matches!(
            deduplicator.observe(dedup_key, observed_time),
            DedupDecision::Duplicate
        ) {
            continue;
        }

        let resolved = time_policy.resolve(Some(observed_time), observed_time);
        let mapped_cot_type = mapping_tables.map_classification(
            observation.classification.as_str(),
            bridge_config.validation.unknown_class_fallback.as_str(),
        );

        projection.push(BridgeProjection {
            stream_id: observation.stream_id.clone(),
            sequence: observation.sequence,
            correlated_uid,
            cot_type: mapped_cot_type.to_owned(),
            classification: observation.classification.clone(),
            time_nanos: system_time_to_unix_nanos(resolved.time),
            stale_nanos: system_time_to_unix_nanos(resolved.stale),
        });
    }

    projection.sort_by(|left, right| {
        (
            left.stream_id.as_str(),
            left.sequence,
            left.correlated_uid.as_str(),
        )
            .cmp(&(
                right.stream_id.as_str(),
                right.sequence,
                right.correlated_uid.as_str(),
            ))
    });
    projection
}

fn release_mapping_tables() -> MappingTables {
    MappingTables {
        class_to_cot: [
            ("friendly".to_owned(), "a-f-G-U-C".to_owned()),
            ("suspect".to_owned(), "a-n-A-C-F".to_owned()),
            ("hostile".to_owned(), "a-n-A-C-F".to_owned()),
            ("unknown".to_owned(), "a-u-A-M-F-Q".to_owned()),
        ]
        .into_iter()
        .collect(),
        behaviour_to_detail: [(
            "loiter".to_owned(),
            BehaviourMapping {
                detail_key: "sapient.behaviour".to_owned(),
                severity: MappingSeverity::Warning,
            },
        )]
        .into_iter()
        .collect(),
    }
}

fn unix_nanos_to_system_time(value: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_nanos(value)
}

fn system_time_to_unix_nanos(value: SystemTime) -> u128 {
    value
        .duration_since(UNIX_EPOCH)
        .expect("gate times should be representable after epoch")
        .as_nanos()
}

fn digest_projection(projection: &[BridgeProjection]) -> String {
    let payload =
        serde_json::to_vec(projection).expect("serializing bridge projection should not fail");
    let mut hasher = Sha256::new();
    hasher.update(payload);
    let digest = hasher.finalize();
    format!("{digest:x}")
}
