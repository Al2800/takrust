use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminResponse {
    pub status_code: u16,
    pub content_type: &'static str,
    pub body: String,
}

pub trait AdminState {
    fn uptime_seconds(&self) -> u64;
    fn metrics_snapshot(&self) -> String;
    fn request_reload(&self) -> Result<(), ReloadError>;
    fn diagnostics_snapshot(&self) -> DiagnosticsSnapshot {
        DiagnosticsSnapshot::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Ok,
    Warn,
    Error,
    Unknown,
}

impl DiagnosticLevel {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Error => "error",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsSnapshot {
    pub transport: DiagnosticLevel,
    pub negotiation: DiagnosticLevel,
    pub bridge: DiagnosticLevel,
    pub notes: Vec<String>,
}

impl Default for DiagnosticsSnapshot {
    fn default() -> Self {
        Self {
            transport: DiagnosticLevel::Unknown,
            negotiation: DiagnosticLevel::Unknown,
            bridge: DiagnosticLevel::Unknown,
            notes: Vec::new(),
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReloadError {
    #[error("reload is disabled")]
    Disabled,
    #[error("reload failed: {reason}")]
    Failed { reason: String },
}

#[must_use]
pub fn handle_health<S: AdminState>(state: &S) -> AdminResponse {
    AdminResponse {
        status_code: 200,
        content_type: "application/json",
        body: format!(
            "{{\"status\":\"ok\",\"uptime_seconds\":{}}}",
            state.uptime_seconds()
        ),
    }
}

#[must_use]
pub fn handle_metrics<S: AdminState>(state: &S) -> AdminResponse {
    AdminResponse {
        status_code: 200,
        content_type: "text/plain; version=0.0.4",
        body: state.metrics_snapshot(),
    }
}

pub fn handle_reload<S: AdminState>(state: &S) -> Result<AdminResponse, ReloadError> {
    state.request_reload()?;
    Ok(AdminResponse {
        status_code: 200,
        content_type: "application/json",
        body: "{\"reloaded\":true}".to_owned(),
    })
}

#[must_use]
pub fn handle_diagnostics<S: AdminState>(state: &S) -> AdminResponse {
    let snapshot = state.diagnostics_snapshot();
    let notes = snapshot
        .notes
        .iter()
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .collect::<Vec<_>>()
        .join(",");

    AdminResponse {
        status_code: 200,
        content_type: "application/json",
        body: format!(
            "{{\"transport\":\"{}\",\"negotiation\":\"{}\",\"bridge\":\"{}\",\"notes\":[{}]}}",
            snapshot.transport.as_str(),
            snapshot.negotiation.as_str(),
            snapshot.bridge.as_str(),
            notes,
        ),
    }
}

fn escape_json_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
