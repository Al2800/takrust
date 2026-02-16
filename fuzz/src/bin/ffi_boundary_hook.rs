use std::io::Read;

use rustak_ffi::{
    rustak_ffi_buffer_free, rustak_ffi_decode_tak_v1, rustak_ffi_encode_tak_v1, RustakFfiBuffer,
    RustakFfiStatus,
};

fn main() {
    let mut data = Vec::new();
    if std::io::stdin().read_to_end(&mut data).is_err() {
        return;
    }
    if data.is_empty() {
        return;
    }

    let split = data.len() / 2;
    let encode_input = if split == 0 {
        data.as_slice()
    } else {
        &data[..split]
    };

    if !encode_input.is_empty() {
        let mut encoded = RustakFfiBuffer::default();
        let encode_status =
            rustak_ffi_encode_tak_v1(encode_input.as_ptr(), encode_input.len(), &mut encoded);

        if encode_status == RustakFfiStatus::Ok && !encoded.ptr.is_null() {
            let mut decoded = RustakFfiBuffer::default();
            let _ = rustak_ffi_decode_tak_v1(encoded.ptr, encoded.len, &mut decoded);
            let _ = rustak_ffi_buffer_free(&mut decoded);
        }

        let _ = rustak_ffi_buffer_free(&mut encoded);
    }

    let mut scratch = RustakFfiBuffer::default();
    let _ = rustak_ffi_decode_tak_v1(data.as_ptr(), data.len(), std::ptr::null_mut());
    let _ = rustak_ffi_encode_tak_v1(std::ptr::null(), data.len(), &mut scratch);
    let _ = rustak_ffi_buffer_free(&mut scratch);
}
