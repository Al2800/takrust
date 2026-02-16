# `rustak-limits` contract

This document defines the canonical resource-budget surface for RusTAK boundary crates.

## Schema

The `Limits` struct is the single source of truth for shared bounds:

- `max_frame_bytes`
- `max_xml_scan_bytes`
- `max_protobuf_bytes`
- `max_queue_messages`
- `max_queue_bytes`
- `max_detail_elements`

## Conservative defaults

The `conservative_defaults()` profile matches the architecture configuration baseline:

| Field | Default |
| --- | ---: |
| `max_frame_bytes` | `1_048_576` |
| `max_xml_scan_bytes` | `1_048_576` |
| `max_protobuf_bytes` | `1_048_576` |
| `max_queue_messages` | `1_024` |
| `max_queue_bytes` | `8_388_608` |
| `max_detail_elements` | `512` |

## Invariants

`Limits::validate()` enforces:

1. Every field is strictly greater than zero.
2. `max_xml_scan_bytes <= max_frame_bytes`.
3. `max_protobuf_bytes <= max_frame_bytes`.
4. `max_queue_bytes >= max_frame_bytes`.
5. `max_queue_messages <= max_queue_bytes` (at least one byte per queued message).

These checks prevent unbounded parsing and queue configurations from entering runtime boundary crates.

## Error taxonomy

Validation failures return `LimitsError`:

- `Zero { field }`
- `XmlScanExceedsFrame { ... }`
- `ProtobufExceedsFrame { ... }`
- `QueueBytesBelowFrame { ... }`
- `QueueMessagesExceedQueueBytes { ... }`
