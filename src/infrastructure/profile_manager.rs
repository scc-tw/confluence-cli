use crate::application::ports::ProfilesStore;
use crate::application::profiles::{ProfileDraft, ProfileSecrets};
use crate::application::runtime::{RuntimeConfig, RuntimeProfiles};
use crate::config::{ConfigFile, ConfigSecretBackend, Profile, load_config, save_config};
use crate::secret::{SecretKind, SecretStore};
use crate::support::{ConfluenceCliError, Result};

pub fn init_profile_config(
    path: &std::path::Path,
    name: &str,
    draft: ProfileDraft,
    secrets: &ProfileSecrets,
    store: &dyn SecretStore,
) -> Result<ConfigFile> {
    let existing = load_config(path)?;
    if !existing.profiles.is_empty() {
        return Err(ConfluenceCliError::Config(
            "config already exists; use `confluence login` to update the active profile, `confluence profile add <name> --domain <domain> ...` to add or update a profile, or `confluence profile use <name>` to switch profiles".to_owned(),
        ));
    }

    let profile = ensure_profile_id(into_profile(draft));

    write_profile_secrets(store, &profile, secrets)?;
    init_config(path, name, profile)
}

pub fn add_or_update_profile(
    path: &std::path::Path,
    name: &str,
    mut draft: ProfileDraft,
    secrets: &ProfileSecrets,
    activate: bool,
    store: &dyn SecretStore,
) -> Result<ConfigFile> {
    let existing = load_config(path)?;
    let existing_profile = existing.profiles.get(name).cloned();
    if draft.id.is_none() {
        draft.id = existing_profile
            .as_ref()
            .and_then(|existing| existing.id.clone());
    }
    if !draft.has_secrets
        && existing_profile
            .as_ref()
            .and_then(|existing| existing.secret_backend.as_ref())
            .is_some()
    {
        draft.has_secrets = true;
    }
    if draft.auth_type.is_none() {
        draft.auth_type = existing_profile
            .as_ref()
            .and_then(|existing| existing.auth_type.clone());
    }
    if draft.email.is_none() {
        draft.email = existing_profile
            .as_ref()
            .and_then(|existing| existing.email.clone());
    }
    if draft.username.is_none() {
        draft.username = existing_profile
            .as_ref()
            .and_then(|existing| existing.username.clone());
    }
    if draft.protocol.is_none() {
        draft.protocol = existing_profile
            .as_ref()
            .and_then(|existing| existing.protocol.clone());
    }
    if draft.api_path.is_none() {
        draft.api_path = existing_profile
            .as_ref()
            .and_then(|existing| existing.api_path.clone());
    }
    if draft.read_only.is_none() {
        draft.read_only = existing_profile
            .as_ref()
            .and_then(|existing| existing.read_only);
    }
    let profile = ensure_profile_id(into_profile(draft));

    let profile = if profile.secret_backend.is_none() {
        if let Some(existing) = existing_profile.as_ref() {
            Profile {
                api_token: existing.api_token.clone(),
                password: existing.password.clone(),
                ..profile
            }
        } else {
            profile
        }
    } else {
        profile
    };

    write_profile_secrets(store, &profile, secrets)?;
    upsert_profile(path, name, profile, activate)
}

pub fn remove_profile_with_secrets(
    path: &std::path::Path,
    name: &str,
    store: &dyn SecretStore,
) -> Result<ConfigFile> {
    if let Some(profile) = load_config(path)?.profiles.get(name).cloned()
        && profile.secret_backend.is_some()
    {
        let profile_id = profile.id.as_deref().unwrap_or(name);
        store.delete(profile_id, SecretKind::ApiToken)?;
        store.delete(profile_id, SecretKind::Password)?;
    }

    remove_profile(path, name)
}

pub fn use_profile(path: &std::path::Path, name: &str) -> Result<ConfigFile> {
    set_active_profile(path, name)
}

pub fn list_profiles(path: &std::path::Path) -> Result<RuntimeConfig> {
    let config = load_config(path)?;
    Ok(runtime_profiles(config))
}

