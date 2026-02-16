# TAK-only Quickstart (`tak_only`)

This path validates the TAK-focused stack without SAPIENT/bridge crates.

## Scope

`tak_only` profile target (from root `Cargo.toml`):
- included: `rustak-core`, `rustak-limits`, `rustak-wire`, `rustak-transport`, `rustak-net`, `rustak-record`
- forbidden: `rustak-sapient`, `rustak-bridge`

## Prerequisites

- Rust stable toolchain (workspace baseline: Rust `1.82+`)
- Run all commands from repository root

## 1) Validate profile boundaries

```bash
cargo test --manifest-path tests/release_profiles/Cargo.toml -- --exact profile_matrix_tak_only_is_consistent
```

Checkpoint:
- test passes and confirms forbidden crates are excluded from the profile matrix.

## 2) Run TAK-only acceptance commands

```bash
cargo test --manifest-path crates/rustak-wire/Cargo.toml
cargo test --manifest-path crates/rustak-transport/Cargo.toml
cargo test --manifest-path crates/rustak-record/Cargo.toml
```

Checkpoint:
- all crate tests pass for wire/transport/record paths.

## 3) Optional full-workspace gate

```bash
cargo run -p xtask -- ci
```

Use this when you need full format/clippy/test verification instead of profile-scoped checks.

## Troubleshooting

- If `profile_matrix_tak_only_is_consistent` fails, align `workspace.metadata.release_profiles.tak_only` and `docs/conformance.md`.
- If a crate test fails, fix the crate first, then re-run the profile matrix test to ensure scope remains TAK-only.
