use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::LengthPrefixedError;

const MAX_VARINT_BYTES: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthPrefixKind {
    U16Be,
    U32Be,
    Varint,
}

pub async fn read_length_prefixed_frame<R>(
    reader: &mut R,
    prefix: LengthPrefixKind,
    max_frame_bytes: usize,
) -> Result<Vec<u8>, LengthPrefixedError>
where
    R: AsyncRead + Unpin,
{
    let frame_len_u64 = read_prefix(reader, prefix).await?;
    let frame_len =
        usize::try_from(frame_len_u64).map_err(|_| LengthPrefixedError::FrameTooLarge {
            frame_len: usize::MAX,
            max_frame_bytes,
        })?;

    if frame_len > max_frame_bytes {
        return Err(LengthPrefixedError::FrameTooLarge {
            frame_len,
            max_frame_bytes,
        });
    }

    let mut payload = vec![0_u8; frame_len];
    reader
        .read_exact(&mut payload)
        .await
        .map_err(LengthPrefixedError::Io)?;
    Ok(payload)
}

pub async fn write_length_prefixed_frame<W>(
    writer: &mut W,
    prefix: LengthPrefixKind,
    payload: &[u8],
    max_frame_bytes: usize,
) -> Result<(), LengthPrefixedError>
where
    W: AsyncWrite + Unpin,
{
    let frame_len = payload.len();
    if frame_len > max_frame_bytes {
        return Err(LengthPrefixedError::FrameTooLarge {
            frame_len,
            max_frame_bytes,
        });
    }

    write_prefix(writer, prefix, frame_len).await?;
    writer
        .write_all(payload)
        .await
        .map_err(LengthPrefixedError::Io)?;
    Ok(())
}

async fn read_prefix<R>(
    reader: &mut R,
    prefix: LengthPrefixKind,
) -> Result<u64, LengthPrefixedError>
where
    R: AsyncRead + Unpin,
{
    match prefix {
        LengthPrefixKind::U16Be => reader
            .read_u16()
            .await
            .map(u64::from)
            .map_err(LengthPrefixedError::Io),
        LengthPrefixKind::U32Be => reader
            .read_u32()
            .await
            .map(u64::from)
            .map_err(LengthPrefixedError::Io),
        LengthPrefixKind::Varint => read_varint_u64(reader).await,
    }
}

async fn write_prefix<W>(
    writer: &mut W,
    prefix: LengthPrefixKind,
    frame_len: usize,
) -> Result<(), LengthPrefixedError>
where
    W: AsyncWrite + Unpin,
{
    match prefix {
        LengthPrefixKind::U16Be => {
            let value =
                u16::try_from(frame_len).map_err(|_| LengthPrefixedError::PrefixOverflow {
                    prefix: "u16_be",
                    frame_len,
                })?;
            writer
                .write_u16(value)
                .await
                .map_err(LengthPrefixedError::Io)
        }
        LengthPrefixKind::U32Be => {
            let value =
                u32::try_from(frame_len).map_err(|_| LengthPrefixedError::PrefixOverflow {
                    prefix: "u32_be",
                    frame_len,
                })?;
            writer
                .write_u32(value)
                .await
                .map_err(LengthPrefixedError::Io)
        }
        LengthPrefixKind::Varint => {
            let value =
                u64::try_from(frame_len).map_err(|_| LengthPrefixedError::PrefixOverflow {
                    prefix: "varint_u64",
                    frame_len,
                })?;
            let encoded = encode_varint_u64(value);
            writer
                .write_all(&encoded)
                .await
                .map_err(LengthPrefixedError::Io)
        }
    }
}

async fn read_varint_u64<R>(reader: &mut R) -> Result<u64, LengthPrefixedError>
where
    R: AsyncRead + Unpin,
{
    let mut value = 0_u64;
    for index in 0..MAX_VARINT_BYTES {
        let byte = reader.read_u8().await.map_err(LengthPrefixedError::Io)?;
        if index == MAX_VARINT_BYTES - 1 && (byte & 0xFE) != 0 {
            return Err(LengthPrefixedError::VarintOverflow);
        }

        value |= u64::from(byte & 0x7F) << (index * 7);
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }

    Err(LengthPrefixedError::VarintTooLong)
}

fn encode_varint_u64(mut value: u64) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(MAX_VARINT_BYTES);
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        encoded.push(byte);
        if value == 0 {
            return encoded;
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::{duplex, AsyncWriteExt};

    use super::{read_length_prefixed_frame, write_length_prefixed_frame, LengthPrefixKind};
    use crate::LengthPrefixedError;

    #[tokio::test]
    async fn reads_u32_prefixed_frame() {
        let (mut writer, mut reader) = duplex(64);
        tokio::spawn(async move {
            writer
                .write_all(&[0x00, 0x00, 0x00, 0x05])
                .await
                .expect("write prefix should work");
            writer
                .write_all(b"hello")
                .await
                .expect("write payload should work");
        });

        let frame = read_length_prefixed_frame(&mut reader, LengthPrefixKind::U32Be, 16)
            .await
            .expect("frame should decode");
        assert_eq!(frame, b"hello");
    }

    #[tokio::test]
    async fn rejects_frame_larger_than_bound() {
        let (mut writer, mut reader) = duplex(64);
        tokio::spawn(async move {
            writer
                .write_all(&[0x00, 0x05])
                .await
                .expect("write prefix should work");
            writer
                .write_all(b"12345")
                .await
                .expect("write payload should work");
        });

        let error = read_length_prefixed_frame(&mut reader, LengthPrefixKind::U16Be, 4)
            .await
            .expect_err("bound should reject oversize frame");

        match error {
            LengthPrefixedError::FrameTooLarge {
                frame_len,
                max_frame_bytes,
            } => {
                assert_eq!(frame_len, 5);
                assert_eq!(max_frame_bytes, 4);
            }
            _ => panic!("unexpected error variant"),
        }
    }

    #[tokio::test]
    async fn write_rejects_frame_larger_than_limit() {
        let (mut writer, _reader) = duplex(64);
        let error = write_length_prefixed_frame(&mut writer, LengthPrefixKind::U16Be, b"12345", 4)
            .await
            .expect_err("bound should reject payload");

        match error {
            LengthPrefixedError::FrameTooLarge {
                frame_len,
                max_frame_bytes,
            } => {
                assert_eq!(frame_len, 5);
                assert_eq!(max_frame_bytes, 4);
            }
            _ => panic!("unexpected error variant"),
        }
    }

    #[tokio::test]
    async fn varint_round_trip() {
        let (mut writer, mut reader) = duplex(128);
        let payload = vec![0xAB; 300];

        let write_payload = payload.clone();
        tokio::spawn(async move {
            write_length_prefixed_frame(
                &mut writer,
                LengthPrefixKind::Varint,
                &write_payload,
                1024,
            )
            .await
            .expect("write should work");
        });

        let decoded = read_length_prefixed_frame(&mut reader, LengthPrefixKind::Varint, 1024)
            .await
            .expect("decode should work");
        assert_eq!(decoded, payload);
    }

    #[tokio::test]
    async fn rejects_overflowing_varint_prefix() {
        let (mut writer, mut reader) = duplex(64);
        tokio::spawn(async move {
            writer
                .write_all(&[0x80; 10])
                .await
                .expect("write should work");
        });

        let error = read_length_prefixed_frame(&mut reader, LengthPrefixKind::Varint, 1024)
            .await
            .expect_err("varint overflow should fail");
        match error {
            LengthPrefixedError::VarintOverflow => {}
            _ => panic!("unexpected error variant"),
        }
    }
}
