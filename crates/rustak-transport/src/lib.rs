use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use bytes::Bytes;
use rustak_io::{MessageEnvelope, MessageSink, MessageSource};
use rustak_limits::{Limits, LimitsError};
use rustak_net::{
    read_delimited_frame, read_length_prefixed_frame, write_delimited_frame,
    write_length_prefixed_frame, DelimiterFrameError, LengthPrefixKind, LengthPrefixedError,
};
use rustak_wire::{
    DowngradePolicy, NegotiationEvent, NegotiationState, Negotiator, TakProtocolVersion, WireFormat,
};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};

pub mod config;
pub mod queue;
pub mod udp;

pub use config::{SendQueueConfig, SendQueueMode};
pub use queue::{
    OutboundSendQueue, QueueEnqueueReport, QueuePriority, SendQueueClassifier, SendQueueError,
};
pub use udp::{apply_mtu_policy, UdpPolicyError, UdpSendDecision};

pub type TransportEnvelope<T> = MessageEnvelope<T>;
pub type TransportSink<T> = dyn MessageSink<T>;
pub type TransportSource<T> = dyn MessageSource<T>;

#[must_use]
pub fn envelope<T>(message: T) -> TransportEnvelope<T> {
    MessageEnvelope::new(message)
}

const XML_FRAME_DELIMITER: &[u8] = b"\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportFraming {
    XmlNewlineDelimited,
    TakProtocolU32LengthPrefixed,
}

