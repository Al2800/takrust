use rustak_limits::Limits;
use rustak_net::{
    read_length_prefixed_frame, write_length_prefixed_frame, LengthPrefixKind, LengthPrefixedError,
};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SapientFrameCodec {
    max_frame_bytes: usize,
}

impl SapientFrameCodec {
    #[must_use]
    pub const fn new(max_frame_bytes: usize) -> Self {
        Self { max_frame_bytes }
    }

    #[must_use]
    pub const fn from_limits(limits: &Limits) -> Self {
        Self::new(limits.max_frame_bytes)
    }

    #[must_use]
    pub const fn max_frame_bytes(&self) -> usize {
        self.max_frame_bytes
    }

    pub async fn read_message<R>(&self, reader: &mut R) -> Result<Vec<u8>, SapientFrameError>
    where
        R: AsyncRead + Unpin,
    {
        read_length_prefixed_frame(reader, LengthPrefixKind::U32Be, self.max_frame_bytes)
            .await
            .map_err(SapientFrameError::from)
    }

    pub async fn write_message<W>(
        &self,
        writer: &mut W,
        payload: &[u8],
    ) -> Result<(), SapientFrameError>
    where
        W: AsyncWrite + Unpin,
    {
        write_length_prefixed_frame(
            writer,
            LengthPrefixKind::U32Be,
            payload,
            self.max_frame_bytes,
        )
        .await
        .map_err(SapientFrameError::from)
    }
}

#[derive(Debug, Error)]
pub enum SapientFrameError {
    #[error(transparent)]
    LengthPrefixed(#[from] LengthPrefixedError),
}

#[cfg(test)]
mod tests {
    use tokio::io::{duplex, AsyncWriteExt};

    use super::{SapientFrameCodec, SapientFrameError};
    use rustak_net::LengthPrefixedError;

    #[tokio::test]
    async fn sapient_messages_round_trip_with_u32_prefix() {
        let (mut writer, mut reader) = duplex(128);
        let payload = b"sapient-frame-payload".to_vec();
        let write_codec = SapientFrameCodec::new(256);

        let write_payload = payload.clone();
        tokio::spawn(async move {
            write_codec
                .write_message(&mut writer, &write_payload)
                .await
                .expect("write should succeed");
        });

        let read_codec = SapientFrameCodec::new(256);
        let decoded = read_codec
            .read_message(&mut reader)
            .await
            .expect("read should succeed");
        assert_eq!(decoded, payload);
    }

    #[tokio::test]
    async fn write_rejects_payload_over_limit() {
        let (mut writer, _reader) = duplex(128);
        let codec = SapientFrameCodec::new(4);

        let error = codec
            .write_message(&mut writer, b"12345")
            .await
            .expect_err("payload should exceed limit");
        match error {
            SapientFrameError::LengthPrefixed(LengthPrefixedError::FrameTooLarge { .. }) => {}
            _ => panic!("unexpected error variant"),
        }
    }

    #[tokio::test]
    async fn read_rejects_prefix_over_limit() {
        let (mut writer, mut reader) = duplex(128);
        tokio::spawn(async move {
            writer
                .write_all(&[0x00, 0x00, 0x00, 0x05])
                .await
                .expect("prefix write should succeed");
            writer
                .write_all(b"12345")
                .await
                .expect("payload write should succeed");
        });

        let codec = SapientFrameCodec::new(4);
        let error = codec
            .read_message(&mut reader)
            .await
            .expect_err("prefixed payload should exceed limit");
        match error {
            SapientFrameError::LengthPrefixed(LengthPrefixedError::FrameTooLarge { .. }) => {}
            _ => panic!("unexpected error variant"),
        }
    }
}
