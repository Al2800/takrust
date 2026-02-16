# RusTAK Conformance Matrix

This document codifies the two supported release profiles and their acceptance
gates so TAK-only consumers and TAK+SAPIENT users can validate expected
behavior without accidental coupling.

Related guides:
- `docs/quickstart_tak_only.md`
- `docs/quickstart_bridge.md`

Shared scaffold references:
- `tests/fixtures/README.md`
- `tests/integration/README.md`

## Profiles

### `tak_only`

Purpose:
- Build and validate the TAK-focused stack without bridge/SAPIENT crates.

Included crates:
- `rustak-core`
- `rustak-limits`
- `rustak-wire`
- `rustak-transport`
- `rustak-net`
- `rustak-record`

Forbidden crates:
- `rustak-sapient`
- `rustak-bridge`

Acceptance commands:
- `cargo test --manifest-path tests/release_profiles/Cargo.toml -- --exact profile_matrix_tak_only_is_consistent`
- `cargo test --manifest-path crates/rustak-wire/Cargo.toml`
- `cargo test --manifest-path crates/rustak-transport/Cargo.toml`
- `cargo test --manifest-path crates/rustak-record/Cargo.toml`

### `tak_sapient`

Purpose:
- Validate TAK stack with SAPIENT and bridge surfaces enabled.

Included crates:
- `rustak-core`
- `rustak-limits`
- `rustak-wire`
- `rustak-transport`
- `rustak-net`
- `rustak-sapient`
- `rustak-bridge`
- `rustak-config`
- `rustak-record`

Acceptance commands:
- `cargo test --manifest-path tests/release_profiles/Cargo.toml -- --exact profile_matrix_tak_sapient_includes_bridge_components`
- `cargo test --manifest-path crates/rustak-sapient/Cargo.toml`
- `cargo test --manifest-path crates/rustak-bridge/Cargo.toml`
- `cargo test --manifest-path crates/rustak-record/Cargo.toml`
- `cargo test --manifest-path crates/rustak-record/Cargo.toml recovery_`

## Gate Policy

Release promotion requires:

1. Profile metadata in root `Cargo.toml` to remain synchronized with this file.
2. Profile matrix tests in `tests/release_profiles/tests/profile_matrix.rs` to pass.
3. Crate-level test commands listed above to pass for the selected profile.
4. deterministic replay gate command (`cargo test --manifest-path crates/rustak-record/Cargo.toml recovery_`) to pass for `tak_sapient`.

If a crate is added to the workspace architecture, update all three:
- root profile metadata,
- this conformance matrix,
- profile matrix tests.

## Fixture and Integration Scaffolding

Current repository scaffolding for architecture-aligned test growth:

- Shared fixture root: `tests/fixtures/`
  - CoT seed payloads: `tests/fixtures/cot/`
  - Certificate fixture templates: `tests/fixtures/certs/`
  - Deterministic scenario seeds: `tests/fixtures/scenarios/`
- Integration execution contract: `tests/integration/README.md`
- Interop harness implementation path: `tests/interop_harness/`

As new conformance suites are added, keep fixture sources centralized under
`tests/fixtures/**` and document each integration entrypoint under
`tests/integration/**`.

## Hardening Hooks

Phase-5 hardening checks are exposed through `xtask`:

- `cargo run -p xtask -- hardening-supply-chain`
- `cargo run -p xtask -- hardening-loom`
- `cargo run -p xtask -- hardening`

Supply-chain prerequisites:

- `cargo-deny`
- `cargo-audit`
- `cargo-vet`

`hardening-loom` runs a workspace check under `RUSTFLAGS="--cfg loom"` when
loom markers are present; otherwise it executes a deterministic baseline
workspace check as the loom-smoke fallback.
