use std::time::Duration;

use rustak_limits::{Limits, LimitsError};
use thiserror::Error;

pub mod correlator;
pub mod dedup;
pub mod mapping;
pub mod time_policy;

pub use correlator::{CorrelationInput, Correlator, CorrelatorConfig, CorrelatorError, UidPolicy};
pub use dedup::{DedupConfig, DedupConfigError, DedupDecision, Deduplicator};
pub use mapping::{BehaviourMapping, MappingSeverity, MappingTables, MappingValidationError};
pub use time_policy::{ResolvedCotTimes, TimePolicy, TimePolicyMode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeConfig {
    pub limits: Limits,
    pub cot_stale_seconds: u32,
    pub max_clock_skew_seconds: u32,
    pub time_policy: TimePolicyMode,
    pub dedup: DedupConfig,
    pub emitter: EmitterConfig,
    pub validation: BridgeValidationConfig,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        let limits = Limits::default();
        Self {
            limits: limits.clone(),
            cot_stale_seconds: 15,
            max_clock_skew_seconds: 5,
            time_policy: TimePolicyMode::ObservedWithSkewClamp,
            dedup: DedupConfig {
                window: Duration::from_millis(500),
                max_keys: limits.max_queue_messages,
            },
            emitter: EmitterConfig {
                max_updates_per_second: 20,
                min_separation: Duration::from_millis(100),
                max_pending_events: limits.max_queue_messages,
            },
            validation: BridgeValidationConfig::default(),
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
        self.dedup.validate(self.limits.max_queue_messages)?;
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
        self.validation.validate()?;

        Ok(())
    }

    #[must_use]
    pub fn build_time_policy(&self) -> TimePolicy {
        TimePolicy::new(
            self.time_policy,
            Duration::from_secs(u64::from(self.max_clock_skew_seconds)),
            Duration::from_secs(u64::from(self.cot_stale_seconds)),
        )
    }

    pub fn validate_with_mappings(
        &self,
        mappings: &MappingTables,
    ) -> Result<(), BridgeConfigError> {
        self.validate()?;
        mappings.validate_with_policy(&self.validation)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmitterConfig {
    pub max_updates_per_second: u32,
    pub min_separation: Duration,
    pub max_pending_events: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeValidationConfig {
    pub strict_startup: bool,
    pub unknown_class_fallback: String,
    pub classification_mapping_entries: usize,
    pub behaviour_mapping_entries: usize,
}

impl Default for BridgeValidationConfig {
    fn default() -> Self {
        Self {
            strict_startup: true,
            unknown_class_fallback: "a-u-A-M-F-Q".to_owned(),
            classification_mapping_entries: 1,
            behaviour_mapping_entries: 1,
        }
    }
}

impl BridgeValidationConfig {
    fn validate(&self) -> Result<(), BridgeConfigError> {
        if !self.strict_startup {
            return Ok(());
        }

        if self.unknown_class_fallback.trim().is_empty() {
            return Err(BridgeConfigError::EmptyUnknownClassFallback);
        }
        if self.classification_mapping_entries == 0 {
            return Err(BridgeConfigError::ZeroClassificationMappingCoverage);
        }
        if self.behaviour_mapping_entries == 0 {
            return Err(BridgeConfigError::ZeroBehaviourMappingCoverage);
        }

        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BridgeConfigError {
    #[error(transparent)]
    InvalidLimits(#[from] LimitsError),

    #[error(transparent)]
    InvalidDedup(#[from] DedupConfigError),

    #[error(transparent)]
    InvalidMappings(#[from] MappingValidationError),

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

    #[error("validation.unknown_class_fallback must not be empty in strict startup mode")]
    EmptyUnknownClassFallback,

    #[error("validation.classification_mapping_entries must be > 0 in strict startup mode")]
    ZeroClassificationMappingCoverage,

    #[error("validation.behaviour_mapping_entries must be > 0 in strict startup mode")]
    ZeroBehaviourMappingCoverage,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{
        BehaviourMapping, BridgeConfig, BridgeConfigError, BridgeValidationConfig, DedupConfig,
        DedupConfigError, MappingSeverity, MappingTables, TimePolicyMode,
    };

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

    #[test]
    fn strict_startup_requires_mapping_coverage() {
        let config = BridgeConfig {
            validation: BridgeValidationConfig {
                strict_startup: true,
                unknown_class_fallback: "a-u-A-M-F-Q".to_owned(),
                classification_mapping_entries: 0,
                behaviour_mapping_entries: 1,
            },
            ..BridgeConfig::default()
        };

        let error = config
            .validate()
            .expect_err("strict mode should reject missing classification mapping coverage");
        assert_eq!(error, BridgeConfigError::ZeroClassificationMappingCoverage);
    }

    #[test]
    fn non_strict_startup_allows_empty_mapping_coverage() {
        let config = BridgeConfig {
            validation: BridgeValidationConfig {
                strict_startup: false,
                unknown_class_fallback: String::new(),
                classification_mapping_entries: 0,
                behaviour_mapping_entries: 0,
            },
            ..BridgeConfig::default()
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn rejects_zero_dedup_window() {
        let config = BridgeConfig {
            dedup: DedupConfig {
                window: Duration::ZERO,
                max_keys: 128,
            },
            ..BridgeConfig::default()
        };

        let error = config
            .validate()
            .expect_err("zero dedup window must fail validation");
        assert_eq!(
            error,
            BridgeConfigError::InvalidDedup(DedupConfigError::ZeroWindow)
        );
    }

    #[test]
    fn build_time_policy_reflects_configured_mode_and_windows() {
        let config = BridgeConfig {
            time_policy: TimePolicyMode::MessageTime,
            max_clock_skew_seconds: 9,
            cot_stale_seconds: 21,
            ..BridgeConfig::default()
        };

        let policy = config.build_time_policy();
        assert_eq!(policy.mode, TimePolicyMode::MessageTime);
        assert_eq!(policy.max_clock_skew, Duration::from_secs(9));
        assert_eq!(policy.cot_stale, Duration::from_secs(21));
    }

    #[test]
    fn strict_mapping_validation_rejects_incomplete_tables() {
        let config = BridgeConfig::default();
        let mappings = MappingTables {
            class_to_cot: Default::default(),
            behaviour_to_detail: [(
                "Loitering".to_owned(),
                BehaviourMapping {
                    detail_key: "sapient.behaviour".to_owned(),
                    severity: MappingSeverity::Warning,
                },
            )]
            .into_iter()
            .collect(),
        };

        let error = config
            .validate_with_mappings(&mappings)
            .expect_err("strict mode should reject empty classification mappings");
        assert!(matches!(error, BridgeConfigError::InvalidMappings(_)));
    }
}
