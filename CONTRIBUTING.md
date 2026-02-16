# Contributing to RusTAK

## Development Prerequisites

- Rust stable toolchain (`1.82+`)
- `cargo`
- `br` and `bv` for bead workflow

## Workflow

1. Pick ready work using Beads:
   - `br ready --json`
   - `bv --robot-triage`
2. Mark ownership before implementation:
   - `br update <bead-id> --status in_progress --json`
3. Keep scope explicit and dependency-aware.
4. Run scoped validation for touched areas first.
5. Add evidence to the bead, close it, and sync:
   - `br comments add <bead-id> "..."`
   - `br close <bead-id> --reason "Completed" --json`
   - `br sync`

## Code Quality Gates

Use repository automation from `xtask`:

- `cargo run -p xtask -- ci`
- `cargo run -p xtask -- fuzz-smoke`
- `cargo run -p xtask -- release-check`

For targeted work, run crate-scoped `cargo test` / `cargo clippy` commands first.

## Commit and Push Expectations

- Use conventional commit prefixes (`feat`, `fix`, `docs`, `test`, `refactor`, `chore`).
- Include bead references in commit message body when applicable.
- Rebase/pull before push when remote has advanced.
- Do not leave completed work stranded locally.

## Documentation Expectations

When behavior or operational flow changes:

- update relevant docs in `docs/`
- keep quickstart and conformance paths synchronized
- include reproducible command examples
