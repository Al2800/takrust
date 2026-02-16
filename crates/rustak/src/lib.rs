use thiserror::Error;

pub mod prelude {
    pub use rustak_core::{
        CoreError, CotDetail, DetailElement, ExtensionBlob, Kinematics, Position, TimestampUtc,
        Track, XmlElement,
    };
    pub use rustak_io::{
        CotEnvelope, CotMessage, CotSink, CotSource, IoError, MessageEnvelope, MessageSink,
        MessageSource, ObservedTime,
    };
    pub use rustak_limits::{Limits, LimitsError};
    pub use rustak_wire::{DowngradePolicy, TakProtocolVersion, WireConfig, WireFormat};
}

pub type Result<T> = std::result::Result<T, RustakError>;

#[derive(Debug, Error)]
pub enum RustakError {
    #[error(transparent)]
    Core(#[from] rustak_core::CoreError),
    #[error(transparent)]
    Timestamp(#[from] rustak_core::TimestampError),
    #[error(transparent)]
    Io(#[from] rustak_io::IoError),
    #[error(transparent)]
    Limits(#[from] rustak_limits::LimitsError),
    #[error(transparent)]
    WireConfig(#[from] rustak_wire::WireConfigError),
    #[error(transparent)]
    WireFrame(#[from] rustak_wire::WireFrameError),
    #[error(transparent)]
    Transport(#[from] rustak_transport::TransportConfigError),
    #[error(transparent)]
    Bridge(#[from] rustak_bridge::BridgeConfigError),
    #[error(transparent)]
    Sapient(#[from] rustak_sapient::SapientConfigError),
    #[error(transparent)]
    Config(#[from] rustak_config::ConfigError),
    #[error(transparent)]
    Admin(#[from] rustak_admin::AdminConfigError),
    #[error(transparent)]
    Record(#[from] rustak_record::RecordWriteError),
}
