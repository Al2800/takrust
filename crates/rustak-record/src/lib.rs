pub mod index;
pub mod integrity;
pub mod interop;
pub mod writer;

use std::io::Write;

use bytes::Bytes;
use rustak_io::{MessageEnvelope, MessageSink, MessageSource};

pub use index::{
    format_rebuild_diagnostics, rebuild_index, ChunkIndex, ChunkIndexEntry, RebuildDiagnostics,
};
pub use integrity::{
    build_integrity_chain, build_integrity_chain_with_signer, verify_integrity_chain,
    IntegrityChain, IntegrityError, IntegrityLink, SignatureProvider, SignatureVerifier,
};
pub use interop::{
    export_annotations_to_pcap, import_annotations_from_pcap, DecodeStatus, InteropError,
    PcapAnnotation, TrafficDirection,
};
pub use writer::{
    recover_chunk_index, ChunkCommit, RecordWriteError, RecoveryReport, TakrecHeader, TakrecWriter,
    DEFAULT_MAX_CHUNK_BYTES,
};

pub type RecordEnvelope<T> = MessageEnvelope<T>;
pub type RecordSink<T> = dyn MessageSink<T>;
pub type RecordSource<T> = dyn MessageSource<T>;

pub fn append_envelope_chunk<W: Write>(
    writer: &mut TakrecWriter<W>,
    envelope: &RecordEnvelope<Bytes>,
) -> Result<ChunkCommit, RecordWriteError> {
    let payload = envelope.raw_frame.as_deref().unwrap_or(&envelope.message);
    writer.append_chunk(payload)
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use crate::{append_envelope_chunk, TakrecHeader, TakrecWriter};
    use rustak_io::MessageEnvelope;

    #[test]
    fn append_envelope_chunk_prefers_raw_frame() {
        let mut writer = TakrecWriter::new(Vec::new(), TakrecHeader::default())
            .expect("writer should initialize");

        let envelope = MessageEnvelope::new(Bytes::from_static(b"decoded-frame"))
            .with_raw_frame(Bytes::from_static(b"<event/>"));

        let commit = append_envelope_chunk(&mut writer, &envelope).expect("append should succeed");
        assert_eq!(commit.payload_len, 8);
    }

    #[test]
    fn append_envelope_chunk_falls_back_to_message_payload() {
        let mut writer = TakrecWriter::new(Vec::new(), TakrecHeader::default())
            .expect("writer should initialize");

        let envelope = MessageEnvelope::new(Bytes::from_static(b"decoded-frame"));
        let commit = append_envelope_chunk(&mut writer, &envelope).expect("append should succeed");
        assert_eq!(commit.payload_len, 13);
    }
}
