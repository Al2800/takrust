use std::collections::HashSet;

use rustak_crypto::{CryptoConfig, CryptoError, ProviderSupport};
use rustak_transport::{TransportConfig, TransportConfigError, TransportFraming};
use rustak_wire::TakProtocolVersion;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct ServerClientConfig {
    pub endpoint: String,
    pub channel_path: String,
    pub required_capabilities: Vec<String>,
    pub transport: TransportConfig,
    pub protocol_version: TakProtocolVersion,
    pub crypto: Option<CryptoConfig>,
    pub provider_support: ProviderSupport,
}

impl Default for ServerClientConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:8089".to_owned(),
            channel_path: "/Marti/api/channels/streaming".to_owned(),
            required_capabilities: vec!["cot-stream".to_owned()],
            transport: TransportConfig::default(),
            protocol_version: TakProtocolVersion::V1,
            crypto: None,
            provider_support: ProviderSupport::default(),
        }
    }
}

impl ServerClientConfig {
    pub fn validate(&self) -> Result<(), ServerConfigError> {
        validate_endpoint(&self.endpoint)?;
        validate_channel_path(&self.channel_path)?;
        validate_capabilities(&self.required_capabilities)?;
        self.transport.validate()?;

        if self.requires_tls() && self.crypto.is_none() {
            return Err(ServerConfigError::TlsEndpointRequiresCryptoConfig);
        }

        if let Some(crypto) = &self.crypto {
            crypto.validate(self.provider_support)?;
        }

        Ok(())
    }

