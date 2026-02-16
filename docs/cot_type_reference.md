# CoT Type Reference

This document is the RusTAK reference baseline for Cursor on Target (CoT) type
strings used across core, bridge, and conformance surfaces.

## CoT Type Shape

A CoT type is a hyphen-delimited taxonomy string (example: `a-f-G-U-C`) and is
modeled as a validated primitive (`CotType`) in `rustak-core`.

High-level pattern:

- `a` prefix: atom/event
- subsequent segments: affiliation/class/function detail

RusTAK treats these values as validated data, not free-form strings.

## Baseline Types Used in Current Flows

| CoT Type | Typical meaning | Current usage |
| --- | --- | --- |
| `a-f-G-U-C` | friendly unit/track baseline | default friendly mapping in bridge tests |
| `a-n-A-C-F` | hostile/suspect air track baseline | hostile/suspect mapping in bridge tests |
| `a-u-A-M-F-Q` | unknown fallback | strict mapping fallback for unmapped classes |

## Mapping Expectations

- Bridge classification mapping must resolve to valid CoT types.
- In strict startup mode, mappings must be complete enough to avoid silent
  fallback drift.
- Unknown-class fallback must be explicitly configured and non-empty.

See `docs/tak_sapient_mapping.md` for pipeline-level mapping behavior.

## Validation Guidance

Use bridge/config strict-startup checks when updating type mappings:

```bash
cargo test --manifest-path crates/rustak-bridge/Cargo.toml strict_startup_requires_mapping_coverage
cargo test --manifest-path crates/rustak-bridge/Cargo.toml strict_mapping_validation_rejects_incomplete_tables
```

## Scope Note

This is a practical RusTAK reference baseline for currently-supported workflows.
It is not a complete MIL-STD-2525 catalog.
