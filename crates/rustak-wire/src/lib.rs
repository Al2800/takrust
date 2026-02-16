use std::time::Duration;

use rustak_limits::{Limits, LimitsError};
use rustak_proto::ProtoError;
use thiserror::Error;

pub mod framing;
pub mod negotiation;

pub use framing::{WireFrameCodec, WireFrameError, LEGACY_XML_DELIMITER};
pub use negotiation::{
    NegotiationEvent, NegotiationEventKind, NegotiationReason, NegotiationState, Negotiator,
    TakProtocolVersion,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireFormat {
    Xml,
    TakProtocolV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DowngradePolicy {
    FailOpen,
    FailClosed,
}

pub fn encode_payload_for_format(
    payload: &[u8],
    format: WireFormat,
) -> Result<Vec<u8>, WirePayloadError> {
    if payload.is_empty() {
        return Err(WirePayloadError::EmptyPayload);
    }

    match format {
        WireFormat::Xml => Ok(payload.to_vec()),
        WireFormat::TakProtocolV1 => rustak_proto::encode_v1_payload(payload).map_err(Into::into),
    }
}

pub fn decode_payload_for_format(
    payload: &[u8],
    format: WireFormat,
) -> Result<Vec<u8>, WirePayloadError> {
    if payload.is_empty() {
        return Err(WirePayloadError::EmptyPayload);
    }

    match format {
        WireFormat::Xml => Ok(payload.to_vec()),
        WireFormat::TakProtocolV1 => rustak_proto::decode_v1_payload(payload).map_err(Into::into),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegotiationConfig {
    pub streaming_timeout: Duration,
    pub mesh_takcontrol_interval: Duration,
    pub mesh_contact_stale_after: Duration,
    pub downgrade_policy: DowngradePolicy,
}

impl Default for NegotiationConfig {
    fn default() -> Self {
        Self {
            streaming_timeout: Duration::from_secs(60),
            mesh_takcontrol_interval: Duration::from_secs(60),
            mesh_contact_stale_after: Duration::from_secs(120),
            downgrade_policy: DowngradePolicy::FailClosed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireConfig {
    pub limits: Limits,
    pub negotiation: NegotiationConfig,
}

impl WireConfig {
    pub fn validate(&self) -> Result<(), WireConfigError> {
        self.limits.validate()?;

        ensure_non_zero_duration("streaming_timeout", self.negotiation.streaming_timeout)?;
        ensure_non_zero_duration(
            "mesh_takcontrol_interval",
            self.negotiation.mesh_takcontrol_interval,
        )?;
        ensure_non_zero_duration(
            "mesh_contact_stale_after",
            self.negotiation.mesh_contact_stale_after,
        )?;

        if self.negotiation.mesh_contact_stale_after < self.negotiation.mesh_takcontrol_interval {
            return Err(WireConfigError::MeshStaleBeforeCadence {
                mesh_contact_stale_after: self.negotiation.mesh_contact_stale_after,
                mesh_takcontrol_interval: self.negotiation.mesh_takcontrol_interval,
            });
        }

        Ok(())
    }

    #[must_use]
    pub fn frame_codec(&self, format: WireFormat) -> WireFrameCodec {
        WireFrameCodec::from_limits(format, &self.limits)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WireConfigError {
    #[error(transparent)]
    InvalidLimits(#[from] LimitsError),

    #[error("{field} must be greater than zero")]
    ZeroDuration { field: &'static str },

    #[error(
        "mesh_contact_stale_after ({mesh_contact_stale_after:?}) must be >= \
         mesh_takcontrol_interval ({mesh_takcontrol_interval:?})"
    )]
    MeshStaleBeforeCadence {
        mesh_contact_stale_after: Duration,
        mesh_takcontrol_interval: Duration,
    },
}

#[derive(Debug, Error)]
pub enum WirePayloadError {
    #[error(transparent)]
    Proto(#[from] ProtoError),

    #[error("payload must not be empty")]
    EmptyPayload,
}

fn ensure_non_zero_duration(field: &'static str, value: Duration) -> Result<(), WireConfigError> {
    if value.is_zero() {
        return Err(WireConfigError::ZeroDuration { field });
    }

    Ok(())
}

#[doc(hidden)]
pub fn fuzz_hook_validate_wire_config(data: &[u8]) -> Result<(), WireConfigError> {
    let mut config = WireConfig::default();
    config.limits.max_frame_bytes = word_at(data, 0);
    config.limits.max_xml_scan_bytes = word_at(data, 2);
    config.limits.max_protobuf_bytes = word_at(data, 4);
    config.limits.max_queue_messages = word_at(data, 6);
    config.limits.max_queue_bytes = word_at(data, 8);
    config.limits.max_detail_elements = word_at(data, 10);
    config.negotiation.streaming_timeout = Duration::from_millis(word_at(data, 12) as u64);
    config.negotiation.mesh_takcontrol_interval = Duration::from_millis(word_at(data, 14) as u64);
    config.negotiation.mesh_contact_stale_after = Duration::from_millis(word_at(data, 16) as u64);
    config.validate()
}

fn word_at(data: &[u8], offset: usize) -> usize {
    let bytes = [
        data.get(offset).copied().unwrap_or_default(),
        data.get(offset + 1).copied().unwrap_or_default(),
    ];
    usize::from(u16::from_be_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{
        decode_payload_for_format, encode_payload_for_format, WireConfig, WireConfigError,
        WireFormat, WirePayloadError,
    };

    #[test]
    fn defaults_validate() {
        let cfg = WireConfig::default();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn rejects_zero_streaming_timeout() {
        let mut cfg = WireConfig::default();
        cfg.negotiation.streaming_timeout = Duration::ZERO;

        assert_eq!(
            cfg.validate(),
            Err(WireConfigError::ZeroDuration {
                field: "streaming_timeout"
            })
        );
    }

    #[test]
    fn rejects_stale_before_interval() {
        let mut cfg = WireConfig::default();
        cfg.negotiation.mesh_takcontrol_interval = Duration::from_secs(30);
        cfg.negotiation.mesh_contact_stale_after = Duration::from_secs(10);

        assert_eq!(
            cfg.validate(),
            Err(WireConfigError::MeshStaleBeforeCadence {
                mesh_contact_stale_after: Duration::from_secs(10),
                mesh_takcontrol_interval: Duration::from_secs(30),
            })
        );
    }

    #[test]
    fn fuzz_hook_handles_arbitrary_bytes_without_panicking() {
        let corpus = [
            &[][..],
            &[0u8; 1][..],
            &[0u8; 4][..],
            &[255u8; 32][..],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9][..],
        ];

        for sample in corpus {
            let _ = super::fuzz_hook_validate_wire_config(sample);
        }
    }

    #[test]
    fn xml_payload_path_is_passthrough() {
        let payload = b"<event uid=\"xml\"/>".to_vec();
        let encoded = encode_payload_for_format(&payload, WireFormat::Xml).expect("encode");
        let decoded = decode_payload_for_format(&encoded, WireFormat::Xml).expect("decode");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn tak_v1_payload_routes_through_proto_codec() {
        let payload = b"<event uid=\"tak-v1\"/>".to_vec();
        let encoded =
            encode_payload_for_format(&payload, WireFormat::TakProtocolV1).expect("encode");
        let decoded =
            decode_payload_for_format(&encoded, WireFormat::TakProtocolV1).expect("decode");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn empty_payload_is_rejected_for_all_formats() {
        let xml_error = encode_payload_for_format(&[], WireFormat::Xml).expect_err("xml empty");
        assert!(matches!(xml_error, WirePayloadError::EmptyPayload));

        let tak_error =
            decode_payload_for_format(&[], WireFormat::TakProtocolV1).expect_err("tak empty");
        assert!(matches!(tak_error, WirePayloadError::EmptyPayload));
    }
}
