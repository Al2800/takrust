use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const NANOS_PER_SECOND: i128 = 1_000_000_000;

/// UTC timestamp represented as nanoseconds since Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimestampUtc {
    unix_nanos: i128,
}

impl TimestampUtc {
    /// The Unix epoch (`1970-01-01T00:00:00Z`).
    pub const UNIX_EPOCH: Self = Self { unix_nanos: 0 };

    /// Returns the current UTC timestamp derived from system clock.
    #[must_use]
    pub fn now() -> Self {
        Self::from_system_time(SystemTime::now())
    }

    /// Creates a timestamp from raw Unix nanoseconds.
    #[must_use]
    pub const fn from_unix_nanos(unix_nanos: i128) -> Self {
        Self { unix_nanos }
    }

    /// Creates a timestamp from Unix whole seconds.
    #[must_use]
    pub const fn from_unix_seconds(seconds: i64) -> Self {
        Self {
            unix_nanos: (seconds as i128) * NANOS_PER_SECOND,
        }
    }

    /// Creates a timestamp from Unix seconds and fractional nanoseconds.
    pub fn from_unix_seconds_nanos(seconds: i64, nanoseconds: u32) -> Result<Self, TimestampError> {
        if nanoseconds >= NANOS_PER_SECOND as u32 {
            return Err(TimestampError::InvalidNanoseconds { nanoseconds });
        }

        let unix_nanos = (seconds as i128) * NANOS_PER_SECOND + (nanoseconds as i128);
        Ok(Self { unix_nanos })
    }

    /// Creates a timestamp from system time.
    #[must_use]
    pub fn from_system_time(value: SystemTime) -> Self {
        match value.duration_since(UNIX_EPOCH) {
            Ok(delta) => Self {
                unix_nanos: duration_to_nanos(delta),
            },
            Err(err) => Self {
                unix_nanos: -duration_to_nanos(err.duration()),
            },
        }
    }

    /// Returns Unix nanoseconds from the epoch.
    #[must_use]
    pub const fn unix_nanos(self) -> i128 {
        self.unix_nanos
    }

    /// Returns Unix whole seconds from the epoch.
    #[must_use]
    pub const fn unix_seconds(self) -> i128 {
        self.unix_nanos.div_euclid(NANOS_PER_SECOND)
    }

    /// Returns the nanosecond fraction of the current second.
    #[must_use]
    pub const fn subsec_nanos(self) -> u32 {
        self.unix_nanos.rem_euclid(NANOS_PER_SECOND) as u32
    }

    /// Converts this timestamp to `SystemTime`.
    pub fn to_system_time(self) -> Result<SystemTime, TimestampError> {
        if self.unix_nanos >= 0 {
            let delta = absolute_nanos_to_duration(self.unix_nanos as u128, self.unix_nanos)?;
            UNIX_EPOCH
                .checked_add(delta)
                .ok_or(TimestampError::OutOfRangeForSystemTime {
                    unix_nanos: self.unix_nanos,
                })
        } else {
            let delta =
                absolute_nanos_to_duration(self.unix_nanos.unsigned_abs(), self.unix_nanos)?;
            UNIX_EPOCH
                .checked_sub(delta)
                .ok_or(TimestampError::OutOfRangeForSystemTime {
                    unix_nanos: self.unix_nanos,
                })
        }
    }
}

impl Default for TimestampUtc {
    fn default() -> Self {
        Self::UNIX_EPOCH
    }
}

#[cfg(feature = "chrono")]
impl From<chrono::DateTime<chrono::Utc>> for TimestampUtc {
    fn from(value: chrono::DateTime<chrono::Utc>) -> Self {
        Self::from_unix_seconds_nanos(value.timestamp(), value.timestamp_subsec_nanos())
            .expect("chrono DateTime provides valid sub-second nanoseconds")
    }
}

#[cfg(feature = "chrono")]
impl TryFrom<TimestampUtc> for chrono::DateTime<chrono::Utc> {
    type Error = TimestampError;

