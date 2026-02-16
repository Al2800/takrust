use std::io::{self, BufWriter, Read, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use thiserror::Error;

pub const DEFAULT_MAX_CHUNK_BYTES: usize = 16 * 1024 * 1024;

const FILE_MAGIC: [u8; 8] = *b"TAKREC01";
const FILE_VERSION: u16 = 1;
const CHUNK_MAGIC: [u8; 4] = *b"CHNK";
const CHUNK_COMMIT_MARKER: u32 = 0xC0DE_CAFE;
const MAX_HEADER_FIELD_LEN: usize = 4 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TakrecHeader {
    pub tool_name: String,
    pub tool_version: String,
    pub protocol_hint: String,
    pub limits_profile: String,
    pub created_unix_nanos: u64,
}

impl TakrecHeader {
    #[must_use]
    pub fn new(
        tool_name: impl Into<String>,
        tool_version: impl Into<String>,
        protocol_hint: impl Into<String>,
        limits_profile: impl Into<String>,
    ) -> Self {
        Self {
            tool_name: tool_name.into(),
            tool_version: tool_version.into(),
            protocol_hint: protocol_hint.into(),
            limits_profile: limits_profile.into(),
            created_unix_nanos: now_unix_nanos(),
        }
    }
}

impl Default for TakrecHeader {
    fn default() -> Self {
        Self {
            tool_name: "rustak".to_string(),
            tool_version: "0.1.0".to_string(),
            protocol_hint: "mixed".to_string(),
            limits_profile: "conservative".to_string(),
            created_unix_nanos: now_unix_nanos(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkCommit {
    pub sequence: u64,
    pub payload_len: u32,
    pub checksum: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryReport {
    pub header: TakrecHeader,
    pub chunks: Vec<ChunkCommit>,
    pub truncated_tail: bool,
}

#[derive(Debug)]
pub struct TakrecWriter<W: Write> {
    sink: BufWriter<W>,
    header: TakrecHeader,
    max_chunk_bytes: usize,
    next_sequence: u64,
}

impl<W: Write> TakrecWriter<W> {
    pub fn new(sink: W, header: TakrecHeader) -> Result<Self, RecordWriteError> {
        let mut writer = Self {
            sink: BufWriter::new(sink),
            header,
            max_chunk_bytes: DEFAULT_MAX_CHUNK_BYTES,
            next_sequence: 0,
        };

        writer.write_header()?;
        writer.flush_boundary()?;
        Ok(writer)
    }

    pub fn with_max_chunk_bytes(mut self, max_chunk_bytes: usize) -> Self {
        self.max_chunk_bytes = max_chunk_bytes;
        self
    }

    #[must_use]
    pub fn header(&self) -> &TakrecHeader {
        &self.header
    }

    #[must_use]
    pub const fn max_chunk_bytes(&self) -> usize {
        self.max_chunk_bytes
    }

    #[must_use]
    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn append_chunk(&mut self, payload: &[u8]) -> Result<ChunkCommit, RecordWriteError> {
        if payload.len() > self.max_chunk_bytes {
            return Err(RecordWriteError::ChunkTooLarge {
                payload_len: payload.len(),
                max_chunk_bytes: self.max_chunk_bytes,
            });
        }

        let payload_len =
            u32::try_from(payload.len()).map_err(|_| RecordWriteError::ChunkTooLarge {
                payload_len: payload.len(),
                max_chunk_bytes: self.max_chunk_bytes,
            })?;
        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(RecordWriteError::SequenceOverflow)?;

        let checksum = crc32fast::hash(payload);
        let commit = ChunkCommit {
            sequence,
            payload_len,
            checksum,
        };

        self.sink.write_all(&CHUNK_MAGIC)?;
        write_u64_le(&mut self.sink, commit.sequence)?;
        write_u32_le(&mut self.sink, commit.payload_len)?;
        write_u32_le(&mut self.sink, commit.checksum)?;
        self.sink.write_all(payload)?;
        write_u32_le(&mut self.sink, CHUNK_COMMIT_MARKER)?;
        self.flush_boundary()?;
        Ok(commit)
    }

    pub fn flush_boundary(&mut self) -> Result<(), RecordWriteError> {
        self.sink.flush().map_err(RecordWriteError::Io)
    }

    pub fn into_inner(mut self) -> Result<W, RecordWriteError> {
        self.flush_boundary()?;
        self.sink
            .into_inner()
            .map_err(|error| RecordWriteError::Io(error.into_error()))
    }

    fn write_header(&mut self) -> Result<(), RecordWriteError> {
        self.sink.write_all(&FILE_MAGIC)?;
        write_u16_le(&mut self.sink, FILE_VERSION)?;
        write_u64_le(&mut self.sink, self.header.created_unix_nanos)?;
        write_len_prefixed_string(&mut self.sink, "tool_name", &self.header.tool_name)?;
        write_len_prefixed_string(&mut self.sink, "tool_version", &self.header.tool_version)?;
        write_len_prefixed_string(&mut self.sink, "protocol_hint", &self.header.protocol_hint)?;
        write_len_prefixed_string(
            &mut self.sink,
            "limits_profile",
            &self.header.limits_profile,
        )?;
        Ok(())
    }
}

pub fn recover_chunk_index<R: Read>(mut source: R) -> Result<RecoveryReport, RecordWriteError> {
    let header = read_header(&mut source)?;
    let mut chunks = Vec::new();
    let mut truncated_tail = false;

    loop {
        let magic = match read_array_status::<4, _>(&mut source)? {
            ReadStatus::Complete(magic) => magic,
            ReadStatus::Eof => break,
            ReadStatus::Truncated => {
                truncated_tail = true;
                break;
            }
        };

        if magic != CHUNK_MAGIC {
            return Err(RecordWriteError::CorruptChunkMagic { found: magic });
        }

        let sequence = match read_u64_status(&mut source)? {
            ReadStatus::Complete(value) => value,
            ReadStatus::Eof | ReadStatus::Truncated => {
                truncated_tail = true;
                break;
            }
        };

        let payload_len = match read_u32_status(&mut source)? {
            ReadStatus::Complete(value) => value,
            ReadStatus::Eof | ReadStatus::Truncated => {
                truncated_tail = true;
                break;
            }
        };

        let expected_checksum = match read_u32_status(&mut source)? {
            ReadStatus::Complete(value) => value,
            ReadStatus::Eof | ReadStatus::Truncated => {
                truncated_tail = true;
                break;
            }
        };

        let payload_len_usize =
            usize::try_from(payload_len).map_err(|_| RecordWriteError::ChunkTooLarge {
                payload_len: usize::MAX,
                max_chunk_bytes: DEFAULT_MAX_CHUNK_BYTES,
            })?;

        let payload = match read_vec_status(&mut source, payload_len_usize)? {
            ReadStatus::Complete(value) => value,
            ReadStatus::Eof | ReadStatus::Truncated => {
                truncated_tail = true;
                break;
            }
        };

        let commit_marker = match read_u32_status(&mut source)? {
            ReadStatus::Complete(value) => value,
            ReadStatus::Eof | ReadStatus::Truncated => {
                truncated_tail = true;
                break;
            }
        };

        if commit_marker != CHUNK_COMMIT_MARKER {
            return Err(RecordWriteError::CommitMarkerMismatch {
                sequence,
                expected: CHUNK_COMMIT_MARKER,
                actual: commit_marker,
            });
        }

        let actual_checksum = crc32fast::hash(&payload);
        if actual_checksum != expected_checksum {
            return Err(RecordWriteError::ChecksumMismatch {
                sequence,
                expected: expected_checksum,
                actual: actual_checksum,
            });
        }

        chunks.push(ChunkCommit {
            sequence,
            payload_len,
            checksum: expected_checksum,
        });
    }

    Ok(RecoveryReport {
        header,
        chunks,
        truncated_tail,
    })
}

#[derive(Debug, Error)]
pub enum RecordWriteError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("header field `{field}` length {len} exceeds u16 limit")]
    HeaderFieldTooLong { field: &'static str, len: usize },

    #[error("header field `{field}` length {len} exceeds read limit {max_len}")]
    HeaderFieldTooLargeOnRead {
        field: &'static str,
        len: usize,
        max_len: usize,
    },

    #[error("invalid takrec magic, found {found:?}")]
    InvalidFileMagic { found: [u8; 8] },

    #[error("unsupported takrec version {found}; expected {expected}")]
    UnsupportedVersion { expected: u16, found: u16 },

    #[error("chunk payload length {payload_len} exceeds max_chunk_bytes {max_chunk_bytes}")]
    ChunkTooLarge {
        payload_len: usize,
        max_chunk_bytes: usize,
    },

    #[error("chunk sequence overflow")]
    SequenceOverflow,

    #[error("corrupt chunk magic, found {found:?}")]
    CorruptChunkMagic { found: [u8; 4] },

    #[error(
        "chunk {sequence} checksum mismatch: expected {expected:#010x}, actual {actual:#010x}"
    )]
    ChecksumMismatch {
        sequence: u64,
        expected: u32,
        actual: u32,
    },

    #[error(
        "chunk {sequence} commit marker mismatch: expected {expected:#010x}, actual {actual:#010x}"
    )]
    CommitMarkerMismatch {
        sequence: u64,
        expected: u32,
        actual: u32,
    },

    #[error("truncated takrec header")]
    TruncatedHeader,
}

fn now_unix_nanos() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration_to_nanos(duration),
        Err(_) => 0,
    }
}

