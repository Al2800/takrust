# RusTAK Conformance Matrix

This document codifies the two supported release profiles and their acceptance
gates so TAK-only consumers and TAK+SAPIENT users can validate expected
behavior without accidental coupling.

Related guides:
- `docs/quickstart_tak_only.md`
- `docs/quickstart_bridge.md`

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
