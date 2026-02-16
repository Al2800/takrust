use std::io::Read;

use crate::{recover_chunk_index, ChunkCommit, RecordWriteError, RecoveryReport, TakrecHeader};

const FILE_HEADER_FIXED_BYTES: u64 = 8 + 2 + 8;
const LEN_PREFIX_BYTES: u64 = 2;
const CHUNK_FIXED_BYTES: u64 = 4 + 8 + 4 + 4 + 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkIndexEntry {
    pub sequence: u64,
    pub offset: u64,
    pub payload_len: u32,
    pub checksum: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebuildDiagnostics {
    pub recovered_chunks: usize,
    pub truncated_tail: bool,
    pub indexed_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkIndex {
    pub header: TakrecHeader,
    pub entries: Vec<ChunkIndexEntry>,
    pub diagnostics: RebuildDiagnostics,
}

impl ChunkIndex {
    #[must_use]
    pub fn find_by_sequence(&self, sequence: u64) -> Option<&ChunkIndexEntry> {
        self.entries.iter().find(|entry| entry.sequence == sequence)
    }
}

pub fn rebuild_index<R: Read>(source: R) -> Result<ChunkIndex, RecordWriteError> {
    let report = recover_chunk_index(source)?;
    Ok(rebuild_index_from_recovery(report))
}

#[must_use]
pub fn rebuild_index_from_recovery(report: RecoveryReport) -> ChunkIndex {
    let header_bytes = header_size_bytes(&report.header);
    let mut running_offset = header_bytes;
    let mut entries = Vec::with_capacity(report.chunks.len());

    for chunk in &report.chunks {
        entries.push(ChunkIndexEntry {
            sequence: chunk.sequence,
            offset: running_offset,
            payload_len: chunk.payload_len,
            checksum: chunk.checksum,
        });
        running_offset = running_offset.saturating_add(chunk_size_bytes(chunk));
    }

    ChunkIndex {
        header: report.header,
        diagnostics: RebuildDiagnostics {
            recovered_chunks: report.chunks.len(),
            truncated_tail: report.truncated_tail,
            indexed_bytes: running_offset,
        },
        entries,
    }
}

#[must_use]
pub fn format_rebuild_diagnostics(index: &ChunkIndex) -> String {
    format!(
        "tool_name={}\nchunks={}\ntruncated_tail={}\nindexed_bytes={}",
        index.header.tool_name,
        index.diagnostics.recovered_chunks,
        index.diagnostics.truncated_tail,
        index.diagnostics.indexed_bytes
    )
}

fn chunk_size_bytes(chunk: &ChunkCommit) -> u64 {
    CHUNK_FIXED_BYTES.saturating_add(u64::from(chunk.payload_len))
}

fn header_size_bytes(header: &TakrecHeader) -> u64 {
    FILE_HEADER_FIXED_BYTES
        .saturating_add(len_prefixed_bytes(&header.tool_name))
        .saturating_add(len_prefixed_bytes(&header.tool_version))
        .saturating_add(len_prefixed_bytes(&header.protocol_hint))
        .saturating_add(len_prefixed_bytes(&header.limits_profile))
}

fn len_prefixed_bytes(value: &str) -> u64 {
    LEN_PREFIX_BYTES.saturating_add(value.len() as u64)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::{format_rebuild_diagnostics, rebuild_index, TakrecHeader, TakrecWriter};

    #[test]
    fn rebuild_index_computes_deterministic_offsets() {
        let header = TakrecHeader::new("index-test", "0.1.0", "tak", "conservative");
        let mut writer = TakrecWriter::new(Vec::new(), header.clone()).expect("writer");
        writer.append_chunk(b"alpha").expect("chunk");
        writer.append_chunk(b"beta").expect("chunk");
        let data = writer.into_inner().expect("inner");

        let index = rebuild_index(Cursor::new(data)).expect("rebuild");
        assert_eq!(index.entries.len(), 2);
        assert_eq!(index.entries[0].sequence, 0);
        assert_eq!(index.entries[1].sequence, 1);
        assert_eq!(index.entries[1].offset, index.entries[0].offset + 24 + 5);
        assert_eq!(index.header, header);
        assert!(!index.diagnostics.truncated_tail);
        assert_eq!(index.diagnostics.recovered_chunks, 2);
    }

    #[test]
    fn rebuild_index_marks_truncated_tail_without_panicking() {
        let mut writer = TakrecWriter::new(Vec::new(), TakrecHeader::default()).expect("writer");
        writer.append_chunk(b"good").expect("chunk");
        writer.append_chunk(b"incomplete").expect("chunk");
        let mut data = writer.into_inner().expect("inner");

        let len = data.len();
        data.truncate(len - 2);

        let index = rebuild_index(Cursor::new(data)).expect("rebuild");
        assert_eq!(index.entries.len(), 1);
        assert!(index.diagnostics.truncated_tail);
    }

    #[test]
    fn diagnostics_format_is_stable_and_machine_parseable() {
        let mut writer = TakrecWriter::new(Vec::new(), TakrecHeader::default()).expect("writer");
        writer.append_chunk(b"diag").expect("chunk");
        let data = writer.into_inner().expect("inner");
        let index = rebuild_index(Cursor::new(data)).expect("rebuild");

        let diagnostics = format_rebuild_diagnostics(&index);
        assert!(diagnostics.contains("tool_name=rustak"));
        assert!(diagnostics.contains("chunks=1"));
        assert!(diagnostics.contains("truncated_tail=false"));
        assert!(diagnostics.contains("indexed_bytes="));
    }
}
