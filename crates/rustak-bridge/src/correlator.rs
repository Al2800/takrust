use std::collections::BTreeMap;

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UidPolicy {
    StablePerObject,
    StablePerDetection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrelatorConfig {
    pub uid_policy: UidPolicy,
    pub uid_prefix: String,
}

impl Default for CorrelatorConfig {
    fn default() -> Self {
        Self {
            uid_policy: UidPolicy::StablePerObject,
            uid_prefix: "trk".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrelationInput {
    pub node_id: String,
    pub object_id: Option<String>,
    pub detection_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrelationEntry {
    pub key: String,
    pub uid: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CorrelatorSnapshot {
    pub entries: Vec<CorrelationEntry>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CorrelatorError {
    #[error("uid_prefix must not be empty")]
    EmptyUidPrefix,

    #[error("node_id must not be empty")]
    EmptyNodeId,

    #[error("object_id is required for stable_per_object policy")]
    MissingObjectId,

    #[error("detection_id is required for stable_per_detection policy")]
    MissingDetectionId,
}

pub struct Correlator {
    config: CorrelatorConfig,
    key_to_uid: BTreeMap<String, String>,
    uid_to_key: BTreeMap<String, String>,
}

impl Correlator {
    pub fn new(config: CorrelatorConfig) -> Result<Self, CorrelatorError> {
        if config.uid_prefix.trim().is_empty() {
            return Err(CorrelatorError::EmptyUidPrefix);
        }

        Ok(Self {
            config,
            key_to_uid: BTreeMap::new(),
            uid_to_key: BTreeMap::new(),
        })
    }

    pub fn correlate(&mut self, input: &CorrelationInput) -> Result<String, CorrelatorError> {
        let key = input.canonical_key(self.config.uid_policy)?;
        if let Some(existing_uid) = self.key_to_uid.get(&key) {
            return Ok(existing_uid.clone());
        }

        let uid = self.allocate_uid(&key);
        self.key_to_uid.insert(key.clone(), uid.clone());
        self.uid_to_key.insert(uid.clone(), key);
        Ok(uid)
    }

    #[must_use]
    pub fn snapshot(&self) -> CorrelatorSnapshot {
        CorrelatorSnapshot {
            entries: self
                .key_to_uid
                .iter()
                .map(|(key, uid)| CorrelationEntry {
                    key: key.clone(),
                    uid: uid.clone(),
                })
                .collect(),
        }
    }

    pub fn restore_from_snapshot(&mut self, snapshot: &CorrelatorSnapshot) {
        self.key_to_uid.clear();
        self.uid_to_key.clear();

        for entry in &snapshot.entries {
            self.key_to_uid.insert(entry.key.clone(), entry.uid.clone());
            self.uid_to_key.insert(entry.uid.clone(), entry.key.clone());
        }
    }

    fn allocate_uid(&self, key: &str) -> String {
        let mut salt: u64 = 0;
        loop {
            let candidate = deterministic_uid(&self.config.uid_prefix, key, salt);
            if !self.uid_to_key.contains_key(&candidate) {
                return candidate;
            }
            salt = salt.saturating_add(1);
        }
    }
}

impl CorrelationInput {
    fn canonical_key(&self, policy: UidPolicy) -> Result<String, CorrelatorError> {
        if self.node_id.trim().is_empty() {
            return Err(CorrelatorError::EmptyNodeId);
        }

        match policy {
            UidPolicy::StablePerObject => {
                let object_id = self
                    .object_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or(CorrelatorError::MissingObjectId)?;

                Ok(format!("node={};object={object_id}", self.node_id.trim()))
            }
            UidPolicy::StablePerDetection => {
                let detection_id = self
                    .detection_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or(CorrelatorError::MissingDetectionId)?;

                Ok(format!(
                    "node={};detection={detection_id}",
                    self.node_id.trim()
                ))
            }
        }
    }
}

fn deterministic_uid(prefix: &str, key: &str, salt: u64) -> String {
    let first_hash = fnv1a64(format!("{prefix}|{key}|{salt}|a").as_bytes());
    let second_hash = fnv1a64(format!("{prefix}|{key}|{salt}|b").as_bytes());
    format!("{prefix}-{first_hash:016x}{second_hash:016x}")
}

fn fnv1a64(input: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    let mut hash = OFFSET_BASIS;
    for byte in input {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use crate::{CorrelationInput, Correlator, CorrelatorConfig, CorrelatorError, UidPolicy};

    #[test]
    fn stable_per_object_reuses_uid_for_same_object_across_detections() {
        let mut correlator =
            Correlator::new(CorrelatorConfig::default()).expect("default config should be valid");

        let first_uid = correlator
            .correlate(&CorrelationInput {
                node_id: "sensor-a".to_owned(),
                object_id: Some("obj-001".to_owned()),
                detection_id: Some("det-100".to_owned()),
            })
            .expect("correlation should succeed");
        let second_uid = correlator
            .correlate(&CorrelationInput {
                node_id: "sensor-a".to_owned(),
                object_id: Some("obj-001".to_owned()),
                detection_id: Some("det-101".to_owned()),
            })
            .expect("correlation should succeed");

        assert_eq!(first_uid, second_uid);
    }

    #[test]
    fn stable_per_detection_distinguishes_detection_ids() {
        let mut correlator = Correlator::new(CorrelatorConfig {
            uid_policy: UidPolicy::StablePerDetection,
            uid_prefix: "trk".to_owned(),
        })
        .expect("config should be valid");

        let first_uid = correlator
            .correlate(&CorrelationInput {
                node_id: "sensor-a".to_owned(),
                object_id: Some("obj-001".to_owned()),
                detection_id: Some("det-100".to_owned()),
            })
            .expect("correlation should succeed");
        let second_uid = correlator
            .correlate(&CorrelationInput {
                node_id: "sensor-a".to_owned(),
                object_id: Some("obj-001".to_owned()),
                detection_id: Some("det-101".to_owned()),
            })
            .expect("correlation should succeed");

        assert_ne!(first_uid, second_uid);
    }

    #[test]
    fn deterministic_uid_is_stable_across_instances() {
        let input = CorrelationInput {
            node_id: "sensor-a".to_owned(),
            object_id: Some("obj-001".to_owned()),
            detection_id: Some("det-100".to_owned()),
        };

        let mut first =
            Correlator::new(CorrelatorConfig::default()).expect("config should be valid");
        let mut second =
            Correlator::new(CorrelatorConfig::default()).expect("config should be valid");

        let first_uid = first.correlate(&input).expect("correlation should succeed");
        let second_uid = second
            .correlate(&input)
            .expect("correlation should succeed");
        assert_eq!(first_uid, second_uid);
    }

    #[test]
    fn snapshot_roundtrip_preserves_correlations() {
        let mut first =
            Correlator::new(CorrelatorConfig::default()).expect("config should be valid");
        let input = CorrelationInput {
            node_id: "sensor-a".to_owned(),
            object_id: Some("obj-001".to_owned()),
            detection_id: Some("det-100".to_owned()),
        };
        let expected_uid = first.correlate(&input).expect("correlation should succeed");
        let snapshot = first.snapshot();

        let mut restored =
            Correlator::new(CorrelatorConfig::default()).expect("config should be valid");
        restored.restore_from_snapshot(&snapshot);

        let restored_uid = restored
            .correlate(&input)
            .expect("restored correlator should keep deterministic uid");
        assert_eq!(restored_uid, expected_uid);
    }

    #[test]
    fn rejects_missing_required_object_id() {
        let mut correlator =
            Correlator::new(CorrelatorConfig::default()).expect("config should be valid");

        let error = correlator
            .correlate(&CorrelationInput {
                node_id: "sensor-a".to_owned(),
                object_id: None,
                detection_id: Some("det-100".to_owned()),
            })
            .expect_err("stable_per_object should require object id");

        assert_eq!(error, CorrelatorError::MissingObjectId);
    }
}
