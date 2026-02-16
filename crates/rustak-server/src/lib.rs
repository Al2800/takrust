use rustak_crypto::{CryptoConfig, CryptoError, ProviderSupport};
use rustak_transport::{TransportConfig, TransportConfigError};
use rustak_wire::WireFormat;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct TakServerConfig {
    pub host: String,
    pub streaming_port: u16,
    pub api_port: u16,
    pub wire_format: WireFormat,
    pub transport: TransportConfig,
    pub crypto: Option<CryptoConfig>,
    pub provider_support: ProviderSupport,
}

impl Default for TakServerConfig {
    fn default() -> Self {
        let mut transport = TransportConfig::default();
        transport.wire_format = WireFormat::Xml;

        Self {
            host: "127.0.0.1".to_owned(),
            streaming_port: 8089,
            api_port: 8443,
            wire_format: WireFormat::Xml,
            transport,
            crypto: None,
            provider_support: ProviderSupport::default(),
        }
    }
}

impl TakServerConfig {
    pub fn validate(&self) -> Result<(), ServerError> {
        if self.host.trim().is_empty() {
            return Err(ServerError::EmptyHost);
        }
        if self.streaming_port == 0 {
            return Err(ServerError::InvalidPort {
                field: "streaming_port",
            });
        }
        if self.api_port == 0 {
            return Err(ServerError::InvalidPort { field: "api_port" });
        }
        if self.streaming_port == self.api_port {
            return Err(ServerError::PortConflict {
                streaming_port: self.streaming_port,
                api_port: self.api_port,
            });
        }

        self.transport.validate()?;
        if self.transport.wire_format != self.wire_format {
            return Err(ServerError::WireFormatMismatch {
                config: self.wire_format,
                transport: self.transport.wire_format,
            });
        }

        if let Some(crypto) = &self.crypto {
            crypto.validate(self.provider_support)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerCapabilities {
    pub supports_streaming: bool,
    pub supports_management_api: bool,
    pub negotiated_wire_format: WireFormat,
}

impl ServerCapabilities {
    #[must_use]
    pub const fn from_wire_format(wire_format: WireFormat) -> Self {
        Self {
            supports_streaming: true,
            supports_management_api: true,
            negotiated_wire_format: wire_format,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerHealth {
    pub connected: bool,
    pub host: String,
    pub streaming_port: u16,
    pub api_port: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TakServerClient {
    config: TakServerConfig,
    capabilities: ServerCapabilities,
    connected: bool,
}

impl TakServerClient {
    pub fn connect(config: TakServerConfig) -> Result<Self, ServerError> {
        config.validate()?;

        Ok(Self {
            capabilities: ServerCapabilities::from_wire_format(config.wire_format),
            config,
            connected: true,
        })
    }

    #[must_use]
    pub const fn is_connected(&self) -> bool {
        self.connected
    }

    #[must_use]
    pub fn cot_channel_config(&self) -> &TransportConfig {
        &self.config.transport
    }

    #[must_use]
    pub const fn capabilities(&self) -> &ServerCapabilities {
        &self.capabilities
    }

    #[must_use]
    pub fn health(&self) -> ServerHealth {
        ServerHealth {
            connected: self.connected,
            host: self.config.host.clone(),
            streaming_port: self.config.streaming_port,
            api_port: self.config.api_port,
        }
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
    }
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("host must not be empty")]
    EmptyHost,

    #[error("{field} must be a non-zero port")]
    InvalidPort { field: &'static str },

    #[error(
        "streaming and api ports must differ (streaming_port={streaming_port}, api_port={api_port})"
    )]
    PortConflict { streaming_port: u16, api_port: u16 },

    #[error("wire format mismatch: config={config:?}, transport={transport:?}")]
    WireFormatMismatch {
        config: WireFormat,
        transport: WireFormat,
    },

    #[error(transparent)]
    InvalidTransport(#[from] TransportConfigError),

    #[error(transparent)]
    InvalidCrypto(#[from] CryptoError),
}
