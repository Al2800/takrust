mod error;

pub use error::LimitsError;

/// Shared, validated resource budgets for TAK/SAPIENT boundaries.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    pub max_frame_bytes: usize,
    pub max_xml_scan_bytes: usize,
    pub max_protobuf_bytes: usize,
    pub max_queue_messages: usize,
    pub max_queue_bytes: usize,
    pub max_detail_elements: usize,
}

impl Limits {
    pub const DEFAULT_MAX_FRAME_BYTES: usize = 1_048_576;
    pub const DEFAULT_MAX_XML_SCAN_BYTES: usize = 1_048_576;
    pub const DEFAULT_MAX_PROTOBUF_BYTES: usize = 1_048_576;
    pub const DEFAULT_MAX_QUEUE_MESSAGES: usize = 1_024;
    pub const DEFAULT_MAX_QUEUE_BYTES: usize = 8_388_608;
    pub const DEFAULT_MAX_DETAIL_ELEMENTS: usize = 512;

    /// Safe defaults aligned with the architecture config example.
    #[must_use]
    pub const fn conservative_defaults() -> Self {
        Self {
            max_frame_bytes: Self::DEFAULT_MAX_FRAME_BYTES,
            max_xml_scan_bytes: Self::DEFAULT_MAX_XML_SCAN_BYTES,
            max_protobuf_bytes: Self::DEFAULT_MAX_PROTOBUF_BYTES,
            max_queue_messages: Self::DEFAULT_MAX_QUEUE_MESSAGES,
            max_queue_bytes: Self::DEFAULT_MAX_QUEUE_BYTES,
            max_detail_elements: Self::DEFAULT_MAX_DETAIL_ELEMENTS,
        }
    }

    /// Validate internal invariants before exposing limits to boundary crates.
    pub fn validate(&self) -> Result<(), LimitsError> {
        ensure_non_zero("max_frame_bytes", self.max_frame_bytes)?;
        ensure_non_zero("max_xml_scan_bytes", self.max_xml_scan_bytes)?;
        ensure_non_zero("max_protobuf_bytes", self.max_protobuf_bytes)?;
        ensure_non_zero("max_queue_messages", self.max_queue_messages)?;
        ensure_non_zero("max_queue_bytes", self.max_queue_bytes)?;
        ensure_non_zero("max_detail_elements", self.max_detail_elements)?;

        if self.max_xml_scan_bytes > self.max_frame_bytes {
            return Err(LimitsError::XmlScanExceedsFrame {
                max_xml_scan_bytes: self.max_xml_scan_bytes,
                max_frame_bytes: self.max_frame_bytes,
            });
        }

        if self.max_protobuf_bytes > self.max_frame_bytes {
            return Err(LimitsError::ProtobufExceedsFrame {
                max_protobuf_bytes: self.max_protobuf_bytes,
                max_frame_bytes: self.max_frame_bytes,
            });
        }

        if self.max_queue_bytes < self.max_frame_bytes {
            return Err(LimitsError::QueueBytesBelowFrame {
                max_queue_bytes: self.max_queue_bytes,
                max_frame_bytes: self.max_frame_bytes,
            });
        }

        if self.max_queue_messages > self.max_queue_bytes {
            return Err(LimitsError::QueueMessagesExceedQueueBytes {
                max_queue_messages: self.max_queue_messages,
                max_queue_bytes: self.max_queue_bytes,
            });
        }

        Ok(())
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self::conservative_defaults()
    }
}

fn ensure_non_zero(field: &'static str, value: usize) -> Result<(), LimitsError> {
    if value == 0 {
        return Err(LimitsError::Zero { field });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Limits, LimitsError};

    #[test]
    fn conservative_defaults_validate() {
        let limits = Limits::conservative_defaults();
        assert!(limits.validate().is_ok());
    }

    #[test]
    fn rejects_zero_values() {
        let mut limits = Limits::conservative_defaults();
        limits.max_queue_messages = 0;

        assert_eq!(
            limits.validate(),
            Err(LimitsError::Zero {
                field: "max_queue_messages"
            })
        );
    }

    #[test]
    fn rejects_xml_scan_exceeding_frame() {
        let mut limits = Limits::conservative_defaults();
        limits.max_xml_scan_bytes = limits.max_frame_bytes + 1;

        assert_eq!(
            limits.validate(),
            Err(LimitsError::XmlScanExceedsFrame {
                max_xml_scan_bytes: limits.max_xml_scan_bytes,
                max_frame_bytes: limits.max_frame_bytes
            })
        );
    }

    #[test]
    fn rejects_protobuf_exceeding_frame() {
        let mut limits = Limits::conservative_defaults();
        limits.max_protobuf_bytes = limits.max_frame_bytes + 1;

        assert_eq!(
            limits.validate(),
            Err(LimitsError::ProtobufExceedsFrame {
                max_protobuf_bytes: limits.max_protobuf_bytes,
                max_frame_bytes: limits.max_frame_bytes
            })
        );
    }

    #[test]
    fn rejects_queue_smaller_than_frame() {
        let mut limits = Limits::conservative_defaults();
        limits.max_queue_bytes = limits.max_frame_bytes - 1;

        assert_eq!(
            limits.validate(),
            Err(LimitsError::QueueBytesBelowFrame {
                max_queue_bytes: limits.max_queue_bytes,
                max_frame_bytes: limits.max_frame_bytes
            })
        );
    }

    #[test]
    fn rejects_queue_message_count_above_queue_bytes() {
        let limits = Limits {
            max_frame_bytes: 128,
            max_xml_scan_bytes: 128,
            max_protobuf_bytes: 128,
            max_queue_messages: 512,
            max_queue_bytes: 256,
            max_detail_elements: 64,
        };

        assert_eq!(
            limits.validate(),
            Err(LimitsError::QueueMessagesExceedQueueBytes {
                max_queue_messages: limits.max_queue_messages,
                max_queue_bytes: limits.max_queue_bytes
            })
        );
    }
}
