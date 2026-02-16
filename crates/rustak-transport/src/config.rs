use rustak_limits::Limits;

use crate::TransportConfigError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendQueueConfig {
    pub max_messages: usize,
    pub max_bytes: usize,
    pub mode: SendQueueMode,
}

impl SendQueueConfig {
    pub(crate) fn validate(&self, limits: &Limits) -> Result<(), TransportConfigError> {
        if self.max_messages == 0 {
            return Err(TransportConfigError::ZeroSendQueueMessages);
        }
        if self.max_bytes == 0 {
            return Err(TransportConfigError::ZeroSendQueueBytes);
        }
        if self.max_messages > limits.max_queue_messages {
            return Err(TransportConfigError::SendQueueMessagesExceedLimits {
                max_messages: self.max_messages,
                limits_max_messages: limits.max_queue_messages,
            });
        }
        if self.max_bytes > limits.max_queue_bytes {
            return Err(TransportConfigError::SendQueueBytesExceedLimits {
                max_bytes: self.max_bytes,
                limits_max_bytes: limits.max_queue_bytes,
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendQueueMode {
    Fifo,
    Priority,
    CoalesceLatestByUid,
}
