use std::{
    net::{Ipv4Addr, SocketAddr},
    str::FromStr,
    time::Duration,
};

use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value as JsonValue;

use crate::{
    CertificatesConfig, ConfigError, CryptoConfig, CryptoProvider, LimitsBinding, LimitsRef,
    LogFormat, LogLevel, LoggingConfig, RevocationPolicy, RustakConfig, SapientConfigSpec,
};
use rustak_bridge::{
    BridgeConfig, BridgeValidationConfig, DedupConfig, EmitterConfig, TimePolicyMode,
};
use rustak_limits::Limits;
use rustak_sapient::SapientConfig;
use rustak_transport::{
    Keepalive, MtuSafety, Protocol, ReconnectPolicy, SendQueueConfig, SendQueueMode,
    TransportConfig, UdpTarget,
};
use rustak_wire::WireFormat;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RustakConfigDocument {
    #[serde(default = "default_transport_document")]
    pub transport: TransportConfigDocument,
    #[serde(default)]
    pub sapient: Option<SapientConfigSpecDocument>,
    #[serde(default)]
    pub bridge: Option<BridgeConfigDocument>,
    #[serde(default)]
    pub crypto: Option<CryptoConfigDocument>,
    #[serde(default)]
    pub certificates: Option<CertificatesConfigDocument>,
    #[serde(default)]
    pub logging: Option<LoggingConfigDocument>,
}

impl From<&RustakConfig> for RustakConfigDocument {
    fn from(value: &RustakConfig) -> Self {
        Self {
            transport: TransportConfigDocument::from(&value.transport),
            sapient: value.sapient.as_ref().map(SapientConfigSpecDocument::from),
            bridge: value.bridge.as_ref().map(BridgeConfigDocument::from),
            crypto: value.crypto.as_ref().map(CryptoConfigDocument::from),
            certificates: value
                .certificates
                .as_ref()
                .map(CertificatesConfigDocument::from),
            logging: value.logging.as_ref().map(LoggingConfigDocument::from),
        }
    }
}

impl TryFrom<RustakConfigDocument> for RustakConfig {
    type Error = ConfigError;

