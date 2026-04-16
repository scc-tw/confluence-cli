use std::path::PathBuf;

use crate::secret::SecretBackend;
use crate::support::{ConfluenceCliError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfigState {
    pub config_path: PathBuf,
    pub config: crate::config::ConfigFile,
    pub resolved_profile: Option<ResolvedProfile>,
    pub migration: Option<ConfigMigration>,
}

#[derive(Debug, Clone)]
pub struct RuntimeContext {
    pub runtime_config: RuntimeConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProfile {
    pub id: String,
    pub name: Option<String>,
    pub domain: String,
    pub protocol: String,
    pub api_path: String,
    pub auth_type: crate::profile::AuthKind,
    pub email: Option<String>,
    pub username: Option<String>,
    pub api_token: Option<String>,
    pub password: Option<String>,
    pub read_only: bool,
    pub secret_backend: Option<SecretBackend>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveOptions {
    pub config_path: Option<PathBuf>,
    pub profile: Option<String>,
}

impl ResolveOptions {
    pub fn new(config_path: Option<PathBuf>, profile: Option<String>) -> Self {
        Self {
            config_path,
            profile,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub profiles: RuntimeProfiles,
    pub resolved_profile: Option<ResolvedProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProfiles {
    pub active_profile: Option<String>,
    pub profiles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConfigMigration {
    pub updated_profiles: Vec<(String, crate::config::Profile)>,
}

impl ConfigMigration {
    pub fn is_empty(&self) -> bool {
        self.updated_profiles.is_empty()
    }
}

pub fn into_runtime_config(state: RuntimeConfigState) -> RuntimeConfig {
    RuntimeConfig {
        profiles: RuntimeProfiles {
            active_profile: state.config.active_profile,
            profiles: state.config.profiles.keys().cloned().collect(),
        },
        resolved_profile: state.resolved_profile,
    }
}

pub fn ensure_writable(runtime: &RuntimeContext) -> Result<()> {
    if runtime
        .runtime_config
        .resolved_profile
        .as_ref()
        .is_some_and(|profile| profile.read_only)
    {
        return Err(ConfluenceCliError::Config(
            "active profile is read-only; this command would mutate Confluence".to_owned(),
        ));
    }

    Ok(())
}
