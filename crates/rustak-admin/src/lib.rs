pub mod config;

pub use config::{AdminConfig, AdminConfigError};

#[cfg(feature = "admin-server")]
pub mod handlers;
#[cfg(feature = "admin-server")]
pub mod server;

#[cfg(feature = "admin-server")]
pub use handlers::{
    handle_diagnostics, handle_health, handle_metrics, handle_reload, AdminResponse, AdminState,
    DiagnosticLevel, DiagnosticsSnapshot, ReloadError,
};
#[cfg(feature = "admin-server")]
pub use server::{AdminServer, AdminServerError};

#[cfg(all(test, feature = "admin-server"))]
mod server_tests {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use crate::{
        AdminConfig, AdminServer, AdminServerError, AdminState, DiagnosticLevel,
        DiagnosticsSnapshot, ReloadError,
    };

    #[derive(Debug)]
    struct MockState {
        uptime_seconds: u64,
        metrics: String,
        diagnostics: DiagnosticsSnapshot,
        allow_reload: bool,
        reload_calls: AtomicUsize,
    }

    impl MockState {
        fn new(
            uptime_seconds: u64,
            metrics: &str,
            diagnostics: DiagnosticsSnapshot,
            allow_reload: bool,
        ) -> Self {
            Self {
                uptime_seconds,
                metrics: metrics.to_owned(),
                diagnostics,
                allow_reload,
                reload_calls: AtomicUsize::new(0),
            }
        }
    }

    impl AdminState for MockState {
        fn uptime_seconds(&self) -> u64 {
            self.uptime_seconds
        }

        fn metrics_snapshot(&self) -> String {
            self.metrics.clone()
        }

        fn request_reload(&self) -> Result<(), ReloadError> {
            if !self.allow_reload {
                return Err(ReloadError::Disabled);
            }
            self.reload_calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        fn diagnostics_snapshot(&self) -> DiagnosticsSnapshot {
            self.diagnostics.clone()
        }
    }

    #[test]
    fn health_and_metrics_dispatches_are_deterministic() {
        let config = AdminConfig {
            enabled: true,
            ..AdminConfig::default()
        };
        let state = Arc::new(MockState::new(
            42,
            "rustak_metric 1",
            DiagnosticsSnapshot {
                transport: DiagnosticLevel::Ok,
                negotiation: DiagnosticLevel::Warn,
                bridge: DiagnosticLevel::Unknown,
                notes: vec!["link flap recovered".to_owned()],
            },
            true,
        ));
        let server = AdminServer::new(config, state).expect("server should construct");

        let health = server
            .dispatch("/healthz")
            .expect("health endpoint should succeed");
        assert_eq!(health.status_code, 200);
        assert_eq!(health.content_type, "application/json");
        assert!(health.body.contains("\"status\":\"ok\""));
        assert!(health.body.contains("\"uptime_seconds\":42"));

        let metrics = server
            .dispatch("/metrics")
            .expect("metrics endpoint should succeed");
        assert_eq!(metrics.status_code, 200);
        assert_eq!(metrics.content_type, "text/plain; version=0.0.4");
        assert_eq!(metrics.body, "rustak_metric 1");

        let diagnostics = server
            .dispatch("/diagnostics")
            .expect("diagnostics endpoint should succeed");
        assert_eq!(diagnostics.status_code, 200);
        assert_eq!(diagnostics.content_type, "application/json");
        assert!(diagnostics.body.contains("\"transport\":\"ok\""));
        assert!(diagnostics.body.contains("\"negotiation\":\"warn\""));
        assert!(diagnostics
            .body
            .contains("\"notes\":[\"link flap recovered\"]"));
    }

    #[test]
    fn reload_dispatch_works_when_explicitly_enabled() {
        let config = AdminConfig {
            enabled: true,
            reload_path: Some("/reload".to_owned()),
            allow_reload: true,
            ..AdminConfig::default()
        };
        let state = Arc::new(MockState::new(
            7,
            "rustak_metric 2",
            DiagnosticsSnapshot::default(),
            true,
        ));
        let server = AdminServer::new(config, state.clone()).expect("server should construct");

        let reload = server
            .dispatch("/reload")
            .expect("reload endpoint should succeed");
        assert_eq!(reload.status_code, 200);
        assert_eq!(reload.body, "{\"reloaded\":true}");
        assert_eq!(state.reload_calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn disabled_server_rejects_dispatch() {
        let config = AdminConfig::default();
        let state = Arc::new(MockState::new(
            7,
            "rustak_metric 2",
            DiagnosticsSnapshot::default(),
            true,
        ));
        let server = AdminServer::new(config, state).expect("server should construct");

        let error = server
            .dispatch("/healthz")
            .expect_err("disabled admin server must reject requests");
        assert!(matches!(error, AdminServerError::Disabled));
    }
}
