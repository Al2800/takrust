use std::time::Duration;

use rustak_limits::{Limits, LimitsError};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeConfig {
    pub limits: Limits,
    pub cot_stale_seconds: u32,
    pub max_clock_skew_seconds: u32,
    pub emitter: EmitterConfig,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        let limits = Limits::default();
        Self {
            limits: limits.clone(),
            cot_stale_seconds: 15,
            max_clock_skew_seconds: 5,
            emitter: EmitterConfig {
                max_updates_per_second: 20,
                min_separation: Duration::from_millis(100),
                max_pending_events: limits.max_queue_messages,
            },
        }
    }
}

impl BridgeConfig {
    pub fn validate(&self) -> Result<(), BridgeConfigError> {
        self.limits.validate()?;

        if self.cot_stale_seconds == 0 {
            return Err(BridgeConfigError::ZeroCotStaleSeconds);
        }
        if self.max_clock_skew_seconds == 0 {
            return Err(BridgeConfigError::ZeroMaxClockSkewSeconds);
        }
        if self.emitter.max_updates_per_second == 0 {
            return Err(BridgeConfigError::ZeroEmitterRateLimit);
        }
        if self.emitter.min_separation.is_zero() {
            return Err(BridgeConfigError::ZeroEmitterMinSeparation);
        }
        if self.emitter.max_pending_events == 0 {
            return Err(BridgeConfigError::ZeroEmitterPendingEvents);
        }
        if self.emitter.max_pending_events > self.limits.max_queue_messages {
            return Err(BridgeConfigError::EmitterPendingEventsExceedLimits {
                max_pending_events: self.emitter.max_pending_events,
                max_queue_messages: self.limits.max_queue_messages,
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmitterConfig {
    pub max_updates_per_second: u32,
    pub min_separation: Duration,
    pub max_pending_events: usize,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BridgeConfigError {
    #[error(transparent)]
    InvalidLimits(#[from] LimitsError),

    #[error("cot_stale_seconds must be > 0")]
    ZeroCotStaleSeconds,

    #[error("max_clock_skew_seconds must be > 0")]
    ZeroMaxClockSkewSeconds,

    #[error("emitter.max_updates_per_second must be > 0")]
    ZeroEmitterRateLimit,

    #[error("emitter.min_separation must be > 0")]
    ZeroEmitterMinSeparation,

    #[error("emitter.max_pending_events must be > 0")]
    ZeroEmitterPendingEvents,

    #[error(
        "emitter.max_pending_events ({max_pending_events}) cannot exceed limits.max_queue_messages ({max_queue_messages})"
    )]
    EmitterPendingEventsExceedLimits {
        max_pending_events: usize,
        max_queue_messages: usize,
    },
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{BridgeConfig, BridgeConfigError};

    #[test]
    fn defaults_validate() {
        let config = BridgeConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn rejects_pending_events_above_limits() {
        let mut config = BridgeConfig::default();
        config.emitter.max_pending_events = config.limits.max_queue_messages + 1;

        let error = config
            .validate()
            .expect_err("pending events above limits must fail");
        assert!(matches!(
            error,
            BridgeConfigError::EmitterPendingEventsExceedLimits { .. }
        ));
    }

    #[test]
    fn rejects_zero_emitter_min_separation() {
        let mut config = BridgeConfig::default();
        config.emitter.min_separation = Duration::ZERO;

        let error = config
            .validate()
            .expect_err("zero emitter min separation must fail");
        assert_eq!(error, BridgeConfigError::ZeroEmitterMinSeparation);
    }
}
