use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand, ValueEnum};
use rustak::RustakError;
use rustak_sapient::SapientCodecError;
use rustak_server::ServerConfigError;
use rustak_wire::{WireFormat, WirePayloadError};
use thiserror::Error;

#[derive(Debug, Parser)]
#[command(
    name = "rustak",
    version,
    about = "Command-line diagnostics and utilities for RusTAK"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Listen(ListenArgs),
    Send(SendArgs),
    Connect(ConnectArgs),
    Sim(SimArgs),
    Replay(ReplayArgs),
    Record(RecordArgs),
    Validate(ValidateArgs),
    Convert(ConvertArgs),
    Certs(CertsArgs),
    Scenario(ScenarioArgs),
    Stress(StressArgs),
    Health(HealthArgs),
    Sapient(SapientArgs),
    Bridge(BridgeArgs),
}

#[derive(Debug, Args)]
pub struct ListenArgs {
    #[arg(long, help = "UDP endpoint to listen on (for example 239.2.3.1:6969)")]
    pub udp: Option<String>,
    #[arg(long, help = "Optional path to rustak YAML config")]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct SendArgs {
    #[arg(long = "type", help = "CoT type string to emit")]
    pub cot_type: Option<String>,
    #[arg(long)]
    pub lat: Option<f64>,
    #[arg(long)]
    pub lon: Option<f64>,
    #[arg(long)]
    pub alt: Option<f64>,
    #[arg(long, help = "Optional path to rustak YAML config")]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ConnectArgs {
    #[arg(long)]
    pub host: Option<String>,
    #[arg(long)]
    pub port: Option<u16>,
    #[arg(long, help = "Optional path to rustak YAML config")]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct SimArgs {
    #[arg(long)]
    pub scenario: Option<PathBuf>,
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long, help = "Optional path to rustak YAML config")]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ReplayArgs {
    #[arg(long)]
    pub input: Option<PathBuf>,
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long)]
    pub speed: Option<f32>,
    #[arg(long, help = "Optional path to rustak YAML config")]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct RecordArgs {
    #[arg(long)]
    pub source: Option<String>,
    #[arg(long)]
    pub output: Option<PathBuf>,
    #[arg(long, help = "Optional path to rustak YAML config")]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ValidationFormat {
    Xml,
    TakV1,
    Sapient,
    Config,
}

#[derive(Debug, Args)]
pub struct ValidateArgs {
    #[arg(long, value_enum)]
    pub format: ValidationFormat,
    #[arg(long, help = "Input file path; defaults to stdin when omitted")]
    pub input: Option<PathBuf>,
    #[arg(long, help = "Optional path to rustak YAML config")]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ConvertFormat {
    Xml,
    TakV1,
}

#[derive(Debug, Args)]
pub struct ConvertArgs {
    #[arg(long, value_enum)]
    pub from: ConvertFormat,
    #[arg(long, value_enum)]
    pub to: ConvertFormat,
    #[arg(long, help = "Input file path; defaults to stdin when omitted")]
    pub input: Option<PathBuf>,
    #[arg(long, help = "Output file path; defaults to stdout when omitted")]
    pub output: Option<PathBuf>,
    #[arg(long, help = "Optional path to rustak YAML config")]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct CertsArgs {
    #[arg(long, help = "Optional path to rustak YAML config")]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ScenarioArgs {
    #[arg(long, help = "Optional path to rustak YAML config")]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct StressArgs {
    #[arg(long)]
    pub count: Option<u32>,
    #[arg(long)]
    pub duration: Option<String>,
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long, help = "Optional path to rustak YAML config")]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct HealthArgs {
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long, help = "Optional path to rustak YAML config")]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct SapientArgs {
    #[arg(long)]
    pub endpoint: Option<String>,
    #[arg(long, help = "Optional path to rustak YAML config")]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct BridgeArgs {
    #[arg(long)]
    pub sapient: Option<String>,
    #[arg(long)]
    pub tak: Option<String>,
    #[arg(long, help = "Optional path to rustak YAML config")]
    pub config: Option<PathBuf>,
}

pub fn run(cli: Cli) -> Result<(), CliError> {
    execute_command(cli.command)
}

