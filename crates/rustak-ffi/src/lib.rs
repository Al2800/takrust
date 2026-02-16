use std::ffi::c_char;
use std::ptr;

pub const RUSTAK_FFI_ABI_MAJOR: u16 = 1;
pub const RUSTAK_FFI_ABI_MINOR: u16 = 0;
pub const RUSTAK_FFI_ABI_PATCH: u16 = 0;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RustakFfiVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

#[repr(C)]
#[derive(Debug)]
pub struct RustakFfiBuffer {
    pub ptr: *mut u8,
    pub len: usize,
    pub capacity: usize,
}

impl Default for RustakFfiBuffer {
    fn default() -> Self {
        Self {
            ptr: ptr::null_mut(),
            len: 0,
            capacity: 0,
        }
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustakFfiStatus {
    Ok = 0,
    NullPointer = 1,
    InvalidLength = 2,
    EmptyInput = 3,
    UnsupportedVersion = 4,
    EncodeError = 5,
    DecodeError = 6,
}

impl RustakFfiStatus {
    const fn from_code(code: i32) -> Self {
        match code {
            0 => Self::Ok,
            1 => Self::NullPointer,
            2 => Self::InvalidLength,
            3 => Self::EmptyInput,
            4 => Self::UnsupportedVersion,
            5 => Self::EncodeError,
            6 => Self::DecodeError,
            _ => Self::DecodeError,
        }
    }
}

#[no_mangle]
pub extern "C" fn rustak_ffi_current_abi_version() -> RustakFfiVersion {
    RustakFfiVersion {
        major: RUSTAK_FFI_ABI_MAJOR,
        minor: RUSTAK_FFI_ABI_MINOR,
        patch: RUSTAK_FFI_ABI_PATCH,
    }
}

#[no_mangle]
/// # Safety
///
/// `out_version` must be either null or a valid, writable pointer to a
/// `RustakFfiVersion`.
pub unsafe extern "C" fn rustak_ffi_negotiate_abi_version(
    requested_major: u16,
    out_version: *mut RustakFfiVersion,
) -> RustakFfiStatus {
    if out_version.is_null() {
        return RustakFfiStatus::NullPointer;
    }

    let version = rustak_ffi_current_abi_version();
    unsafe { out_version.write(version) };

    if requested_major == version.major {
        RustakFfiStatus::Ok
    } else {
        RustakFfiStatus::UnsupportedVersion
    }
}

#[no_mangle]
pub extern "C" fn rustak_ffi_status_message(status_code: i32) -> *const c_char {
    const OK: &[u8] = b"ok\0";
    const NULL_POINTER: &[u8] = b"null pointer\0";
    const INVALID_LENGTH: &[u8] = b"invalid length\0";
    const EMPTY_INPUT: &[u8] = b"empty input\0";
    const UNSUPPORTED_VERSION: &[u8] = b"unsupported ABI major version\0";
    const ENCODE_ERROR: &[u8] = b"encode error\0";
    const DECODE_ERROR: &[u8] = b"decode error\0";

    let bytes = match RustakFfiStatus::from_code(status_code) {
        RustakFfiStatus::Ok => OK,
        RustakFfiStatus::NullPointer => NULL_POINTER,
        RustakFfiStatus::InvalidLength => INVALID_LENGTH,
        RustakFfiStatus::EmptyInput => EMPTY_INPUT,
        RustakFfiStatus::UnsupportedVersion => UNSUPPORTED_VERSION,
        RustakFfiStatus::EncodeError => ENCODE_ERROR,
        RustakFfiStatus::DecodeError => DECODE_ERROR,
    };
    bytes.as_ptr().cast::<c_char>()
}

#[no_mangle]
/// # Safety
///
/// `input_ptr` must point to `input_len` readable bytes, and `out_buffer`
/// must be a valid, writable pointer to a `RustakFfiBuffer`.
pub unsafe extern "C" fn rustak_ffi_encode_tak_v1(
    input_ptr: *const u8,
    input_len: usize,
    out_buffer: *mut RustakFfiBuffer,
) -> RustakFfiStatus {
    let input = match unsafe { copy_input(input_ptr, input_len) } {
        Ok(input) => input,
        Err(status) => return status,
    };

    let encoded = match rustak_proto::encode_v1_payload(&input) {
        Ok(encoded) => encoded,
        Err(_) => return RustakFfiStatus::EncodeError,
    };

    unsafe { write_vec_to_out_buffer(encoded, out_buffer) }
}

#[no_mangle]
/// # Safety
///
/// `input_ptr` must point to `input_len` readable bytes, and `out_buffer`
/// must be a valid, writable pointer to a `RustakFfiBuffer`.
pub unsafe extern "C" fn rustak_ffi_decode_tak_v1(
    input_ptr: *const u8,
    input_len: usize,
    out_buffer: *mut RustakFfiBuffer,
) -> RustakFfiStatus {
    let input = match unsafe { copy_input(input_ptr, input_len) } {
        Ok(input) => input,
        Err(status) => return status,
    };

    let decoded = match rustak_proto::decode_v1_payload(&input) {
        Ok(decoded) => decoded,
        Err(_) => return RustakFfiStatus::DecodeError,
    };

    unsafe { write_vec_to_out_buffer(decoded, out_buffer) }
}

#[no_mangle]
/// # Safety
///
/// `buffer` must be either null or a valid pointer to a `RustakFfiBuffer`
/// previously initialized by RusTAK FFI APIs.
pub unsafe extern "C" fn rustak_ffi_buffer_free(buffer: *mut RustakFfiBuffer) -> RustakFfiStatus {
    if buffer.is_null() {
        return RustakFfiStatus::NullPointer;
    }

    let buffer_ref = unsafe { &mut *buffer };

    if buffer_ref.ptr.is_null() {
        if buffer_ref.len == 0 && buffer_ref.capacity == 0 {
            return RustakFfiStatus::Ok;
        }
        return RustakFfiStatus::InvalidLength;
    }

    if buffer_ref.len > buffer_ref.capacity {
        return RustakFfiStatus::InvalidLength;
    }

    unsafe {
        let _ = Vec::from_raw_parts(buffer_ref.ptr, buffer_ref.len, buffer_ref.capacity);
    };

    buffer_ref.ptr = ptr::null_mut();
    buffer_ref.len = 0;
    buffer_ref.capacity = 0;
    RustakFfiStatus::Ok
}

unsafe fn copy_input(input_ptr: *const u8, input_len: usize) -> Result<Vec<u8>, RustakFfiStatus> {
    if input_ptr.is_null() {
        return Err(RustakFfiStatus::NullPointer);
    }
    if input_len == 0 {
        return Err(RustakFfiStatus::EmptyInput);
    }
    if input_len > isize::MAX as usize {
        return Err(RustakFfiStatus::InvalidLength);
    }

    let bytes = unsafe { std::slice::from_raw_parts(input_ptr, input_len) };
    Ok(bytes.to_vec())
}

unsafe fn write_vec_to_out_buffer(
    mut bytes: Vec<u8>,
    out_buffer: *mut RustakFfiBuffer,
) -> RustakFfiStatus {
    if out_buffer.is_null() {
        return RustakFfiStatus::NullPointer;
    }

    let output = RustakFfiBuffer {
        ptr: bytes.as_mut_ptr(),
        len: bytes.len(),
        capacity: bytes.capacity(),
    };
    std::mem::forget(bytes);

    unsafe { out_buffer.write(output) };
    RustakFfiStatus::Ok
}

#[cfg(test)]
mod tests {
    use super::{
        rustak_ffi_buffer_free, rustak_ffi_current_abi_version, rustak_ffi_decode_tak_v1,
        rustak_ffi_encode_tak_v1, rustak_ffi_negotiate_abi_version, RustakFfiBuffer,
        RustakFfiStatus, RustakFfiVersion, RUSTAK_FFI_ABI_MAJOR,
    };

    #[test]
    fn matching_major_version_negotiates_successfully() {
        let mut negotiated = RustakFfiVersion {
            major: 0,
            minor: 0,
            patch: 0,
        };
        let status =
            unsafe { rustak_ffi_negotiate_abi_version(RUSTAK_FFI_ABI_MAJOR, &mut negotiated) };
        assert_eq!(status, RustakFfiStatus::Ok);
        assert_eq!(negotiated, rustak_ffi_current_abi_version());
    }

    #[test]
    fn mismatched_major_version_returns_unsupported() {
        let mut negotiated = RustakFfiVersion {
            major: 0,
            minor: 0,
            patch: 0,
        };
        let status =
            unsafe { rustak_ffi_negotiate_abi_version(RUSTAK_FFI_ABI_MAJOR + 1, &mut negotiated) };
        assert_eq!(status, RustakFfiStatus::UnsupportedVersion);
        assert_eq!(negotiated, rustak_ffi_current_abi_version());
    }

    #[test]
    fn encode_decode_round_trip_respects_buffer_ownership_contract() {
        let source = b"<event uid=\"ffi-roundtrip\"/>".to_vec();
        let mut encoded = RustakFfiBuffer::default();
        let mut decoded = RustakFfiBuffer::default();

        let encode_status =
            unsafe { rustak_ffi_encode_tak_v1(source.as_ptr(), source.len(), &mut encoded) };
        assert_eq!(encode_status, RustakFfiStatus::Ok);

        let decode_status =
            unsafe { rustak_ffi_decode_tak_v1(encoded.ptr, encoded.len, &mut decoded) };
        assert_eq!(decode_status, RustakFfiStatus::Ok);

        let round_trip = unsafe { std::slice::from_raw_parts(decoded.ptr, decoded.len) };
        assert_eq!(round_trip, source.as_slice());

        assert_eq!(
            unsafe { rustak_ffi_buffer_free(&mut encoded) },
            RustakFfiStatus::Ok
        );
        assert_eq!(
            unsafe { rustak_ffi_buffer_free(&mut decoded) },
            RustakFfiStatus::Ok
        );
        assert!(encoded.ptr.is_null());
        assert!(decoded.ptr.is_null());
    }

    #[test]
    fn null_output_buffer_is_rejected() {
        let source = b"<event uid=\"ffi-null\"/>".to_vec();
        let status = unsafe {
            rustak_ffi_encode_tak_v1(source.as_ptr(), source.len(), std::ptr::null_mut())
        };
        assert_eq!(status, RustakFfiStatus::NullPointer);
    }
}