impl From<WireFormat> for TransportFraming {
    fn from(value: WireFormat) -> Self {
        match value {
            WireFormat::Xml => Self::XmlNewlineDelimited,
            WireFormat::TakProtocolV1 => Self::TakProtocolU32LengthPrefixed,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransportConfig {
    pub protocol: Protocol,
    pub wire_format: WireFormat,
    pub limits: Limits,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub keepalive: Option<Keepalive>,
    pub reconnect_policy: ReconnectPolicy,
    pub mtu_safety: Option<MtuSafety>,
    pub send_queue: SendQueueConfig,
}

impl Default for TransportConfig {
    fn default() -> Self {
        let limits = Limits::default();
        Self {
            protocol: Protocol::Tcp {
                addr: SocketAddr::from(([127, 0, 0, 1], 8089)),
            },
            wire_format: WireFormat::Xml,
            read_timeout: Duration::from_secs(15),
            write_timeout: Duration::from_secs(15),
            keepalive: Some(Keepalive {
                interval: Duration::from_secs(10),
                timeout: Duration::from_secs(3),
            }),
            reconnect_policy: ReconnectPolicy::default(),
            mtu_safety: Some(MtuSafety {
                max_udp_payload_bytes: 1_200,
                drop_oversize: true,
            }),
            send_queue: SendQueueConfig {
                max_messages: limits.max_queue_messages,
                max_bytes: limits.max_queue_bytes,
                mode: SendQueueMode::CoalesceLatestByUid,
            },
            limits,
        }
    }
}

impl TransportConfig {
    pub fn validate(&self) -> Result<(), TransportConfigError> {
        self.limits.validate()?;

        ensure_non_zero_duration("read_timeout", self.read_timeout)?;
        ensure_non_zero_duration("write_timeout", self.write_timeout)?;

        if let Some(keepalive) = &self.keepalive {
            ensure_non_zero_duration("keepalive.interval", keepalive.interval)?;
            ensure_non_zero_duration("keepalive.timeout", keepalive.timeout)?;
            if keepalive.timeout > keepalive.interval {
                return Err(TransportConfigError::KeepaliveTimeoutExceedsInterval {
                    timeout: keepalive.timeout,
                    interval: keepalive.interval,
                });
            }
        }

        self.reconnect_policy.validate()?;
        self.send_queue.validate(&self.limits)?;

        if let Some(mtu_safety) = &self.mtu_safety {
            if mtu_safety.max_udp_payload_bytes == 0 {
                return Err(TransportConfigError::ZeroUdpPayloadLimit);
            }

            if mtu_safety.max_udp_payload_bytes > self.limits.max_frame_bytes {
                return Err(TransportConfigError::MtuPayloadExceedsFrame {
                    max_udp_payload_bytes: mtu_safety.max_udp_payload_bytes,
                    max_frame_bytes: self.limits.max_frame_bytes,
                });
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keepalive {
    pub interval: Duration,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Protocol {
    Udp {
        bind_addr: SocketAddr,
        target: UdpTarget,
    },
    Tcp {
        addr: SocketAddr,
    },
    Tls {
        addr: SocketAddr,
        server_name: String,
    },
    WebSocket {
        url: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UdpTarget {
    Unicast(SocketAddr),
    Multicast { group: Ipv4Addr, port: u16 },
    Broadcast { port: u16 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReconnectPolicy {
    pub enabled: bool,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_factor: f64,
    pub jitter: f64,
    pub max_retries: Option<u32>,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_factor: 2.0,
            jitter: 0.2,
            max_retries: None,
        }
    }
}

impl ReconnectPolicy {
    fn validate(&self) -> Result<(), TransportConfigError> {
        if self.enabled {
            ensure_non_zero_duration("reconnect.initial_delay", self.initial_delay)?;
            ensure_non_zero_duration("reconnect.max_delay", self.max_delay)?;
        }

        if self.backoff_factor < 1.0 {
            return Err(TransportConfigError::BackoffFactorTooSmall {
                backoff_factor: self.backoff_factor,
            });
        }

        if !(0.0..=1.0).contains(&self.jitter) {
            return Err(TransportConfigError::JitterOutOfRange {
                jitter: self.jitter,
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MtuSafety {
    pub max_udp_payload_bytes: usize,
    pub drop_oversize: bool,
}

#[derive(Debug, Error, PartialEq)]
pub enum TransportConfigError {
    #[error(transparent)]
    InvalidLimits(#[from] LimitsError),

    #[error("{field} must be greater than zero")]
    ZeroDuration { field: &'static str },

    #[error("keepalive timeout ({timeout:?}) must not exceed interval ({interval:?})")]
    KeepaliveTimeoutExceedsInterval {
        timeout: Duration,
        interval: Duration,
    },

    #[error("reconnect.backoff_factor must be >= 1.0, got {backoff_factor}")]
    BackoffFactorTooSmall { backoff_factor: f64 },

    #[error("reconnect.jitter must be within [0.0, 1.0], got {jitter}")]
    JitterOutOfRange { jitter: f64 },

    #[error("mtu_safety.max_udp_payload_bytes must be > 0")]
    ZeroUdpPayloadLimit,

    #[error(
        "mtu_safety.max_udp_payload_bytes ({max_udp_payload_bytes}) cannot exceed \
         limits.max_frame_bytes ({max_frame_bytes})"
    )]
    MtuPayloadExceedsFrame {
        max_udp_payload_bytes: usize,
        max_frame_bytes: usize,
    },

    #[error("send_queue.max_messages must be > 0")]
    ZeroSendQueueMessages,

    #[error("send_queue.max_bytes must be > 0")]
    ZeroSendQueueBytes,

    #[error(
        "send_queue.max_messages ({max_messages}) cannot exceed limits.max_queue_messages ({limits_max_messages})"
    )]
    SendQueueMessagesExceedLimits {
        max_messages: usize,
        limits_max_messages: usize,
    },

    #[error(
        "send_queue.max_bytes ({max_bytes}) cannot exceed limits.max_queue_bytes ({limits_max_bytes})"
    )]
    SendQueueBytesExceedLimits {
        max_bytes: usize,
        limits_max_bytes: usize,
    },
}

#[derive(Debug, Error)]
pub enum TransportComposeError {
    #[error(transparent)]
    InvalidConfig(#[from] TransportConfigError),

    #[error(transparent)]
    LengthPrefixed(#[from] LengthPrefixedError),

    #[error(transparent)]
    Delimited(#[from] DelimiterFrameError),
}

#[derive(Debug)]
pub struct TransportSender<W> {
    writer: W,
    framing: TransportFraming,
    max_frame_bytes: usize,
}

impl<W> TransportSender<W> {
    pub fn new(writer: W, config: &TransportConfig) -> Result<Self, TransportComposeError> {
        let (framing, max_frame_bytes) = framing_settings(config)?;
        Ok(Self {
            writer,
            framing,
            max_frame_bytes,
        })
    }

    #[must_use]
    pub fn framing(&self) -> TransportFraming {
        self.framing
    }

    #[must_use]
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W> TransportSender<W>
where
    W: AsyncWrite + Unpin,
{
    pub async fn send_frame(&mut self, payload: &[u8]) -> Result<(), TransportComposeError> {
        send_frame_with_framing(
            &mut self.writer,
            self.framing,
            payload,
            self.max_frame_bytes,
        )
        .await
    }

    pub async fn send_envelope(
        &mut self,
        envelope: TransportEnvelope<Vec<u8>>,
    ) -> Result<(), TransportComposeError> {
        self.send_frame(&envelope.message).await
    }
}

#[derive(Debug)]
pub struct TransportReceiver<R> {
    reader: R,
    framing: TransportFraming,
    max_frame_bytes: usize,
}

impl<R> TransportReceiver<R> {
    pub fn new(reader: R, config: &TransportConfig) -> Result<Self, TransportComposeError> {
        let (framing, max_frame_bytes) = framing_settings(config)?;
        Ok(Self {
            reader,
            framing,
            max_frame_bytes,
        })
    }

    #[must_use]
    pub fn framing(&self) -> TransportFraming {
        self.framing
    }

    #[must_use]
    pub fn into_inner(self) -> R {
        self.reader
    }
}

impl<R> TransportReceiver<R>
where
    R: AsyncRead + Unpin,
{
    pub async fn recv_frame(&mut self) -> Result<Vec<u8>, TransportComposeError> {
        recv_frame_with_framing(&mut self.reader, self.framing, self.max_frame_bytes).await
    }

    pub async fn recv_envelope(
        &mut self,
    ) -> Result<TransportEnvelope<Vec<u8>>, TransportComposeError> {
        let frame = self.recv_frame().await?;
        let raw_frame = Bytes::copy_from_slice(&frame);
        Ok(TransportEnvelope::new(frame).with_raw_frame(raw_frame))
    }
}

#[derive(Debug)]
pub struct TransportConnection<IO> {
    io: IO,
    framing: TransportFraming,
    max_frame_bytes: usize,
    negotiator: Negotiator,
}

impl<IO> TransportConnection<IO> {
    pub fn new(
        io: IO,
        config: &TransportConfig,
        downgrade_policy: DowngradePolicy,
    ) -> Result<Self, TransportComposeError> {
        let (framing, max_frame_bytes) = framing_settings(config)?;
        Ok(Self {
            io,
            framing,
            max_frame_bytes,
            negotiator: Negotiator::new(downgrade_policy),
        })
    }

    #[must_use]
    pub fn framing(&self) -> TransportFraming {
        self.framing
    }

    #[must_use]
    pub fn negotiation_state(&self) -> NegotiationState {
        self.negotiator.state()
    }

    pub fn begin_upgrade_attempt(&mut self) -> NegotiationEvent {
        self.negotiator.begin_upgrade_attempt()
    }

    pub fn observe_supported_version(&mut self, version: TakProtocolVersion) -> NegotiationEvent {
        self.negotiator.observe_supported_version(version)
    }

    pub fn observe_timeout(&mut self) -> NegotiationEvent {
        self.negotiator.observe_timeout()
    }

    pub fn observe_malformed_control(&mut self) -> NegotiationEvent {
        self.negotiator.observe_malformed_control()
    }

    pub fn observe_unsupported_version(&mut self) -> NegotiationEvent {
        self.negotiator.observe_unsupported_version()
    }

    pub fn observe_policy_denied(&mut self) -> NegotiationEvent {
        self.negotiator.observe_policy_denied()
    }

    #[must_use]
    pub fn into_inner(self) -> IO {
        self.io
    }
}

impl<IO> TransportConnection<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    pub async fn send_frame(&mut self, payload: &[u8]) -> Result<(), TransportComposeError> {
        send_frame_with_framing(&mut self.io, self.framing, payload, self.max_frame_bytes).await
    }

    pub async fn send_envelope(
        &mut self,
        envelope: TransportEnvelope<Vec<u8>>,
    ) -> Result<(), TransportComposeError> {
        self.send_frame(&envelope.message).await
    }

    pub async fn recv_frame(&mut self) -> Result<Vec<u8>, TransportComposeError> {
        recv_frame_with_framing(&mut self.io, self.framing, self.max_frame_bytes).await
    }

    pub async fn recv_envelope(
        &mut self,
    ) -> Result<TransportEnvelope<Vec<u8>>, TransportComposeError> {
        let frame = self.recv_frame().await?;
        let raw_frame = Bytes::copy_from_slice(&frame);
        Ok(TransportEnvelope::new(frame).with_raw_frame(raw_frame))
    }
}

fn framing_settings(
    config: &TransportConfig,
) -> Result<(TransportFraming, usize), TransportComposeError> {
    config.validate()?;
    Ok((
        TransportFraming::from(config.wire_format),
        config.limits.max_frame_bytes,
    ))
}

async fn send_frame_with_framing<W>(
    writer: &mut W,
    framing: TransportFraming,
    payload: &[u8],
    max_frame_bytes: usize,
) -> Result<(), TransportComposeError>
where
    W: AsyncWrite + Unpin,
{
    match framing {
        TransportFraming::XmlNewlineDelimited => {
            write_delimited_frame(writer, payload, XML_FRAME_DELIMITER, max_frame_bytes).await?;
        }
        TransportFraming::TakProtocolU32LengthPrefixed => {
            write_length_prefixed_frame(writer, LengthPrefixKind::U32Be, payload, max_frame_bytes)
                .await?;
        }
    }
    Ok(())
}

async fn recv_frame_with_framing<R>(
    reader: &mut R,
    framing: TransportFraming,
    max_frame_bytes: usize,
) -> Result<Vec<u8>, TransportComposeError>
where
    R: AsyncRead + Unpin,
{
    match framing {
        TransportFraming::XmlNewlineDelimited => {
            read_delimited_frame(reader, XML_FRAME_DELIMITER, max_frame_bytes, false)
                .await
                .map_err(TransportComposeError::from)
        }
        TransportFraming::TakProtocolU32LengthPrefixed => {
            read_length_prefixed_frame(reader, LengthPrefixKind::U32Be, max_frame_bytes)
                .await
                .map_err(TransportComposeError::from)
        }
    }
}

fn ensure_non_zero_duration(
    field: &'static str,
    duration: Duration,
) -> Result<(), TransportConfigError> {
    if duration.is_zero() {
        return Err(TransportConfigError::ZeroDuration { field });
    }

    Ok(())
}

#[doc(hidden)]
pub fn fuzz_hook_validate_transport_config(data: &[u8]) -> Result<(), TransportConfigError> {
    let mut config = TransportConfig::default();
    config.limits.max_frame_bytes = word_at(data, 0);
    config.limits.max_xml_scan_bytes = word_at(data, 2);
    config.limits.max_protobuf_bytes = word_at(data, 4);
    config.limits.max_queue_messages = word_at(data, 6);
    config.limits.max_queue_bytes = word_at(data, 8);
    config.limits.max_detail_elements = word_at(data, 10);

    config.read_timeout = Duration::from_millis(word_at(data, 12) as u64);
    config.write_timeout = Duration::from_millis(word_at(data, 14) as u64);

    if let Some(keepalive) = config.keepalive.as_mut() {
        keepalive.interval = Duration::from_millis(word_at(data, 16) as u64);
        keepalive.timeout = Duration::from_millis(word_at(data, 18) as u64);
    }

    config.send_queue.max_messages = word_at(data, 20);
    config.send_queue.max_bytes = word_at(data, 22);

    if let Some(mtu_safety) = config.mtu_safety.as_mut() {
        mtu_safety.max_udp_payload_bytes = word_at(data, 24);
        mtu_safety.drop_oversize = byte_at(data, 26).is_multiple_of(2);
    }

    config.reconnect_policy.backoff_factor = (byte_at(data, 27) as f64) / 64.0;
    config.reconnect_policy.jitter = (byte_at(data, 28) as f64) / 128.0;

    config.validate()
}

fn word_at(data: &[u8], offset: usize) -> usize {
    let bytes = [
        data.get(offset).copied().unwrap_or_default(),
        data.get(offset + 1).copied().unwrap_or_default(),
    ];
    usize::from(u16::from_be_bytes(bytes))
}

fn byte_at(data: &[u8], offset: usize) -> u8 {
    data.get(offset).copied().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use std::time::Duration;

    use rustak_limits::Limits;
    use rustak_wire::{DowngradePolicy, NegotiationEventKind, NegotiationReason, WireFormat};
    use tokio::io::duplex;

    use crate::{
        envelope, TransportConfig, TransportConfigError, TransportConnection, TransportFraming,
        TransportReceiver, TransportSender,
    };

    #[test]
    fn defaults_validate() {
        let cfg = TransportConfig::default();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn rejects_send_queue_messages_above_limits() {
        let mut cfg = TransportConfig::default();
        cfg.send_queue.max_messages = cfg.limits.max_queue_messages + 1;

        assert_eq!(
            cfg.validate(),
            Err(TransportConfigError::SendQueueMessagesExceedLimits {
                max_messages: cfg.send_queue.max_messages,
                limits_max_messages: cfg.limits.max_queue_messages,
            })
        );
    }

    #[test]
    fn rejects_keepalive_timeout_exceeding_interval() {
        let mut cfg = TransportConfig::default();
        let keepalive = cfg.keepalive.as_mut().expect("default keepalive");
        keepalive.interval = Duration::from_secs(3);
        keepalive.timeout = Duration::from_secs(10);

        assert_eq!(
            cfg.validate(),
            Err(TransportConfigError::KeepaliveTimeoutExceedsInterval {
                timeout: Duration::from_secs(10),
                interval: Duration::from_secs(3),
            })
        );
    }

    #[test]
    fn rejects_send_queue_bytes_above_limits() {
        let mut cfg = TransportConfig::default();
        cfg.send_queue.max_bytes = cfg.limits.max_queue_bytes + 1;

        assert_eq!(
            cfg.validate(),
            Err(TransportConfigError::SendQueueBytesExceedLimits {
                max_bytes: cfg.send_queue.max_bytes,
                limits_max_bytes: cfg.limits.max_queue_bytes,
            })
        );
    }

    #[test]
    fn fuzz_hook_handles_arbitrary_bytes_without_panicking() {
        let corpus = [
            &[][..],
            &[0u8; 1][..],
            &[0u8; 6][..],
            &[255u8; 64][..],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9][..],
        ];

        for sample in corpus {
            let _ = super::fuzz_hook_validate_transport_config(sample);
        }
    }

    #[test]
    fn envelope_helper_wraps_message_with_default_metadata() {
        let value = String::from("transport-message");
        let env = envelope(value.clone());
        assert_eq!(env.message, value);
        assert!(env.peer.is_none());
        assert!(env.raw_frame.is_none());
    }

    #[tokio::test]
    async fn sender_receiver_round_trip_xml_delimited_framing() {
        let (client, server) = duplex(128);
        let limits = Limits {
            max_frame_bytes: 64,
            max_xml_scan_bytes: 64,
            max_protobuf_bytes: 64,
            ..Limits::default()
        };
        let cfg = TransportConfig {
            wire_format: WireFormat::Xml,
            limits,
            mtu_safety: None,
            ..TransportConfig::default()
        };

        let mut sender = TransportSender::new(client, &cfg).expect("config should be valid");
        let mut receiver = TransportReceiver::new(server, &cfg).expect("config should be valid");

        sender
            .send_frame(b"<event uid=\"transport\"/>")
            .await
            .expect("send should succeed");
        let frame = receiver.recv_frame().await.expect("receive should succeed");

        assert_eq!(sender.framing(), TransportFraming::XmlNewlineDelimited);
        assert_eq!(receiver.framing(), TransportFraming::XmlNewlineDelimited);
        assert_eq!(frame, b"<event uid=\"transport\"/>");
    }

    #[tokio::test]
    async fn sender_receiver_round_trip_tak_length_prefixed_framing() {
        let (client, server) = duplex(128);
        let limits = Limits {
            max_frame_bytes: 64,
            max_xml_scan_bytes: 64,
            max_protobuf_bytes: 64,
            ..Limits::default()
        };
        let cfg = TransportConfig {
            wire_format: WireFormat::TakProtocolV1,
            limits,
            mtu_safety: None,
            ..TransportConfig::default()
        };

        let mut sender = TransportSender::new(client, &cfg).expect("config should be valid");
        let mut receiver = TransportReceiver::new(server, &cfg).expect("config should be valid");

        sender
            .send_frame(&[0xDE, 0xAD, 0xBE, 0xEF])
            .await
            .expect("send should succeed");
        let frame = receiver.recv_frame().await.expect("receive should succeed");

        assert_eq!(
            sender.framing(),
            TransportFraming::TakProtocolU32LengthPrefixed
        );
        assert_eq!(
            receiver.framing(),
            TransportFraming::TakProtocolU32LengthPrefixed
        );
        assert_eq!(frame, &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[tokio::test]
    async fn connection_wraps_frames_and_reuses_wire_negotiator() {
        let (client, server) = duplex(128);
        let limits = Limits {
            max_frame_bytes: 64,
            max_xml_scan_bytes: 64,
            max_protobuf_bytes: 64,
            ..Limits::default()
        };
        let cfg = TransportConfig {
            wire_format: WireFormat::TakProtocolV1,
            limits,
            mtu_safety: None,
            ..TransportConfig::default()
        };

        let mut connection = TransportConnection::new(client, &cfg, DowngradePolicy::FailOpen)
            .expect("connection should build");
        let mut receiver = TransportReceiver::new(server, &cfg).expect("receiver should build");

        connection.begin_upgrade_attempt();
        let event = connection.observe_timeout();
        assert_eq!(event.kind, NegotiationEventKind::FallbackToLegacy);
        assert_eq!(event.reason, Some(NegotiationReason::Timeout));

        connection
            .send_frame(b"<tak-proto/>")
            .await
            .expect("send should succeed");
        let envelope = receiver
            .recv_envelope()
            .await
            .expect("receive envelope should succeed");

        assert_eq!(envelope.message, b"<tak-proto/>");
        assert_eq!(
            envelope.raw_frame,
            Some(Bytes::from_static(b"<tak-proto/>"))
        );
    }
}
