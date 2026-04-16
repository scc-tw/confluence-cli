use crate::application::runtime::{ResolveOptions, RuntimeConfig, RuntimeContext, RuntimeProfiles};
use crate::secret::SecretStore;
use crate::support::Result;

use super::resolve::{RuntimeConfigState, resolve_runtime_state};

pub fn load_runtime_context_with_store(
    options: &ResolveOptions,
    secret_store: &dyn SecretStore,
) -> Result<RuntimeContext> {
    Ok(RuntimeContext {
        runtime_config: load_runtime_with_store(options, Some(secret_store))?,
    })
}

#[cfg(test)]
pub fn load_runtime(options: &ResolveOptions) -> Result<RuntimeConfig> {
    load_runtime_with_store(options, None)
}

pub fn load_runtime_with_store(
    options: &ResolveOptions,
    secret_store: Option<&dyn SecretStore>,
) -> Result<RuntimeConfig> {
    let path = options
        .config_path
        .clone()
        .unwrap_or_else(crate::config::default_config_path);

    let config = crate::config::load_config(&path)?;
    let state = resolve_runtime_state(path.clone(), config, options, secret_store)?;
    if let Some(migration) = &state.migration
        && !migration.is_empty()
    {
        crate::config::save_config(&path, &state.config)?;
    }

    Ok(into_runtime_config(state))
}

fn into_runtime_config(state: RuntimeConfigState) -> RuntimeConfig {
    RuntimeConfig {
        profiles: RuntimeProfiles {
            active_profile: state.config.active_profile,
            profiles: state.config.profiles.keys().cloned().collect(),
        },
        resolved_profile: state.resolved_profile,
    }
}
