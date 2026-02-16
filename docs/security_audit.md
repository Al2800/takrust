# Security Audit Guide

This guide defines a repeatable security-audit pass for RusTAK.

## Audit Objectives

- preserve bounded parsing and transport behavior
- validate strict startup and mapping policy enforcement
- verify replay integrity/determinism gates
- confirm control-plane exposure stays least-privilege

## Priority Surfaces

1. framing/parsing boundaries (`rustak-wire`, `rustak-sapient`, `rustak-cot`)
2. limits/backpressure enforcement (`rustak-limits`, transport/bridge integrations)
3. bridge correlation + mapping strictness (`rustak-bridge`, `rustak-config`)
4. replay/integrity chains (`rustak-record`, interop harness)
5. admin endpoint posture (`rustak-admin`)

## Recommended Audit Command Set

```bash
cargo test --manifest-path tests/release_profiles/Cargo.toml
cargo test --manifest-path crates/rustak-wire/Cargo.toml
cargo test --manifest-path crates/rustak-sapient/Cargo.toml
cargo test --manifest-path crates/rustak-bridge/Cargo.toml
cargo test --manifest-path crates/rustak-record/Cargo.toml recovery_
cargo test --manifest-path tests/interop_harness/Cargo.toml bridge_replay_rc_gate_end_to_end_is_deterministic_under_replay_and_reconnect
cargo test --manifest-path crates/rustak-admin/Cargo.toml --features admin-server
```

## Findings Triage Template

For each finding, capture:

- component + file path
- reproducible command and output
- exploitability and impact scope
- mitigation options
- owner and target fix window

## Release Blocking Conditions

Block release when any of these fail:

- strict startup validation invariants
- deterministic replay digest gate
- malformed-frame fail-safe behavior
- cert/config policy safety checks

## Related Docs

- `SECURITY.md`
- `docs/deployment_guide.md`
- `docs/operator_playbook.md`
- `docs/tak_sapient_mapping.md`
