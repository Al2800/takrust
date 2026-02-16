use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminConfig {
    pub enabled: bool,
    pub bind: SocketAddr,
    pub health_path: String,
    pub metrics_path: String,
    pub diagnostics_path: String,
    pub reload_path: Option<String>,
    pub allow_reload: bool,
    pub allow_non_loopback_bind: bool,
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9091),
            health_path: "/healthz".to_owned(),
            metrics_path: "/metrics".to_owned(),
            diagnostics_path: "/diagnostics".to_owned(),
            reload_path: None,
            allow_reload: false,
            allow_non_loopback_bind: false,
        }
    }
}

impl AdminConfig {
    pub fn validate(&self) -> Result<(), AdminConfigError> {
        validate_path("health_path", &self.health_path)?;
        validate_path("metrics_path", &self.metrics_path)?;
        validate_path("diagnostics_path", &self.diagnostics_path)?;
        if self.health_path == self.metrics_path {
            return Err(AdminConfigError::DuplicatePath {
                first: "health_path",
                second: "metrics_path",
                path: self.health_path.clone(),
            });
        }
        if self.health_path == self.diagnostics_path {
            return Err(AdminConfigError::DuplicatePath {
                first: "health_path",
                second: "diagnostics_path",
                path: self.health_path.clone(),
            });
        }
        if self.metrics_path == self.diagnostics_path {
            return Err(AdminConfigError::DuplicatePath {
                first: "metrics_path",
                second: "diagnostics_path",
                path: self.metrics_path.clone(),
            });
        }

        if let Some(path) = &self.reload_path {
            validate_path("reload_path", path)?;
            if path == &self.health_path {
                return Err(AdminConfigError::DuplicatePath {
                    first: "reload_path",
                    second: "health_path",
                    path: path.clone(),
                });
            }
            if path == &self.metrics_path {
                return Err(AdminConfigError::DuplicatePath {
                    first: "reload_path",
                    second: "metrics_path",
                    path: path.clone(),
                });
            }
            if path == &self.diagnostics_path {
                return Err(AdminConfigError::DuplicatePath {
                    first: "reload_path",
                    second: "diagnostics_path",
                    path: path.clone(),
                });
            }
            if !self.allow_reload {
                return Err(AdminConfigError::ReloadPathRequiresEnable);
            }
        }

        if self.enabled && !self.allow_non_loopback_bind && !self.bind.ip().is_loopback() {
            return Err(AdminConfigError::NonLoopbackBindDisallowed { bind: self.bind });
        }

        Ok(())
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AdminConfigError {
    #[error("path {field} must not be empty")]
    EmptyPath { field: &'static str },
    #[error("path {field} must start with '/'")]
    PathMustStartWithSlash { field: &'static str },
    #[error("path {field} must not be '/'")]
    RootPathNotAllowed { field: &'static str },
    #[error("paths {first} and {second} must be unique (got {path})")]
    DuplicatePath {
        first: &'static str,
        second: &'static str,
        path: String,
    },
    #[error("reload_path requires allow_reload=true")]
    ReloadPathRequiresEnable,
    #[error(
        "admin bind address must be loopback unless allow_non_loopback_bind=true (got {bind})"
    )]
    NonLoopbackBindDisallowed { bind: SocketAddr },
}

fn validate_path(field: &'static str, path: &str) -> Result<(), AdminConfigError> {
    if path.trim().is_empty() {
        return Err(AdminConfigError::EmptyPath { field });
    }
    if !path.starts_with('/') {
        return Err(AdminConfigError::PathMustStartWithSlash { field });
    }
    if path == "/" {
        return Err(AdminConfigError::RootPathNotAllowed { field });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use crate::config::{AdminConfig, AdminConfigError};

    #[test]
    fn defaults_are_secure_and_valid() {
        let config = AdminConfig::default();
        assert!(!config.enabled);
        assert!(!config.allow_reload);
        assert!(!config.allow_non_loopback_bind);
        assert_eq!(config.reload_path, None);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn rejects_non_loopback_bind_when_enabled() {
        let config = AdminConfig {
            enabled: true,
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 9091),
            ..AdminConfig::default()
        };

        let error = config
            .validate()
            .expect_err("non-loopback bind must fail with secure defaults");
        assert!(matches!(
            error,
            AdminConfigError::NonLoopbackBindDisallowed { .. }
        ));
    }

    #[test]
    fn reload_path_requires_allow_reload() {
        let config = AdminConfig {
            reload_path: Some("/reload".to_owned()),
            allow_reload: false,
            ..AdminConfig::default()
        };

        let error = config
            .validate()
            .expect_err("reload path must require explicit allow_reload");
        assert_eq!(error, AdminConfigError::ReloadPathRequiresEnable);
    }

    #[test]
    fn rejects_duplicate_diagnostics_path() {
        let config = AdminConfig {
            diagnostics_path: "/metrics".to_owned(),
            ..AdminConfig::default()
        };

        let error = config
            .validate()
            .expect_err("diagnostics path must be unique");
        assert!(matches!(
            error,
            AdminConfigError::DuplicatePath {
                first: "metrics_path",
                second: "diagnostics_path",
                ..
            }
        ));
    }
}
