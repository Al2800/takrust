# TAK Server API Notes

This document captures RusTAK integration expectations for TAK Server surfaces.

## Scope

RusTAK treats TAK Server integration as two layers:

- **Streaming channel**: bounded CoT stream send/receive path
- **Management/API layer**: auth/session and control-plane interactions

The current repository includes transport/admin/replay contracts and architecture
targets for a dedicated `rustak-server` crate.

## Connection Expectations

- mTLS is the default posture for server-facing transport.
- Connection setup must use explicit limits and bounded framing.
- Reconnect behavior must remain deterministic for replayability and audit.

## Streaming Contracts

Expected behavior for the streaming path:

- send validated CoT events
- receive/decode bounded frames
- surface explicit error taxonomy for negotiation/framing failures

Related references:

- `docs/deployment_guide.md`
- `docs/conformance.md`
- `docs/operator_playbook.md`

## Integration Boundaries

Until `rustak-server` lands, server-focused validation is split across:

- transport checks (`rustak-transport`)
- wire/negotiation checks (`rustak-wire`)
- replay/integrity checks (`rustak-record`, `tests/interop_harness`)

## Operational Guardrails

- keep cert material handling isolated and auditable
- fail fast on invalid startup configuration
- prefer deterministic gates for release readiness

## Planned Surface (Design Reference)

Planned `rustak-server` API categories:

- connection/session setup
- streaming channel handle acquisition
- server capability and health interaction
- mission/data-package style management operations

This section is design guidance and will be updated to concrete API docs once
the crate is implemented.
