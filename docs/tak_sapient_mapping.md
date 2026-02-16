# TAK ↔ SAPIENT Mapping Reference

This document describes the RusTAK bridge mapping pipeline and policy surfaces.

## Pipeline Overview

Bridge processing is modeled as deterministic stages:

1. ingest SAPIENT observation/event
2. correlate to stable TAK UID
3. apply dedup and time policy
4. map classification/behavior to CoT type + details
5. emit bounded TAK event stream

## Correlation

Correlation policy maps SAPIENT identity dimensions (node/object/detection) to a
stable CoT UID strategy. Deterministic replay depends on stable mapping behavior
across reconnect/replay conditions.

## Time Policy and Idempotence

Bridge policy controls:

- message-time vs observed-time preference
- skew clamping and stale-window calculation
- dedup window/key cardinality behavior

These must remain deterministic to keep replay gates stable.

## Classification and Behavior Mapping

Mapping tables provide:

- `class_to_cot`: SAPIENT class → CoT type
- `behaviour_to_detail`: behavior labels → CoT detail extensions

Strict startup mode requires non-empty mapping coverage and explicit unknown
fallback handling.

## Strict Startup Guardrails

Startup should fail when:

- mapping coverage is incomplete in strict mode
- unknown fallback class is empty in strict mode
- bridge limits exceed transport limits

## Validation Commands

```bash
cargo test --manifest-path crates/rustak-bridge/Cargo.toml strict_startup_requires_mapping_coverage
cargo test --manifest-path crates/rustak-bridge/Cargo.toml strict_mapping_validation_rejects_incomplete_tables
cargo test --manifest-path crates/rustak-config/Cargo.toml strict_startup_rejects_bridge_limits_above_transport_limits
```

## Related Docs

- `docs/cot_type_reference.md`
- `docs/sapient_reference.md`
- `docs/quickstart_bridge.md`
- `docs/operator_playbook.md`
