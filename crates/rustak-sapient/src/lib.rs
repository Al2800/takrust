use std::time::Duration;

use rustak_limits::{Limits, LimitsError};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SapientConfig {
    pub limits: Limits,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub tcp_nodelay: bool,
}

impl Default for SapientConfig {
    fn default() -> Self {
        Self {
            limits: Limits::default(),
            read_timeout: Duration::from_secs(15),
            write_timeout: Duration::from_secs(15),
            tcp_nodelay: true,
        }
    }
}

impl SapientConfig {
    pub fn validate(&self) -> Result<(), SapientConfigError> {
        self.limits.validate()?;
        ensure_non_zero_duration("read_timeout", self.read_timeout)?;
        ensure_non_zero_duration("write_timeout", self.write_timeout)?;
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SapientConfigError {
    #[error(transparent)]
    InvalidLimits(#[from] LimitsError),

    #[error("{field} must be greater than zero")]
    ZeroDuration { field: &'static str },
}

fn ensure_non_zero_duration(
    field: &'static str,
    duration: Duration,
) -> Result<(), SapientConfigError> {
    if duration.is_zero() {
        return Err(SapientConfigError::ZeroDuration { field });
    }

    Ok(())
}

#[doc(hidden)]
pub fn fuzz_hook_validate_sapient_config(data: &[u8]) -> Result<(), SapientConfigError> {
    let mut config = SapientConfig::default();
    config.limits.max_frame_bytes = word_at(data, 0);
    config.limits.max_xml_scan_bytes = word_at(data, 2);
    config.limits.max_protobuf_bytes = word_at(data, 4);
    config.limits.max_queue_messages = word_at(data, 6);
    config.limits.max_queue_bytes = word_at(data, 8);
    config.limits.max_detail_elements = word_at(data, 10);
    config.read_timeout = Duration::from_millis(word_at(data, 12) as u64);
    config.write_timeout = Duration::from_millis(word_at(data, 14) as u64);
    config.tcp_nodelay = byte_at(data, 16) % 2 == 0;
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

    use crate::{SapientConfig, SapientConfigError};

    #[test]
    fn defaults_validate() {
        let cfg = SapientConfig::default();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn rejects_zero_read_timeout() {
        let mut cfg = SapientConfig::default();
        cfg.read_timeout = Duration::ZERO;
        assert_eq!(
            cfg.validate(),
            Err(SapientConfigError::ZeroDuration {
                field: "read_timeout"
            })
        );
    }

    #[test]
    fn rejects_invalid_limits_from_central_contract() {
        let mut cfg = SapientConfig::default();
        cfg.limits.max_frame_bytes = 0;

        let error = cfg
            .validate()
            .expect_err("invalid shared limits must propagate");
        assert!(matches!(error, SapientConfigError::InvalidLimits(_)));
    }

    #[test]
    fn fuzz_hook_handles_arbitrary_bytes_without_panicking() {
        let corpus = [
            &[][..],
            &[0u8; 2][..],
            &[0u8; 6][..],
            &[255u8; 32][..],
            &[10, 20, 30, 40, 50, 60][..],
        ];

        for sample in corpus {
            let _ = super::fuzz_hook_validate_sapient_config(sample);
        }
    }
}
