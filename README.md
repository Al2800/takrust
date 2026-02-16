# RusTAK (`takrust`)

RusTAK is a Rust-first TAK ecosystem workspace focused on robust CoT/TAK protocol support, secure transport, deterministic simulation/record-replay, and TAK <-> SAPIENT interoperability.

The repository is currently in architecture-to-implementation bootstrap. Foundational technical direction lives in:

- `rustak_architecture.md`
- `rustak_architecture_v2_foundational_20260216_101917.md`
- `AGENTS.md`

## Quickstart Paths

Use the profile-specific quickstarts:

- TAK-only path: `docs/quickstart_tak_only.md`
- TAK + SAPIENT bridge path: `docs/quickstart_bridge.md`
- Profile gate matrix: `docs/conformance.md`

These guides map directly to `workspace.metadata.release_profiles` in root `Cargo.toml`
and the profile matrix tests under `tests/release_profiles/tests/profile_matrix.rs`.

## Reference Docs

- CoT type baseline: `docs/cot_type_reference.md`
- TAK Server integration notes: `docs/tak_server_api.md`
- SAPIENT framing/version notes: `docs/sapient_reference.md`
- TAK↔SAPIENT mapping policy: `docs/tak_sapient_mapping.md`
- Security audit checklist: `docs/security_audit.md`

## Governance

- Security policy: `SECURITY.md`
- Contribution guide: `CONTRIBUTING.md`
- Community conduct: `CODE_OF_CONDUCT.md`

## `xtask` automation commands

This repository uses an `xtask` binary crate for consistent local/CI orchestration.

### Run commands

```bash
cargo run -p xtask -- ci
cargo run -p xtask -- fuzz-smoke
cargo run -p xtask -- release-check
```

### Command behavior

- `ci`
  - Runs `cargo fmt --all -- --check`
  - Runs `cargo clippy --workspace --all-targets -- -D warnings`
  - Runs `cargo test --workspace`
- `fuzz-smoke`
  - If `fuzz/Cargo.toml` exists: runs `cargo fuzz list`
  - Otherwise: runs `cargo check --workspace --all-targets` as deterministic fallback
- `release-check`
  - Runs `cargo check --workspace --all-targets`
  - Runs `cargo test --workspace --all-features`
  - Runs `cargo doc --workspace --no-deps`

### Exit codes

- `0`: success
- `1`: command execution failure
- `2`: usage error
