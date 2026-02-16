use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::time::{Duration, SystemTime};

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DedupConfig {
    pub window: Duration,
    pub max_keys: usize,
}

impl DedupConfig {
    pub fn validate(&self, max_queue_messages: usize) -> Result<(), DedupConfigError> {
        if self.window.is_zero() {
            return Err(DedupConfigError::ZeroWindow);
        }
        if self.max_keys == 0 {
            return Err(DedupConfigError::ZeroMaxKeys);
        }
        if self.max_keys > max_queue_messages {
            return Err(DedupConfigError::MaxKeysExceedLimits {
                max_keys: self.max_keys,
                max_queue_messages,
            });
        }

        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DedupConfigError {
    #[error("dedup.window must be > 0")]
    ZeroWindow,

    #[error("dedup.max_keys must be > 0")]
    ZeroMaxKeys,

    #[error(
        "dedup.max_keys ({max_keys}) cannot exceed limits.max_queue_messages ({max_queue_messages})"
    )]
    MaxKeysExceedLimits {
        max_keys: usize,
        max_queue_messages: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DedupDecision {
    Accepted,
    Duplicate,
}

pub struct Deduplicator<Key> {
    config: DedupConfig,
    seen: HashMap<Key, SystemTime>,
    order: VecDeque<(Key, SystemTime)>,
}

impl<Key> Deduplicator<Key>
where
    Key: Eq + Hash + Clone,
{
    pub fn new(config: DedupConfig, max_queue_messages: usize) -> Result<Self, DedupConfigError> {
        config.validate(max_queue_messages)?;
        Ok(Self {
            config,
            seen: HashMap::new(),
            order: VecDeque::new(),
        })
    }

    pub fn observe(&mut self, key: Key, observed_at: SystemTime) -> DedupDecision {
        self.prune_expired(observed_at);

        if let Some(previous_seen) = self.seen.get(&key) {
            if is_duplicate(*previous_seen, observed_at, self.config.window) {
                return DedupDecision::Duplicate;
            }
        }

        self.seen.insert(key.clone(), observed_at);
        self.order.push_back((key, observed_at));
        self.enforce_capacity();
        DedupDecision::Accepted
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }

    fn prune_expired(&mut self, observed_at: SystemTime) {
        while let Some((key, seen_at)) = self.order.front().cloned() {
            if !is_expired(seen_at, observed_at, self.config.window) {
                break;
            }

            self.order.pop_front();
            if matches!(self.seen.get(&key), Some(current_seen) if *current_seen == seen_at) {
                self.seen.remove(&key);
            }
        }
    }

    fn enforce_capacity(&mut self) {
        while self.seen.len() > self.config.max_keys {
            let Some((evicted_key, evicted_seen_at)) = self.order.pop_front() else {
                break;
            };

            if matches!(
                self.seen.get(&evicted_key),
                Some(current_seen) if *current_seen == evicted_seen_at
            ) {
                self.seen.remove(&evicted_key);
            }
        }
    }
}

fn is_duplicate(previous_seen: SystemTime, observed_at: SystemTime, window: Duration) -> bool {
    match observed_at.duration_since(previous_seen) {
        Ok(delta) => delta <= window,
        Err(_) => true,
    }
}

fn is_expired(seen_at: SystemTime, observed_at: SystemTime, window: Duration) -> bool {
    match observed_at.duration_since(seen_at) {
        Ok(age) => age > window,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use crate::{DedupConfig, DedupConfigError, DedupDecision, Deduplicator};

    fn observed_at(seconds: u64, millis: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds) + Duration::from_millis(millis)
    }

    #[test]
    fn rejects_zero_window() {
        let config = DedupConfig {
            window: Duration::ZERO,
            max_keys: 1,
        };
        assert_eq!(
            config.validate(4),
            Err(DedupConfigError::ZeroWindow),
            "zero dedup window should fail validation"
        );
    }

    #[test]
    fn rejects_max_keys_above_limits() {
        let config = DedupConfig {
            window: Duration::from_millis(500),
            max_keys: 10,
        };
        assert_eq!(
            config.validate(5),
            Err(DedupConfigError::MaxKeysExceedLimits {
                max_keys: 10,
                max_queue_messages: 5,
            }),
            "max_keys above shared queue limits must fail"
        );
    }

    #[test]
    fn accepts_first_occurrence() {
        let mut deduplicator = Deduplicator::new(
            DedupConfig {
                window: Duration::from_millis(500),
                max_keys: 8,
            },
            8,
        )
        .expect("dedup config should be valid");

        assert_eq!(
            deduplicator.observe("alpha", observed_at(10, 0)),
            DedupDecision::Accepted
        );
    }

    #[test]
    fn rejects_duplicate_within_window() {
        let mut deduplicator = Deduplicator::new(
            DedupConfig {
                window: Duration::from_millis(500),
                max_keys: 8,
            },
            8,
        )
        .expect("dedup config should be valid");

        assert_eq!(
            deduplicator.observe("alpha", observed_at(10, 0)),
            DedupDecision::Accepted
        );
        assert_eq!(
            deduplicator.observe("alpha", observed_at(10, 300)),
            DedupDecision::Duplicate
        );
    }

    #[test]
    fn accepts_repeated_key_after_window_expiry() {
        let mut deduplicator = Deduplicator::new(
            DedupConfig {
                window: Duration::from_millis(500),
                max_keys: 8,
            },
            8,
        )
        .expect("dedup config should be valid");

        assert_eq!(
            deduplicator.observe("alpha", observed_at(10, 0)),
            DedupDecision::Accepted
        );
        assert_eq!(
            deduplicator.observe("alpha", observed_at(11, 0)),
            DedupDecision::Accepted,
            "same key after dedup window should be accepted as a new event"
        );
    }

    #[test]
    fn rejects_out_of_order_replay_for_same_key() {
        let mut deduplicator = Deduplicator::new(
            DedupConfig {
                window: Duration::from_millis(500),
                max_keys: 8,
            },
            8,
        )
        .expect("dedup config should be valid");

        assert_eq!(
            deduplicator.observe("alpha", observed_at(10, 800)),
            DedupDecision::Accepted
        );
        assert_eq!(
            deduplicator.observe("alpha", observed_at(10, 100)),
            DedupDecision::Duplicate,
            "older replay should be dropped deterministically"
        );
    }

    #[test]
    fn evicts_oldest_key_when_capacity_is_reached() {
        let mut deduplicator = Deduplicator::new(
            DedupConfig {
                window: Duration::from_secs(30),
                max_keys: 2,
            },
            2,
        )
        .expect("dedup config should be valid");

        assert_eq!(
            deduplicator.observe("alpha", observed_at(10, 0)),
            DedupDecision::Accepted
        );
        assert_eq!(
            deduplicator.observe("bravo", observed_at(10, 100)),
            DedupDecision::Accepted
        );
        assert_eq!(
            deduplicator.observe("charlie", observed_at(10, 200)),
            DedupDecision::Accepted
        );
        assert_eq!(deduplicator.len(), 2);
        assert_eq!(
            deduplicator.observe("alpha", observed_at(10, 300)),
            DedupDecision::Accepted,
            "oldest key should have been evicted and no longer considered duplicate"
        );
    }
}
