use std::io::{self, Read};
use std::time::Duration;

use rustak_transport::TransportConfig;

fn main() {
    let mut data = Vec::new();
    let _ = io::stdin().read_to_end(&mut data);

    let mut config = TransportConfig::default();

    if let Some(value) = as_usize(&data, 0) {
        config.limits.max_frame_bytes = value;
    }
    if let Some(value) = as_usize(&data, 2) {
        config.limits.max_queue_messages = value;
    }
    if let Some(value) = as_usize(&data, 4) {
        config.limits.max_queue_bytes = value;
    }
    if let Some(value) = as_usize(&data, 6) {
        config.send_queue.max_messages = value;
    }
    if let Some(value) = as_usize(&data, 8) {
        config.send_queue.max_bytes = value;
    }
    if let Some(value) = as_millis(&data, 10) {
        config.read_timeout = Duration::from_millis(value);
    }
    if let Some(value) = as_millis(&data, 12) {
        config.write_timeout = Duration::from_millis(value);
    }
    if let Some(value) = as_usize(&data, 14) {
        if let Some(mtu) = config.mtu_safety.as_mut() {
            mtu.max_udp_payload_bytes = value;
        }
    }

    let _ = config.validate();
}

fn as_usize(data: &[u8], offset: usize) -> Option<usize> {
    let bytes = data.get(offset..offset + 2)?;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]) as usize)
}

fn as_millis(data: &[u8], offset: usize) -> Option<u64> {
    let bytes = data.get(offset..offset + 2)?;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]) as u64)
}
