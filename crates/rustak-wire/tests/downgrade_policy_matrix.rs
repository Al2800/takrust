use rustak_wire::{
    DowngradePolicy, NegotiationEventKind, NegotiationReason, NegotiationState, Negotiator,
    TakProtocolVersion,
};

#[derive(Debug, Clone, Copy)]
enum PeerScenario {
    Compliant,
    LegacyOnly,
    MalformedControl,
    Timeout,
}

fn run_scenario(
    policy: DowngradePolicy,
    scenario: PeerScenario,
) -> (rustak_wire::NegotiationEvent, NegotiationState) {
    let mut negotiator = Negotiator::new(policy);
    negotiator.begin_upgrade_attempt();

    let event = match scenario {
        PeerScenario::Compliant => negotiator.observe_supported_version(TakProtocolVersion::V1),
        PeerScenario::LegacyOnly => negotiator.observe_unsupported_version(),
        PeerScenario::MalformedControl => negotiator.observe_malformed_control(),
        PeerScenario::Timeout => negotiator.observe_timeout(),
    };

    (event, negotiator.state())
}

#[test]
fn compliant_peers_upgrade_regardless_of_policy() {
    for policy in [DowngradePolicy::FailOpen, DowngradePolicy::FailClosed] {
        let (event, state) = run_scenario(policy, PeerScenario::Compliant);
        assert_eq!(event.kind, NegotiationEventKind::UpgradeAccepted);
        assert_eq!(event.reason, None);
        assert_eq!(state, NegotiationState::Upgraded(TakProtocolVersion::V1));
    }
}

#[test]
fn legacy_only_peers_follow_downgrade_policy_matrix() {
    let (open_event, open_state) =
        run_scenario(DowngradePolicy::FailOpen, PeerScenario::LegacyOnly);
    assert_eq!(open_event.kind, NegotiationEventKind::FallbackToLegacy);
    assert_eq!(
        open_event.reason,
        Some(NegotiationReason::UnsupportedVersion)
    );
    assert_eq!(open_state, NegotiationState::LegacyXml);

    let (closed_event, closed_state) =
        run_scenario(DowngradePolicy::FailClosed, PeerScenario::LegacyOnly);
    assert_eq!(closed_event.kind, NegotiationEventKind::Terminated);
    assert_eq!(
        closed_event.reason,
        Some(NegotiationReason::UnsupportedVersion)
    );
    assert_eq!(
        closed_state,
        NegotiationState::Terminated {
            reason: NegotiationReason::UnsupportedVersion
        }
    );
}

#[test]
fn malformed_control_peers_follow_downgrade_policy_matrix() {
    let (open_event, open_state) =
        run_scenario(DowngradePolicy::FailOpen, PeerScenario::MalformedControl);
    assert_eq!(open_event.kind, NegotiationEventKind::FallbackToLegacy);
    assert_eq!(open_event.reason, Some(NegotiationReason::MalformedControl));
    assert_eq!(open_state, NegotiationState::LegacyXml);

    let (closed_event, closed_state) =
        run_scenario(DowngradePolicy::FailClosed, PeerScenario::MalformedControl);
    assert_eq!(closed_event.kind, NegotiationEventKind::Terminated);
    assert_eq!(
        closed_event.reason,
        Some(NegotiationReason::MalformedControl)
    );
    assert_eq!(
        closed_state,
        NegotiationState::Terminated {
            reason: NegotiationReason::MalformedControl
        }
    );
}

#[test]
fn timeout_peers_follow_downgrade_policy_matrix() {
    let (open_event, open_state) = run_scenario(DowngradePolicy::FailOpen, PeerScenario::Timeout);
    assert_eq!(open_event.kind, NegotiationEventKind::FallbackToLegacy);
    assert_eq!(open_event.reason, Some(NegotiationReason::Timeout));
    assert_eq!(open_state, NegotiationState::LegacyXml);

    let (closed_event, closed_state) =
        run_scenario(DowngradePolicy::FailClosed, PeerScenario::Timeout);
    assert_eq!(closed_event.kind, NegotiationEventKind::Terminated);
    assert_eq!(closed_event.reason, Some(NegotiationReason::Timeout));
    assert_eq!(
        closed_state,
        NegotiationState::Terminated {
            reason: NegotiationReason::Timeout
        }
    );
}
