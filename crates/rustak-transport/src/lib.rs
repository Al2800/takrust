use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use rustak_io::{MessageEnvelope, MessageSink, MessageSource};
use rustak_limits::{Limits, LimitsError};
use rustak_wire::WireFormat;
use thiserror::Error;

pub type TransportEnvelope<T> = MessageEnvelope<T>;
pub type TransportSink<T> = dyn MessageSink<T>;
pub type TransportSource<T> = dyn MessageSource<T>;

#[must_use]
pub fn envelope<T>(message: T) -> TransportEnvelope<T> {
    MessageEnvelope::new(message)
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendQueueConfig {
    pub max_messages: usize,
    pub max_bytes: usize,
    pub mode: SendQueueMode,
}

impl SendQueueConfig {
    fn validate(&self, limits: &Limits) -> Result<(), TransportConfigError> {
        if self.max_messages == 0 {
            return Err(TransportConfigError::ZeroSendQueueMessages);
        }
        if self.max_bytes == 0 {
            return Err(TransportConfigError::ZeroSendQueueBytes);
        }
        if self.max_messages > limits.max_queue_messages {
            return Err(TransportConfigError::SendQueueMessagesExceedLimits {
                max_messages: self.max_messages,
                limits_max_messages: limits.max_queue_messages,
            });
        }
        if self.max_bytes > limits.max_queue_bytes {
            return Err(TransportConfigError::SendQueueBytesExceedLimits {
                max_bytes: self.max_bytes,
                limits_max_bytes: limits.max_queue_bytes,
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendQueueMode {
    Fifo,
    Priority,
    CoalesceLatestByUid,
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
        mtu_safety.drop_oversize = byte_at(data, 26) % 2 == 0;
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
    use std::time::Duration;

    use crate::{envelope, TransportConfig, TransportConfigError};

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
}
