use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rustak_bridge::{
    BridgeConfig, DedupConfig, DedupDecision, Deduplicator, ResolvedCotTimes, TimePolicyMode,
};

#[derive(Debug, Clone, Copy)]
struct ReplayInput {
    uid: &'static str,
    observed_at: SystemTime,
    message_time: Option<SystemTime>,
}

fn at(seconds: u64, millis: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds) + Duration::from_millis(millis)
}

fn run_replay(
    config: &BridgeConfig,
    inputs: &[ReplayInput],
) -> Vec<(String, DedupDecision, ResolvedCotTimes)> {
    let policy = config.build_time_policy();
    let mut deduplicator = Deduplicator::new(config.dedup, config.limits.max_queue_messages)
        .expect("dedup config from bridge config should be valid");

    inputs
        .iter()
        .map(|input| {
            let resolved = policy.resolve(input.message_time, input.observed_at);
            let decision = deduplicator.observe(input.uid.to_owned(), resolved.time);
            (input.uid.to_owned(), decision, resolved)
        })
        .collect()
}

#[test]
fn replay_sequence_decisions_are_deterministic() {
    let config = BridgeConfig::default();
    let inputs = [
        ReplayInput {
            uid: "track-alpha",
            observed_at: at(100, 0),
            message_time: Some(at(101, 0)),
        },
        ReplayInput {
            uid: "track-bravo",
            observed_at: at(100, 300),
            message_time: Some(at(100, 200)),
        },
        ReplayInput {
            uid: "track-alpha",
            observed_at: at(100, 450),
            message_time: Some(at(101, 0)),
        },
        ReplayInput {
            uid: "track-alpha",
            observed_at: at(99, 900),
            message_time: Some(at(99, 900)),
        },
        ReplayInput {
            uid: "track-alpha",
            observed_at: at(101, 600),
            message_time: Some(at(101, 600)),
        },
    ];

    let first_run = run_replay(&config, &inputs);
    let second_run = run_replay(&config, &inputs);
    assert_eq!(first_run, second_run);

    let decisions = first_run
        .iter()
        .map(|(_, decision, _)| *decision)
        .collect::<Vec<_>>();
    assert_eq!(
        decisions,
        vec![
            DedupDecision::Accepted,
            DedupDecision::Accepted,
            DedupDecision::Duplicate,
            DedupDecision::Duplicate,
            DedupDecision::Accepted,
        ],
    );
}

#[test]
fn reconnect_boundary_preserves_idempotence_window_behavior() {
    let defaults = BridgeConfig::default();
    let config = BridgeConfig {
        time_policy: TimePolicyMode::ObservedTime,
        dedup: DedupConfig {
            window: Duration::from_millis(500),
            max_keys: 16,
        },
        ..defaults
    };

    let pre_disconnect = [
        ReplayInput {
            uid: "sensor-a:track-001",
            observed_at: at(200, 0),
            message_time: Some(at(200, 0)),
        },
        ReplayInput {
            uid: "sensor-a:track-002",
            observed_at: at(200, 100),
            message_time: Some(at(200, 100)),
        },
    ];
    let replay_after_reconnect = [
        ReplayInput {
            uid: "sensor-a:track-001",
            observed_at: at(200, 250),
            message_time: Some(at(200, 0)),
        },
        ReplayInput {
            uid: "sensor-a:track-003",
            observed_at: at(200, 300),
            message_time: Some(at(200, 300)),
        },
        ReplayInput {
            uid: "sensor-a:track-001",
            observed_at: at(201, 0),
            message_time: Some(at(201, 0)),
        },
    ];

    let mut combined = pre_disconnect.to_vec();
    combined.extend_from_slice(&replay_after_reconnect);

    let results = run_replay(&config, &combined);
    let decisions = results
        .iter()
        .map(|(_, decision, _)| *decision)
        .collect::<Vec<_>>();
    assert_eq!(
        decisions,
        vec![
            DedupDecision::Accepted,
            DedupDecision::Accepted,
            DedupDecision::Duplicate,
            DedupDecision::Accepted,
            DedupDecision::Accepted,
        ],
    );
}