fn execute_command(command: Command) -> Result<(), CliError> {
    match command {
        Command::Listen(args) => {
            validate_optional_config(args.config.as_deref())?;
            validate_transport_defaults()?;
            scaffolded("listen")
        }
        Command::Send(args) => {
            validate_optional_config(args.config.as_deref())?;
            validate_transport_defaults()?;
            scaffolded("send")
        }
        Command::Connect(args) => {
            validate_optional_config(args.config.as_deref())?;
            validate_server_defaults()?;
            scaffolded("connect")
        }
        Command::Sim(args) => {
            validate_optional_config(args.config.as_deref())?;
            scaffolded("sim")
        }
        Command::Replay(args) => {
            validate_optional_config(args.config.as_deref())?;
            scaffolded("replay")
        }
        Command::Record(args) => {
            validate_optional_config(args.config.as_deref())?;
            scaffolded("record")
        }
        Command::Validate(args) => run_validate(args),
        Command::Convert(args) => run_convert(args),
        Command::Certs(args) => {
            validate_optional_config(args.config.as_deref())?;
            scaffolded("certs")
        }
        Command::Scenario(args) => {
            validate_optional_config(args.config.as_deref())?;
            scaffolded("scenario")
        }
        Command::Stress(args) => {
            validate_optional_config(args.config.as_deref())?;
            validate_transport_defaults()?;
            scaffolded("stress")
        }
        Command::Health(args) => {
            validate_optional_config(args.config.as_deref())?;
            validate_server_defaults()?;
            scaffolded("health")
        }
        Command::Sapient(args) => {
            validate_optional_config(args.config.as_deref())?;
            validate_sapient_defaults()?;
            scaffolded("sapient")
        }
        Command::Bridge(args) => {
            validate_optional_config(args.config.as_deref())?;
            validate_sapient_defaults()?;
            validate_transport_defaults()?;
            scaffolded("bridge")
        }
    }
}

fn run_validate(args: ValidateArgs) -> Result<(), CliError> {
    validate_optional_config(args.config.as_deref())?;

    match args.format {
        ValidationFormat::Config => {
            let Some(path) = args.input.as_deref() else {
                return Err(CliError::ConfigFormatRequiresInputPath);
            };
            validate_optional_config(Some(path))
        }
        ValidationFormat::Xml => {
            validate_wire_defaults()?;
            let payload = read_input_bytes(args.input.as_deref())?;
            validate_wire_payload(&payload, WireFormat::Xml)
        }
        ValidationFormat::TakV1 => {
            validate_wire_defaults()?;
            let payload = read_input_bytes(args.input.as_deref())?;
            validate_wire_payload(&payload, WireFormat::TakProtocolV1)
        }
        ValidationFormat::Sapient => {
            validate_sapient_defaults()?;
            let payload = read_input_bytes(args.input.as_deref())?;
            ensure_non_empty(&payload)?;
            rustak_sapient::SapientConfig::default()
                .codec()
                .validate_payload(&payload)
                .map_err(CliError::SapientCodec)
        }
    }
}

fn run_convert(args: ConvertArgs) -> Result<(), CliError> {
    validate_optional_config(args.config.as_deref())?;
    validate_wire_defaults()?;
    let payload = read_input_bytes(args.input.as_deref())?;
    let converted = convert_payload(&payload, args.from, args.to)?;
    write_output_bytes(&converted, args.output.as_deref())
}

fn convert_payload(
    payload: &[u8],
    from: ConvertFormat,
    to: ConvertFormat,
) -> Result<Vec<u8>, CliError> {
    ensure_non_empty(payload)?;

    let cot_xml = match from {
        ConvertFormat::Xml => {
            validate_wire_payload(payload, WireFormat::Xml)?;
            payload.to_vec()
        }
        ConvertFormat::TakV1 => {
            rustak_wire::decode_payload_for_format(payload, WireFormat::TakProtocolV1)?
        }
    };

    match to {
        ConvertFormat::Xml => Ok(cot_xml),
        ConvertFormat::TakV1 => Ok(rustak_wire::encode_payload_for_format(
            &cot_xml,
            WireFormat::TakProtocolV1,
        )?),
    }
}

fn validate_wire_payload(payload: &[u8], format: WireFormat) -> Result<(), CliError> {
    ensure_non_empty(payload)?;
    let encoded = rustak_wire::encode_payload_for_format(payload, format)?;
    let decoded = rustak_wire::decode_payload_for_format(&encoded, format)?;
    if decoded != payload {
        return Err(CliError::WireRoundTripMismatch { format });
    }

    Ok(())
}

fn validate_optional_config(path: Option<&Path>) -> Result<(), CliError> {
    if let Some(path) = path {
        let config = rustak_config::RustakConfig::load(path)
            .map_err(|source| CliError::Facade(RustakError::Config(source)))?;
        config
            .validate_startup()
            .map_err(|source| CliError::Facade(RustakError::Config(source)))?;
    }
    Ok(())
}

fn validate_wire_defaults() -> Result<(), CliError> {
    rustak::prelude::WireConfig::default()
        .validate()
        .map_err(|source| CliError::Facade(RustakError::WireConfig(source)))
}

fn validate_transport_defaults() -> Result<(), CliError> {
    rustak_transport::TransportConfig::default()
        .validate()
        .map_err(|source| CliError::Facade(RustakError::Transport(source)))
}

