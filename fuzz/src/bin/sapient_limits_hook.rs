use std::io::{self, Read};
use std::time::Duration;

use rustak_sapient::SapientConfig;

fn main() {
    let mut data = Vec::new();
    let _ = io::stdin().read_to_end(&mut data);

    let mut config = SapientConfig::default();

    if let Some(value) = as_usize(&data, 0) {
        config.limits.max_frame_bytes = value;
    }
    if let Some(value) = as_usize(&data, 2) {
        config.limits.max_protobuf_bytes = value;
    }
    if let Some(value) = as_usize(&data, 4) {
        config.limits.max_queue_bytes = value;
    }
    if let Some(value) = as_millis(&data, 6) {
        config.read_timeout = Duration::from_millis(value);
    }
    if let Some(value) = as_millis(&data, 8) {
        config.write_timeout = Duration::from_millis(value);
    }
    if let Some(value) = data.get(10) {
        config.tcp_nodelay = value % 2 == 0;
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
