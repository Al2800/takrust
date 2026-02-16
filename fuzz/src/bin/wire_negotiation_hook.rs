use std::io::{self, Read};

use rustak_wire::negotiation::events::{NegotiationTelemetry, NegotiationTelemetryEvent};
use rustak_wire::{DowngradePolicy, Negotiator};

fn main() {
    let mut data = Vec::new();
    let _ = io::stdin().read_to_end(&mut data);

    let policy = if data.first().copied().unwrap_or_default() & 1 == 0 {
        DowngradePolicy::FailOpen
    } else {
        DowngradePolicy::FailClosed
    };

    let session_id = read_u64_le(&data[..data.len().min(8)]);
    let mut negotiator = Negotiator::new(policy);
    let mut telemetry = NegotiationTelemetry::default();

    let _ = negotiator.begin_upgrade_attempt_with_telemetry(session_id, &mut telemetry);

    let mut cursor = 8usize;
    let mut observed_any_frame = false;
    while cursor < data.len() {
        observed_any_frame = true;
        let declared_len = usize::from(data[cursor] % 16);
        cursor += 1;
        let end = cursor.saturating_add(declared_len).min(data.len());
        let frame = &data[cursor..end];
        cursor = end;
        let _ = negotiator.observe_control_frame_with_telemetry(session_id, frame, &mut telemetry);
    }

    if !observed_any_frame {
        let _ = negotiator.observe_control_frame_with_telemetry(session_id, &[], &mut telemetry);
    }

    for event in telemetry.drain() {
        let payload = event.encode_record_payload();
        let _ = NegotiationTelemetryEvent::decode_record_payload(&payload);
    }
}

fn read_u64_le(bytes: &[u8]) -> u64 {
    let mut out = [0u8; 8];
    let len = bytes.len().min(out.len());
    out[..len].copy_from_slice(&bytes[..len]);
    u64::from_le_bytes(out)
}
