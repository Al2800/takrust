use std::path::PathBuf;

use rustak_crypto::{CryptoConfig, CryptoProviderMode, IdentitySource, RevocationPolicy};
use rustak_server::{ConnectionContract, ServerClientConfig, ServerClientError, StreamingClient};
use rustak_transport::TransportFraming;
use rustak_wire::TakProtocolVersion;

fn secure_config() -> ServerClientConfig {
    ServerClientConfig {
        endpoint: "https://tak.example:8443".to_owned(),
        channel_path: "/Marti/api/channels/streaming".to_owned(),
        required_capabilities: vec!["cot-stream".to_owned(), "streaming".to_owned()],
        crypto: Some(CryptoConfig {
            provider: CryptoProviderMode::Ring,
            revocation: RevocationPolicy::Prefer,
            identity: IdentitySource::Pkcs12File {
                archive_path: PathBuf::from("tests/fixtures/certs/dev_identity.p12"),
                password: Some("dev-pass".to_owned()),
            },
        }),
        ..ServerClientConfig::default()
    }
}

#[test]
fn connect_contract_rejects_unreachable_server() {
    let client = StreamingClient::new(secure_config()).expect("config should validate");
    let contract = ConnectionContract {
        server_reachable: false,
        supports_tls: true,
        advertised_channels: vec!["/Marti/api/channels/streaming".to_owned()],
        advertised_capabilities: vec!["cot-stream".to_owned(), "streaming".to_owned()],
    };

    let error = client
        .connect_contract(&contract)
        .expect_err("unreachable server should fail");
    assert!(matches!(error, ServerClientError::ServerUnreachable { .. }));
}

#[test]
fn connect_contract_rejects_missing_channel() {
    let client = StreamingClient::new(secure_config()).expect("config should validate");
    let contract = ConnectionContract {
        server_reachable: true,
        supports_tls: true,
        advertised_channels: vec!["/Marti/api/channels/status".to_owned()],
        advertised_capabilities: vec!["cot-stream".to_owned(), "streaming".to_owned()],
    };

    let error = client
        .connect_contract(&contract)
        .expect_err("missing streaming channel should fail");
    assert!(matches!(error, ServerClientError::MissingChannel { .. }));
}

#[test]
fn connect_contract_returns_session_on_valid_boundary_contract() {
    let client = StreamingClient::new(secure_config()).expect("config should validate");
    let contract = ConnectionContract {
        server_reachable: true,
        supports_tls: true,
        advertised_channels: vec![
            "/Marti/api/channels/health".to_owned(),
            "/Marti/api/channels/streaming".to_owned(),
        ],
        advertised_capabilities: vec![
            "cot-stream".to_owned(),
            "streaming".to_owned(),
            "legacy-xml".to_owned(),
        ],
    };

    let session = client
        .connect_contract(&contract)
        .expect("matching contract should establish session");

    assert_eq!(session.endpoint, "https://tak.example:8443");
    assert_eq!(session.channel_path, "/Marti/api/channels/streaming");
    assert_eq!(session.protocol_version, TakProtocolVersion::V1);
    assert_eq!(session.framing, TransportFraming::XmlNewlineDelimited);
    assert_eq!(
        session.negotiated_capabilities,
        vec!["cot-stream".to_owned(), "streaming".to_owned()]
    );
}
