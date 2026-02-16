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

## Crypto Provider and Certificate Store Contracts

`rustak-crypto` provides the baseline certificate loading and provider-selection
contracts used by secure transport surfaces.

- Provider modes:
  - `Ring`
  - `AwsLcRs`
  - `AwsLcRsFips` (requires explicit runtime FIPS support)
- Identity source contracts:
  - `PemFiles` (`ca_cert_path`, `client_cert_path`, `client_key_path`)
  - `Pkcs12File` (`archive_path`, optional password)

Validation command:

```bash
cargo test --manifest-path crates/rustak-crypto/Cargo.toml
```

## Related References

- `docs/tak_server_api.md` for TAK Server integration boundaries.
- `docs/tak_sapient_mapping.md` for bridge mapping/correlation policy.
- `docs/security_audit.md` for release-blocking audit checklist.

## Bridge RC Checklist (Deterministic Replay Gate)

Before promoting a bridge-enabled release candidate, run and record all checks below:

1. Bridge policy contracts
   - `cargo test --manifest-path crates/rustak-bridge/Cargo.toml`
2. Record recovery/integrity baseline
   - `cargo test --manifest-path crates/rustak-record/Cargo.toml recovery_`
3. Deterministic replay gate (interop harness)
   - `cargo test --manifest-path tests/interop_harness/Cargo.toml bridge_replay_rc_gate_end_to_end_is_deterministic_under_replay_and_reconnect`

Release should be blocked if any command fails or if the replay gate digest changes without an approved semantic-change review.

## TAK Server Docker Smoke Harness

Use the integration harness to run an environment-backed TAK Server stream
smoke check:

```bash
export RUSTAK_TAK_SERVER_IMAGE=<tak-server-image>
tests/integration/run_tak_server_smoke.sh
```

Optional overrides:

- `RUSTAK_TAK_SERVER_STREAM_HOST` (default `127.0.0.1`)
- `RUSTAK_TAK_SERVER_STREAM_PORT` (default `8089`)
- `RUSTAK_TAK_SERVER_STREAM_PATH` (default `/Marti/api/channels/streaming`)
