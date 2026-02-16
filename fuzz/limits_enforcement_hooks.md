# Limits Enforcement Fuzz Hooks

This repository exposes deterministic fuzz hooks in boundary crates so fuzzers
can exercise limits validation paths without constructing full protocol stacks:

- `rustak_wire::fuzz_hook_validate_wire_config`
- `rustak_transport::fuzz_hook_validate_transport_config`
- `rustak_sapient::fuzz_hook_validate_sapient_config`

Each hook accepts arbitrary bytes and maps them to configuration fields before
calling the crate's `validate()` entry point. The hooks are designed to:

1. Never panic on malformed/short inputs.
2. Exercise shared `rustak_limits::Limits` invariant checks.
3. Exercise crate-specific enforcement (timeouts, queue bounds, MTU bounds).

These hooks can be used by:

- in-crate tests (current implementation),
- future `cargo-fuzz` targets,
- external property-based harnesses.
