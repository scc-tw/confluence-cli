use crate::application::runtime::{RuntimeConfig, RuntimeProfiles};
use crate::config::{
    ConfigFile, ConfigSecretBackend, Profile, init_config, load_config, remove_profile,
    set_active_profile, upsert_profile,
};
use crate::secret::{KeyringSecretStore, SecretKind, SecretStore};
use crate::support::{ConfluenceCliError, Result};

#[derive(Debug, Clone, Default)]
pub struct ProfileSecrets {
    pub api_token: Option<String>,
    pub password: Option<String>,
}

pub fn init_profile_config(
    path: &std::path::Path,
    name: &str,
    profile: Profile,
    secrets: &ProfileSecrets,
) -> Result<ConfigFile> {
    let existing = load_config(path)?;
    if !existing.profiles.is_empty() {
        return Err(ConfluenceCliError::Config(
            "config already exists; use profile add or profile use instead".to_owned(),
        ));
    }

    let store = KeyringSecretStore;
    write_profile_secrets(&store, &profile, secrets)?;
    init_config(path, name, profile)
}

pub fn add_or_update_profile(
    path: &std::path::Path,
    name: &str,
    mut profile: Profile,
    secrets: &ProfileSecrets,
    activate: bool,
) -> Result<ConfigFile> {
    let existing = load_config(path)?;
    if profile.id.is_none() {
        profile.id = existing
            .profiles
            .get(name)
            .and_then(|existing| existing.id.clone());
    }

    let store = KeyringSecretStore;
    write_profile_secrets(&store, &profile, secrets)?;
    upsert_profile(path, name, profile, activate)
}

pub fn remove_profile_with_secrets(path: &std::path::Path, name: &str) -> Result<ConfigFile> {
    if let Some(profile) = load_config(path)?.profiles.get(name).cloned()
        && profile.secret_backend.is_some()
    {
        let store = KeyringSecretStore;
        let profile_id = profile.id.as_deref().unwrap_or(name);
        store.delete(profile_id, SecretKind::ApiToken)?;
        store.delete(profile_id, SecretKind::Password)?;
    }

    remove_profile(path, name)
}

pub fn use_profile(path: &std::path::Path, name: &str) -> Result<ConfigFile> {
    set_active_profile(path, name)
}

pub fn runtime_profiles(config: ConfigFile) -> RuntimeConfig {
    RuntimeConfig {
        profiles: RuntimeProfiles {
            active_profile: config.active_profile,
            profiles: config.profiles.keys().cloned().collect(),
        },
        resolved_profile: None,
    }
}

pub fn attach_secret_backend(profile: &mut Profile, has_secret: bool) {
    profile.secret_backend = if has_secret {
        Some(ConfigSecretBackend::Keyring)
    } else {
        None
    };
}

fn write_profile_secrets(
    store: &dyn SecretStore,
    profile: &Profile,
    secrets: &ProfileSecrets,
) -> Result<()> {
    let profile_id = profile
        .id
        .as_deref()
        .ok_or_else(|| ConfluenceCliError::Config("profile id missing".to_owned()))?;

    if let Some(api_token) = secrets.api_token.as_deref() {
        store.set(profile_id, SecretKind::ApiToken, api_token)?;
    }
    if let Some(password) = secrets.password.as_deref() {
        store.set(profile_id, SecretKind::Password, password)?;
    }

    Ok(())
}
