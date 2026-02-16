use std::collections::BTreeMap;

use thiserror::Error;

use crate::BridgeValidationConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BehaviourMapping {
    pub detail_key: String,
    pub severity: MappingSeverity,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MappingTables {
    pub class_to_cot: BTreeMap<String, String>,
    pub behaviour_to_detail: BTreeMap<String, BehaviourMapping>,
}

impl MappingTables {
    pub fn validate_with_policy(
        &self,
        policy: &BridgeValidationConfig,
    ) -> Result<(), MappingValidationError> {
        if !policy.strict_startup {
            return Ok(());
        }

        if policy.unknown_class_fallback.trim().is_empty() {
            return Err(MappingValidationError::EmptyUnknownClassFallback);
        }

        if self.class_to_cot.len() < policy.classification_mapping_entries {
            return Err(MappingValidationError::InsufficientClassificationCoverage {
                required: policy.classification_mapping_entries,
                actual: self.class_to_cot.len(),
            });
        }

        if self.behaviour_to_detail.len() < policy.behaviour_mapping_entries {
            return Err(MappingValidationError::InsufficientBehaviourCoverage {
                required: policy.behaviour_mapping_entries,
                actual: self.behaviour_to_detail.len(),
            });
        }

        for (classification, cot_type) in &self.class_to_cot {
            if classification.trim().is_empty() {
                return Err(MappingValidationError::EmptyClassificationKey);
            }
            if cot_type.trim().is_empty() {
                return Err(MappingValidationError::EmptyCotType {
                    classification: classification.clone(),
                });
            }
        }

        for (behaviour, mapping) in &self.behaviour_to_detail {
            if behaviour.trim().is_empty() {
                return Err(MappingValidationError::EmptyBehaviourKey);
            }
            if mapping.detail_key.trim().is_empty() {
                return Err(MappingValidationError::EmptyBehaviourDetailKey {
                    behaviour: behaviour.clone(),
                });
            }
        }

        Ok(())
    }

    #[must_use]
    pub fn map_classification<'a>(&'a self, classification: &str, fallback: &'a str) -> &'a str {
        self.class_to_cot
            .get(classification)
            .map(String::as_str)
            .unwrap_or(fallback)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MappingValidationError {
    #[error("validation.unknown_class_fallback must not be empty in strict startup mode")]
    EmptyUnknownClassFallback,

    #[error(
        "classification mapping coverage is insufficient for strict startup: required {required}, found {actual}"
    )]
    InsufficientClassificationCoverage { required: usize, actual: usize },

    #[error(
        "behaviour mapping coverage is insufficient for strict startup: required {required}, found {actual}"
    )]
    InsufficientBehaviourCoverage { required: usize, actual: usize },

    #[error("classification mapping contains an empty classification key")]
    EmptyClassificationKey,

    #[error("classification mapping for '{classification}' must provide a non-empty CoT type")]
    EmptyCotType { classification: String },

    #[error("behaviour mapping contains an empty behaviour key")]
    EmptyBehaviourKey,

    #[error("behaviour mapping for '{behaviour}' must provide a non-empty detail_key")]
    EmptyBehaviourDetailKey { behaviour: String },
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{
        BehaviourMapping, BridgeValidationConfig, MappingSeverity, MappingTables,
        MappingValidationError,
    };

    fn strict_policy() -> BridgeValidationConfig {
        BridgeValidationConfig {
            strict_startup: true,
            unknown_class_fallback: "a-u-A-M-F-Q".to_owned(),
            classification_mapping_entries: 1,
            behaviour_mapping_entries: 1,
        }
    }

    fn valid_tables() -> MappingTables {
        MappingTables {
            class_to_cot: [("UAS/Multirotor".to_owned(), "a-h-A-M-F-Q".to_owned())]
                .into_iter()
                .collect(),
            behaviour_to_detail: [(
                "Loitering".to_owned(),
                BehaviourMapping {
                    detail_key: "sapient.behaviour".to_owned(),
                    severity: MappingSeverity::Warning,
                },
            )]
            .into_iter()
            .collect(),
        }
    }

    #[test]
    fn strict_mode_rejects_empty_unknown_fallback() {
        let tables = valid_tables();
        let policy = BridgeValidationConfig {
            strict_startup: true,
            unknown_class_fallback: String::new(),
            classification_mapping_entries: 1,
            behaviour_mapping_entries: 1,
        };

        let error = tables
            .validate_with_policy(&policy)
            .expect_err("strict startup should reject empty fallback");
        assert_eq!(error, MappingValidationError::EmptyUnknownClassFallback);
    }

    #[test]
    fn strict_mode_rejects_insufficient_coverage() {
        let tables = MappingTables::default();
        let error = tables
            .validate_with_policy(&strict_policy())
            .expect_err("strict startup should reject empty mapping tables");
        assert_eq!(
            error,
            MappingValidationError::InsufficientClassificationCoverage {
                required: 1,
                actual: 0,
            }
        );
    }

    #[test]
    fn strict_mode_rejects_empty_behaviour_detail_key() {
        let mut tables = valid_tables();
        tables.behaviour_to_detail = [(
            "Loitering".to_owned(),
            BehaviourMapping {
                detail_key: String::new(),
                severity: MappingSeverity::Warning,
            },
        )]
        .into_iter()
        .collect();

        let error = tables
            .validate_with_policy(&strict_policy())
            .expect_err("strict startup should reject empty detail keys");
        assert_eq!(
            error,
            MappingValidationError::EmptyBehaviourDetailKey {
                behaviour: "Loitering".to_owned(),
            }
        );
    }

    #[test]
    fn non_strict_mode_allows_incomplete_tables() {
        let tables = MappingTables::default();
        let policy = BridgeValidationConfig {
            strict_startup: false,
            unknown_class_fallback: String::new(),
            classification_mapping_entries: 5,
            behaviour_mapping_entries: 5,
        };
        assert!(tables.validate_with_policy(&policy).is_ok());
    }

    #[test]
    fn map_classification_uses_fallback_for_unknown_values() {
        let tables = valid_tables();
        assert_eq!(
            tables.map_classification("UAS/Multirotor", "a-u-A-M-F-Q"),
            "a-h-A-M-F-Q"
        );
        assert_eq!(
            tables.map_classification("Unknown", "a-u-A-M-F-Q"),
            "a-u-A-M-F-Q"
        );
    }

    #[test]
    fn strict_mode_rejects_empty_classification_key() {
        let mut tables = valid_tables();
        tables.class_to_cot = [("".to_owned(), "a-h-A-M-F-Q".to_owned())]
            .into_iter()
            .collect::<BTreeMap<_, _>>();

        let error = tables
            .validate_with_policy(&strict_policy())
            .expect_err("strict startup should reject empty classification keys");
        assert_eq!(error, MappingValidationError::EmptyClassificationKey);
    }
}
