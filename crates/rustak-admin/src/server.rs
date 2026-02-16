use std::sync::Arc;

use thiserror::Error;

use crate::{
    config::{AdminConfig, AdminConfigError},
    handlers::{
        handle_diagnostics, handle_health, handle_metrics, handle_reload, AdminResponse,
        AdminState, ReloadError,
    },
};

#[derive(Debug, Error)]
pub enum AdminServerError {
    #[error("admin server is disabled")]
    Disabled,
    #[error("unknown admin path: {path}")]
    UnknownPath { path: String },
    #[error("reload endpoint is disabled")]
    ReloadDisabled,
    #[error(transparent)]
    Reload(#[from] ReloadError),
}

#[derive(Debug)]
pub struct AdminServer<S: AdminState> {
    config: AdminConfig,
    state: Arc<S>,
}

impl<S: AdminState> AdminServer<S> {
    pub fn new(config: AdminConfig, state: Arc<S>) -> Result<Self, AdminConfigError> {
        config.validate()?;
        Ok(Self { config, state })
    }

    pub fn config(&self) -> &AdminConfig {
        &self.config
    }

    pub fn dispatch(&self, path: &str) -> Result<AdminResponse, AdminServerError> {
        if !self.config.enabled {
            return Err(AdminServerError::Disabled);
        }

        if path == self.config.health_path {
            return Ok(handle_health(self.state.as_ref()));
        }
        if path == self.config.metrics_path {
            return Ok(handle_metrics(self.state.as_ref()));
        }
        if path == self.config.diagnostics_path {
            return Ok(handle_diagnostics(self.state.as_ref()));
        }
        if let Some(reload_path) = &self.config.reload_path {
            if path == reload_path {
                if !self.config.allow_reload {
                    return Err(AdminServerError::ReloadDisabled);
                }
                return Ok(handle_reload(self.state.as_ref())?);
            }
        }

        Err(AdminServerError::UnknownPath {
            path: path.to_owned(),
        })
    }
}
