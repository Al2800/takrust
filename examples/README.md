# Scenario and Transport Example Matrix

This directory defines a minimal, deterministic example matrix for TAK-only and bridge-enabled workflows.

## Matrix assets

- `examples/scenario_matrix.yaml` — machine-readable scenario definitions.

## Coverage goals

The matrix explicitly covers:

- send path validation (`xml` + TAK v1 framing),
- listen path validation (receiver framing symmetry),
- replay/recovery validation,
- bridge profile and deterministic replay behavior,
- failure/recovery checks for strict mapping and malformed negotiation control frames.

## Fast validation pass

Run the highest-signal checks in this order:

```bash
cargo test --manifest-path tests/release_profiles/Cargo.toml -- --exact profile_matrix_tak_only_is_consistent
cargo test --manifest-path crates/rustak-transport/Cargo.toml sender_receiver_round_trip_xml_delimited_framing
cargo test --manifest-path crates/rustak-transport/Cargo.toml sender_receiver_round_trip_tak_length_prefixed_framing
cargo test --manifest-path crates/rustak-record/Cargo.toml recovery_
cargo test --manifest-path tests/release_profiles/Cargo.toml -- --exact profile_matrix_tak_sapient_includes_bridge_components
cargo test --manifest-path crates/rustak-bridge/Cargo.toml replay_sequence_decisions_are_deterministic
cargo test --manifest-path crates/rustak-bridge/Cargo.toml strict_mapping_validation_rejects_incomplete_tables
cargo test --manifest-path crates/rustak-wire/Cargo.toml malformed_control_fixtures_remain_fail_closed_terminated
cargo test --manifest-path crates/rustak-wire/Cargo.toml malformed_control_fixtures_remain_fail_open_fallback
```

For release-candidate signoff, pair this matrix with the deterministic replay RC gate in `tests/interop_harness/tests/deterministic_replay_gate.rs`.
