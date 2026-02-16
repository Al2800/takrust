use thiserror::Error;

use super::{
    NegotiationEvent, NegotiationEventKind, NegotiationReason, NegotiationState, TakProtocolVersion,
};

pub const NEGOTIATION_TELEMETRY_CHANNEL: &str = "wire.negotiation.v1";
pub const CONTROL_FRAME_VERSION_MARKER: u8 = b'V';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NegotiationTelemetryEvent {
    pub session_id: u64,
    pub sequence: u64,
    pub state: NegotiationState,
    pub event: NegotiationEvent,
}

impl NegotiationTelemetryEvent {
    #[must_use]
    pub fn encode_record_payload(self) -> Vec<u8> {
        let reason = self
            .event
            .reason
            .map(reason_code)
            .unwrap_or_else(|| "none".to_string());

        format!(
            "session={};sequence={};state={};kind={};reason={}",
            self.session_id,
            self.sequence,
            state_code(self.state),
            event_kind_code(self.event.kind),
            reason
        )
        .into_bytes()
    }

    pub fn decode_record_payload(payload: &[u8]) -> Result<Self, TelemetryDecodeError> {
        let text = std::str::from_utf8(payload).map_err(TelemetryDecodeError::InvalidUtf8)?;
        let mut session_id = None;
        let mut sequence = None;
        let mut state = None;
        let mut kind = None;
        let mut reason = None;

        for field in text.split(';') {
            let (key, value) =
                field
                    .split_once('=')
                    .ok_or_else(|| TelemetryDecodeError::MalformedField {
                        field: field.to_string(),
                    })?;
            match key {
                "session" => {
                    session_id = Some(value.parse::<u64>().map_err(|_| {
                        TelemetryDecodeError::InvalidNumber {
                            field: "session".to_string(),
                            value: value.to_string(),
                        }
                    })?);
                }
                "sequence" => {
                    sequence = Some(value.parse::<u64>().map_err(|_| {
                        TelemetryDecodeError::InvalidNumber {
                            field: "sequence".to_string(),
                            value: value.to_string(),
                        }
                    })?);
                }
                "state" => {
                    state = Some(parse_state_code(value)?);
                }
                "kind" => {
                    kind = Some(parse_event_kind(value)?);
                }
                "reason" => {
                    reason = Some(parse_reason(value)?);
                }
                _ => {}
            }
        }

        let session_id = session_id.ok_or_else(|| TelemetryDecodeError::MissingField {
            field: "session".to_string(),
        })?;
        let sequence = sequence.ok_or_else(|| TelemetryDecodeError::MissingField {
            field: "sequence".to_string(),
        })?;
        let state = state.ok_or_else(|| TelemetryDecodeError::MissingField {
            field: "state".to_string(),
        })?;
        let kind = kind.ok_or_else(|| TelemetryDecodeError::MissingField {
            field: "kind".to_string(),
        })?;
        let reason = reason.ok_or_else(|| TelemetryDecodeError::MissingField {
            field: "reason".to_string(),
        })?;

        Ok(Self {
            session_id,
            sequence,
            state,
            event: NegotiationEvent { kind, reason },
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NegotiationTelemetry {
    next_sequence: u64,
    events: Vec<NegotiationTelemetryEvent>,
}

impl NegotiationTelemetry {
    pub fn emit(
        &mut self,
        session_id: u64,
        state: NegotiationState,
        event: NegotiationEvent,
    ) -> NegotiationTelemetryEvent {
        let emitted = NegotiationTelemetryEvent {
            session_id,
            sequence: self.next_sequence,
            state,
            event,
        };
        self.next_sequence += 1;
        self.events.push(emitted);
        emitted
    }

    #[must_use]
    pub fn events(&self) -> &[NegotiationTelemetryEvent] {
        &self.events
    }

    pub fn drain(&mut self) -> Vec<NegotiationTelemetryEvent> {
        std::mem::take(&mut self.events)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TelemetryDecodeError {
    #[error("telemetry payload is not valid UTF-8")]
    InvalidUtf8(#[from] std::str::Utf8Error),

    #[error("malformed telemetry field `{field}`")]
    MalformedField { field: String },

    #[error("missing telemetry field `{field}`")]
    MissingField { field: String },

    #[error("invalid numeric field `{field}` value `{value}`")]
    InvalidNumber { field: String, value: String },

    #[error("unknown negotiation state code `{code}`")]
    UnknownState { code: String },

    #[error("unknown negotiation event kind code `{code}`")]
    UnknownKind { code: String },

    #[error("unknown negotiation reason code `{code}`")]
    UnknownReason { code: String },
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ControlFrameError {
    #[error("control frame is empty")]
    EmptyFrame,

    #[error("control frame marker `{marker:#04x}` is invalid")]
    InvalidMarker { marker: u8 },

    #[error("control frame missing version byte")]
    MissingVersion,

    #[error("unsupported TAK version `{version}`")]
    UnsupportedVersion { version: u8 },
}

pub fn parse_control_frame(frame: &[u8]) -> Result<TakProtocolVersion, ControlFrameError> {
    let marker = *frame.first().ok_or(ControlFrameError::EmptyFrame)?;
    if marker != CONTROL_FRAME_VERSION_MARKER {
        return Err(ControlFrameError::InvalidMarker { marker });
    }

    let version = *frame.get(1).ok_or(ControlFrameError::MissingVersion)?;
    match version {
        1 => Ok(TakProtocolVersion::V1),
        _ => Err(ControlFrameError::UnsupportedVersion { version }),
    }
}

fn state_code(state: NegotiationState) -> String {
    match state {
        NegotiationState::LegacyXml => "legacy_xml".to_string(),
        NegotiationState::AwaitingResponse => "awaiting_response".to_string(),
        NegotiationState::Upgraded(TakProtocolVersion::V1) => "upgraded:v1".to_string(),
        NegotiationState::Terminated { reason } => format!("terminated:{}", reason_code(reason)),
    }
}

fn parse_state_code(value: &str) -> Result<NegotiationState, TelemetryDecodeError> {
    if value == "legacy_xml" {
        return Ok(NegotiationState::LegacyXml);
    }
    if value == "awaiting_response" {
        return Ok(NegotiationState::AwaitingResponse);
    }
    if value == "upgraded:v1" {
        return Ok(NegotiationState::Upgraded(TakProtocolVersion::V1));
    }
    if let Some(reason) = value.strip_prefix("terminated:") {
        return Ok(NegotiationState::Terminated {
            reason: parse_reason_code(reason)?,
        });
    }

    Err(TelemetryDecodeError::UnknownState {
        code: value.to_string(),
    })
}

fn event_kind_code(kind: NegotiationEventKind) -> &'static str {
    match kind {
        NegotiationEventKind::NoChange => "no_change",
        NegotiationEventKind::UpgradeAccepted => "upgrade_accepted",
        NegotiationEventKind::FallbackToLegacy => "fallback_to_legacy",
        NegotiationEventKind::Terminated => "terminated",
    }
}

fn parse_event_kind(value: &str) -> Result<NegotiationEventKind, TelemetryDecodeError> {
    match value {
        "no_change" => Ok(NegotiationEventKind::NoChange),
        "upgrade_accepted" => Ok(NegotiationEventKind::UpgradeAccepted),
        "fallback_to_legacy" => Ok(NegotiationEventKind::FallbackToLegacy),
        "terminated" => Ok(NegotiationEventKind::Terminated),
        _ => Err(TelemetryDecodeError::UnknownKind {
            code: value.to_string(),
        }),
    }
}

fn reason_code(reason: NegotiationReason) -> String {
    match reason {
        NegotiationReason::Timeout => "timeout".to_string(),
        NegotiationReason::MalformedControl => "malformed_control".to_string(),
        NegotiationReason::UnsupportedVersion => "unsupported_version".to_string(),
        NegotiationReason::PolicyDenied => "policy_denied".to_string(),
    }
}

fn parse_reason(value: &str) -> Result<Option<NegotiationReason>, TelemetryDecodeError> {
    if value == "none" {
        return Ok(None);
    }
    parse_reason_code(value).map(Some)
}

fn parse_reason_code(value: &str) -> Result<NegotiationReason, TelemetryDecodeError> {
    match value {
        "timeout" => Ok(NegotiationReason::Timeout),
        "malformed_control" => Ok(NegotiationReason::MalformedControl),
        "unsupported_version" => Ok(NegotiationReason::UnsupportedVersion),
        "policy_denied" => Ok(NegotiationReason::PolicyDenied),
        _ => Err(TelemetryDecodeError::UnknownReason {
            code: value.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_control_frame, NegotiationTelemetry, NegotiationTelemetryEvent, TelemetryDecodeError,
        CONTROL_FRAME_VERSION_MARKER, NEGOTIATION_TELEMETRY_CHANNEL,
    };
    use crate::negotiation::{
        NegotiationEvent, NegotiationEventKind, NegotiationReason, NegotiationState,
        TakProtocolVersion,
    };

    #[test]
    fn control_frame_parsing_handles_supported_and_malformed_inputs() {
        assert_eq!(
            parse_control_frame(&[CONTROL_FRAME_VERSION_MARKER, 1]),
            Ok(TakProtocolVersion::V1)
        );
        assert!(parse_control_frame(&[]).is_err());
        assert!(parse_control_frame(&[CONTROL_FRAME_VERSION_MARKER]).is_err());
        assert!(parse_control_frame(&[CONTROL_FRAME_VERSION_MARKER, 9]).is_err());
        assert!(parse_control_frame(&[0x00, 1]).is_err());
    }

    #[test]
    fn telemetry_buffer_assigns_monotonic_sequence_numbers() {
        let mut telemetry = NegotiationTelemetry::default();

        let first = telemetry.emit(
            77,
            NegotiationState::AwaitingResponse,
            NegotiationEvent::no_change(),
        );
        let second = telemetry.emit(
            77,
            NegotiationState::LegacyXml,
            NegotiationEvent {
                kind: NegotiationEventKind::FallbackToLegacy,
                reason: Some(NegotiationReason::MalformedControl),
            },
        );

        assert_eq!(first.sequence, 0);
        assert_eq!(second.sequence, 1);
        assert_eq!(telemetry.events().len(), 2);
        assert_eq!(NEGOTIATION_TELEMETRY_CHANNEL, "wire.negotiation.v1");
    }

    #[test]
    fn telemetry_payload_round_trips() {
        let event = NegotiationTelemetryEvent {
            session_id: 123,
            sequence: 9,
            state: NegotiationState::Terminated {
                reason: NegotiationReason::Timeout,
            },
            event: NegotiationEvent {
                kind: NegotiationEventKind::Terminated,
                reason: Some(NegotiationReason::Timeout),
            },
        };

        let payload = event.encode_record_payload();
        let decoded =
            NegotiationTelemetryEvent::decode_record_payload(&payload).expect("decode should pass");

        assert_eq!(decoded, event);
    }

    #[test]
    fn telemetry_decode_rejects_unknown_state_codes() {
        let payload = b"session=1;sequence=0;state=unknown;kind=no_change;reason=none".to_vec();
        let error =
            NegotiationTelemetryEvent::decode_record_payload(&payload).expect_err("must fail");

        assert!(matches!(error, TelemetryDecodeError::UnknownState { .. }));
    }
}
