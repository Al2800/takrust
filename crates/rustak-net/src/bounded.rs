use std::cmp::min;

use tokio::io::{AsyncRead, AsyncReadExt};

use crate::BoundedReadError;

const DEFAULT_CHUNK_SIZE: usize = 8 * 1024;
const DISCARD_BUFFER_SIZE: usize = 1024;

/// A reader wrapper that enforces a strict maximum number of bytes read.
#[derive(Debug)]
pub struct BoundedReader<R> {
    inner: R,
    max_bytes: usize,
    consumed: usize,
    chunk_size: usize,
}

impl<R> BoundedReader<R> {
    #[must_use]
    pub const fn new(inner: R, max_bytes: usize) -> Self {
        Self {
            inner,
            max_bytes,
            consumed: 0,
            chunk_size: DEFAULT_CHUNK_SIZE,
        }
    }

    #[must_use]
    pub const fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    #[must_use]
    pub const fn consumed(&self) -> usize {
        self.consumed
    }

    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.max_bytes.saturating_sub(self.consumed)
    }

    pub fn set_chunk_size(&mut self, chunk_size: usize) {
        self.chunk_size = chunk_size.max(1);
    }

    #[must_use]
    pub fn into_inner(self) -> R {
        self.inner
    }

    fn ensure_capacity(&self, additional: usize) -> Result<(), BoundedReadError> {
        let attempted = self
            .consumed
            .checked_add(additional)
            .ok_or(BoundedReadError::IntegerOverflow)?;

        if attempted > self.max_bytes {
            return Err(BoundedReadError::LimitExceeded {
                max_bytes: self.max_bytes,
                attempted,
            });
        }

        Ok(())
    }

    fn increment(&mut self, amount: usize) -> Result<(), BoundedReadError> {
        self.consumed = self
            .consumed
            .checked_add(amount)
            .ok_or(BoundedReadError::IntegerOverflow)?;
        Ok(())
    }
}

impl<R> BoundedReader<R>
where
    R: AsyncRead + Unpin,
{
    pub async fn read_exact(&mut self, byte_count: usize) -> Result<Vec<u8>, BoundedReadError> {
        self.ensure_capacity(byte_count)?;

        let mut buffer = vec![0_u8; byte_count];
        self.inner
            .read_exact(&mut buffer)
            .await
            .map_err(BoundedReadError::Io)?;
        self.increment(byte_count)?;
        Ok(buffer)
    }

    pub async fn read_up_to(&mut self, byte_count: usize) -> Result<Vec<u8>, BoundedReadError> {
        let target = min(byte_count, self.remaining());
        if target == 0 {
            return Ok(Vec::new());
        }

        let mut buffer = vec![0_u8; target];
        let read = self
            .inner
            .read(&mut buffer)
            .await
            .map_err(BoundedReadError::Io)?;
        buffer.truncate(read);
        self.increment(read)?;
        Ok(buffer)
    }

    pub async fn discard_exact(&mut self, byte_count: usize) -> Result<(), BoundedReadError> {
        self.ensure_capacity(byte_count)?;

        let mut remaining = byte_count;
        let mut scratch = [0_u8; DISCARD_BUFFER_SIZE];
        while remaining > 0 {
            let to_read = min(remaining, scratch.len());
            self.inner
                .read_exact(&mut scratch[..to_read])
                .await
                .map_err(BoundedReadError::Io)?;
            remaining -= to_read;
            self.increment(to_read)?;
        }

        Ok(())
    }

    pub async fn read_to_end(&mut self) -> Result<Vec<u8>, BoundedReadError> {
        let mut output = Vec::new();
        let chunk_size = self.chunk_size.max(1);

        loop {
            if self.remaining() == 0 {
                let mut probe = [0_u8; 1];
                let read = self
                    .inner
                    .read(&mut probe)
                    .await
                    .map_err(BoundedReadError::Io)?;

                if read == 0 {
                    return Ok(output);
                }

                return Err(BoundedReadError::LimitExceeded {
                    max_bytes: self.max_bytes,
                    attempted: self.consumed.saturating_add(read),
                });
            }

            let to_read = min(self.remaining(), chunk_size);
            let start = output.len();
            output.resize(start + to_read, 0);

            let read = self
                .inner
                .read(&mut output[start..])
                .await
                .map_err(BoundedReadError::Io)?;
            if read == 0 {
                output.truncate(start);
                return Ok(output);
            }

            output.truncate(start + read);
            self.increment(read)?;
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::{duplex, AsyncWriteExt};

    use super::BoundedReader;
    use crate::BoundedReadError;

    #[tokio::test]
    async fn read_exact_within_limit() {
        let (mut writer, reader) = duplex(32);
        tokio::spawn(async move {
            writer.write_all(b"hello").await.expect("write should work");
        });

        let mut bounded = BoundedReader::new(reader, 5);
        let frame = bounded.read_exact(5).await.expect("read should succeed");
        assert_eq!(frame, b"hello");
        assert_eq!(bounded.consumed(), 5);
    }

    #[tokio::test]
    async fn read_exact_rejects_limit_overrun() {
        let (_writer, reader) = duplex(32);
        let mut bounded = BoundedReader::new(reader, 4);
        let error = bounded
            .read_exact(5)
            .await
            .expect_err("bound check should fail");

        match error {
            BoundedReadError::LimitExceeded {
                max_bytes,
                attempted,
            } => {
                assert_eq!(max_bytes, 4);
                assert_eq!(attempted, 5);
            }
            _ => panic!("unexpected error variant"),
        }
    }

    #[tokio::test]
    async fn read_to_end_accepts_exact_limit_at_eof() {
        let (mut writer, reader) = duplex(64);
        tokio::spawn(async move {
            writer
                .write_all(b"exactly-five")
                .await
                .expect("write should work");
        });

        let mut bounded = BoundedReader::new(reader, 12);
        let output = bounded.read_to_end().await.expect("read should succeed");
        assert_eq!(output, b"exactly-five");
    }

    #[tokio::test]
    async fn read_to_end_rejects_extra_bytes_after_limit() {
        let (mut writer, reader) = duplex(64);
        tokio::spawn(async move {
            writer
                .write_all(b"too-many-bytes")
                .await
                .expect("write should work");
        });

        let mut bounded = BoundedReader::new(reader, 3);
        let error = bounded
            .read_to_end()
            .await
            .expect_err("extra bytes should fail");

        match error {
            BoundedReadError::LimitExceeded {
                max_bytes,
                attempted,
            } => {
                assert_eq!(max_bytes, 3);
                assert_eq!(attempted, 4);
            }
            _ => panic!("unexpected error variant"),
        }
    }
}
