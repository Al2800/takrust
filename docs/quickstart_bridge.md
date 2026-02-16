# TAK + SAPIENT Bridge Quickstart (`tak_sapient`)

This path validates bridge-enabled operation with strict config/mapping checks and deterministic replay verification.

## Scope

`tak_sapient` profile target (from root `Cargo.toml`):
- includes TAK crates plus `rustak-sapient`, `rustak-bridge`, `rustak-config`, `rustak-record`
- no forbidden crates in this profile

## Prerequisites

- Rust stable toolchain (workspace baseline: Rust `1.82+`)
- Run commands from repository root
- Recommended: complete `docs/quickstart_tak_only.md` first

## 1) Validate bridge profile matrix

```bash
cargo test --manifest-path tests/release_profiles/Cargo.toml -- --exact profile_matrix_tak_sapient_includes_bridge_components
```

Checkpoint:
- test passes and confirms bridge/SAPIENT/config crates are included.

## 2) Validate strict startup and mapping coverage checks

```bash
cargo test --manifest-path crates/rustak-config/Cargo.toml strict_startup_rejects_bridge_limits_above_transport_limits
cargo test --manifest-path crates/rustak-bridge/Cargo.toml strict_startup_requires_mapping_coverage
cargo test --manifest-path crates/rustak-bridge/Cargo.toml strict_mapping_validation_rejects_incomplete_tables
```

Checkpoint:
- strict startup gate rejects invalid bridge/transport limits and incomplete mapping tables.

## 3) Run bridge acceptance commands

```bash
cargo test --manifest-path crates/rustak-sapient/Cargo.toml
cargo test --manifest-path crates/rustak-bridge/Cargo.toml
cargo test --manifest-path crates/rustak-record/Cargo.toml
```

Checkpoint:
- SAPIENT codec/session and bridge policy suites pass.

## 4) Run deterministic replay gate

```bash
cargo test --manifest-path crates/rustak-record/Cargo.toml recovery_
```

Checkpoint:
- replay/recovery gate passes for bridge release confidence.

## 5) Optional CLI command topology check

```bash
cargo run -p rustak-cli -- --help
```

Checkpoint:
- CLI help renders bridge/SAPIENT-facing entrypoints in a single command tree.
- Runtime bridge execution is scaffolded and currently returns an explicit not-implemented error.

## Troubleshooting

- If profile test fails, align root `Cargo.toml` release profile metadata with `docs/conformance.md`.
- If strict mapping tests fail, fix `BridgeValidationConfig`/mapping coverage inputs before retrying runtime validation.
