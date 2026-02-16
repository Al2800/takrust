use std::{io::Read, path::Path, time::Duration};

use rustak_bridge::{BridgeConfig, BridgeConfigError};
use rustak_limits::{Limits, LimitsError};
use rustak_sapient::{SapientConfig, SapientConfigError};
use rustak_transport::{TransportConfig, TransportConfigError};
use thiserror::Error;

mod redact;
mod schema;
mod validate;

pub use schema::json_schema;

#[derive(Debug, Clone, PartialEq)]
pub struct RustakConfig {
    pub transport: TransportConfig,
    pub sapient: Option<SapientConfigSpec>,
    pub bridge: Option<BridgeConfig>,
    pub crypto: Option<CryptoConfig>,
    pub certificates: Option<CertificatesConfig>,
    pub logging: Option<LoggingConfig>,
}

impl Default for RustakConfig {
    fn default() -> Self {
        Self {
            transport: TransportConfig::default(),
            sapient: None,
            bridge: None,
            crypto: None,
            certificates: None,
            logging: Some(LoggingConfig::default()),
        }
    }
}

impl RustakConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path_ref = path.as_ref();
        let file = std::fs::File::open(path_ref).map_err(|source| ConfigError::ReadConfig {
            path: path_ref.display().to_string(),
            source,
        })?;
        Self::from_reader(file)
    }

    pub fn from_reader(reader: impl Read) -> Result<Self, ConfigError> {
        let document: schema::RustakConfigDocument =
            serde_yaml::from_reader(reader).map_err(ConfigError::DeserializeConfig)?;
        let config = Self::try_from(document)?;
        config.validate()?;
        Ok(config)
    }

    pub fn from_yaml_str(yaml: &str) -> Result<Self, ConfigError> {
        let document: schema::RustakConfigDocument =
            serde_yaml::from_str(yaml).map_err(ConfigError::DeserializeConfig)?;
        let config = Self::try_from(document)?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        self.transport.validate()?;
        if let Some(sapient) = self.resolve_sapient()? {
            sapient.validate()?;
        }
        if let Some(bridge) = &self.bridge {
            bridge.validate()?;
        }

        if let Some(crypto) = &self.crypto {
            if let Some(pin) = &crypto.server_spki_pin {
                if pin.trim().is_empty() {
                    return Err(ConfigError::EmptySensitiveField {
                        field: "crypto.server_spki_pin",
                    });
                }
            }
        }

        if let Some(certificates) = &self.certificates {
            if certificates.ca_cert.trim().is_empty() {
                return Err(ConfigError::EmptyField {
                    field: "certificates.ca_cert",
                });
            }
            if certificates.client_cert.trim().is_empty() {
                return Err(ConfigError::EmptyField {
                    field: "certificates.client_cert",
                });
            }
            if certificates.client_key.trim().is_empty() {
                return Err(ConfigError::EmptySensitiveField {
                    field: "certificates.client_key",
                });
            }
        }

        if let Some(logging) = &self.logging {
            for path in &logging.redact {
                if path.trim().is_empty() {
                    return Err(ConfigError::EmptyField {
                        field: "logging.redact[]",
                    });
                }
            }
        }

        Ok(())
    }

    pub fn validate_startup(&self) -> Result<(), ConfigError> {
        validate::validate_startup(self)
    }

    pub fn resolve_sapient(&self) -> Result<Option<SapientConfig>, ConfigError> {
        self.sapient
            .as_ref()
            .map(|spec| spec.resolve(&self.transport.limits))
            .transpose()
    }

    pub fn to_redacted_yaml(&self) -> Result<String, ConfigError> {
        redact::to_redacted_yaml(self)
    }

    #[must_use]
    pub fn json_schema() -> serde_json::Value {
        schema::json_schema()
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CryptoConfig {
    pub provider: CryptoProvider,
    pub revocation: RevocationPolicy,
    pub server_spki_pin: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoProvider {
    Ring,
    AwsLcRs,
    AwsLcRsFips,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevocationPolicy {
    Off,
    Prefer,
    Require,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificatesConfig {
    pub ca_cert: String,
    pub client_cert: String,
    pub client_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggingConfig {
    pub level: LogLevel,
    pub format: LogFormat,
    pub redact: Vec<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            format: LogFormat::Json,
            redact: vec![
                "certificates.client_key".to_owned(),
                "certificates.client_cert".to_owned(),
                "crypto.server_spki_pin".to_owned(),
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Json,
    Pretty,
    Compact,
}

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
    InvalidBridge(#[from] BridgeConfigError),

    #[error(transparent)]
    InvalidLimits(#[from] LimitsError),

    #[error("limits reference path must not be empty")]
    EmptyLimitsReferencePath,

    #[error("unsupported limits reference: {reference}")]
    UnknownLimitsReference { reference: String },

    #[error("failed to read config file {path}: {source}")]
    ReadConfig {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse config yaml: {0}")]
    DeserializeConfig(#[source] serde_yaml::Error),

    #[error("failed to render config yaml: {0}")]
    SerializeConfig(#[source] serde_yaml::Error),

    #[error("config field {field} must not be empty")]
    EmptyField { field: &'static str },

    #[error("sensitive config field {field} must not be empty")]
    EmptySensitiveField { field: &'static str },

    #[error("invalid socket address for {field}: {value}")]
    InvalidAddress { field: &'static str, value: String },

    #[error("invalid IPv4 address for {field}: {value}")]
    InvalidIpv4Address { field: &'static str, value: String },

    #[error("invalid duration for {field}: {value}")]
    InvalidDuration { field: &'static str, value: String },

    #[error("missing required config field: {field}")]
    MissingField { field: &'static str },

    #[error("conflicting config fields: {left} and {right}")]
    ConflictingFields {
        left: &'static str,
        right: &'static str,
    },

    #[error(
        "strict startup requires bridge.limits.max_frame_bytes ({bridge_max_frame_bytes}) <= transport.limits.max_frame_bytes ({transport_max_frame_bytes})"
    )]
    StrictStartupBridgeFrameLimitExceedsTransport {
        bridge_max_frame_bytes: usize,
        transport_max_frame_bytes: usize,
    },

    #[error(
        "strict startup requires bridge.emitter.max_pending_events ({bridge_pending_events}) <= transport.limits.max_queue_messages ({transport_max_queue_messages})"
    )]
    StrictStartupBridgePendingEventsExceedTransport {
        bridge_pending_events: usize,
        transport_max_queue_messages: usize,
    },
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::Duration};

    use rustak_bridge::BridgeConfig;
    use rustak_limits::Limits;
    use rustak_transport::TransportConfig;

    use crate::{
        ConfigError, CryptoConfig, CryptoProvider, LegacyTransportSizeKnobs, LimitsBinding,
        LimitsRef, LogFormat, LogLevel, LoggingConfig, RevocationPolicy, RustakConfig,
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
            ..RustakConfig::default()
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
            ..RustakConfig::default()
        };

        let error = config
            .resolve_sapient()
            .expect_err("unknown reference should fail");
        assert!(matches!(
            error,
            ConfigError::UnknownLimitsReference { reference }
            if reference == "transport.old_limits"
        ));
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

    #[test]
    fn loads_yaml_and_validates() {
        let yaml = r#"
transport:
  protocol:
    type: tcp
    addr: 127.0.0.1:8089
  wire_format: xml
  limits:
    max_frame_bytes: 1048576
    max_xml_scan_bytes: 1048576
    max_protobuf_bytes: 1048576
    max_queue_messages: 1024
    max_queue_bytes: 8388608
    max_detail_elements: 512
  read_timeout: 15s
  write_timeout: 15s
  keepalive:
    interval: 10s
    timeout: 3s
  reconnect:
    enabled: true
    initial_delay: 1s
    max_delay: 60s
    backoff_factor: 2.0
    jitter: 0.2
  send_queue:
    mode: coalesce_latest_by_uid
    max_messages: 1024
    max_bytes: 8388608
sapient:
  version: bsi_flex_335_v2_0
  limits_ref: transport.limits
  read_timeout: 15s
  write_timeout: 15s
  tcp_nodelay: true
"#;

        let config = RustakConfig::from_yaml_str(yaml).expect("yaml should parse");
        assert_eq!(config.transport.limits.max_frame_bytes, 1_048_576);
        assert!(config.resolve_sapient().is_ok());
    }

    #[test]
    fn redacts_sensitive_fields_in_rendered_yaml() {
        let config = RustakConfig {
            crypto: Some(CryptoConfig {
                provider: CryptoProvider::Ring,
                revocation: RevocationPolicy::Prefer,
                server_spki_pin: Some("super-secret-pin".to_owned()),
            }),
            certificates: Some(crate::CertificatesConfig {
                ca_cert: "/etc/rustak/ca.pem".to_owned(),
                client_cert: "/etc/rustak/client.pem".to_owned(),
                client_key: "/etc/rustak/client-key.pem".to_owned(),
            }),
            logging: Some(LoggingConfig {
                level: LogLevel::Info,
                format: LogFormat::Json,
                redact: vec!["crypto.server_spki_pin".to_owned()],
            }),
            ..RustakConfig::default()
        };

        let rendered = config
            .to_redacted_yaml()
            .expect("redacted render should work");
        assert!(rendered.contains("[REDACTED]"));
        assert!(!rendered.contains("super-secret-pin"));
        assert!(!rendered.contains("/etc/rustak/client-key.pem"));
    }

    #[test]
    fn schema_contains_top_level_transport() {
        let schema = RustakConfig::json_schema();
        let props = schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .expect("schema must contain properties");
        assert!(props.contains_key("transport"));
    }

    #[test]
    fn detects_conflicting_sapient_limits_inputs() {
        let yaml = r#"
transport:
  protocol:
    type: tcp
    addr: 127.0.0.1:8089
sapient:
  version: bsi_flex_335_v2_0
  limits_ref: transport.limits
  limits:
    max_frame_bytes: 1048576
    max_xml_scan_bytes: 1048576
    max_protobuf_bytes: 1048576
    max_queue_messages: 1024
    max_queue_bytes: 8388608
    max_detail_elements: 512
"#;

        let error = RustakConfig::from_yaml_str(yaml).expect_err("conflict should fail");
        assert!(matches!(
            error,
            ConfigError::ConflictingFields {
                left: "sapient.limits",
                right: "sapient.limits_ref"
            }
        ));
    }

    #[test]
    fn load_from_file_path() {
        let yaml = r#"
transport:
  protocol:
    type: tcp
    addr: 127.0.0.1:8089
"#;

        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("target/test-rustak-config-load.yaml");
        std::fs::write(&path, yaml).expect("must write fixture");

        let config = RustakConfig::load(&path).expect("load should work");
        assert!(matches!(
            config.transport.protocol,
            rustak_transport::Protocol::Tcp { .. }
        ));
    }

    #[test]
    fn strict_startup_rejects_bridge_limits_above_transport_limits() {
        let mut config = RustakConfig::default();
        let mut bridge = BridgeConfig::default();
        bridge.validation.strict_startup = true;
        bridge.limits.max_frame_bytes = config.transport.limits.max_frame_bytes + 1;
        config.bridge = Some(bridge);

        let error = config
            .validate_startup()
            .expect_err("strict startup should fail cross-config limit mismatch");
        assert!(matches!(
            error,
            ConfigError::StrictStartupBridgeFrameLimitExceedsTransport { .. }
        ));
    }
}
