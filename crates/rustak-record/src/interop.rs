use std::io::{self, Read, Write};

use thiserror::Error;

const PCAP_MAGIC_LE: u32 = 0xA1B2_C3D4;
const PCAP_VERSION_MAJOR: u16 = 2;
const PCAP_VERSION_MINOR: u16 = 4;
const PCAP_THISZONE: i32 = 0;
const PCAP_SIGFIGS: u32 = 0;
const PCAP_SNAPLEN: u32 = 65_535;
const PCAP_LINKTYPE_USER0: u32 = 147;

const PACKET_HEADER_LEN: usize = 16;
const ANNOTATION_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrafficDirection {
    Inbound,
    Outbound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeStatus {
    Decoded,
    Opaque,
    Malformed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcapAnnotation {
    pub timestamp_micros: u64,
    pub direction: TrafficDirection,
    pub protocol: String,
    pub peer: String,
    pub decode_status: DecodeStatus,
    pub payload: Vec<u8>,
}

impl PcapAnnotation {
    #[must_use]
    pub fn new(
        timestamp_micros: u64,
        direction: TrafficDirection,
        protocol: impl Into<String>,
        peer: impl Into<String>,
        decode_status: DecodeStatus,
        payload: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            timestamp_micros,
            direction,
            protocol: protocol.into(),
            peer: peer.into(),
            decode_status,
            payload: payload.into(),
        }
    }
}

#[derive(Debug, Error)]
pub enum InteropError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("truncated pcap global header")]
    TruncatedGlobalHeader,

    #[error("invalid pcap magic {found:#010x}")]
    InvalidMagic { found: u32 },

    #[error("unsupported pcap version {major}.{minor}")]
    UnsupportedPcapVersion { major: u16, minor: u16 },

    #[error("truncated pcap packet header")]
    TruncatedPacketHeader,

    #[error("truncated pcap packet payload: expected {expected} bytes, found {actual}")]
    TruncatedPacketPayload { expected: usize, actual: usize },

    #[error("annotation frame version {found} is unsupported")]
    UnsupportedAnnotationVersion { found: u8 },

    #[error("annotation direction value {value} is invalid")]
    InvalidDirection { value: u8 },

    #[error("annotation decode-status value {value} is invalid")]
    InvalidDecodeStatus { value: u8 },

    #[error("{field} length {len} exceeds protocol bounds")]
    FieldTooLarge { field: &'static str, len: usize },
}

pub fn export_annotations_to_pcap<W: Write>(
    mut sink: W,
    annotations: &[PcapAnnotation],
) -> Result<(), InteropError> {
    write_u32_le(&mut sink, PCAP_MAGIC_LE)?;
    write_u16_le(&mut sink, PCAP_VERSION_MAJOR)?;
    write_u16_le(&mut sink, PCAP_VERSION_MINOR)?;
    write_i32_le(&mut sink, PCAP_THISZONE)?;
    write_u32_le(&mut sink, PCAP_SIGFIGS)?;
    write_u32_le(&mut sink, PCAP_SNAPLEN)?;
    write_u32_le(&mut sink, PCAP_LINKTYPE_USER0)?;

    for annotation in annotations {
        let ts_sec = u32::try_from(annotation.timestamp_micros / 1_000_000).map_err(|_| {
            InteropError::FieldTooLarge {
                field: "timestamp_micros",
                len: usize::MAX,
            }
        })?;
        let ts_usec = (annotation.timestamp_micros % 1_000_000) as u32;
        let frame = encode_annotation_frame(annotation)?;
        let frame_len_u32 =
            u32::try_from(frame.len()).map_err(|_| InteropError::FieldTooLarge {
                field: "payload",
                len: frame.len(),
            })?;

        write_u32_le(&mut sink, ts_sec)?;
        write_u32_le(&mut sink, ts_usec)?;
        write_u32_le(&mut sink, frame_len_u32)?;
        write_u32_le(&mut sink, frame_len_u32)?;
        sink.write_all(&frame)?;
    }

    Ok(())
}

