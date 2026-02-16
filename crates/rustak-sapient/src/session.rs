use std::collections::VecDeque;

use rustak_limits::Limits;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionDirection {
    Inbound,
    Outbound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SapientSessionBuffers {
    max_frame_bytes: usize,
    max_queue_messages: usize,
    max_queue_bytes: usize,
    inbound: VecDeque<Vec<u8>>,
    outbound: VecDeque<Vec<u8>>,
    inbound_bytes: usize,
    outbound_bytes: usize,
}

impl SapientSessionBuffers {
    #[must_use]
    pub fn from_limits(limits: &Limits) -> Self {
        Self {
            max_frame_bytes: limits.max_frame_bytes,
            max_queue_messages: limits.max_queue_messages,
            max_queue_bytes: limits.max_queue_bytes,
            inbound: VecDeque::new(),
            outbound: VecDeque::new(),
            inbound_bytes: 0,
            outbound_bytes: 0,
        }
    }

    #[must_use]
    pub const fn max_frame_bytes(&self) -> usize {
        self.max_frame_bytes
    }

    #[must_use]
    pub const fn max_queue_messages(&self) -> usize {
        self.max_queue_messages
    }

    #[must_use]
    pub const fn max_queue_bytes(&self) -> usize {
        self.max_queue_bytes
    }

    #[must_use]
    pub fn inbound_len(&self) -> usize {
        self.inbound.len()
    }

    #[must_use]
    pub fn outbound_len(&self) -> usize {
        self.outbound.len()
    }

    #[must_use]
    pub const fn inbound_bytes(&self) -> usize {
        self.inbound_bytes
    }

    #[must_use]
    pub const fn outbound_bytes(&self) -> usize {
        self.outbound_bytes
    }

    pub fn push_inbound(&mut self, payload: Vec<u8>) -> Result<(), SapientSessionError> {
        self.push(SessionDirection::Inbound, payload)
    }

    pub fn push_outbound(&mut self, payload: Vec<u8>) -> Result<(), SapientSessionError> {
        self.push(SessionDirection::Outbound, payload)
    }

    pub fn pop_inbound(&mut self) -> Option<Vec<u8>> {
        self.pop(SessionDirection::Inbound)
    }

    pub fn pop_outbound(&mut self) -> Option<Vec<u8>> {
        self.pop(SessionDirection::Outbound)
    }

    fn push(
        &mut self,
        direction: SessionDirection,
        payload: Vec<u8>,
    ) -> Result<(), SapientSessionError> {
        if payload.len() > self.max_frame_bytes {
            return Err(SapientSessionError::FrameTooLarge {
                direction,
                actual_bytes: payload.len(),
                max_frame_bytes: self.max_frame_bytes,
            });
        }

        let current_messages = match direction {
            SessionDirection::Inbound => self.inbound.len(),
            SessionDirection::Outbound => self.outbound.len(),
        };
        if current_messages >= self.max_queue_messages {
            return Err(SapientSessionError::QueueMessagesExceeded {
                direction,
                max_queue_messages: self.max_queue_messages,
            });
        }

        let current_bytes = match direction {
            SessionDirection::Inbound => self.inbound_bytes,
            SessionDirection::Outbound => self.outbound_bytes,
        };
        let attempted_bytes = current_bytes + payload.len();
        if attempted_bytes > self.max_queue_bytes {
            return Err(SapientSessionError::QueueBytesExceeded {
                direction,
                attempted_bytes,
                max_queue_bytes: self.max_queue_bytes,
            });
        }

        match direction {
            SessionDirection::Inbound => {
                self.inbound.push_back(payload);
                self.inbound_bytes = attempted_bytes;
            }
            SessionDirection::Outbound => {
                self.outbound.push_back(payload);
                self.outbound_bytes = attempted_bytes;
            }
        }

        Ok(())
    }

    fn pop(&mut self, direction: SessionDirection) -> Option<Vec<u8>> {
        match direction {
            SessionDirection::Inbound => self.inbound.pop_front().inspect(|payload| {
                self.inbound_bytes = self.inbound_bytes.saturating_sub(payload.len());
            }),
            SessionDirection::Outbound => self.outbound.pop_front().inspect(|payload| {
                self.outbound_bytes = self.outbound_bytes.saturating_sub(payload.len());
            }),
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SapientSessionError {
    #[error("{direction:?} frame size {actual_bytes} exceeds max_frame_bytes {max_frame_bytes}")]
    FrameTooLarge {
        direction: SessionDirection,
        actual_bytes: usize,
        max_frame_bytes: usize,
    },

    #[error("{direction:?} queue exceeds max_queue_messages {max_queue_messages}")]
    QueueMessagesExceeded {
        direction: SessionDirection,
        max_queue_messages: usize,
    },

    #[error(
        "{direction:?} queue bytes {attempted_bytes} exceed max_queue_bytes {max_queue_bytes}"
    )]
    QueueBytesExceeded {
        direction: SessionDirection,
        attempted_bytes: usize,
        max_queue_bytes: usize,
    },
}

#[cfg(test)]
mod tests {
    use rustak_limits::Limits;

    use super::{SapientSessionBuffers, SapientSessionError};

    fn limits(max_frame: usize, max_messages: usize, max_bytes: usize) -> Limits {
        Limits {
            max_frame_bytes: max_frame,
            max_xml_scan_bytes: max_frame,
            max_protobuf_bytes: max_frame,
            max_queue_messages: max_messages,
            max_queue_bytes: max_bytes,
            max_detail_elements: 8,
        }
    }

    #[test]
    fn rejects_inbound_frames_over_frame_limit() {
        let mut session = SapientSessionBuffers::from_limits(&limits(4, 4, 64));
        let error = session
            .push_inbound(b"12345".to_vec())
            .expect_err("oversize inbound frame must fail");
        assert!(matches!(error, SapientSessionError::FrameTooLarge { .. }));
    }

    #[test]
    fn rejects_queue_growth_beyond_message_limit() {
        let mut session = SapientSessionBuffers::from_limits(&limits(16, 1, 64));
        session
            .push_outbound(b"a".to_vec())
            .expect("first payload should fit");
        let error = session
            .push_outbound(b"b".to_vec())
            .expect_err("second payload should exceed message cap");
        assert!(matches!(
            error,
            SapientSessionError::QueueMessagesExceeded { .. }
        ));
    }

    #[test]
    fn rejects_queue_growth_beyond_byte_limit() {
        let mut session = SapientSessionBuffers::from_limits(&limits(16, 4, 5));
        session
            .push_inbound(b"abc".to_vec())
            .expect("first payload should fit");
        let error = session
            .push_inbound(b"def".to_vec())
            .expect_err("second payload should exceed byte cap");
        assert!(matches!(
            error,
            SapientSessionError::QueueBytesExceeded { .. }
        ));
    }

    #[test]
    fn preserves_fifo_order_with_bounded_accounting() {
        let mut session = SapientSessionBuffers::from_limits(&limits(32, 4, 64));
        session
            .push_inbound(b"first".to_vec())
            .expect("first should fit");
        session
            .push_inbound(b"second".to_vec())
            .expect("second should fit");

        assert_eq!(session.inbound_bytes(), b"first".len() + b"second".len());
        assert_eq!(session.pop_inbound(), Some(b"first".to_vec()));
        assert_eq!(session.pop_inbound(), Some(b"second".to_vec()));
        assert_eq!(session.inbound_bytes(), 0);
        assert_eq!(session.inbound_len(), 0);
    }
}