    fn requires_tls(&self) -> bool {
        self.endpoint.starts_with("https://")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionContract {
    pub server_reachable: bool,
    pub supports_tls: bool,
    pub advertised_channels: Vec<String>,
    pub advertised_capabilities: Vec<String>,
}

impl ConnectionContract {
    #[must_use]
    pub fn local_simulated() -> Self {
        Self {
            server_reachable: true,
            supports_tls: false,
            advertised_channels: vec!["/Marti/api/channels/streaming".to_owned()],
            advertised_capabilities: vec!["cot-stream".to_owned()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamingSession {
    pub endpoint: String,
    pub channel_path: String,
    pub framing: TransportFraming,
    pub protocol_version: TakProtocolVersion,
    pub negotiated_capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StreamingClient {
    config: ServerClientConfig,
}

impl StreamingClient {
    pub fn new(config: ServerClientConfig) -> Result<Self, ServerConfigError> {
        config.validate()?;
        Ok(Self { config })
    }

    #[must_use]
    pub fn config(&self) -> &ServerClientConfig {
        &self.config
    }

    pub fn connect_contract(
        &self,
        contract: &ConnectionContract,
    ) -> Result<StreamingSession, ServerClientError> {
        if !contract.server_reachable {
            return Err(ServerClientError::ServerUnreachable {
                endpoint: self.config.endpoint.clone(),
            });
        }

        if self.config.requires_tls() && !contract.supports_tls {
            return Err(ServerClientError::TlsRequired);
        }

        if !contract
            .advertised_channels
            .iter()
            .any(|path| path == &self.config.channel_path)
        {
            return Err(ServerClientError::MissingChannel {
                channel_path: self.config.channel_path.clone(),
            });
        }

        for capability in &self.config.required_capabilities {
            if !contract
                .advertised_capabilities
                .iter()
                .any(|item| item == capability)
            {
                return Err(ServerClientError::MissingCapability {
                    capability: capability.clone(),
                });
            }
        }

        Ok(StreamingSession {
            endpoint: self.config.endpoint.clone(),
            channel_path: self.config.channel_path.clone(),
            framing: TransportFraming::from(self.config.transport.wire_format),
            protocol_version: self.config.protocol_version,
            negotiated_capabilities: self.config.required_capabilities.clone(),
        })
    }
}

#[derive(Debug, Error)]
pub enum ServerConfigError {
    #[error("endpoint must not be empty")]
    EmptyEndpoint,

    #[error("endpoint must start with http:// or https://")]
    EndpointMustBeHttpOrHttps,

    #[error("channel_path must not be empty")]
    EmptyChannelPath,

    #[error("channel_path must start with '/'")]
    ChannelPathMustStartWithSlash,

    #[error("channel_path must not be '/'")]
    RootChannelPathNotAllowed,

    #[error("required capability must not be empty")]
    EmptyCapability,

    #[error("duplicate required capability `{capability}`")]
    DuplicateCapability { capability: String },

    #[error("https endpoint requires crypto config")]
    TlsEndpointRequiresCryptoConfig,

    #[error(transparent)]
    InvalidTransport(#[from] TransportConfigError),

    #[error(transparent)]
    InvalidCrypto(#[from] CryptoError),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ServerClientError {
    #[error("server unreachable at `{endpoint}`")]
    ServerUnreachable { endpoint: String },

    #[error("tls is required by client endpoint but server contract does not support it")]
    TlsRequired,

    #[error("server does not advertise required channel `{channel_path}`")]
    MissingChannel { channel_path: String },

    #[error("server does not advertise required capability `{capability}`")]
    MissingCapability { capability: String },
}

fn validate_endpoint(endpoint: &str) -> Result<(), ServerConfigError> {
    if endpoint.trim().is_empty() {
        return Err(ServerConfigError::EmptyEndpoint);
    }

    if !(endpoint.starts_with("http://") || endpoint.starts_with("https://")) {
        return Err(ServerConfigError::EndpointMustBeHttpOrHttps);
    }

    Ok(())
}

fn validate_channel_path(path: &str) -> Result<(), ServerConfigError> {
    if path.trim().is_empty() {
        return Err(ServerConfigError::EmptyChannelPath);
    }
    if !path.starts_with('/') {
        return Err(ServerConfigError::ChannelPathMustStartWithSlash);
    }
    if path == "/" {
        return Err(ServerConfigError::RootChannelPathNotAllowed);
    }

    Ok(())
}

fn validate_capabilities(capabilities: &[String]) -> Result<(), ServerConfigError> {
    let mut seen = HashSet::new();
    for capability in capabilities {
        if capability.trim().is_empty() {
            return Err(ServerConfigError::EmptyCapability);
        }
        if !seen.insert(capability) {
            return Err(ServerConfigError::DuplicateCapability {
                capability: capability.clone(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ServerClientConfig, ServerConfigError, StreamingClient};
    use rustak_crypto::{
        CryptoConfig, CryptoProviderMode, IdentitySource, ProviderSupport, RevocationPolicy,
    };
    use rustak_wire::TakProtocolVersion;
    use std::path::PathBuf;

    #[test]
    fn rejects_https_endpoint_without_crypto() {
        let config = ServerClientConfig {
            endpoint: "https://tak.example:8443".to_owned(),
            ..ServerClientConfig::default()
        };

        let error = StreamingClient::new(config)
            .expect_err("https endpoint should require crypto configuration");
        assert!(matches!(
            error,
            ServerConfigError::TlsEndpointRequiresCryptoConfig
        ));
    }

    #[test]
    fn rejects_duplicate_capabilities() {
        let config = ServerClientConfig {
            required_capabilities: vec!["cot-stream".to_owned(), "cot-stream".to_owned()],
            ..ServerClientConfig::default()
        };

        let error =
            StreamingClient::new(config).expect_err("duplicate capabilities should be rejected");
        assert!(matches!(
            error,
            ServerConfigError::DuplicateCapability { .. }
        ));
    }

    #[test]
    fn accepts_https_endpoint_with_crypto_config() {
        let config = ServerClientConfig {
            endpoint: "https://tak.example:8443".to_owned(),
            crypto: Some(CryptoConfig {
                provider: CryptoProviderMode::Ring,
                revocation: RevocationPolicy::Prefer,
                identity: IdentitySource::Pkcs12File {
                    archive_path: PathBuf::from("tests/fixtures/certs/dev_identity.p12"),
                    password: Some("dev-pass".to_owned()),
                },
            }),
            protocol_version: TakProtocolVersion::V1,
            ..ServerClientConfig::default()
        };

        assert!(StreamingClient::new(config).is_ok());
    }

    #[test]
    fn rejects_fips_provider_without_support() {
        let config = ServerClientConfig {
            endpoint: "https://tak.example:8443".to_owned(),
            crypto: Some(CryptoConfig {
                provider: CryptoProviderMode::AwsLcRsFips,
                revocation: RevocationPolicy::Prefer,
                identity: IdentitySource::Pkcs12File {
                    archive_path: PathBuf::from("tests/fixtures/certs/dev_identity.p12"),
                    password: Some("dev-pass".to_owned()),
                },
            }),
            ..ServerClientConfig::default()
        };

        let error =
            StreamingClient::new(config).expect_err("fips provider should require support flag");
        assert!(matches!(error, ServerConfigError::InvalidCrypto(_)));
    }

    #[test]
    fn accepts_fips_provider_with_support() {
        let config = ServerClientConfig {
            endpoint: "https://tak.example:8443".to_owned(),
            crypto: Some(CryptoConfig {
                provider: CryptoProviderMode::AwsLcRsFips,
                revocation: RevocationPolicy::Prefer,
                identity: IdentitySource::Pkcs12File {
                    archive_path: PathBuf::from("tests/fixtures/certs/dev_identity.p12"),
                    password: Some("dev-pass".to_owned()),
                },
            }),
            provider_support: ProviderSupport::with_fips_enabled(true),
            ..ServerClientConfig::default()
        };

        assert!(StreamingClient::new(config).is_ok());
    }
}
