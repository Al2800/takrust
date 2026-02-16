# Wire downgrade policy matrix

This matrix documents deterministic negotiation behavior for the explicit `DowngradePolicy` modes.

| Peer scenario | `FailOpen` | `FailClosed` | Reason code |
| --- | --- | --- | --- |
| Compliant (`TakProtocolVersion::V1`) | Upgrade to TAK protocol | Upgrade to TAK protocol | _none_ |
| Legacy-only / unsupported version | Fallback to legacy XML | Terminate negotiation/connection | `UnsupportedVersion` |
| Malformed control event | Fallback to legacy XML | Terminate negotiation/connection | `MalformedControl` |
| Timeout while awaiting response | Fallback to legacy XML | Terminate negotiation/connection | `Timeout` |
| Explicit policy deny | Terminate negotiation/connection | Terminate negotiation/connection | `PolicyDenied` |

Automated assertions for this matrix live in:

- `crates/rustak-wire/tests/downgrade_policy_matrix.rs`
- `crates/rustak-wire/src/negotiation.rs` unit tests

Protocol payload conformance assertions (XML passthrough vs TAK v1 proto payload path) live in:

- `tests/protocol_conformance/fixtures/proto_v1_cot_message.xml`
- `crates/rustak-wire/tests/proto_payload_matrix.rs`