pub struct ProfileManager<'a> {
    path: std::path::PathBuf,
    store: &'a dyn SecretStore,
}

impl<'a> ProfileManager<'a> {
    pub fn new(path: std::path::PathBuf, store: &'a dyn SecretStore) -> Self {
        Self { path, store }
    }
}

impl ProfilesStore for ProfileManager<'_> {
    fn init_profile(
        &self,
        name: &str,
        draft: ProfileDraft,
        secrets: &ProfileSecrets,
    ) -> Result<RuntimeConfig> {
        let config = init_profile_config(&self.path, name, draft, secrets, self.store)?;
        Ok(runtime_profiles(config))
    }

    fn add_or_update_profile(
        &self,
        name: &str,
        draft: ProfileDraft,
        secrets: &ProfileSecrets,
        activate: bool,
    ) -> Result<RuntimeConfig> {
        let config = add_or_update_profile(&self.path, name, draft, secrets, activate, self.store)?;
        Ok(runtime_profiles(config))
    }

    fn use_profile(&self, name: &str) -> Result<RuntimeConfig> {
        let config = use_profile(&self.path, name)?;
        Ok(runtime_profiles(config))
    }

    fn remove_profile(&self, name: &str) -> Result<RuntimeConfig> {
        let config = remove_profile_with_secrets(&self.path, name, self.store)?;
        Ok(runtime_profiles(config))
    }

    fn list_profiles(&self) -> Result<RuntimeConfig> {
        list_profiles(&self.path)
    }
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

fn into_profile(draft: ProfileDraft) -> Profile {
    Profile {
        id: draft.id,
        domain: Some(draft.domain),
        protocol: draft.protocol,
        api_path: draft.api_path,
        auth_type: draft.auth_type,
        email: draft.email,
        username: draft.username,
        api_token: None,
        password: None,
        read_only: draft.read_only,
        secret_backend: if draft.has_secrets {
            Some(ConfigSecretBackend::Keyring)
        } else {
            None
        },
    }
}

fn ensure_profile_id(mut profile: Profile) -> Profile {
    if profile.id.is_none() {
        profile.id = Some(uuid::Uuid::new_v4().to_string());
    }
    profile
}

pub(crate) fn init_config(
    path: &std::path::Path,
    profile_name: &str,
    profile: Profile,
) -> Result<ConfigFile> {
    let mut config = ConfigFile {
        active_profile: Some(profile_name.to_owned()),
        ..ConfigFile::default()
    };
    config
        .profiles
        .insert(profile_name.to_owned(), ensure_profile_id(profile));
    save_config(path, &config)?;
    Ok(config)
}

pub(crate) fn upsert_profile(
    path: &std::path::Path,
    profile_name: &str,
    profile: Profile,
    set_active: bool,
) -> Result<ConfigFile> {
    let mut config = load_config(path)?;
    config
        .profiles
        .insert(profile_name.to_owned(), ensure_profile_id(profile));
    if set_active {
        config.active_profile = Some(profile_name.to_owned());
    }
    save_config(path, &config)?;
    Ok(config)
}

pub(crate) fn set_active_profile(path: &std::path::Path, profile_name: &str) -> Result<ConfigFile> {
    let mut config = load_config(path)?;
    if !config.profiles.contains_key(profile_name) {
        return Err(ConfluenceCliError::Config(format!(
            "profile '{profile_name}' not found"
        )));
    }

    config.active_profile = Some(profile_name.to_owned());
    save_config(path, &config)?;
    Ok(config)
}

pub(crate) fn remove_profile(path: &std::path::Path, profile_name: &str) -> Result<ConfigFile> {
    let mut config = load_config(path)?;
    if config.profiles.remove(profile_name).is_none() {
        return Err(ConfluenceCliError::Config(format!(
            "profile '{profile_name}' not found"
        )));
    }

    if config.active_profile.as_deref() == Some(profile_name) {
        config.active_profile = config.profiles.keys().next().cloned();
    }

    save_config(path, &config)?;
    Ok(config)
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
