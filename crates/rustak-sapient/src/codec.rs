use rustak_limits::Limits;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{SapientConfig, SapientFrameCodec, SapientFrameError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SapientCodec {
    frame_codec: SapientFrameCodec,
    max_frame_bytes: usize,
}

impl SapientCodec {
    #[must_use]
    pub fn from_limits(limits: &Limits) -> Self {
        Self {
            frame_codec: SapientFrameCodec::from_limits(limits),
            max_frame_bytes: limits.max_frame_bytes,
        }
    }

    #[must_use]
    pub fn from_config(config: &SapientConfig) -> Self {
        Self::from_limits(&config.limits)
    }

    #[must_use]
    pub const fn max_frame_bytes(&self) -> usize {
        self.max_frame_bytes
    }

    pub fn validate_payload(&self, payload: &[u8]) -> Result<(), SapientCodecError> {
        if payload.len() > self.max_frame_bytes {
            return Err(SapientCodecError::FrameTooLarge {
                actual_bytes: payload.len(),
                max_frame_bytes: self.max_frame_bytes,
            });
        }
        Ok(())
    }

    pub async fn read_message<R>(&self, reader: &mut R) -> Result<Vec<u8>, SapientCodecError>
    where
        R: AsyncRead + Unpin,
    {
        let payload = self.frame_codec.read_message(reader).await?;
        self.validate_payload(&payload)?;
        Ok(payload)
    }

    pub async fn write_message<W>(
        &self,
        writer: &mut W,
        payload: &[u8],
    ) -> Result<(), SapientCodecError>
    where
        W: AsyncWrite + Unpin,
    {
        self.validate_payload(payload)?;
        self.frame_codec.write_message(writer, payload).await?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum SapientCodecError {
    #[error("payload size {actual_bytes} exceeds max_frame_bytes {max_frame_bytes}")]
    FrameTooLarge {
        actual_bytes: usize,
        max_frame_bytes: usize,
    },

    #[error(transparent)]
    Frame(#[from] SapientFrameError),
}

#[cfg(test)]
mod tests {
    use rustak_limits::Limits;
    use tokio::io::{duplex, AsyncWriteExt};

    use super::{SapientCodec, SapientCodecError};

    #[test]
    fn validate_payload_rejects_oversize_messages() {
        let limits = Limits {
            max_frame_bytes: 4,
            max_xml_scan_bytes: 4,
            max_protobuf_bytes: 4,
            max_queue_messages: 8,
            max_queue_bytes: 128,
            max_detail_elements: 8,
        };
        let codec = SapientCodec::from_limits(&limits);

        let error = codec
            .validate_payload(b"12345")
            .expect_err("oversize payload must be rejected");
        assert!(matches!(error, SapientCodecError::FrameTooLarge { .. }));
    }

    #[tokio::test]
    async fn read_write_round_trip_uses_limits_aware_codec() {
        let limits = Limits {
            max_frame_bytes: 64,
            max_xml_scan_bytes: 64,
            max_protobuf_bytes: 64,
            max_queue_messages: 8,
            max_queue_bytes: 256,
            max_detail_elements: 8,
        };
        let codec = SapientCodec::from_limits(&limits);

        let (mut writer, mut reader) = duplex(128);
        tokio::spawn(async move {
            writer
                .write_all(&[0x00, 0x00, 0x00, 0x03])
                .await
                .expect("prefix should write");
            writer
                .write_all(b"tak")
                .await
                .expect("payload should write");
        });

        let decoded = codec
            .read_message(&mut reader)
            .await
            .expect("frame should decode");
        assert_eq!(decoded, b"tak");
    }
}
