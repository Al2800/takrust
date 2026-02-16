# RusTAK Deployment Guide (Admin Surface)

## Feature Gating

`rustak-admin` is feature-gated and disabled by default.

```toml
[dependencies]
rustak-admin = { path = "../crates/rustak-admin", default-features = false }
```

Enable endpoint dispatch support only in service binaries that need it:

```toml
[dependencies]
rustak-admin = { path = "../crates/rustak-admin", default-features = false, features = ["admin-server"] }
```

## Secure Defaults

`AdminConfig::default()` is intentionally safe:

- `enabled = false`
- `allow_reload = false`
- `allow_non_loopback_bind = false`
- `bind = 127.0.0.1:9091`
- `reload_path = None`

This keeps admin controls off unless a deployment explicitly opts in.

## Enabling Admin Endpoints Safely

When enabling admin endpoints:

1. Keep loopback bind unless you have an authenticated reverse proxy boundary.
2. Set `reload_path` only with `allow_reload = true`.
3. Keep endpoint paths unique and non-root (`/healthz`, `/metrics`, optional `/reload`).

## Production Posture

- Prefer exposing admin endpoints only behind local sidecars/proxies.
- Treat reload capability as privileged control plane access.
- Keep non-loopback bind opt-in and audited per environment.

## Bridge RC Checklist (Deterministic Replay Gate)

Before promoting a bridge-enabled release candidate, run and record all checks below:

1. Bridge policy contracts
   - `cargo test --manifest-path crates/rustak-bridge/Cargo.toml`
2. Record recovery/integrity baseline
   - `cargo test --manifest-path crates/rustak-record/Cargo.toml recovery_`
3. Deterministic replay gate (interop harness)
   - `cargo test --manifest-path tests/interop_harness/Cargo.toml bridge_replay_rc_gate_end_to_end_is_deterministic_under_replay_and_reconnect`

Release should be blocked if any command fails or if the replay gate digest changes without an approved semantic-change review.
