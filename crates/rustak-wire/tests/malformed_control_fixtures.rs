use rustak_wire::negotiation::events::ControlFrameError;
use rustak_wire::{DowngradePolicy, NegotiationEventKind, NegotiationReason, Negotiator};

#[test]
fn malformed_control_fixtures_remain_fail_closed_terminated() {
    let fixtures: &[&[u8]] = &[
        b"",
        b"X",
        b"\x00\x01",
        b"V",
        b"V\xff",
        b"\xff\x00\xff",
        b"not-a-control-frame",
    ];

    for fixture in fixtures {
        let mut negotiator = Negotiator::new(DowngradePolicy::FailClosed);
        negotiator.begin_upgrade_attempt();
        let event = negotiator.observe_control_frame(fixture);
        let parse_result = rustak_wire::negotiation::events::parse_control_frame(fixture);

        if parse_result.is_ok() {
            assert_eq!(event.kind, NegotiationEventKind::UpgradeAccepted);
            continue;
        }

        let parse_error = parse_result.expect_err("fixture should be malformed or unsupported");
        match parse_error {
            ControlFrameError::UnsupportedVersion { .. } => {
                assert_eq!(event.kind, NegotiationEventKind::Terminated);
                assert_eq!(event.reason, Some(NegotiationReason::UnsupportedVersion));
            }
            _ => {
                assert_eq!(event.kind, NegotiationEventKind::Terminated);
                assert_eq!(event.reason, Some(NegotiationReason::MalformedControl));
            }
        }
    }
}

#[test]
fn malformed_control_fixtures_remain_fail_open_fallback() {
    let fixtures: &[&[u8]] = &[b"", b"X", b"\x00\x01", b"V", b"V\xff", b"bad"];

    for fixture in fixtures {
        let mut negotiator = Negotiator::new(DowngradePolicy::FailOpen);
        negotiator.begin_upgrade_attempt();
        let event = negotiator.observe_control_frame(fixture);
        let parse_result = rustak_wire::negotiation::events::parse_control_frame(fixture);

        if parse_result.is_ok() {
            assert_eq!(event.kind, NegotiationEventKind::UpgradeAccepted);
            continue;
        }

        assert_eq!(event.kind, NegotiationEventKind::FallbackToLegacy);
        let parse_error = parse_result.expect_err("fixture should be malformed or unsupported");
        match parse_error {
            ControlFrameError::UnsupportedVersion { .. } => {
                assert_eq!(event.reason, Some(NegotiationReason::UnsupportedVersion));
            }
            _ => {
                assert_eq!(event.reason, Some(NegotiationReason::MalformedControl));
            }
        }
    }
}
