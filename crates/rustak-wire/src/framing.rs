use rustak_limits::Limits;
use rustak_net::{
    read_delimited_frame, read_length_prefixed_frame, write_delimited_frame,
    write_length_prefixed_frame, DelimiterFrameError, LengthPrefixKind, LengthPrefixedError,
};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::WireFormat;

pub const LEGACY_XML_DELIMITER: &[u8] = b"\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireFrameCodec {
    format: WireFormat,
    max_xml_scan_bytes: usize,
    max_protobuf_bytes: usize,
}

impl WireFrameCodec {
    #[must_use]
    pub const fn new(
        format: WireFormat,
        max_xml_scan_bytes: usize,
        max_protobuf_bytes: usize,
    ) -> Self {
        Self {
            format,
            max_xml_scan_bytes,
            max_protobuf_bytes,
        }
    }

    #[must_use]
    pub const fn from_limits(format: WireFormat, limits: &Limits) -> Self {
        Self::new(format, limits.max_xml_scan_bytes, limits.max_protobuf_bytes)
    }

    #[must_use]
    pub const fn format(&self) -> WireFormat {
        self.format
    }

    pub async fn read_frame<R>(&self, reader: &mut R) -> Result<Vec<u8>, WireFrameError>
    where
        R: AsyncRead + Unpin,
    {
        match self.format {
            WireFormat::Xml => {
                read_delimited_frame(reader, LEGACY_XML_DELIMITER, self.max_xml_scan_bytes, false)
                    .await
                    .map_err(WireFrameError::from)
            }
            WireFormat::TakProtocolV1 => read_length_prefixed_frame(
                reader,
                LengthPrefixKind::Varint,
                self.max_protobuf_bytes,
            )
            .await
            .map_err(WireFrameError::from),
        }
    }

    pub async fn write_frame<W>(&self, writer: &mut W, payload: &[u8]) -> Result<(), WireFrameError>
    where
        W: AsyncWrite + Unpin,
    {
        match self.format {
            WireFormat::Xml => write_delimited_frame(
                writer,
                payload,
                LEGACY_XML_DELIMITER,
                self.max_xml_scan_bytes,
            )
            .await
            .map_err(WireFrameError::from),
            WireFormat::TakProtocolV1 => write_length_prefixed_frame(
                writer,
                LengthPrefixKind::Varint,
                payload,
                self.max_protobuf_bytes,
            )
            .await
            .map_err(WireFrameError::from),
        }
    }
}

#[derive(Debug, Error)]
pub enum WireFrameError {
    #[error(transparent)]
    Delimiter(#[from] DelimiterFrameError),

    #[error(transparent)]
    LengthPrefixed(#[from] LengthPrefixedError),
}

#[cfg(test)]
mod tests {
    use tokio::io::{duplex, AsyncWriteExt};

    use super::{WireFrameCodec, WireFrameError};
    use crate::WireFormat;
    use rustak_net::{DelimiterFrameError, LengthPrefixedError};

    #[tokio::test]
    async fn xml_frames_round_trip_with_delimiter_framing() {
        let (mut writer, mut reader) = duplex(128);
        let codec = WireFrameCodec::new(WireFormat::Xml, 128, 128);
        let payload = b"<event uid=\"abc\" />";

        tokio::spawn(async move {
            codec
                .write_frame(&mut writer, payload)
                .await
                .expect("xml write should succeed");
        });

        let read_codec = WireFrameCodec::new(WireFormat::Xml, 128, 128);
        let decoded = read_codec
            .read_frame(&mut reader)
            .await
            .expect("xml read should succeed");
        assert_eq!(decoded, payload);
    }

    #[tokio::test]
    async fn tak_proto_frames_round_trip_with_varint_framing() {
        let (mut writer, mut reader) = duplex(1024);
        let payload = vec![0xA5; 300];
        let write_codec = WireFrameCodec::new(WireFormat::TakProtocolV1, 1024, 1024);

        let write_payload = payload.clone();
        tokio::spawn(async move {
            write_codec
                .write_frame(&mut writer, &write_payload)
                .await
                .expect("tak write should succeed");
        });

        let read_codec = WireFrameCodec::new(WireFormat::TakProtocolV1, 1024, 1024);
        let decoded = read_codec
            .read_frame(&mut reader)
            .await
            .expect("tak read should succeed");
        assert_eq!(decoded, payload);
    }

    #[tokio::test]
    async fn xml_read_rejects_scan_over_limit() {
        let (mut writer, mut reader) = duplex(64);
        tokio::spawn(async move {
            writer
                .write_all(b"0123456789\n")
                .await
                .expect("write should succeed");
        });

        let codec = WireFrameCodec::new(WireFormat::Xml, 4, 1024);
        let error = codec
            .read_frame(&mut reader)
            .await
            .expect_err("xml scan should exceed bound");

        match error {
            WireFrameError::Delimiter(DelimiterFrameError::FrameTooLarge { .. }) => {}
            _ => panic!("unexpected error variant"),
        }
    }

    #[tokio::test]
    async fn tak_write_rejects_payload_above_protobuf_bound() {
        let (mut writer, _reader) = duplex(128);
        let codec = WireFrameCodec::new(WireFormat::TakProtocolV1, 1024, 4);
        let error = codec
            .write_frame(&mut writer, b"12345")
            .await
            .expect_err("tak payload should exceed bound");

        match error {
            WireFrameError::LengthPrefixed(LengthPrefixedError::FrameTooLarge { .. }) => {}
            _ => panic!("unexpected error variant"),
        }
    }
}
