# SAPIENT Reference

This document defines RusTAK's SAPIENT interoperability baseline.

## Protocol Framing

RusTAK treats SAPIENT transport frames as:

- TCP stream
- 4-byte little-endian length prefix
- bounded payload size checks before decode

Frame validation must fail closed on malformed or oversize inputs.

## Version Support

Current baseline follows architecture guidance for BSI Flex 335 v2.0-oriented
flows with versioned fixture/conformance coverage in:

- `tests/sapient_conformance/**`
- `crates/rustak-sapient/tests/**`

## Message Families

SAPIENT flows typically include:

- status/health style reporting
- detection/track style payloads
- alert/tasking style control payloads

RusTAK keeps SAPIENT semantics distinct from CoT and maps between them through
explicit bridge policy rules.

## Safety and Limits

- all SAPIENT ingress paths must honor centralized limits
- session timeouts/reconnect behavior must be deterministic
- decode failures must return explicit errors (no silent fallback)

## Validation Commands

```bash
cargo test --manifest-path crates/rustak-sapient/Cargo.toml
cargo test --manifest-path crates/rustak-sapient/Cargo.toml -- --exact fixture_versions_match_supported_schema_set
```

If fixture test names evolve, use the full crate suite command as the stable
gate.

## Related Docs

- `docs/quickstart_bridge.md`
- `docs/tak_sapient_mapping.md`
- `docs/conformance.md`
