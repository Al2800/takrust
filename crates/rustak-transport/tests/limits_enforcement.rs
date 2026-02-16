use rustak_transport::{
    fuzz_hook_validate_transport_config, TransportConfig, TransportConfigError,
};

#[test]
fn rejects_mtu_payload_above_frame_limit() {
    let mut config = TransportConfig::default();
    let frame_limit = 128;
    config.limits.max_frame_bytes = frame_limit;
    config.limits.max_xml_scan_bytes = frame_limit;
    config.limits.max_protobuf_bytes = frame_limit;
    config.limits.max_queue_messages = 16;
    config.limits.max_queue_bytes = 4_096;
    config.limits.max_detail_elements = 16;
    config.send_queue.max_messages = 16;
    config.send_queue.max_bytes = 4_096;

    let mtu = config
        .mtu_safety
        .as_mut()
        .expect("default config includes mtu safety");
    mtu.max_udp_payload_bytes = frame_limit + 1;

    let error = config
        .validate()
        .expect_err("MTU payload beyond frame budget must fail");
    assert!(matches!(
        error,
        TransportConfigError::MtuPayloadExceedsFrame { .. }
    ));
}

#[test]
fn fuzz_hook_accepts_arbitrary_corpus_without_panics() {
    let corpus = [
        &[][..],
        &[0u8; 2][..],
        &[255u8; 32][..],
        &[1, 0, 255, 0, 1, 2, 3, 4, 5][..],
    ];

    for sample in corpus {
        let _ = fuzz_hook_validate_transport_config(sample);
    }
}