    fn try_from(value: RustakConfigDocument) -> Result<Self, Self::Error> {
        Ok(Self {
            transport: value.transport.try_into()?,
            sapient: value.sapient.map(TryInto::try_into).transpose()?,
            bridge: value.bridge.map(Into::into),
            crypto: value.crypto.map(Into::into),
            certificates: value.certificates.map(Into::into),
            logging: value.logging.map(Into::into),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TransportConfigDocument {
    pub protocol: ProtocolDocument,
    #[serde(default = "default_wire_format_document")]
    pub wire_format: WireFormatDocument,
    #[serde(default = "default_limits_document")]
    pub limits: LimitsDocument,
    #[serde(default = "default_read_timeout_document")]
    pub read_timeout: DurationDocument,
    #[serde(default = "default_write_timeout_document")]
    pub write_timeout: DurationDocument,
    #[serde(default)]
    pub keepalive: Option<KeepaliveDocument>,
    #[serde(rename = "reconnect", default = "default_reconnect_policy_document")]
    pub reconnect_policy: ReconnectPolicyDocument,
    #[serde(default)]
    pub mtu_safety: Option<MtuSafetyDocument>,
    #[serde(default = "default_send_queue_document")]
    pub send_queue: SendQueueConfigDocument,
}

impl From<&TransportConfig> for TransportConfigDocument {
    fn from(value: &TransportConfig) -> Self {
        Self {
            protocol: ProtocolDocument::from(&value.protocol),
            wire_format: WireFormatDocument::from(value.wire_format),
            limits: LimitsDocument::from(&value.limits),
            read_timeout: DurationDocument::from_duration(value.read_timeout),
            write_timeout: DurationDocument::from_duration(value.write_timeout),
            keepalive: value.keepalive.as_ref().map(KeepaliveDocument::from),
            reconnect_policy: ReconnectPolicyDocument::from(&value.reconnect_policy),
            mtu_safety: value.mtu_safety.as_ref().map(MtuSafetyDocument::from),
            send_queue: SendQueueConfigDocument::from(&value.send_queue),
        }
    }
}

impl TryFrom<TransportConfigDocument> for TransportConfig {
    type Error = ConfigError;

    fn try_from(value: TransportConfigDocument) -> Result<Self, Self::Error> {
        Ok(Self {
            protocol: value.protocol.try_into()?,
            wire_format: value.wire_format.into(),
            limits: value.limits.into(),
            read_timeout: value.read_timeout.into_duration(),
            write_timeout: value.write_timeout.into_duration(),
            keepalive: value.keepalive.map(Into::into),
            reconnect_policy: value.reconnect_policy.into(),
            mtu_safety: value.mtu_safety.map(Into::into),
            send_queue: value.send_queue.into(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ProtocolDocument {
    Tcp {
        addr: String,
    },
    Tls {
        addr: String,
        server_name: String,
    },
    UdpUnicast {
        bind_addr: String,
        target_addr: String,
    },
    UdpMulticast {
        bind_addr: String,
        group: String,
        port: u16,
    },
    UdpBroadcast {
        bind_addr: String,
        port: u16,
    },
    WebSocket {
        url: String,
    },
}

impl From<&Protocol> for ProtocolDocument {
    fn from(value: &Protocol) -> Self {
        match value {
            Protocol::Udp { bind_addr, target } => match target {
                UdpTarget::Unicast(target_addr) => Self::UdpUnicast {
                    bind_addr: bind_addr.to_string(),
                    target_addr: target_addr.to_string(),
                },
                UdpTarget::Multicast { group, port } => Self::UdpMulticast {
                    bind_addr: bind_addr.to_string(),
                    group: group.to_string(),
                    port: *port,
                },
                UdpTarget::Broadcast { port } => Self::UdpBroadcast {
                    bind_addr: bind_addr.to_string(),
                    port: *port,
                },
            },
            Protocol::Tcp { addr } => Self::Tcp {
                addr: addr.to_string(),
            },
            Protocol::Tls { addr, server_name } => Self::Tls {
                addr: addr.to_string(),
                server_name: server_name.clone(),
            },
            Protocol::WebSocket { url } => Self::WebSocket { url: url.clone() },
        }
    }
}

impl TryFrom<ProtocolDocument> for Protocol {
    type Error = ConfigError;

    fn try_from(value: ProtocolDocument) -> Result<Self, Self::Error> {
        match value {
            ProtocolDocument::Tcp { addr } => Ok(Self::Tcp {
                addr: parse_socket_addr("transport.protocol.addr", addr)?,
            }),
            ProtocolDocument::Tls { addr, server_name } => Ok(Self::Tls {
                addr: parse_socket_addr("transport.protocol.addr", addr)?,
                server_name,
            }),
            ProtocolDocument::UdpUnicast {
                bind_addr,
                target_addr,
            } => Ok(Self::Udp {
                bind_addr: parse_socket_addr("transport.protocol.bind_addr", bind_addr)?,
                target: UdpTarget::Unicast(parse_socket_addr(
                    "transport.protocol.target_addr",
                    target_addr,
                )?),
            }),
            ProtocolDocument::UdpMulticast {
                bind_addr,
                group,
                port,
            } => Ok(Self::Udp {
                bind_addr: parse_socket_addr("transport.protocol.bind_addr", bind_addr)?,
                target: UdpTarget::Multicast {
                    group: parse_ipv4_addr("transport.protocol.group", group)?,
                    port,
                },
            }),
            ProtocolDocument::UdpBroadcast { bind_addr, port } => Ok(Self::Udp {
                bind_addr: parse_socket_addr("transport.protocol.bind_addr", bind_addr)?,
                target: UdpTarget::Broadcast { port },
            }),
            ProtocolDocument::WebSocket { url } => Ok(Self::WebSocket { url }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WireFormatDocument {
    Xml,
    #[serde(rename = "tak_v1", alias = "tak_protocol_v1")]
    TakV1,
}

impl From<WireFormat> for WireFormatDocument {
    fn from(value: WireFormat) -> Self {
        match value {
            WireFormat::Xml => Self::Xml,
            WireFormat::TakProtocolV1 => Self::TakV1,
        }
    }
}

impl From<WireFormatDocument> for WireFormat {
    fn from(value: WireFormatDocument) -> Self {
        match value {
            WireFormatDocument::Xml => Self::Xml,
            WireFormatDocument::TakV1 => Self::TakProtocolV1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct LimitsDocument {
    pub max_frame_bytes: usize,
    pub max_xml_scan_bytes: usize,
    pub max_protobuf_bytes: usize,
    pub max_queue_messages: usize,
    pub max_queue_bytes: usize,
    pub max_detail_elements: usize,
}

impl From<&Limits> for LimitsDocument {
    fn from(value: &Limits) -> Self {
        Self {
            max_frame_bytes: value.max_frame_bytes,
            max_xml_scan_bytes: value.max_xml_scan_bytes,
            max_protobuf_bytes: value.max_protobuf_bytes,
            max_queue_messages: value.max_queue_messages,
            max_queue_bytes: value.max_queue_bytes,
            max_detail_elements: value.max_detail_elements,
        }
    }
}

impl From<LimitsDocument> for Limits {
    fn from(value: LimitsDocument) -> Self {
        Self {
            max_frame_bytes: value.max_frame_bytes,
            max_xml_scan_bytes: value.max_xml_scan_bytes,
            max_protobuf_bytes: value.max_protobuf_bytes,
            max_queue_messages: value.max_queue_messages,
            max_queue_bytes: value.max_queue_bytes,
            max_detail_elements: value.max_detail_elements,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
#[schemars(with = "String")]
pub(crate) struct DurationDocument(Duration);

impl DurationDocument {
    pub(crate) const fn from_duration(value: Duration) -> Self {
        Self(value)
    }

    pub(crate) const fn into_duration(self) -> Duration {
        self.0
    }
}

impl Serialize for DurationDocument {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format_duration(self.0))
    }
}

impl<'de> Deserialize<'de> for DurationDocument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum RawDuration {
            Text(String),
            Milliseconds(u64),
        }

        let raw = RawDuration::deserialize(deserializer)?;
        match raw {
            RawDuration::Text(text) => parse_duration(&text)
                .map(Self)
                .ok_or_else(|| serde::de::Error::custom(format!("invalid duration: {text}"))),
            RawDuration::Milliseconds(ms) => Ok(Self(Duration::from_millis(ms))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct KeepaliveDocument {
    pub interval: DurationDocument,
    pub timeout: DurationDocument,
}

impl From<&Keepalive> for KeepaliveDocument {
    fn from(value: &Keepalive) -> Self {
        Self {
            interval: DurationDocument::from_duration(value.interval),
            timeout: DurationDocument::from_duration(value.timeout),
        }
    }
}

impl From<KeepaliveDocument> for Keepalive {
    fn from(value: KeepaliveDocument) -> Self {
        Self {
            interval: value.interval.into_duration(),
            timeout: value.timeout.into_duration(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReconnectPolicyDocument {
    pub enabled: bool,
    pub initial_delay: DurationDocument,
    pub max_delay: DurationDocument,
    pub backoff_factor: f64,
    pub jitter: f64,
    #[serde(default)]
    pub max_retries: Option<u32>,
}

impl From<&ReconnectPolicy> for ReconnectPolicyDocument {
    fn from(value: &ReconnectPolicy) -> Self {
        Self {
            enabled: value.enabled,
            initial_delay: DurationDocument::from_duration(value.initial_delay),
            max_delay: DurationDocument::from_duration(value.max_delay),
            backoff_factor: value.backoff_factor,
            jitter: value.jitter,
            max_retries: value.max_retries,
        }
    }
}

impl From<ReconnectPolicyDocument> for ReconnectPolicy {
    fn from(value: ReconnectPolicyDocument) -> Self {
        Self {
            enabled: value.enabled,
            initial_delay: value.initial_delay.into_duration(),
            max_delay: value.max_delay.into_duration(),
            backoff_factor: value.backoff_factor,
            jitter: value.jitter,
            max_retries: value.max_retries,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct MtuSafetyDocument {
    pub max_udp_payload_bytes: usize,
    pub drop_oversize: bool,
}

impl From<&MtuSafety> for MtuSafetyDocument {
    fn from(value: &MtuSafety) -> Self {
        Self {
            max_udp_payload_bytes: value.max_udp_payload_bytes,
            drop_oversize: value.drop_oversize,
        }
    }
}

impl From<MtuSafetyDocument> for MtuSafety {
    fn from(value: MtuSafetyDocument) -> Self {
        Self {
            max_udp_payload_bytes: value.max_udp_payload_bytes,
            drop_oversize: value.drop_oversize,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SendQueueConfigDocument {
    pub max_messages: usize,
    pub max_bytes: usize,
    pub mode: SendQueueModeDocument,
}

impl From<&SendQueueConfig> for SendQueueConfigDocument {
    fn from(value: &SendQueueConfig) -> Self {
        Self {
            max_messages: value.max_messages,
            max_bytes: value.max_bytes,
            mode: SendQueueModeDocument::from(value.mode.clone()),
        }
    }
}

impl From<SendQueueConfigDocument> for SendQueueConfig {
    fn from(value: SendQueueConfigDocument) -> Self {
        Self {
            max_messages: value.max_messages,
            max_bytes: value.max_bytes,
            mode: value.mode.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SendQueueModeDocument {
    Fifo,
    Priority,
    CoalesceLatestByUid,
}

impl From<SendQueueMode> for SendQueueModeDocument {
    fn from(value: SendQueueMode) -> Self {
        match value {
            SendQueueMode::Fifo => Self::Fifo,
            SendQueueMode::Priority => Self::Priority,
            SendQueueMode::CoalesceLatestByUid => Self::CoalesceLatestByUid,
        }
    }
}

impl From<SendQueueModeDocument> for SendQueueMode {
    fn from(value: SendQueueModeDocument) -> Self {
        match value {
            SendQueueModeDocument::Fifo => Self::Fifo,
            SendQueueModeDocument::Priority => Self::Priority,
            SendQueueModeDocument::CoalesceLatestByUid => Self::CoalesceLatestByUid,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SapientConfigSpecDocument {
    pub version: String,
    #[serde(default)]
    pub limits: Option<LimitsDocument>,
    #[serde(default)]
    pub limits_ref: Option<String>,
    #[serde(default = "default_read_timeout_document")]
    pub read_timeout: DurationDocument,
    #[serde(default = "default_write_timeout_document")]
    pub write_timeout: DurationDocument,
    #[serde(default = "default_tcp_nodelay")]
    pub tcp_nodelay: bool,
}

impl From<&SapientConfigSpec> for SapientConfigSpecDocument {
    fn from(value: &SapientConfigSpec) -> Self {
        let (limits, limits_ref) = match &value.limits {
            LimitsBinding::Inline(limits) => (Some(LimitsDocument::from(limits)), None),
            LimitsBinding::Reference(reference) => (None, Some(reference.path().to_owned())),
        };

        Self {
            version: value.version.clone(),
            limits,
            limits_ref,
            read_timeout: DurationDocument::from_duration(value.read_timeout),
            write_timeout: DurationDocument::from_duration(value.write_timeout),
            tcp_nodelay: value.tcp_nodelay,
        }
    }
}

impl TryFrom<SapientConfigSpecDocument> for SapientConfigSpec {
    type Error = ConfigError;

    fn try_from(value: SapientConfigSpecDocument) -> Result<Self, Self::Error> {
        let limits = match (value.limits, value.limits_ref) {
            (Some(_), Some(_)) => {
                return Err(ConfigError::ConflictingFields {
                    left: "sapient.limits",
                    right: "sapient.limits_ref",
                })
            }
            (Some(limits), None) => LimitsBinding::Inline(limits.into()),
            (None, Some(path)) => LimitsBinding::Reference(LimitsRef::new(path)?),
            (None, None) => LimitsBinding::Reference(LimitsRef::new("transport.limits")?),
        };

        Ok(Self {
            version: value.version,
            limits,
            read_timeout: value.read_timeout.into_duration(),
            write_timeout: value.write_timeout.into_duration(),
            tcp_nodelay: value.tcp_nodelay,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BridgeConfigDocument {
    #[serde(default = "default_limits_document")]
    pub limits: LimitsDocument,
    #[serde(default = "default_bridge_cot_stale_seconds")]
    pub cot_stale_seconds: u32,
    #[serde(default = "default_bridge_max_clock_skew_seconds")]
    pub max_clock_skew_seconds: u32,
    #[serde(default = "default_bridge_time_policy_document")]
    pub time_policy: TimePolicyModeDocument,
    #[serde(default = "default_bridge_dedup_document")]
    pub dedup: BridgeDedupDocument,
    #[serde(default = "default_bridge_emitter_document")]
    pub emitter: BridgeEmitterDocument,
    #[serde(default = "default_bridge_validation_document")]
    pub validation: BridgeValidationDocument,
}

impl From<&BridgeConfig> for BridgeConfigDocument {
    fn from(value: &BridgeConfig) -> Self {
        Self {
            limits: LimitsDocument::from(&value.limits),
            cot_stale_seconds: value.cot_stale_seconds,
            max_clock_skew_seconds: value.max_clock_skew_seconds,
            time_policy: TimePolicyModeDocument::from(value.time_policy),
            dedup: BridgeDedupDocument::from(&value.dedup),
            emitter: BridgeEmitterDocument::from(&value.emitter),
            validation: BridgeValidationDocument::from(&value.validation),
        }
    }
}

impl From<BridgeConfigDocument> for BridgeConfig {
    fn from(value: BridgeConfigDocument) -> Self {
        Self {
            limits: value.limits.into(),
            cot_stale_seconds: value.cot_stale_seconds,
            max_clock_skew_seconds: value.max_clock_skew_seconds,
            time_policy: value.time_policy.into(),
            dedup: value.dedup.into(),
            emitter: value.emitter.into(),
            validation: value.validation.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TimePolicyModeDocument {
    MessageTime,
    ObservedTime,
    ObservedWithSkewClamp,
}

impl From<TimePolicyMode> for TimePolicyModeDocument {
    fn from(value: TimePolicyMode) -> Self {
        match value {
            TimePolicyMode::MessageTime => Self::MessageTime,
            TimePolicyMode::ObservedTime => Self::ObservedTime,
            TimePolicyMode::ObservedWithSkewClamp => Self::ObservedWithSkewClamp,
        }
    }
}

impl From<TimePolicyModeDocument> for TimePolicyMode {
    fn from(value: TimePolicyModeDocument) -> Self {
        match value {
            TimePolicyModeDocument::MessageTime => Self::MessageTime,
            TimePolicyModeDocument::ObservedTime => Self::ObservedTime,
            TimePolicyModeDocument::ObservedWithSkewClamp => Self::ObservedWithSkewClamp,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BridgeDedupDocument {
    pub window: DurationDocument,
    pub max_keys: usize,
}

impl From<&DedupConfig> for BridgeDedupDocument {
    fn from(value: &DedupConfig) -> Self {
        Self {
            window: DurationDocument::from_duration(value.window),
            max_keys: value.max_keys,
        }
    }
}

impl From<BridgeDedupDocument> for DedupConfig {
    fn from(value: BridgeDedupDocument) -> Self {
        Self {
            window: value.window.into_duration(),
            max_keys: value.max_keys,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BridgeEmitterDocument {
    pub max_updates_per_second: u32,
    pub min_separation: DurationDocument,
    pub max_pending_events: usize,
}

impl From<&EmitterConfig> for BridgeEmitterDocument {
    fn from(value: &EmitterConfig) -> Self {
        Self {
            max_updates_per_second: value.max_updates_per_second,
            min_separation: DurationDocument::from_duration(value.min_separation),
            max_pending_events: value.max_pending_events,
        }
    }
}

impl From<BridgeEmitterDocument> for EmitterConfig {
    fn from(value: BridgeEmitterDocument) -> Self {
        Self {
            max_updates_per_second: value.max_updates_per_second,
            min_separation: value.min_separation.into_duration(),
            max_pending_events: value.max_pending_events,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BridgeValidationDocument {
    #[serde(default = "default_true")]
    pub strict_startup: bool,
    #[serde(default = "default_unknown_class_fallback")]
    pub unknown_class_fallback: String,
    #[serde(default = "default_bridge_classification_mapping_entries")]
    pub classification_mapping_entries: usize,
    #[serde(default = "default_bridge_behaviour_mapping_entries")]
    pub behaviour_mapping_entries: usize,
}

impl From<&BridgeValidationConfig> for BridgeValidationDocument {
    fn from(value: &BridgeValidationConfig) -> Self {
        Self {
            strict_startup: value.strict_startup,
            unknown_class_fallback: value.unknown_class_fallback.clone(),
            classification_mapping_entries: value.classification_mapping_entries,
            behaviour_mapping_entries: value.behaviour_mapping_entries,
        }
    }
}

impl From<BridgeValidationDocument> for BridgeValidationConfig {
    fn from(value: BridgeValidationDocument) -> Self {
        Self {
            strict_startup: value.strict_startup,
            unknown_class_fallback: value.unknown_class_fallback,
            classification_mapping_entries: value.classification_mapping_entries,
            behaviour_mapping_entries: value.behaviour_mapping_entries,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CryptoConfigDocument {
    pub provider: CryptoProviderDocument,
    pub revocation: RevocationPolicyDocument,
    #[serde(default)]
    pub server_spki_pin: Option<String>,
}

impl From<&CryptoConfig> for CryptoConfigDocument {
    fn from(value: &CryptoConfig) -> Self {
        Self {
            provider: CryptoProviderDocument::from(value.provider),
            revocation: RevocationPolicyDocument::from(value.revocation),
            server_spki_pin: value.server_spki_pin.clone(),
        }
    }
}

impl From<CryptoConfigDocument> for CryptoConfig {
    fn from(value: CryptoConfigDocument) -> Self {
        Self {
            provider: value.provider.into(),
            revocation: value.revocation.into(),
            server_spki_pin: value.server_spki_pin,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CryptoProviderDocument {
    Ring,
    AwsLcRs,
    AwsLcRsFips,
}

impl From<CryptoProvider> for CryptoProviderDocument {
    fn from(value: CryptoProvider) -> Self {
        match value {
            CryptoProvider::Ring => Self::Ring,
            CryptoProvider::AwsLcRs => Self::AwsLcRs,
            CryptoProvider::AwsLcRsFips => Self::AwsLcRsFips,
        }
    }
}

impl From<CryptoProviderDocument> for CryptoProvider {
    fn from(value: CryptoProviderDocument) -> Self {
        match value {
            CryptoProviderDocument::Ring => Self::Ring,
            CryptoProviderDocument::AwsLcRs => Self::AwsLcRs,
            CryptoProviderDocument::AwsLcRsFips => Self::AwsLcRsFips,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RevocationPolicyDocument {
    Off,
    Prefer,
    Require,
}

impl From<RevocationPolicy> for RevocationPolicyDocument {
    fn from(value: RevocationPolicy) -> Self {
        match value {
            RevocationPolicy::Off => Self::Off,
            RevocationPolicy::Prefer => Self::Prefer,
            RevocationPolicy::Require => Self::Require,
        }
    }
}

impl From<RevocationPolicyDocument> for RevocationPolicy {
    fn from(value: RevocationPolicyDocument) -> Self {
        match value {
            RevocationPolicyDocument::Off => Self::Off,
            RevocationPolicyDocument::Prefer => Self::Prefer,
            RevocationPolicyDocument::Require => Self::Require,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CertificatesConfigDocument {
    pub ca_cert: String,
    pub client_cert: String,
    pub client_key: String,
}

impl From<&CertificatesConfig> for CertificatesConfigDocument {
    fn from(value: &CertificatesConfig) -> Self {
        Self {
            ca_cert: value.ca_cert.clone(),
            client_cert: value.client_cert.clone(),
            client_key: value.client_key.clone(),
        }
    }
}

impl From<CertificatesConfigDocument> for CertificatesConfig {
    fn from(value: CertificatesConfigDocument) -> Self {
        Self {
            ca_cert: value.ca_cert,
            client_cert: value.client_cert,
            client_key: value.client_key,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct LoggingConfigDocument {
    pub level: LogLevelDocument,
    pub format: LogFormatDocument,
    #[serde(default)]
    pub redact: Vec<String>,
}

impl From<&LoggingConfig> for LoggingConfigDocument {
    fn from(value: &LoggingConfig) -> Self {
        Self {
            level: LogLevelDocument::from(value.level),
            format: LogFormatDocument::from(value.format),
            redact: value.redact.clone(),
        }
    }
}

impl From<LoggingConfigDocument> for LoggingConfig {
    fn from(value: LoggingConfigDocument) -> Self {
        Self {
            level: value.level.into(),
            format: value.format.into(),
            redact: value.redact,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LogLevelDocument {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<LogLevel> for LogLevelDocument {
    fn from(value: LogLevel) -> Self {
        match value {
            LogLevel::Trace => Self::Trace,
            LogLevel::Debug => Self::Debug,
            LogLevel::Info => Self::Info,
            LogLevel::Warn => Self::Warn,
            LogLevel::Error => Self::Error,
        }
    }
}

impl From<LogLevelDocument> for LogLevel {
    fn from(value: LogLevelDocument) -> Self {
        match value {
            LogLevelDocument::Trace => Self::Trace,
            LogLevelDocument::Debug => Self::Debug,
            LogLevelDocument::Info => Self::Info,
            LogLevelDocument::Warn => Self::Warn,
            LogLevelDocument::Error => Self::Error,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LogFormatDocument {
    Json,
    Pretty,
    Compact,
}

impl From<LogFormat> for LogFormatDocument {
    fn from(value: LogFormat) -> Self {
        match value {
            LogFormat::Json => Self::Json,
            LogFormat::Pretty => Self::Pretty,
            LogFormat::Compact => Self::Compact,
        }
    }
}

impl From<LogFormatDocument> for LogFormat {
    fn from(value: LogFormatDocument) -> Self {
        match value {
            LogFormatDocument::Json => Self::Json,
            LogFormatDocument::Pretty => Self::Pretty,
            LogFormatDocument::Compact => Self::Compact,
        }
    }
}

pub fn json_schema() -> JsonValue {
    serde_json::to_value(schema_for!(RustakConfigDocument)).unwrap_or(JsonValue::Null)
}

fn default_transport_document() -> TransportConfigDocument {
    TransportConfigDocument::from(&TransportConfig::default())
}

fn default_limits_document() -> LimitsDocument {
    LimitsDocument::from(&Limits::default())
}

fn default_wire_format_document() -> WireFormatDocument {
    WireFormatDocument::from(WireFormat::Xml)
}

fn default_read_timeout_document() -> DurationDocument {
    DurationDocument::from_duration(Duration::from_secs(15))
}

fn default_write_timeout_document() -> DurationDocument {
    DurationDocument::from_duration(Duration::from_secs(15))
}

fn default_reconnect_policy_document() -> ReconnectPolicyDocument {
    ReconnectPolicyDocument::from(&ReconnectPolicy::default())
}

fn default_send_queue_document() -> SendQueueConfigDocument {
    SendQueueConfigDocument::from(&TransportConfig::default().send_queue)
}

fn default_bridge_cot_stale_seconds() -> u32 {
    BridgeConfig::default().cot_stale_seconds
}

fn default_bridge_max_clock_skew_seconds() -> u32 {
    BridgeConfig::default().max_clock_skew_seconds
}

fn default_bridge_time_policy_document() -> TimePolicyModeDocument {
    TimePolicyModeDocument::from(BridgeConfig::default().time_policy)
}

fn default_bridge_dedup_document() -> BridgeDedupDocument {
    BridgeDedupDocument::from(&BridgeConfig::default().dedup)
}

fn default_bridge_emitter_document() -> BridgeEmitterDocument {
    BridgeEmitterDocument::from(&BridgeConfig::default().emitter)
}

fn default_bridge_validation_document() -> BridgeValidationDocument {
    BridgeValidationDocument::from(&BridgeConfig::default().validation)
}

fn default_true() -> bool {
    true
}

fn default_unknown_class_fallback() -> String {
    BridgeValidationConfig::default().unknown_class_fallback
}

fn default_bridge_classification_mapping_entries() -> usize {
    BridgeValidationConfig::default().classification_mapping_entries
}

fn default_bridge_behaviour_mapping_entries() -> usize {
    BridgeValidationConfig::default().behaviour_mapping_entries
}

fn default_tcp_nodelay() -> bool {
    SapientConfig::default().tcp_nodelay
}

fn parse_socket_addr(field: &'static str, value: String) -> Result<SocketAddr, ConfigError> {
    SocketAddr::from_str(&value).map_err(|_| ConfigError::InvalidAddress { field, value })
}

fn parse_ipv4_addr(field: &'static str, value: String) -> Result<Ipv4Addr, ConfigError> {
    Ipv4Addr::from_str(&value).map_err(|_| ConfigError::InvalidIpv4Address { field, value })
}

fn parse_duration(raw: &str) -> Option<Duration> {
    let text = raw.trim();
    if let Some(value) = text.strip_suffix("ms") {
        return value.trim().parse::<u64>().ok().map(Duration::from_millis);
    }
    if let Some(value) = text.strip_suffix('s') {
        return value.trim().parse::<u64>().ok().map(Duration::from_secs);
    }
    if let Some(value) = text.strip_suffix('m') {
        return value
            .trim()
            .parse::<u64>()
            .ok()
            .map(|minutes| Duration::from_secs(minutes.saturating_mul(60)));
    }
    if let Some(value) = text.strip_suffix('h') {
        return value
            .trim()
            .parse::<u64>()
            .ok()
            .map(|hours| Duration::from_secs(hours.saturating_mul(60 * 60)));
    }

    text.parse::<u64>().ok().map(Duration::from_secs)
}

fn format_duration(duration: Duration) -> String {
    if duration.subsec_nanos() == 0 {
        return format!("{}s", duration.as_secs());
    }

    format!("{}ms", duration.as_millis())
}
