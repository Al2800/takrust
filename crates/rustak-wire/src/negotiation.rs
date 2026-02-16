use crate::DowngradePolicy;

#[path = "events.rs"]
pub mod events;

use events::{ControlFrameError, NegotiationTelemetry, NegotiationTelemetryEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TakProtocolVersion {
    V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegotiationState {
    LegacyXml,
    AwaitingResponse,
    Upgraded(TakProtocolVersion),
    Terminated { reason: NegotiationReason },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegotiationEventKind {
    NoChange,
    UpgradeAccepted,
    FallbackToLegacy,
    Terminated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegotiationReason {
    Timeout,
    MalformedControl,
    UnsupportedVersion,
    PolicyDenied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NegotiationEvent {
    pub kind: NegotiationEventKind,
    pub reason: Option<NegotiationReason>,
}

impl NegotiationEvent {
    #[must_use]
    pub const fn no_change() -> Self {
        Self {
            kind: NegotiationEventKind::NoChange,
            reason: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Negotiator {
    policy: DowngradePolicy,
    state: NegotiationState,
}

impl Negotiator {
    #[must_use]
    pub const fn new(policy: DowngradePolicy) -> Self {
        Self {
            policy,
            state: NegotiationState::LegacyXml,
        }
    }

    #[must_use]
    pub const fn state(&self) -> NegotiationState {
        self.state
    }

    pub fn begin_upgrade_attempt(&mut self) -> NegotiationEvent {
        if self.state != NegotiationState::LegacyXml {
            return NegotiationEvent::no_change();
        }

        self.state = NegotiationState::AwaitingResponse;
        NegotiationEvent::no_change()
    }

    pub fn observe_supported_version(&mut self, version: TakProtocolVersion) -> NegotiationEvent {
        if self.state != NegotiationState::AwaitingResponse {
            return NegotiationEvent::no_change();
        }

        self.state = NegotiationState::Upgraded(version);
        NegotiationEvent {
            kind: NegotiationEventKind::UpgradeAccepted,
            reason: None,
        }
    }

    pub fn observe_timeout(&mut self) -> NegotiationEvent {
        self.resolve_downgrade_or_terminate(NegotiationReason::Timeout)
    }

    pub fn observe_malformed_control(&mut self) -> NegotiationEvent {
        self.resolve_downgrade_or_terminate(NegotiationReason::MalformedControl)
    }

    pub fn observe_unsupported_version(&mut self) -> NegotiationEvent {
        self.resolve_downgrade_or_terminate(NegotiationReason::UnsupportedVersion)
    }

    pub fn observe_policy_denied(&mut self) -> NegotiationEvent {
        if matches!(self.state, NegotiationState::Terminated { .. }) {
            return NegotiationEvent::no_change();
        }

        self.state = NegotiationState::Terminated {
            reason: NegotiationReason::PolicyDenied,
        };
        NegotiationEvent {
            kind: NegotiationEventKind::Terminated,
            reason: Some(NegotiationReason::PolicyDenied),
        }
    }

    pub fn observe_control_frame(&mut self, frame: &[u8]) -> NegotiationEvent {
        match events::parse_control_frame(frame) {
            Ok(version) => self.observe_supported_version(version),
            Err(ControlFrameError::UnsupportedVersion { .. }) => self.observe_unsupported_version(),
            Err(_) => self.observe_malformed_control(),
        }
    }

    pub fn begin_upgrade_attempt_with_telemetry(
        &mut self,
        session_id: u64,
        telemetry: &mut NegotiationTelemetry,
    ) -> NegotiationTelemetryEvent {
        let event = self.begin_upgrade_attempt();
        self.emit_telemetry(session_id, event, telemetry)
    }

    pub fn observe_supported_version_with_telemetry(
        &mut self,
        session_id: u64,
        version: TakProtocolVersion,
        telemetry: &mut NegotiationTelemetry,
    ) -> NegotiationTelemetryEvent {
        let event = self.observe_supported_version(version);
        self.emit_telemetry(session_id, event, telemetry)
    }

    pub fn observe_timeout_with_telemetry(
        &mut self,
        session_id: u64,
        telemetry: &mut NegotiationTelemetry,
    ) -> NegotiationTelemetryEvent {
        let event = self.observe_timeout();
        self.emit_telemetry(session_id, event, telemetry)
    }

    pub fn observe_malformed_control_with_telemetry(
        &mut self,
        session_id: u64,
        telemetry: &mut NegotiationTelemetry,
    ) -> NegotiationTelemetryEvent {
        let event = self.observe_malformed_control();
        self.emit_telemetry(session_id, event, telemetry)
    }

    pub fn observe_unsupported_version_with_telemetry(
        &mut self,
        session_id: u64,
        telemetry: &mut NegotiationTelemetry,
    ) -> NegotiationTelemetryEvent {
        let event = self.observe_unsupported_version();
        self.emit_telemetry(session_id, event, telemetry)
    }

    pub fn observe_policy_denied_with_telemetry(
        &mut self,
        session_id: u64,
        telemetry: &mut NegotiationTelemetry,
    ) -> NegotiationTelemetryEvent {
        let event = self.observe_policy_denied();
        self.emit_telemetry(session_id, event, telemetry)
    }

    pub fn observe_control_frame_with_telemetry(
        &mut self,
        session_id: u64,
        frame: &[u8],
        telemetry: &mut NegotiationTelemetry,
    ) -> NegotiationTelemetryEvent {
        let event = self.observe_control_frame(frame);
        self.emit_telemetry(session_id, event, telemetry)
    }

    fn emit_telemetry(
        &self,
        session_id: u64,
        event: NegotiationEvent,
        telemetry: &mut NegotiationTelemetry,
    ) -> NegotiationTelemetryEvent {
        telemetry.emit(session_id, self.state, event)
    }

    fn resolve_downgrade_or_terminate(&mut self, reason: NegotiationReason) -> NegotiationEvent {
        if self.state != NegotiationState::AwaitingResponse {
            return NegotiationEvent::no_change();
        }

        match self.policy {
            DowngradePolicy::FailOpen => {
                self.state = NegotiationState::LegacyXml;
                NegotiationEvent {
                    kind: NegotiationEventKind::FallbackToLegacy,
                    reason: Some(reason),
                }
            }
            DowngradePolicy::FailClosed => {
                self.state = NegotiationState::Terminated { reason };
                NegotiationEvent {
                    kind: NegotiationEventKind::Terminated,
                    reason: Some(reason),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::negotiation::events::{NegotiationTelemetry, NEGOTIATION_TELEMETRY_CHANNEL};
    use crate::negotiation::{
        NegotiationEventKind, NegotiationReason, NegotiationState, Negotiator, TakProtocolVersion,
    };
    use crate::DowngradePolicy;

    #[test]
    fn fail_open_timeout_falls_back_to_legacy() {
        let mut negotiator = Negotiator::new(DowngradePolicy::FailOpen);
        negotiator.begin_upgrade_attempt();

        let event = negotiator.observe_timeout();
        assert_eq!(event.kind, NegotiationEventKind::FallbackToLegacy);
        assert_eq!(event.reason, Some(NegotiationReason::Timeout));
        assert_eq!(negotiator.state(), NegotiationState::LegacyXml);
    }

    #[test]
    fn fail_closed_timeout_terminates() {
        let mut negotiator = Negotiator::new(DowngradePolicy::FailClosed);
        negotiator.begin_upgrade_attempt();

        let event = negotiator.observe_timeout();
        assert_eq!(event.kind, NegotiationEventKind::Terminated);
        assert_eq!(event.reason, Some(NegotiationReason::Timeout));
        assert_eq!(
            negotiator.state(),
            NegotiationState::Terminated {
                reason: NegotiationReason::Timeout
            }
        );
    }

    #[test]
    fn malformed_control_uses_policy_path() {
        let mut fail_open = Negotiator::new(DowngradePolicy::FailOpen);
        fail_open.begin_upgrade_attempt();
        let open_event = fail_open.observe_malformed_control();
        assert_eq!(open_event.kind, NegotiationEventKind::FallbackToLegacy);
        assert_eq!(open_event.reason, Some(NegotiationReason::MalformedControl));
        assert_eq!(fail_open.state(), NegotiationState::LegacyXml);

        let mut fail_closed = Negotiator::new(DowngradePolicy::FailClosed);
        fail_closed.begin_upgrade_attempt();
        let closed_event = fail_closed.observe_malformed_control();
        assert_eq!(closed_event.kind, NegotiationEventKind::Terminated);
        assert_eq!(
            closed_event.reason,
            Some(NegotiationReason::MalformedControl)
        );
        assert_eq!(
            fail_closed.state(),
            NegotiationState::Terminated {
                reason: NegotiationReason::MalformedControl
            }
        );
    }

    #[test]
    fn unsupported_version_uses_policy_path() {
        let mut fail_open = Negotiator::new(DowngradePolicy::FailOpen);
        fail_open.begin_upgrade_attempt();
        let open_event = fail_open.observe_unsupported_version();
        assert_eq!(open_event.kind, NegotiationEventKind::FallbackToLegacy);
        assert_eq!(
            open_event.reason,
            Some(NegotiationReason::UnsupportedVersion)
        );
        assert_eq!(fail_open.state(), NegotiationState::LegacyXml);

        let mut fail_closed = Negotiator::new(DowngradePolicy::FailClosed);
        fail_closed.begin_upgrade_attempt();
        let closed_event = fail_closed.observe_unsupported_version();
        assert_eq!(closed_event.kind, NegotiationEventKind::Terminated);
        assert_eq!(
            closed_event.reason,
            Some(NegotiationReason::UnsupportedVersion)
        );
        assert_eq!(
            fail_closed.state(),
            NegotiationState::Terminated {
                reason: NegotiationReason::UnsupportedVersion
            }
        );
    }

    #[test]
    fn supported_version_upgrades_when_waiting() {
        let mut negotiator = Negotiator::new(DowngradePolicy::FailClosed);
        negotiator.begin_upgrade_attempt();

        let event = negotiator.observe_supported_version(TakProtocolVersion::V1);
        assert_eq!(event.kind, NegotiationEventKind::UpgradeAccepted);
        assert_eq!(event.reason, None);
        assert_eq!(
            negotiator.state(),
            NegotiationState::Upgraded(TakProtocolVersion::V1)
        );
    }

    #[test]
    fn policy_denied_terminates_from_any_non_terminated_state() {
        let mut negotiator = Negotiator::new(DowngradePolicy::FailOpen);
        let event = negotiator.observe_policy_denied();
        assert_eq!(event.kind, NegotiationEventKind::Terminated);
        assert_eq!(event.reason, Some(NegotiationReason::PolicyDenied));
        assert_eq!(
            negotiator.state(),
            NegotiationState::Terminated {
                reason: NegotiationReason::PolicyDenied
            }
        );
    }

    #[test]
    fn non_awaiting_downgrade_events_are_noops() {
        let mut negotiator = Negotiator::new(DowngradePolicy::FailClosed);

        let event = negotiator.observe_timeout();
        assert_eq!(event.kind, NegotiationEventKind::NoChange);
        assert_eq!(event.reason, None);
        assert_eq!(negotiator.state(), NegotiationState::LegacyXml);
    }

    #[test]
    fn control_frame_observation_classifies_supported_unsupported_and_malformed() {
        let mut supported = Negotiator::new(DowngradePolicy::FailClosed);
        supported.begin_upgrade_attempt();
        let upgraded = supported.observe_control_frame(b"V\x01");
        assert_eq!(upgraded.kind, NegotiationEventKind::UpgradeAccepted);
        assert_eq!(
            supported.state(),
            NegotiationState::Upgraded(TakProtocolVersion::V1)
        );

        let mut unsupported = Negotiator::new(DowngradePolicy::FailOpen);
        unsupported.begin_upgrade_attempt();
        let fallback = unsupported.observe_control_frame(b"V\x02");
        assert_eq!(fallback.kind, NegotiationEventKind::FallbackToLegacy);
        assert_eq!(fallback.reason, Some(NegotiationReason::UnsupportedVersion));

        let mut malformed = Negotiator::new(DowngradePolicy::FailClosed);
        malformed.begin_upgrade_attempt();
        let terminated = malformed.observe_control_frame(b"X\x01");
        assert_eq!(terminated.kind, NegotiationEventKind::Terminated);
        assert_eq!(terminated.reason, Some(NegotiationReason::MalformedControl));
    }

    #[test]
    fn telemetry_emitters_link_events_to_session_and_state() {
        let mut negotiator = Negotiator::new(DowngradePolicy::FailOpen);
        let mut telemetry = NegotiationTelemetry::default();

        let begin = negotiator.begin_upgrade_attempt_with_telemetry(41, &mut telemetry);
        assert_eq!(begin.session_id, 41);
        assert_eq!(begin.sequence, 0);
        assert_eq!(begin.state, NegotiationState::AwaitingResponse);
        assert_eq!(begin.event.kind, NegotiationEventKind::NoChange);

        let fallback =
            negotiator.observe_control_frame_with_telemetry(41, b"V\x02", &mut telemetry);
        assert_eq!(fallback.sequence, 1);
        assert_eq!(fallback.state, NegotiationState::LegacyXml);
        assert_eq!(fallback.event.kind, NegotiationEventKind::FallbackToLegacy);
        assert_eq!(
            fallback.event.reason,
            Some(NegotiationReason::UnsupportedVersion)
        );

        let payload = fallback.encode_record_payload();
        let decoded =
            crate::negotiation::events::NegotiationTelemetryEvent::decode_record_payload(&payload)
                .expect("payload should decode");
        assert_eq!(decoded, fallback);

        assert_eq!(NEGOTIATION_TELEMETRY_CHANNEL, "wire.negotiation.v1");
        assert_eq!(telemetry.events().len(), 2);
    }
}
