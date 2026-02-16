use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BoundedReadError {
    #[error("read of {attempted} bytes would exceed max bound of {max_bytes} bytes")]
    LimitExceeded { max_bytes: usize, attempted: usize },

    #[error("bounded read arithmetic overflow while tracking bytes")]
    IntegerOverflow,

    #[error("I/O error: {0}")]
    Io(#[source] io::Error),
}

#[derive(Debug, Error)]
pub enum LengthPrefixedError {
    #[error("frame length {frame_len} exceeds max frame bound {max_frame_bytes}")]
    FrameTooLarge {
        frame_len: usize,
        max_frame_bytes: usize,
    },

    #[error("frame length {frame_len} cannot be represented by {prefix} prefix")]
    PrefixOverflow {
        prefix: &'static str,
        frame_len: usize,
    },

    #[error("varint prefix exceeded 10-byte maximum")]
    VarintTooLong,

    #[error("varint prefix overflows u64")]
    VarintOverflow,

    #[error("I/O error: {0}")]
    Io(#[source] io::Error),
}

#[derive(Debug, Error)]
pub enum DelimiterFrameError {
    #[error("delimiter cannot be empty")]
    EmptyDelimiter,

    #[error("scan length {scanned} exceeds max frame bound {max_frame_bytes}")]
    FrameTooLarge {
        max_frame_bytes: usize,
        scanned: usize,
    },

    #[error("stream ended before delimiter was found after scanning {scanned} bytes")]
    UnexpectedEof { scanned: usize },

    #[error("I/O error: {0}")]
    Io(#[source] io::Error),
}
