#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReplayObservation {
    pub stream_id: String,
    pub sequence: u64,
    pub timestamp_nanos: u64,
    pub uid: String,
    pub cot_type: String,
    pub classification: String,
    pub behavior: String,
    pub confidence: f64,
}

#[derive(Debug)]
pub enum HarnessError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for HarnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::Json(error) => write!(f, "JSON parse error: {error}"),
        }
    }
}

impl std::error::Error for HarnessError {}

impl From<std::io::Error> for HarnessError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for HarnessError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub fn load_replay_fixture(path: impl AsRef<Path>) -> Result<Vec<ReplayObservation>, HarnessError> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice::<Vec<ReplayObservation>>(&bytes)?)
}

#[must_use]
pub fn canonicalize(observations: &[ReplayObservation]) -> Vec<ReplayObservation> {
    let mut canonical = observations.to_vec();
    canonical.sort_by(|left, right| {
        (
            left.stream_id.as_str(),
            left.sequence,
            left.timestamp_nanos,
            left.uid.as_str(),
            left.cot_type.as_str(),
        )
            .cmp(&(
                right.stream_id.as_str(),
                right.sequence,
                right.timestamp_nanos,
                right.uid.as_str(),
                right.cot_type.as_str(),
            ))
    });
    canonical
}

#[must_use]
pub fn deterministic_replay_digest(observations: &[ReplayObservation]) -> String {
    let canonical = canonicalize(observations);
    let payload = serde_json::to_vec(&canonical)
        .expect("serializing canonicalized replay observations should not fail");
    let mut hasher = Sha256::new();
    hasher.update(payload);
    let digest = hasher.finalize();
    format!("{digest:x}")
}
