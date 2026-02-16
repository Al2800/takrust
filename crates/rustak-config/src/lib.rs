use std::time::Duration;

use rustak_limits::{Limits, LimitsError};
use rustak_sapient::{SapientConfig, SapientConfigError};
use rustak_transport::{TransportConfig, TransportConfigError};
use thiserror::Error;

/// Top-level typed configuration contract.
#[derive(Debug, Clone, PartialEq)]
pub struct RustakConfig {
    pub transport: TransportConfig,
    pub sapient: Option<SapientConfigSpec>,
}

impl RustakConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.transport.validate()?;
        if let Some(sapient) = self.resolve_sapient()? {
            sapient.validate()?;
        }
        Ok(())
    }

    pub fn resolve_sapient(&self) -> Result<Option<SapientConfig>, ConfigError> {
        self.sapient
            .as_ref()
            .map(|spec| spec.resolve(&self.transport.limits))
            .transpose()
    }
}

/// SAPIENT config may use inline limits or reference transport limits by path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SapientConfigSpec {
    pub version: String,
    pub limits: LimitsBinding,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub tcp_nodelay: bool,
}

impl SapientConfigSpec {
    pub fn resolve(&self, transport_limits: &Limits) -> Result<SapientConfig, ConfigError> {
        let limits = match &self.limits {
            LimitsBinding::Inline(limits) => limits.clone(),
            LimitsBinding::Reference(reference) => reference.resolve(transport_limits)?,
        };

        let config = SapientConfig {
            limits,
            read_timeout: self.read_timeout,
            write_timeout: self.write_timeout,
            tcp_nodelay: self.tcp_nodelay,
        };
        config.validate()?;
        Ok(config)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LimitsBinding {
    Inline(Limits),
    Reference(LimitsRef),
}

/// Explicit reference path for sharing limits between config sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimitsRef {
    path: String,
}

impl LimitsRef {
    pub fn new(path: impl Into<String>) -> Result<Self, ConfigError> {
        let path = path.into();
        if path.trim().is_empty() {
            return Err(ConfigError::EmptyLimitsReferencePath);
        }
        Ok(Self { path })
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    fn resolve(&self, transport_limits: &Limits) -> Result<Limits, ConfigError> {
        if self.path == "transport.limits" {
            return Ok(transport_limits.clone());
        }

        Err(ConfigError::UnknownLimitsReference {
            reference: self.path.clone(),
        })
    }
}

/// Migration helper for legacy ad-hoc size knobs.
///
/// Deprecated/duplicate boundary-size fields can be mapped into the canonical `Limits`
/// object with this struct, instead of keeping independent knobs on runtime configs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LegacyTransportSizeKnobs {
    pub max_frame_bytes: Option<usize>,
    pub max_xml_scan_bytes: Option<usize>,
    pub max_protobuf_bytes: Option<usize>,
    pub max_queue_messages: Option<usize>,
    pub max_queue_bytes: Option<usize>,
    pub max_detail_elements: Option<usize>,
}

impl LegacyTransportSizeKnobs {
    pub fn apply_to(self, mut limits: Limits) -> Limits {
        if let Some(value) = self.max_frame_bytes {
            limits.max_frame_bytes = value;
        }
        if let Some(value) = self.max_xml_scan_bytes {
            limits.max_xml_scan_bytes = value;
        }
        if let Some(value) = self.max_protobuf_bytes {
            limits.max_protobuf_bytes = value;
        }
        if let Some(value) = self.max_queue_messages {
            limits.max_queue_messages = value;
        }
        if let Some(value) = self.max_queue_bytes {
            limits.max_queue_bytes = value;
        }
        if let Some(value) = self.max_detail_elements {
            limits.max_detail_elements = value;
        }
        limits
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error(transparent)]
    InvalidTransport(#[from] TransportConfigError),

    #[error(transparent)]
    InvalidSapient(#[from] SapientConfigError),

    #[error(transparent)]
    InvalidLimits(#[from] LimitsError),

    #[error("limits reference path must not be empty")]
    EmptyLimitsReferencePath,

    #[error("unsupported limits reference: {reference}")]
    UnknownLimitsReference { reference: String },
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rustak_limits::Limits;
    use rustak_transport::TransportConfig;

    use crate::{
        ConfigError, LegacyTransportSizeKnobs, LimitsBinding, LimitsRef, RustakConfig,
        SapientConfigSpec,
    };

    #[test]
    fn resolves_sapient_reference_to_transport_limits() {
        let transport = TransportConfig::default();
        let config = RustakConfig {
            sapient: Some(SapientConfigSpec {
                version: "bsi_flex_335_v2_0".to_owned(),
                limits: LimitsBinding::Reference(
                    LimitsRef::new("transport.limits").expect("valid limits ref"),
                ),
                read_timeout: Duration::from_secs(15),
                write_timeout: Duration::from_secs(15),
                tcp_nodelay: true,
            }),
            transport: transport.clone(),
        };

        let resolved = config
            .resolve_sapient()
            .expect("reference should resolve")
            .expect("sapient config should exist");
        assert_eq!(resolved.limits, transport.limits);
    }

    #[test]
    fn rejects_unknown_limits_reference() {
        let config = RustakConfig {
            sapient: Some(SapientConfigSpec {
                version: "bsi_flex_335_v2_0".to_owned(),
                limits: LimitsBinding::Reference(
                    LimitsRef::new("transport.old_limits").expect("non-empty path"),
                ),
                read_timeout: Duration::from_secs(15),
                write_timeout: Duration::from_secs(15),
                tcp_nodelay: true,
            }),
            transport: TransportConfig::default(),
        };

        let error = config
            .resolve_sapient()
            .expect_err("unknown reference should fail");

        match error {
            ConfigError::UnknownLimitsReference { reference } => {
                assert_eq!(reference, "transport.old_limits")
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn legacy_knobs_map_into_limits() {
        let base = Limits::conservative_defaults();
        let migrated = LegacyTransportSizeKnobs {
            max_frame_bytes: Some(512 * 1024),
            max_xml_scan_bytes: Some(512 * 1024),
            max_protobuf_bytes: Some(512 * 1024),
            max_queue_bytes: Some(4 * 1024 * 1024),
            max_queue_messages: Some(256),
            ..LegacyTransportSizeKnobs::default()
        }
        .apply_to(base);

        assert_eq!(migrated.max_frame_bytes, 512 * 1024);
        assert_eq!(migrated.max_queue_bytes, 4 * 1024 * 1024);
        assert_eq!(migrated.max_queue_messages, 256);
        assert!(migrated.validate().is_ok());
    }
}