fn duration_to_nanos(duration: Duration) -> u64 {
    duration
        .as_secs()
        .saturating_mul(1_000_000_000)
        .saturating_add(u64::from(duration.subsec_nanos()))
}

fn write_u16_le<W: Write>(sink: &mut W, value: u16) -> io::Result<()> {
    sink.write_all(&value.to_le_bytes())
}

fn write_u32_le<W: Write>(sink: &mut W, value: u32) -> io::Result<()> {
    sink.write_all(&value.to_le_bytes())
}

fn write_u64_le<W: Write>(sink: &mut W, value: u64) -> io::Result<()> {
    sink.write_all(&value.to_le_bytes())
}

fn write_len_prefixed_string<W: Write>(
    sink: &mut W,
    field: &'static str,
    value: &str,
) -> Result<(), RecordWriteError> {
    let len = value.len();
    let len_u16 =
        u16::try_from(len).map_err(|_| RecordWriteError::HeaderFieldTooLong { field, len })?;

    write_u16_le(sink, len_u16)?;
    sink.write_all(value.as_bytes())?;
    Ok(())
}

fn read_header<R: Read>(source: &mut R) -> Result<TakrecHeader, RecordWriteError> {
    let magic = read_array_required::<8, _>(source, RecordWriteError::TruncatedHeader)?;
    if magic != FILE_MAGIC {
        return Err(RecordWriteError::InvalidFileMagic { found: magic });
    }

    let version = read_u16_required(source, RecordWriteError::TruncatedHeader)?;
    if version != FILE_VERSION {
        return Err(RecordWriteError::UnsupportedVersion {
            expected: FILE_VERSION,
            found: version,
        });
    }

    let created_unix_nanos = read_u64_required(source, RecordWriteError::TruncatedHeader)?;
    let tool_name = read_len_prefixed_string(source, "tool_name", MAX_HEADER_FIELD_LEN)?;
    let tool_version = read_len_prefixed_string(source, "tool_version", MAX_HEADER_FIELD_LEN)?;
    let protocol_hint = read_len_prefixed_string(source, "protocol_hint", MAX_HEADER_FIELD_LEN)?;
    let limits_profile = read_len_prefixed_string(source, "limits_profile", MAX_HEADER_FIELD_LEN)?;

    Ok(TakrecHeader {
        tool_name,
        tool_version,
        protocol_hint,
        limits_profile,
        created_unix_nanos,
    })
}