pub fn import_annotations_from_pcap<R: Read>(
    mut source: R,
) -> Result<Vec<PcapAnnotation>, InteropError> {
    let mut header = [0u8; 24];
    read_exact_or_truncated(
        &mut source,
        &mut header,
        InteropError::TruncatedGlobalHeader,
    )?;

    let magic = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    if magic != PCAP_MAGIC_LE {
        return Err(InteropError::InvalidMagic { found: magic });
    }

    let major = u16::from_le_bytes([header[4], header[5]]);
    let minor = u16::from_le_bytes([header[6], header[7]]);
    if major != PCAP_VERSION_MAJOR || minor != PCAP_VERSION_MINOR {
        return Err(InteropError::UnsupportedPcapVersion { major, minor });
    }

    let mut annotations = Vec::new();
    while let Some(packet_header) = read_packet_header(&mut source)? {
        let ts_sec = u32::from_le_bytes([
            packet_header[0],
            packet_header[1],
            packet_header[2],
            packet_header[3],
        ]);
        let ts_usec = u32::from_le_bytes([
            packet_header[4],
            packet_header[5],
            packet_header[6],
            packet_header[7],
        ]);
        let incl_len = u32::from_le_bytes([
            packet_header[8],
            packet_header[9],
            packet_header[10],
            packet_header[11],
        ]);
        let incl_len_usize = incl_len as usize;

        let mut payload = vec![0u8; incl_len_usize];
        let read = source.read(&mut payload)?;
        if read < incl_len_usize {
            return Err(InteropError::TruncatedPacketPayload {
                expected: incl_len_usize,
                actual: read,
            });
        }

        let timestamp_micros = u64::from(ts_sec)
            .saturating_mul(1_000_000)
            .saturating_add(u64::from(ts_usec));
        let annotation = decode_annotation_frame(timestamp_micros, &payload)?;
        annotations.push(annotation);
    }

    Ok(annotations)
}

fn encode_annotation_frame(annotation: &PcapAnnotation) -> Result<Vec<u8>, InteropError> {
    let protocol_bytes = annotation.protocol.as_bytes();
    let peer_bytes = annotation.peer.as_bytes();
    let protocol_len =
        u16::try_from(protocol_bytes.len()).map_err(|_| InteropError::FieldTooLarge {
            field: "protocol",
            len: protocol_bytes.len(),
        })?;
    let peer_len = u16::try_from(peer_bytes.len()).map_err(|_| InteropError::FieldTooLarge {
        field: "peer",
        len: peer_bytes.len(),
    })?;
    let payload_len =
        u32::try_from(annotation.payload.len()).map_err(|_| InteropError::FieldTooLarge {
            field: "payload",
            len: annotation.payload.len(),
        })?;

    let mut frame =
        Vec::with_capacity(12 + protocol_bytes.len() + peer_bytes.len() + annotation.payload.len());
    frame.push(ANNOTATION_VERSION);
    frame.push(direction_code(annotation.direction));
    frame.push(status_code(annotation.decode_status));
    frame.push(0);
    frame.extend_from_slice(&protocol_len.to_le_bytes());
    frame.extend_from_slice(&peer_len.to_le_bytes());
    frame.extend_from_slice(&payload_len.to_le_bytes());
    frame.extend_from_slice(protocol_bytes);
    frame.extend_from_slice(peer_bytes);
    frame.extend_from_slice(&annotation.payload);

    Ok(frame)
}

fn decode_annotation_frame(
    timestamp_micros: u64,
    frame: &[u8],
) -> Result<PcapAnnotation, InteropError> {
    if frame.len() < 12 {
        return Err(InteropError::TruncatedPacketPayload {
            expected: 12,
            actual: frame.len(),
        });
    }

    let version = frame[0];
    if version != ANNOTATION_VERSION {
        return Err(InteropError::UnsupportedAnnotationVersion { found: version });
    }

    let direction = parse_direction(frame[1])?;
    let decode_status = parse_status(frame[2])?;
    let protocol_len = u16::from_le_bytes([frame[4], frame[5]]) as usize;
    let peer_len = u16::from_le_bytes([frame[6], frame[7]]) as usize;
    let payload_len = u32::from_le_bytes([frame[8], frame[9], frame[10], frame[11]]) as usize;

    let expected_len = 12usize
        .saturating_add(protocol_len)
        .saturating_add(peer_len)
        .saturating_add(payload_len);
    if frame.len() < expected_len {
        return Err(InteropError::TruncatedPacketPayload {
            expected: expected_len,
            actual: frame.len(),
        });
    }

    let mut cursor = 12usize;
    let protocol = String::from_utf8_lossy(&frame[cursor..cursor + protocol_len]).to_string();
    cursor += protocol_len;
    let peer = String::from_utf8_lossy(&frame[cursor..cursor + peer_len]).to_string();
    cursor += peer_len;
    let payload = frame[cursor..cursor + payload_len].to_vec();

    Ok(PcapAnnotation {
        timestamp_micros,
        direction,
        protocol,
        peer,
        decode_status,
        payload,
    })
}

