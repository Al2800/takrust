use std::time::Duration;

use rustak::prelude::{Limits, Position, TakProtocolVersion, WireConfig, WireFormat};
use rustak::RustakError;

#[test]
fn prelude_exposes_core_and_wire_types() {
    let position = Position::new(51.5, -0.1).expect("position should validate");
    assert_eq!(position.latitude(), 51.5);
    assert_eq!(position.longitude(), -0.1);

    let limits = Limits::default();
    assert!(limits.validate().is_ok());

    let wire_format = WireFormat::TakProtocolV1;
    assert_eq!(wire_format, WireFormat::TakProtocolV1);
    assert_eq!(TakProtocolVersion::V1, TakProtocolVersion::V1);
}

#[test]
fn unified_error_wraps_core_errors() {
    let core_error = Position::new(200.0, 0.0).expect_err("latitude outside valid range");
    let facade_error: RustakError = core_error.into();

    assert!(matches!(facade_error, RustakError::Core(_)));
}

#[test]
fn unified_error_wraps_wire_config_errors() {
    let mut config = WireConfig::default();
    config.negotiation.streaming_timeout = Duration::ZERO;

    let wire_error = config
        .validate()
        .expect_err("zero timeout should fail wire config validation");
    let facade_error: RustakError = wire_error.into();

    assert!(matches!(facade_error, RustakError::WireConfig(_)));
}
