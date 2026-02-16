use thiserror::Error;

/// Validation failures for the shared `Limits` contract.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LimitsError {
    #[error("{field} must be greater than zero")]
    Zero { field: &'static str },

    #[error(
        "max_xml_scan_bytes ({max_xml_scan_bytes}) cannot exceed max_frame_bytes ({max_frame_bytes})"
    )]
    XmlScanExceedsFrame {
        max_xml_scan_bytes: usize,
        max_frame_bytes: usize,
    },

    #[error(
        "max_protobuf_bytes ({max_protobuf_bytes}) cannot exceed max_frame_bytes ({max_frame_bytes})"
    )]
    ProtobufExceedsFrame {
        max_protobuf_bytes: usize,
        max_frame_bytes: usize,
    },

    #[error("max_queue_bytes ({max_queue_bytes}) must be >= max_frame_bytes ({max_frame_bytes})")]
    QueueBytesBelowFrame {
        max_queue_bytes: usize,
        max_frame_bytes: usize,
    },

    #[error(
        "max_queue_messages ({max_queue_messages}) cannot exceed max_queue_bytes ({max_queue_bytes}); \
         each queued message requires at least one byte"
    )]
    QueueMessagesExceedQueueBytes {
        max_queue_messages: usize,
        max_queue_bytes: usize,
    },
}
