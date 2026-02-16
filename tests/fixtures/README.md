# RusTAK Fixture Seeds

This directory is the shared seed-fixture root for protocol conformance,
integration smoke tests, and deterministic replay checks.

## Layout

- `cot/` — sample CoT XML messages for parser and round-trip checks.
- `certs/` — non-production certificate fixture templates for local mTLS testing.
- `scenarios/` — deterministic scenario/replay payload seeds.

## Fixture Contract

- Keep fixture payloads deterministic and synthetic (no sensitive data).
- Prefer adding reusable fixtures here and referencing them from crate-local tests.
- Document new fixture intent in this tree so CI and local test paths remain explicit.