    fn try_from(value: TimestampUtc) -> Result<Self, Self::Error> {
        use chrono::TimeZone;

        let seconds = i64::try_from(value.unix_seconds()).map_err(|_| {
            TimestampError::OutOfRangeForChrono {
                unix_nanos: value.unix_nanos,
            }
        })?;

        chrono::Utc
            .timestamp_opt(seconds, value.subsec_nanos())
            .single()
            .ok_or(TimestampError::OutOfRangeForChrono {
                unix_nanos: value.unix_nanos,
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimestampError {
    InvalidNanoseconds { nanoseconds: u32 },
    OutOfRangeForSystemTime { unix_nanos: i128 },
    OutOfRangeForChrono { unix_nanos: i128 },
}

impl fmt::Display for TimestampError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNanoseconds { nanoseconds } => {
                write!(f, "nanoseconds must be < 1_000_000_000, got {nanoseconds}")
            }
            Self::OutOfRangeForSystemTime { unix_nanos } => {
                write!(f, "timestamp {unix_nanos}ns is out of range for SystemTime")
            }
            Self::OutOfRangeForChrono { unix_nanos } => {
                write!(
                    f,
                    "timestamp {unix_nanos}ns is out of range for chrono::DateTime<Utc>"
                )
            }
        }
    }
}

impl std::error::Error for TimestampError {}

fn duration_to_nanos(delta: Duration) -> i128 {
    (delta.as_secs() as i128) * NANOS_PER_SECOND + (delta.subsec_nanos() as i128)
}

fn absolute_nanos_to_duration(nanos: u128, source_nanos: i128) -> Result<Duration, TimestampError> {
    let seconds = nanos / (NANOS_PER_SECOND as u128);
    let subsec = (nanos % (NANOS_PER_SECOND as u128)) as u32;
    let seconds_u64 =
        u64::try_from(seconds).map_err(|_| TimestampError::OutOfRangeForSystemTime {
            unix_nanos: source_nanos,
        })?;
    Ok(Duration::new(seconds_u64, subsec))
}

#[cfg(test)]
mod tests {
    use super::{TimestampError, TimestampUtc};
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn from_seconds_and_nanos_rejects_invalid_fraction() {
        let result = TimestampUtc::from_unix_seconds_nanos(42, 1_000_000_000);
        assert_eq!(
            result,
            Err(TimestampError::InvalidNanoseconds {
                nanoseconds: 1_000_000_000
            })
        );
    }

    #[test]
    fn negative_timestamp_reports_canonical_subsecond() {
        let timestamp = TimestampUtc::from_unix_nanos(-1);
        assert_eq!(timestamp.unix_seconds(), -1);
        assert_eq!(timestamp.subsec_nanos(), 999_999_999);
    }

    #[test]
    fn system_time_roundtrip_after_epoch() {
        let system_time = UNIX_EPOCH + Duration::new(1_700_000_000, 123_456_789);
        let timestamp = TimestampUtc::from_system_time(system_time);
        let roundtrip = timestamp
            .to_system_time()
            .expect("roundtrip should succeed");
        assert_eq!(roundtrip, system_time);
    }

    #[test]
    fn system_time_roundtrip_before_epoch() {
        let system_time = UNIX_EPOCH - Duration::new(10, 42);
        let timestamp = TimestampUtc::from_system_time(system_time);
        let roundtrip = timestamp
            .to_system_time()
            .expect("roundtrip should succeed");
        assert_eq!(roundtrip, system_time);
    }

    #[cfg(feature = "chrono")]
    #[test]
    fn chrono_roundtrip_preserves_value() {
        use chrono::TimeZone;

        let date_time = chrono::Utc
            .timestamp_opt(1_700_000_000, 987_654_321)
            .single()
            .expect("valid datetime");
        let timestamp = TimestampUtc::from(date_time);
        let roundtrip = chrono::DateTime::<chrono::Utc>::try_from(timestamp)
            .expect("chrono conversion should succeed");
        assert_eq!(roundtrip, date_time);
    }
}
