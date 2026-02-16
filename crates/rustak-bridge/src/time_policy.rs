use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimePolicyMode {
    MessageTime,
    ObservedTime,
    ObservedWithSkewClamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimePolicy {
    pub mode: TimePolicyMode,
    pub max_clock_skew: Duration,
    pub cot_stale: Duration,
}

impl TimePolicy {
    #[must_use]
    pub const fn new(mode: TimePolicyMode, max_clock_skew: Duration, cot_stale: Duration) -> Self {
        Self {
            mode,
            max_clock_skew,
            cot_stale,
        }
    }

    #[must_use]
    pub fn resolve(
        &self,
        message_time: Option<SystemTime>,
        observed_time: SystemTime,
    ) -> ResolvedCotTimes {
        let resolved_time = match self.mode {
            TimePolicyMode::MessageTime => message_time.unwrap_or(observed_time),
            TimePolicyMode::ObservedTime => observed_time,
            TimePolicyMode::ObservedWithSkewClamp => {
                let candidate = message_time.unwrap_or(observed_time);
                clamp_to_observed_window(candidate, observed_time, self.max_clock_skew)
            }
        };

        let stale = resolved_time
            .checked_add(self.cot_stale)
            .unwrap_or(resolved_time);
        ResolvedCotTimes {
            time: resolved_time,
            start: resolved_time,
            stale,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedCotTimes {
    pub time: SystemTime,
    pub start: SystemTime,
    pub stale: SystemTime,
}

fn clamp_to_observed_window(
    candidate: SystemTime,
    observed: SystemTime,
    max_clock_skew: Duration,
) -> SystemTime {
    let lower = observed
        .checked_sub(max_clock_skew)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    if candidate < lower {
        return lower;
    }

    if let Some(upper) = observed.checked_add(max_clock_skew) {
        if candidate > upper {
            return upper;
        }
    }

    candidate
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use crate::{TimePolicy, TimePolicyMode};

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    #[test]
    fn message_time_policy_prefers_message_time() {
        let policy = TimePolicy::new(
            TimePolicyMode::MessageTime,
            Duration::from_secs(5),
            Duration::from_secs(15),
        );

        let resolved = policy.resolve(Some(at(120)), at(100));
        assert_eq!(resolved.time, at(120));
        assert_eq!(resolved.start, at(120));
        assert_eq!(resolved.stale, at(135));
    }

    #[test]
    fn message_time_policy_falls_back_to_observed_time() {
        let policy = TimePolicy::new(
            TimePolicyMode::MessageTime,
            Duration::from_secs(5),
            Duration::from_secs(15),
        );

        let resolved = policy.resolve(None, at(100));
        assert_eq!(resolved.time, at(100));
        assert_eq!(resolved.start, at(100));
        assert_eq!(resolved.stale, at(115));
    }

    #[test]
    fn observed_time_policy_ignores_message_time() {
        let policy = TimePolicy::new(
            TimePolicyMode::ObservedTime,
            Duration::from_secs(5),
            Duration::from_secs(15),
        );

        let resolved = policy.resolve(Some(at(140)), at(100));
        assert_eq!(resolved.time, at(100));
        assert_eq!(resolved.start, at(100));
        assert_eq!(resolved.stale, at(115));
    }

    #[test]
    fn skew_clamp_clamps_future_message_time() {
        let policy = TimePolicy::new(
            TimePolicyMode::ObservedWithSkewClamp,
            Duration::from_secs(5),
            Duration::from_secs(15),
        );

        let resolved = policy.resolve(Some(at(120)), at(100));
        assert_eq!(
            resolved.time,
            at(105),
            "future message time above skew window must be clamped"
        );
    }

    #[test]
    fn skew_clamp_clamps_past_message_time() {
        let policy = TimePolicy::new(
            TimePolicyMode::ObservedWithSkewClamp,
            Duration::from_secs(5),
            Duration::from_secs(15),
        );

        let resolved = policy.resolve(Some(at(90)), at(100));
        assert_eq!(
            resolved.time,
            at(95),
            "past message time below skew window must be clamped"
        );
    }

    #[test]
    fn skew_clamp_keeps_message_time_within_window() {
        let policy = TimePolicy::new(
            TimePolicyMode::ObservedWithSkewClamp,
            Duration::from_secs(5),
            Duration::from_secs(15),
        );

        let resolved = policy.resolve(Some(at(103)), at(100));
        assert_eq!(resolved.time, at(103));
        assert_eq!(resolved.start, at(103));
        assert_eq!(resolved.stale, at(118));
    }
}
