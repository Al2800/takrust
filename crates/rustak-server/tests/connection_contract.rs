use rustak_server::{ServerError, TakServerClient, TakServerConfig};
use rustak_wire::WireFormat;

#[test]
fn connect_exposes_channel_capabilities_and_health() {
    let mut config = TakServerConfig::default();
    config.host = "tak.example.local".to_owned();
    config.wire_format = WireFormat::TakProtocolV1;
    config.transport.wire_format = WireFormat::TakProtocolV1;

    let mut client = TakServerClient::connect(config).expect("connect should succeed");
    assert!(client.is_connected());
    assert!(client.capabilities().supports_streaming);
    assert!(client.capabilities().supports_management_api);
    assert_eq!(
        client.capabilities().negotiated_wire_format,
        WireFormat::TakProtocolV1
    );
    assert_eq!(
        client.cot_channel_config().wire_format,
        WireFormat::TakProtocolV1
    );

    let health = client.health();
    assert!(health.connected);
    assert_eq!(health.host, "tak.example.local");
    assert_eq!(health.streaming_port, 8089);
    assert_eq!(health.api_port, 8443);

    client.disconnect();
    assert!(!client.health().connected);
}

#[test]
fn connect_rejects_invalid_contract_boundaries() {
    let mut empty_host = TakServerConfig::default();
    empty_host.host.clear();
    let host_error = TakServerClient::connect(empty_host).expect_err("empty host must fail");
    assert!(matches!(host_error, ServerError::EmptyHost));

    let mut wire_mismatch = TakServerConfig::default();
    wire_mismatch.wire_format = WireFormat::TakProtocolV1;
    wire_mismatch.transport.wire_format = WireFormat::Xml;
    let mismatch_error =
        TakServerClient::connect(wire_mismatch).expect_err("wire mismatch must fail");
    assert!(matches!(
        mismatch_error,
        ServerError::WireFormatMismatch { .. }
    ));
}
