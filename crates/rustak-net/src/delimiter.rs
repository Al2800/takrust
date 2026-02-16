use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::DelimiterFrameError;

pub async fn read_delimited_frame<R>(
    reader: &mut R,
    delimiter: &[u8],
    max_frame_bytes: usize,
    include_delimiter: bool,
) -> Result<Vec<u8>, DelimiterFrameError>
where
    R: AsyncRead + Unpin,
{
    if delimiter.is_empty() {
        return Err(DelimiterFrameError::EmptyDelimiter);
    }

    let mut frame = Vec::new();
    let mut byte = [0_u8; 1];

    loop {
        let read = reader
            .read(&mut byte)
            .await
            .map_err(DelimiterFrameError::Io)?;
        if read == 0 {
            return Err(DelimiterFrameError::UnexpectedEof {
                scanned: frame.len(),
            });
        }

        frame.push(byte[0]);
        if frame.len() > max_frame_bytes {
            return Err(DelimiterFrameError::FrameTooLarge {
                max_frame_bytes,
                scanned: frame.len(),
            });
        }

        if frame.ends_with(delimiter) {
            if !include_delimiter {
                let len_without_delimiter = frame.len() - delimiter.len();
                frame.truncate(len_without_delimiter);
            }
            return Ok(frame);
        }
    }
}

pub async fn write_delimited_frame<W>(
    writer: &mut W,
    payload: &[u8],
    delimiter: &[u8],
    max_frame_bytes: usize,
) -> Result<(), DelimiterFrameError>
where
    W: AsyncWrite + Unpin,
{
    if delimiter.is_empty() {
        return Err(DelimiterFrameError::EmptyDelimiter);
    }

    let total_len =
        payload
            .len()
            .checked_add(delimiter.len())
            .ok_or(DelimiterFrameError::FrameTooLarge {
                max_frame_bytes,
                scanned: usize::MAX,
            })?;
    if total_len > max_frame_bytes {
        return Err(DelimiterFrameError::FrameTooLarge {
            max_frame_bytes,
            scanned: total_len,
        });
    }

    writer
        .write_all(payload)
        .await
        .map_err(DelimiterFrameError::Io)?;
    writer
        .write_all(delimiter)
        .await
        .map_err(DelimiterFrameError::Io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use tokio::io::{duplex, AsyncWriteExt};

    use super::{read_delimited_frame, write_delimited_frame};
    use crate::DelimiterFrameError;

    #[tokio::test]
    async fn reads_until_delimiter_and_strips_by_default() {
        let (mut writer, mut reader) = duplex(64);
        tokio::spawn(async move {
            writer
                .write_all(b"alpha\nrest")
                .await
                .expect("write should work");
        });

        let frame = read_delimited_frame(&mut reader, b"\n", 16, false)
            .await
            .expect("frame should decode");
        assert_eq!(frame, b"alpha");
    }

    #[tokio::test]
    async fn reads_until_delimiter_and_keeps_marker_when_requested() {
        let (mut writer, mut reader) = duplex(64);
        tokio::spawn(async move {
            writer
                .write_all(b"alpha\nrest")
                .await
                .expect("write should work");
        });

        let frame = read_delimited_frame(&mut reader, b"\n", 16, true)
            .await
            .expect("frame should decode");
        assert_eq!(frame, b"alpha\n");
    }

    #[tokio::test]
    async fn rejects_scan_over_limit() {
        let (mut writer, mut reader) = duplex(64);
        tokio::spawn(async move {
            writer
                .write_all(b"abcdef")
                .await
                .expect("write should work");
        });

        let error = read_delimited_frame(&mut reader, b"\n", 4, false)
            .await
            .expect_err("scan should exceed limit");
        match error {
            DelimiterFrameError::FrameTooLarge {
                max_frame_bytes,
                scanned,
            } => {
                assert_eq!(max_frame_bytes, 4);
                assert_eq!(scanned, 5);
            }
            _ => panic!("unexpected error variant"),
        }
    }

    #[tokio::test]
    async fn write_rejects_frames_that_exceed_limit() {
        let (mut writer, _reader) = duplex(64);
        let error = write_delimited_frame(&mut writer, b"hello", b"\n", 5)
            .await
            .expect_err("write should fail");

        match error {
            DelimiterFrameError::FrameTooLarge {
                max_frame_bytes,
                scanned,
            } => {
                assert_eq!(max_frame_bytes, 5);
                assert_eq!(scanned, 6);
            }
            _ => panic!("unexpected error variant"),
        }
    }
}
