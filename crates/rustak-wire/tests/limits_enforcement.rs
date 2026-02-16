use std::time::Duration;

use rustak_wire::{WireConfig, WireConfigError};

#[test]
fn rejects_adversarial_limits_combinations() {
    let mut config = WireConfig::default();
    let adversarial_cases = [(0, 64, 64), (64, 65, 64), (64, 64, 65)];

    for (frame, xml_scan, protobuf) in adversarial_cases {
        config.limits.max_frame_bytes = frame;
        config.limits.max_xml_scan_bytes = xml_scan;
        config.limits.max_protobuf_bytes = protobuf;
        config.limits.max_queue_bytes = 256;
        config.limits.max_queue_messages = 8;
        config.limits.max_detail_elements = 8;

        let error = config.validate().expect_err("adversarial limits must fail");
        assert!(matches!(error, WireConfigError::InvalidLimits(_)));
    }
}

#[test]
fn rejects_mesh_stale_window_shorter_than_cadence() {
    let mut config = WireConfig::default();
    config.negotiation.mesh_takcontrol_interval = Duration::from_secs(30);
    config.negotiation.mesh_contact_stale_after = Duration::from_secs(10);

    let error = config
        .validate()
        .expect_err("stale window under cadence must fail");
    assert!(matches!(
        error,
        WireConfigError::MeshStaleBeforeCadence { .. }
    ));
}
