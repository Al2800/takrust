use std::time::Duration;

use rustak_wire::TakProtocolVersion;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommoConfig {
    pub takcontrol_interval: Duration,
    pub contact_stale_after: Duration,
    pub max_messages_per_tick: usize,
}

impl Default for CommoConfig {
    fn default() -> Self {
        Self {
            takcontrol_interval: Duration::from_secs(60),
            contact_stale_after: Duration::from_secs(120),
            max_messages_per_tick: 128,
        }
    }
}

impl CommoConfig {
    pub fn validate(&self) -> Result<(), CommoConfigError> {
        if self.takcontrol_interval.is_zero() {
            return Err(CommoConfigError::ZeroTakControlInterval);
        }

        if self.contact_stale_after < self.takcontrol_interval {
            return Err(CommoConfigError::StaleBeforeCadence {
                stale_after: self.contact_stale_after,
                cadence: self.takcontrol_interval,
            });
        }

        if self.max_messages_per_tick == 0 {
            return Err(CommoConfigError::ZeroMessageBudget);
        }

        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CommoConfigError {
    #[error("takcontrol_interval must be greater than zero")]
    ZeroTakControlInterval,

    #[error("contact_stale_after ({stale_after:?}) must be >= takcontrol_interval ({cadence:?})")]
    StaleBeforeCadence {
        stale_after: Duration,
        cadence: Duration,
    },

    #[error("max_messages_per_tick must be greater than zero")]
    ZeroMessageBudget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactCapabilities {
    pub uid: String,
    pub supported_versions: Vec<TakProtocolVersion>,
}

impl ContactCapabilities {
    pub fn new(
        uid: impl Into<String>,
        supported_versions: Vec<TakProtocolVersion>,
    ) -> Result<Self, ContactError> {
        let uid = uid.into();
        if uid.trim().is_empty() {
            return Err(ContactError::EmptyUid);
        }

        if supported_versions.is_empty() {
            return Err(ContactError::NoVersions);
        }

        Ok(Self {
            uid,
            supported_versions,
        })
    }

    #[must_use]
    pub fn supports(&self, version: TakProtocolVersion) -> bool {
        self.supported_versions
            .iter()
            .any(|candidate| *candidate == version)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContactError {
    #[error("uid must not be empty")]
    EmptyUid,

    #[error("contact must advertise at least one protocol version")]
    NoVersions,
}

#[must_use]
pub fn select_mesh_version(contacts: &[ContactCapabilities]) -> Option<TakProtocolVersion> {
    if contacts.is_empty() {
        return None;
    }

    if contacts
        .iter()
        .all(|contact| contact.supports(TakProtocolVersion::V1))
    {
        return Some(TakProtocolVersion::V1);
    }

    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CadenceBudget {
    max_messages_per_tick: usize,
    consumed_this_tick: usize,
}

impl CadenceBudget {
    #[must_use]
    pub const fn new(max_messages_per_tick: usize) -> Self {
        Self {
            max_messages_per_tick,
            consumed_this_tick: 0,
        }
    }

    pub fn try_consume(&mut self, messages: usize) -> bool {
        if messages == 0 {
            return true;
        }

        let Some(updated) = self.consumed_this_tick.checked_add(messages) else {
            return false;
        };

        if updated > self.max_messages_per_tick {
            return false;
        }

        self.consumed_this_tick = updated;
        true
    }

    pub fn reset_tick(&mut self) {
        self.consumed_this_tick = 0;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rustak_wire::TakProtocolVersion;

    use crate::{
        select_mesh_version, CadenceBudget, CommoConfig, CommoConfigError, ContactCapabilities,
    };

    #[test]
    fn config_rejects_stale_window_shorter_than_cadence() {
        let config = CommoConfig {
            takcontrol_interval: Duration::from_secs(30),
            contact_stale_after: Duration::from_secs(10),
            max_messages_per_tick: 64,
        };

        assert_eq!(
            config.validate(),
            Err(CommoConfigError::StaleBeforeCadence {
                stale_after: Duration::from_secs(10),
                cadence: Duration::from_secs(30),
            })
        );
    }

    #[test]
    fn mesh_version_is_selected_when_all_contacts_support_v1() {
        let alpha =
            ContactCapabilities::new("alpha", vec![TakProtocolVersion::V1]).expect("valid");
        let bravo =
            ContactCapabilities::new("bravo", vec![TakProtocolVersion::V1]).expect("valid");

        assert_eq!(
            select_mesh_version(&[alpha, bravo]),
            Some(TakProtocolVersion::V1)
        );
    }

    #[test]
    fn cadence_budget_rejects_consumption_above_tick_limit() {
        let mut budget = CadenceBudget::new(3);

        assert!(budget.try_consume(2));
        assert!(!budget.try_consume(2));

        budget.reset_tick();
        assert!(budget.try_consume(3));
    }
}