fn validate_sapient_defaults() -> Result<(), CliError> {
    rustak_sapient::SapientConfig::default()
        .validate()
        .map_err(|source| CliError::Facade(RustakError::Sapient(source)))
}

fn validate_server_defaults() -> Result<(), CliError> {
    rustak_server::ServerClientConfig::default()
        .validate()
        .map_err(CliError::ServerConfig)
}

fn ensure_non_empty(payload: &[u8]) -> Result<(), CliError> {
    if payload.is_empty() {
        return Err(CliError::EmptyInput);
    }

    Ok(())
}

fn read_input_bytes(input: Option<&Path>) -> Result<Vec<u8>, CliError> {
    match input {
        Some(path) => fs::read(path).map_err(|source| CliError::InputRead {
            path: path.display().to_string(),
            source,
        }),
        None => {
            let mut payload = Vec::new();
            io::stdin()
                .read_to_end(&mut payload)
                .map_err(|source| CliError::StdinRead { source })?;
            Ok(payload)
        }
    }
}

fn write_output_bytes(payload: &[u8], output: Option<&Path>) -> Result<(), CliError> {
    match output {
        Some(path) => fs::write(path, payload).map_err(|source| CliError::OutputWrite {
            path: path.display().to_string(),
            source,
        }),
        None => io::stdout()
            .write_all(payload)
            .map_err(|source| CliError::StdoutWrite { source }),
    }
}

fn scaffolded(command: &'static str) -> Result<(), CliError> {
    Err(CliError::NotImplemented { command })
}

#[derive(Debug, Error)]
pub enum CliError {
    #[error("`{command}` is scaffolded but not implemented yet")]
    NotImplemented { command: &'static str },

    #[error(transparent)]
    Facade(#[from] RustakError),

    #[error(transparent)]
    WirePayload(#[from] WirePayloadError),

    #[error(transparent)]
    SapientCodec(SapientCodecError),

    #[error(transparent)]
    ServerConfig(ServerConfigError),

    #[error("`validate --format config` requires `--input <path-to-rustak.yaml>`")]
    ConfigFormatRequiresInputPath,

    #[error("wire payload round-trip mismatch for format `{format:?}`")]
    WireRoundTripMismatch { format: WireFormat },

    #[error("failed to read input file `{path}`: {source}")]
    InputRead { path: String, source: io::Error },

    #[error("failed to read stdin: {source}")]
    StdinRead { source: io::Error },

    #[error("input payload must not be empty")]
    EmptyInput,

    #[error("failed to write output file `{path}`: {source}")]
    OutputWrite { path: String, source: io::Error },

    #[error("failed to write stdout: {source}")]
    StdoutWrite { source: io::Error },
}

impl CliError {
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        1
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{
        convert_payload, execute_command, validate_wire_payload, Cli, CliError, Command,
        ConvertFormat, ListenArgs, ValidateArgs, ValidationFormat,
    };

    #[test]
    fn required_entrypoint_commands_parse() {
        assert!(Cli::try_parse_from(["rustak", "listen"]).is_ok());
        assert!(Cli::try_parse_from(["rustak", "send"]).is_ok());
        assert!(Cli::try_parse_from(["rustak", "validate", "--format", "xml"]).is_ok());
        assert!(Cli::try_parse_from(["rustak", "sapient"]).is_ok());
        assert!(Cli::try_parse_from(["rustak", "bridge"]).is_ok());
    }

    #[test]
    fn validate_config_requires_input_path() {
        let error = execute_command(Command::Validate(ValidateArgs {
            format: ValidationFormat::Config,
            input: None,
            config: None,
        }))
        .expect_err("config validation should require input path");

        assert!(matches!(error, CliError::ConfigFormatRequiresInputPath));
    }

    #[test]
    fn xml_validation_routes_through_wire_payload_path() {
        let payload = b"<event uid=\"unit-test\"/>".to_vec();
        validate_wire_payload(&payload, rustak_wire::WireFormat::Xml)
            .expect("xml payload should be wire-roundtrippable");
    }

    #[test]
    fn convert_round_trip_between_xml_and_tak_v1_is_lossless() {
        let xml = b"<event uid=\"unit-test\"/>".to_vec();
        let encoded = convert_payload(&xml, ConvertFormat::Xml, ConvertFormat::TakV1)
            .expect("xml->tak conversion should succeed");
        let decoded = convert_payload(&encoded, ConvertFormat::TakV1, ConvertFormat::Xml)
            .expect("tak->xml conversion should succeed");
        assert_eq!(decoded, xml);
    }

    #[test]
    fn listen_is_explicitly_scaffolded() {
        let error = execute_command(Command::Listen(ListenArgs {
            udp: None,
            config: None,
        }))
        .expect_err("listen should currently be scaffolded");
        assert!(matches!(
            error,
            CliError::NotImplemented { command: "listen" }
        ));
    }
}