fn read_len_prefixed_string<R: Read>(
    source: &mut R,
    field: &'static str,
    max_len: usize,
) -> Result<String, RecordWriteError> {
    let len = usize::from(read_u16_required(
        source,
        RecordWriteError::TruncatedHeader,
    )?);
    if len > max_len {
        return Err(RecordWriteError::HeaderFieldTooLargeOnRead {
            field,
            len,
            max_len,
        });
    }

    let bytes = read_vec_required(source, len, RecordWriteError::TruncatedHeader)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn read_u16_required<R: Read>(
    source: &mut R,
    on_truncated: RecordWriteError,
) -> Result<u16, RecordWriteError> {
    let bytes = read_array_required::<2, _>(source, on_truncated)?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_u64_required<R: Read>(
    source: &mut R,
    on_truncated: RecordWriteError,
) -> Result<u64, RecordWriteError> {
    let bytes = read_array_required::<8, _>(source, on_truncated)?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_vec_required<R: Read>(
    source: &mut R,
    len: usize,
    on_truncated: RecordWriteError,
) -> Result<Vec<u8>, RecordWriteError> {
    match read_vec_status(source, len)? {
        ReadStatus::Complete(value) => Ok(value),
        ReadStatus::Eof | ReadStatus::Truncated => Err(on_truncated),
    }
}

fn read_array_required<const N: usize, R: Read>(
    source: &mut R,
    on_truncated: RecordWriteError,
) -> Result<[u8; N], RecordWriteError> {
    match read_array_status::<N, _>(source)? {
        ReadStatus::Complete(value) => Ok(value),
        ReadStatus::Eof | ReadStatus::Truncated => Err(on_truncated),
    }
}

fn read_u32_status<R: Read>(source: &mut R) -> Result<ReadStatus<u32>, io::Error> {
    Ok(match read_array_status::<4, _>(source)? {
        ReadStatus::Complete(bytes) => ReadStatus::Complete(u32::from_le_bytes(bytes)),
        ReadStatus::Eof => ReadStatus::Eof,
        ReadStatus::Truncated => ReadStatus::Truncated,
    })
}

fn read_u64_status<R: Read>(source: &mut R) -> Result<ReadStatus<u64>, io::Error> {
    Ok(match read_array_status::<8, _>(source)? {
        ReadStatus::Complete(bytes) => ReadStatus::Complete(u64::from_le_bytes(bytes)),
        ReadStatus::Eof => ReadStatus::Eof,
        ReadStatus::Truncated => ReadStatus::Truncated,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadStatus<T> {
    Complete(T),
    Eof,
    Truncated,
}

fn read_array_status<const N: usize, R: Read>(
    source: &mut R,
) -> Result<ReadStatus<[u8; N]>, io::Error> {
    let mut bytes = [0_u8; N];
    let mut read_total = 0;
    while read_total < N {
        let read = source.read(&mut bytes[read_total..])?;
        if read == 0 {
            if read_total == 0 {
                return Ok(ReadStatus::Eof);
            }
            return Ok(ReadStatus::Truncated);
        }
        read_total += read;
    }

    Ok(ReadStatus::Complete(bytes))
}

fn read_vec_status<R: Read>(source: &mut R, len: usize) -> Result<ReadStatus<Vec<u8>>, io::Error> {
    let mut bytes = vec![0_u8; len];
    if len == 0 {
        return Ok(ReadStatus::Complete(bytes));
    }

    let mut read_total = 0;
    while read_total < len {
        let read = source.read(&mut bytes[read_total..])?;
        if read == 0 {
            if read_total == 0 {
                return Ok(ReadStatus::Eof);
            }
            return Ok(ReadStatus::Truncated);
        }
        read_total += read;
    }

    Ok(ReadStatus::Complete(bytes))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{
        recover_chunk_index, RecordWriteError, TakrecHeader, TakrecWriter, CHUNK_COMMIT_MARKER,
    };

    #[test]
    fn writes_and_recovers_complete_stream() {
        let header = TakrecHeader::new("unit-test", "0.1.0", "tak", "conservative");
        let mut writer = TakrecWriter::new(Vec::new(), header.clone()).expect("writer");
        let first = writer.append_chunk(b"alpha").expect("chunk");
        let second = writer.append_chunk(b"beta").expect("chunk");
        let data = writer.into_inner().expect("inner");

        let report = recover_chunk_index(Cursor::new(data)).expect("recover");
        assert_eq!(report.header, header);
        assert_eq!(report.chunks, vec![first, second]);
        assert!(!report.truncated_tail);
    }

    #[test]
    fn recovery_drops_truncated_tail_chunk() {
        let mut writer = TakrecWriter::new(Vec::new(), TakrecHeader::default()).expect("writer");
        writer.append_chunk(b"good").expect("chunk");
        writer.append_chunk(b"incomplete").expect("chunk");
        let mut data = writer.into_inner().expect("inner");

        let len = data.len();
        data.truncate(len - 3);

        let report = recover_chunk_index(Cursor::new(data)).expect("recover");
        assert_eq!(report.chunks.len(), 1);
        assert!(report.truncated_tail);
    }

    #[test]
    fn recovery_detects_checksum_mismatch() {
        let mut writer = TakrecWriter::new(Vec::new(), TakrecHeader::default()).expect("writer");
        writer.append_chunk(b"abcdef").expect("chunk");
        let mut data = writer.into_inner().expect("inner");

        let payload_pos = data
            .windows(6)
            .position(|window| window == b"abcdef")
            .expect("payload must exist");
        data[payload_pos] ^= 0xFF;

        let error = recover_chunk_index(Cursor::new(data)).expect_err("checksum mismatch");
        match error {
            RecordWriteError::ChecksumMismatch {
                sequence,
                expected: _,
                actual: _,
            } => assert_eq!(sequence, 0),
            _ => panic!("unexpected error variant"),
        }
    }

    #[test]
    fn append_chunk_rejects_payload_above_max() {
        let writer = TakrecWriter::new(Vec::new(), TakrecHeader::default()).expect("writer");
        let mut writer = writer.with_max_chunk_bytes(4);
        let error = writer
            .append_chunk(b"12345")
            .expect_err("oversized payload should fail");

        match error {
            RecordWriteError::ChunkTooLarge {
                payload_len,
                max_chunk_bytes,
            } => {
                assert_eq!(payload_len, 5);
                assert_eq!(max_chunk_bytes, 4);
            }
            _ => panic!("unexpected error variant"),
        }
    }

    #[test]
    fn recovery_detects_commit_marker_mismatch() {
        let mut writer = TakrecWriter::new(Vec::new(), TakrecHeader::default()).expect("writer");
        writer.append_chunk(b"marker").expect("chunk");
        let mut data = writer.into_inner().expect("inner");

        let marker_bytes = CHUNK_COMMIT_MARKER.to_le_bytes();
        let marker_pos = data
            .windows(marker_bytes.len())
            .position(|window| window == marker_bytes)
            .expect("marker exists");
        data[marker_pos] = 0x00;

        let error = recover_chunk_index(Cursor::new(data)).expect_err("marker mismatch");
        match error {
            RecordWriteError::CommitMarkerMismatch {
                sequence,
                expected,
                actual,
            } => {
                assert_eq!(sequence, 0);
                assert_eq!(expected, CHUNK_COMMIT_MARKER);
                assert_ne!(actual, CHUNK_COMMIT_MARKER);
            }
            _ => panic!("unexpected error variant"),
        }
    }
}
