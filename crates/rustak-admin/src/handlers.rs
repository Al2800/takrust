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
    let mut escaped = String::with_capacity(value.len());

    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0C}' => escaped.push_str("\\f"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            control if control <= '\u{1F}' => {
                escaped.push_str("\\u");
                let value = control as u32;
                escaped.push(char::from_digit((value >> 12) & 0xF, 16).unwrap_or('0'));
                escaped.push(char::from_digit((value >> 8) & 0xF, 16).unwrap_or('0'));
                escaped.push(char::from_digit((value >> 4) & 0xF, 16).unwrap_or('0'));
                escaped.push(char::from_digit(value & 0xF, 16).unwrap_or('0'));
            }
            regular => escaped.push(regular),
        }
    }

    escaped
}

#[cfg(test)]
mod tests {
    use super::{
        handle_diagnostics, AdminState, DiagnosticLevel, DiagnosticsSnapshot, ReloadError,
    };

    struct DiagnosticsOnlyState;

    impl AdminState for DiagnosticsOnlyState {
        fn uptime_seconds(&self) -> u64 {
            0
        }

        fn metrics_snapshot(&self) -> String {
            String::new()
        }

        fn request_reload(&self) -> Result<(), ReloadError> {
            Ok(())
        }

        fn diagnostics_snapshot(&self) -> DiagnosticsSnapshot {
            DiagnosticsSnapshot {
                transport: DiagnosticLevel::Warn,
                negotiation: DiagnosticLevel::Error,
                bridge: DiagnosticLevel::Ok,
                notes: vec!["line1\nline2\t\"quoted\"\\slash\r".to_owned()],
            }
        }
    }

    #[test]
    fn diagnostics_json_escapes_control_characters() {
        let response = handle_diagnostics(&DiagnosticsOnlyState);
        assert!(response.body.contains("\\n"));
        assert!(response.body.contains("\\t"));
        assert!(response.body.contains("\\\"quoted\\\""));
        assert!(response.body.contains("\\\\slash"));
        assert!(response.body.contains("\\r"));
        assert!(!response.body.contains("line1\nline2\t"));
    }
}
