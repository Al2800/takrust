# RusTAK Operator Playbook

This runbook maps common operator intents to concrete commands and expected
signals for `tak_only` and `tak_sapient` profiles.

## 0) Pick deployment profile first

```bash
# TAK-only
cargo test --manifest-path tests/release_profiles/Cargo.toml -- --exact profile_matrix_tak_only_is_consistent

# TAK+SAPIENT bridge
cargo test --manifest-path tests/release_profiles/Cargo.toml -- --exact profile_matrix_tak_sapient_includes_bridge_components
```

If either profile check fails, stop and resolve profile metadata drift before
continuing triage.

## 1) Intent-to-command matrix

This matrix is aligned with `examples/scenario_matrix.yaml`.

| Intent | Profile | Command | Expected signal | Recovery command |
| --- | --- | --- | --- | --- |
| Send path validation | `tak_only` | `cargo test --manifest-path crates/rustak-transport/Cargo.toml sender_receiver_round_trip_xml_delimited_framing` | XML-delimited round trip passes | `cargo test --manifest-path tests/release_profiles/Cargo.toml -- --exact profile_matrix_tak_only_is_consistent` |
| Listen path validation | `tak_only` | `cargo test --manifest-path crates/rustak-transport/Cargo.toml sender_receiver_round_trip_tak_length_prefixed_framing` | TAK-v1 length-prefixed round trip passes | same command rerun after framing/config fix |
| Replay/recovery baseline | `tak_only` | `cargo test --manifest-path crates/rustak-record/Cargo.toml recovery_` | `recovery_*` tests pass | same command rerun after record/index fix |
| Bridge profile boundary | `tak_sapient` | `cargo test --manifest-path tests/release_profiles/Cargo.toml -- --exact profile_matrix_tak_sapient_includes_bridge_components` | bridge/sapient/config crates included | same command after metadata sync |
| Bridge replay determinism | `tak_sapient` | `cargo test --manifest-path crates/rustak-bridge/Cargo.toml replay_sequence_decisions_are_deterministic` | replay decision vector remains stable | rerun after dedup/correlation/time-policy fix |
| Strict mapping failure path | `tak_sapient` | `cargo test --manifest-path crates/rustak-bridge/Cargo.toml strict_mapping_validation_rejects_incomplete_tables` | incomplete mappings are rejected | `cargo test --manifest-path crates/rustak-bridge/Cargo.toml strict_startup_requires_mapping_coverage` |
| Malformed control-frame handling | `tak_sapient` | `cargo test --manifest-path crates/rustak-wire/Cargo.toml malformed_control_fixtures_remain_fail_closed_terminated` | malformed frame terminates in fail-closed mode | `cargo test --manifest-path crates/rustak-wire/Cargo.toml malformed_control_fixtures_remain_fail_open_fallback` |

## 2) Health and metrics triage

If admin server is enabled (`rustak-admin` with `admin-server` feature), verify:

- `GET /healthz` → HTTP `200`, body shape: `{"status":"ok","uptime_seconds":...}`
- `GET /metrics` → HTTP `200`, content type `text/plain; version=0.0.4`
- `POST /reload` → HTTP `200` with `{"reloaded":true}` only when `allow_reload=true`

If control-plane behavior is unexpected, run:

```bash
cargo test --manifest-path crates/rustak-admin/Cargo.toml
cargo test --manifest-path crates/rustak-admin/Cargo.toml --features admin-server
```

Config guardrails to verify:
- loopback bind is enforced unless `allow_non_loopback_bind=true`
- endpoint paths are unique and non-root
- `reload_path` requires `allow_reload=true`

## 3) Negotiation and transport failures

Symptoms:
- profile mismatch or negotiation fallback surprises
- malformed control-frame behavior drift
- sender/receiver framing regressions

Run:

```bash
cargo test --manifest-path crates/rustak-wire/Cargo.toml
cargo test --manifest-path crates/rustak-transport/Cargo.toml
cargo test --manifest-path crates/rustak-wire/Cargo.toml malformed_control_fixtures_remain_fail_closed_terminated
cargo test --manifest-path crates/rustak-wire/Cargo.toml malformed_control_fixtures_remain_fail_open_fallback
```

## 4) Limits breach and strict-startup failures

Symptoms:
- frame-too-large, queue saturation, or backpressure instability
- startup rejection due to transport/bridge limits mismatch

Run:

```bash
cargo test --manifest-path crates/rustak-limits/Cargo.toml
cargo test --manifest-path crates/rustak-config/Cargo.toml strict_startup_rejects_bridge_limits_above_transport_limits
```

Expected posture:
- limits remain bounded and deterministic
- strict startup fails fast on invalid bridge/transport sizing

## 5) Bridge mapping, time policy, and idempotence

Symptoms:
- unstable CoT times
- UID churn under reconnect/replay
- duplicate emissions that should be deduplicated

Run:

```bash
cargo test --manifest-path crates/rustak-bridge/Cargo.toml
cargo test --manifest-path crates/rustak-bridge/Cargo.toml strict_startup_requires_mapping_coverage
cargo test --manifest-path crates/rustak-bridge/Cargo.toml strict_mapping_validation_rejects_incomplete_tables
```

## 6) Replay gate and release confidence

For bridge release-candidate signoff:

```bash
cargo test --manifest-path crates/rustak-record/Cargo.toml recovery_
cargo test --manifest-path tests/interop_harness/Cargo.toml bridge_replay_rc_gate_end_to_end_is_deterministic_under_replay_and_reconnect
```

Escalate and block release if:
- replay digest changes without approved semantic-change review
- replay/reconnect causes deterministic projection divergence

## 7) Incident notes template

Capture for every incident:

- profile (`tak_only` or `tak_sapient`)
- exact command and failing output
- startup-time vs runtime failure
- replay reproduction result
- first known good commit/build

Use this template to keep RCA deterministic and reduce repeated triage loops.