fn read_packet_header<R: Read>(
    source: &mut R,
) -> Result<Option<[u8; PACKET_HEADER_LEN]>, InteropError> {
    let mut header = [0u8; PACKET_HEADER_LEN];
    let first = source.read(&mut header[..1])?;
    if first == 0 {
        return Ok(None);
    }

    source
        .read_exact(&mut header[1..])
        .map_err(|error| match error.kind() {
            io::ErrorKind::UnexpectedEof => InteropError::TruncatedPacketHeader,
            _ => InteropError::Io(error),
        })?;

    Ok(Some(header))
}

fn read_exact_or_truncated<R: Read>(
    source: &mut R,
    bytes: &mut [u8],
    truncated_error: InteropError,
) -> Result<(), InteropError> {
    source
        .read_exact(bytes)
        .map_err(|error| match error.kind() {
            io::ErrorKind::UnexpectedEof => truncated_error,
            _ => InteropError::Io(error),
        })
}

fn direction_code(direction: TrafficDirection) -> u8 {
    match direction {
        TrafficDirection::Inbound => 0,
        TrafficDirection::Outbound => 1,
    }
}

fn parse_direction(value: u8) -> Result<TrafficDirection, InteropError> {
    match value {
        0 => Ok(TrafficDirection::Inbound),
        1 => Ok(TrafficDirection::Outbound),
        _ => Err(InteropError::InvalidDirection { value }),
    }
}

fn status_code(status: DecodeStatus) -> u8 {
    match status {
        DecodeStatus::Decoded => 0,
        DecodeStatus::Opaque => 1,
        DecodeStatus::Malformed => 2,
    }
}

fn parse_status(value: u8) -> Result<DecodeStatus, InteropError> {
    match value {
        0 => Ok(DecodeStatus::Decoded),
        1 => Ok(DecodeStatus::Opaque),
        2 => Ok(DecodeStatus::Malformed),
        _ => Err(InteropError::InvalidDecodeStatus { value }),
    }
}

fn write_u16_le<W: Write>(sink: &mut W, value: u16) -> io::Result<()> {
    sink.write_all(&value.to_le_bytes())
}

fn write_u32_le<W: Write>(sink: &mut W, value: u32) -> io::Result<()> {
    sink.write_all(&value.to_le_bytes())
}

fn write_i32_le<W: Write>(sink: &mut W, value: i32) -> io::Result<()> {
    sink.write_all(&value.to_le_bytes())
}

#[cfg(test)]
mod tests {
    use super::{
        export_annotations_to_pcap, import_annotations_from_pcap, DecodeStatus, InteropError,
        PcapAnnotation, TrafficDirection,
    };

    #[test]
    fn pcap_annotation_roundtrip_preserves_metadata_and_payload() {
        let annotations = vec![
            PcapAnnotation::new(
                1_720_000_000_111_222,
                TrafficDirection::Inbound,
                "tak",
                "peer-a",
                DecodeStatus::Decoded,
                b"<event/>".to_vec(),
            ),
            PcapAnnotation::new(
                1_720_000_000_333_444,
                TrafficDirection::Outbound,
                "sapient",
                "peer-b",
                DecodeStatus::Opaque,
                b"\x01\x02\x03".to_vec(),
            ),
        ];

        let mut bytes = Vec::new();
        export_annotations_to_pcap(&mut bytes, &annotations).expect("export");
        let imported = import_annotations_from_pcap(bytes.as_slice()).expect("import");
        assert_eq!(imported, annotations);
    }

    #[test]
    fn import_rejects_unknown_annotation_frame_version() {
        let annotations = [PcapAnnotation::new(
            100,
            TrafficDirection::Inbound,
            "tak",
            "peer-x",
            DecodeStatus::Decoded,
            b"payload".to_vec(),
        )];
        let mut bytes = Vec::new();
        export_annotations_to_pcap(&mut bytes, &annotations).expect("export");

        let payload_version_offset = 24 + 16;
        bytes[payload_version_offset] = 9;

        let error = import_annotations_from_pcap(bytes.as_slice()).expect_err("must fail");
        assert!(matches!(
            error,
            InteropError::UnsupportedAnnotationVersion { found: 9 }
        ));
    }

    #[test]
    fn import_reports_truncated_packet_payloads() {
        let annotations = [PcapAnnotation::new(
            100,
            TrafficDirection::Inbound,
            "tak",
            "peer-y",
            DecodeStatus::Malformed,
            b"abc123".to_vec(),
        )];
        let mut bytes = Vec::new();
        export_annotations_to_pcap(&mut bytes, &annotations).expect("export");
        bytes.truncate(bytes.len() - 3);

        let error = import_annotations_from_pcap(bytes.as_slice()).expect_err("must fail");
        assert!(matches!(error, InteropError::TruncatedPacketPayload { .. }));
    }
}
