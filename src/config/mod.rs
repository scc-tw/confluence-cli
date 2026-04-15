use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::secret::{SecretKind, SecretStore};
use crate::support::{ConfluenceCliError, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum AuthKind {
    Basic,
    Bearer,
    Mtls,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum SecretBackend {
    Keyring,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Profile {
    pub id: Option<String>,
    pub domain: Option<String>,
    pub protocol: Option<String>,
    pub api_path: Option<String>,
    pub auth_type: Option<AuthKind>,
    pub email: Option<String>,
    pub username: Option<String>,
    pub api_token: Option<String>,
    pub password: Option<String>,
    pub read_only: Option<bool>,
    pub secret_backend: Option<SecretBackend>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ConfigFile {
    pub active_profile: Option<String>,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProfile {
    pub id: String,
    pub name: Option<String>,
    pub domain: String,
    pub protocol: String,
    pub api_path: String,
    pub auth_type: AuthKind,
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
    pub config: ConfigFile,
    pub resolved_profile: Option<ResolvedProfile>,
}

pub fn load_config(path: &Path) -> Result<ConfigFile> {
    if !path.exists() {
        return Ok(ConfigFile::default());
    }

    let content = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content)?)
}

pub fn save_config(path: &Path, config: &ConfigFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(config)?;
    fs::write(path, content)?;
    Ok(())
}

pub fn default_config_path() -> PathBuf {
    if let Some(user_profile) = env::var_os("USERPROFILE") {
        return PathBuf::from(user_profile)
            .join(".config")
            .join("confluence-cli")
            .join("config.json");
    }

    PathBuf::from("config.json")
}

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
        .unwrap_or_else(default_config_path);

    let mut config = load_config(&path)?;
    let profile_name = options
        .profile
        .clone()
        .or_else(|| env::var("CONFLUENCE_PROFILE").ok())
        .or_else(|| config.active_profile.clone());

    let resolved_profile = if let Some(name) = profile_name.as_ref() {
        let resolved = resolve_profile_with_store(&mut config, name, secret_store)?;
        save_config(&path, &config)?;
        Some(resolved)
    } else {
        None
    };

    Ok(RuntimeConfig {
        config,
        resolved_profile,
    })
}

pub fn init_config(path: &Path, profile_name: &str, profile: Profile) -> Result<ConfigFile> {
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

pub fn upsert_profile(
    path: &Path,
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

pub fn set_active_profile(path: &Path, profile_name: &str) -> Result<ConfigFile> {
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

pub fn remove_profile(path: &Path, profile_name: &str) -> Result<ConfigFile> {
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

pub fn resolve_profile(config: &ConfigFile, profile_name: &str) -> Result<ResolvedProfile> {
    let mut config = config.clone();
    resolve_profile_with_store(&mut config, profile_name, None)
}

pub fn resolve_profile_with_store(
    config: &mut ConfigFile,
    profile_name: &str,
    secret_store: Option<&dyn SecretStore>,
) -> Result<ResolvedProfile> {
    let profile = config
        .profiles
        .get(profile_name)
        .ok_or_else(|| ConfluenceCliError::Config(format!("profile '{profile_name}' not found")))?;

    let (resolved, migrated_profile) = resolve_from_profile(
        Some(profile_name.to_owned()),
        profile_name,
        profile.clone(),
        secret_store,
    )?;

    if let Some(profile) = migrated_profile {
        config.profiles.insert(profile_name.to_owned(), profile);
    }

    Ok(resolved)
}

fn resolve_from_profile(
    name: Option<String>,
    profile_name: &str,
    mut profile: Profile,
    secret_store: Option<&dyn SecretStore>,
) -> Result<(ResolvedProfile, Option<Profile>)> {
    let id_was_missing = profile.id.is_none();
    profile = ensure_profile_id(profile);
    let profile_id = profile
        .id
        .clone()
        .ok_or_else(|| ConfluenceCliError::Config("profile id missing".to_owned()))?;

    let domain = env::var("CONFLUENCE_DOMAIN")
        .ok()
        .or_else(|| profile.domain.clone())
        .ok_or_else(|| ConfluenceCliError::Config("missing Confluence domain".to_owned()))?;

    let protocol = env::var("CONFLUENCE_PROTOCOL")
        .ok()
        .or_else(|| profile.protocol.clone())
        .unwrap_or_else(|| "https".to_owned());

    let api_path = env::var("CONFLUENCE_API_PATH")
        .ok()
        .or_else(|| profile.api_path.clone())
        .unwrap_or_else(|| infer_api_path(&domain));

    let auth_type = env::var("CONFLUENCE_AUTH_TYPE")
        .ok()
        .map(|raw| parse_auth_kind(&raw))
        .transpose()?
        .or_else(|| profile.auth_type.clone())
        .unwrap_or(AuthKind::Basic);

    let email = env::var("CONFLUENCE_EMAIL")
        .ok()
        .or_else(|| profile.email.clone());

    let username = env::var("CONFLUENCE_USERNAME")
        .ok()
        .or_else(|| profile.username.clone());

    let (api_token, api_token_migrated) = resolve_secret(
        &profile_id,
        profile_name,
        profile.secret_backend.as_ref(),
        SecretKind::ApiToken,
        env::var("CONFLUENCE_API_TOKEN").ok(),
        profile.api_token.clone(),
        secret_store,
    )?;

    let (password, password_migrated) = resolve_secret(
        &profile_id,
        profile_name,
        profile.secret_backend.as_ref(),
        SecretKind::Password,
        env::var("CONFLUENCE_PASSWORD").ok(),
        profile.password.clone(),
        secret_store,
    )?;

    let read_only = env::var("CONFLUENCE_READ_ONLY")
        .ok()
        .map(|value| parse_bool(&value))
        .transpose()?
        .or(profile.read_only)
        .unwrap_or(false);

    validate_auth(
        &auth_type,
        email.as_deref(),
        username.as_deref(),
        api_token.as_deref(),
        password.as_deref(),
    )?;

    let migrated_profile = if id_was_missing || api_token_migrated || password_migrated {
        if api_token_migrated {
            profile.api_token = None;
        }
        if password_migrated {
            profile.password = None;
        }
        profile.secret_backend = Some(SecretBackend::Keyring);
        Some(profile.clone())
    } else {
        None
    };

    Ok((
        ResolvedProfile {
            id: profile_id,
            name,
            domain,
            protocol,
            api_path,
            auth_type,
            email,
            username,
            api_token,
            password,
            read_only,
            secret_backend: profile.secret_backend.clone(),
        },
        migrated_profile,
    ))
}

fn resolve_secret(
    profile_id: &str,
    profile_name: &str,
    backend: Option<&SecretBackend>,
    kind: SecretKind,
    env_value: Option<String>,
    legacy_value: Option<String>,
    secret_store: Option<&dyn SecretStore>,
) -> Result<(Option<String>, bool)> {
    if let Some(value) = env_value {
        return Ok((Some(value), false));
    }

    if matches!(backend, Some(SecretBackend::Keyring)) {
        let store = secret_store.ok_or_else(|| {
            ConfluenceCliError::Config(format!(
                "profile '{profile_name}' expects keyring-backed secrets, but no secret store is configured"
            ))
        })?;

        if let Some(value) = store.get(profile_id, kind)? {
            return Ok((Some(value), false));
        }
    }

    if let Some(value) = legacy_value {
        if let Some(store) = secret_store {
            store.set(profile_id, kind, &value)?;
            return Ok((Some(value), true));
        }

        return Ok((Some(value), false));
    }

    Ok((None, false))
}

fn ensure_profile_id(mut profile: Profile) -> Profile {
    if profile.id.is_none() {
        profile.id = Some(Uuid::new_v4().to_string());
    }
    profile
}

fn infer_api_path(domain: &str) -> String {
    if domain.ends_with(".atlassian.net") {
        "/wiki/rest/api".to_owned()
    } else {
        "/rest/api".to_owned()
    }
}

fn parse_auth_kind(value: &str) -> Result<AuthKind> {
    match value.to_ascii_lowercase().as_str() {
        "basic" => Ok(AuthKind::Basic),
        "bearer" => Ok(AuthKind::Bearer),
        "mtls" => Ok(AuthKind::Mtls),
        other => Err(ConfluenceCliError::Config(format!(
            "unsupported auth type '{other}'"
        ))),
    }
}

fn parse_bool(value: &str) -> Result<bool> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        other => Err(ConfluenceCliError::Config(format!(
            "unsupported boolean value '{other}'"
        ))),
    }
}

fn validate_auth(
    auth_type: &AuthKind,
    email: Option<&str>,
    username: Option<&str>,
    api_token: Option<&str>,
    password: Option<&str>,
) -> Result<()> {
    match auth_type {
        AuthKind::Basic => {
            let has_identity = email.is_some() || username.is_some();
            let has_secret = api_token.is_some() || password.is_some();
            if !has_identity || !has_secret {
                return Err(ConfluenceCliError::Config(
                    "basic auth requires email/username and api token/password".to_owned(),
                ));
            }
        }
        AuthKind::Bearer => {
            if api_token.is_none() {
                return Err(ConfluenceCliError::Config(
                    "bearer auth requires CONFLUENCE_API_TOKEN".to_owned(),
                ));
            }
        }
        AuthKind::Mtls => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secret::{MemorySecretStore, SecretKind, SecretStore};
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvGuard {
        original: Vec<(String, Option<String>)>,
    }

    impl EnvGuard {
        fn set(pairs: &[(&str, &str)]) -> Self {
            let mut original = Vec::new();
            for (key, value) in pairs {
                original.push(((*key).to_owned(), std::env::var(key).ok()));
                unsafe { std::env::set_var(key, value) };
            }

            Self { original }
        }

        fn clear(keys: &[&str]) -> Self {
            let mut original = Vec::new();
            for key in keys {
                original.push(((*key).to_owned(), std::env::var(key).ok()));
                unsafe { std::env::remove_var(key) };
            }

            Self { original }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in self.original.drain(..) {
                match value {
                    Some(value) => unsafe { std::env::set_var(&key, value) },
                    None => unsafe { std::env::remove_var(&key) },
                }
            }
        }
    }

    #[test]
    fn resolves_profile_from_file() {
        let _lock = env_lock().lock().expect("env lock should succeed");
        let _guard = EnvGuard::clear(&[
            "CONFLUENCE_DOMAIN",
            "CONFLUENCE_PROTOCOL",
            "CONFLUENCE_API_PATH",
            "CONFLUENCE_AUTH_TYPE",
            "CONFLUENCE_EMAIL",
            "CONFLUENCE_USERNAME",
            "CONFLUENCE_API_TOKEN",
            "CONFLUENCE_PASSWORD",
            "CONFLUENCE_READ_ONLY",
            "CONFLUENCE_PROFILE",
        ]);

        let dir = tempdir().expect("tempdir should be created");
        let path = dir.path().join("config.json");
        fs::write(
            &path,
            r#"{
                "active_profile": "work",
                "profiles": {
                    "work": {
                        "domain": "cycraft-corp.atlassian.net",
                        "auth_type": "basic",
                        "email": "oscar@example.com",
                        "api_token": "token-1",
                        "read_only": true
                    }
                }
            }"#,
        )
        .expect("config should be written");

        let runtime =
            load_runtime(&ResolveOptions::new(Some(path), None)).expect("runtime should load");
        let profile = runtime
            .resolved_profile
            .expect("resolved profile should exist");

        assert_eq!(profile.domain, "cycraft-corp.atlassian.net");
        assert_eq!(profile.api_path, "/wiki/rest/api");
        assert_eq!(profile.email.as_deref(), Some("oscar@example.com"));
        assert_eq!(profile.api_token.as_deref(), Some("token-1"));
        assert!(profile.read_only);
    }

    #[test]
    fn environment_overrides_profile_values() {
        let _lock = env_lock().lock().expect("env lock should succeed");
        let _clear = EnvGuard::clear(&[
            "CONFLUENCE_DOMAIN",
            "CONFLUENCE_API_TOKEN",
            "CONFLUENCE_EMAIL",
            "CONFLUENCE_READ_ONLY",
            "CONFLUENCE_PROFILE",
        ]);

        let dir = tempdir().expect("tempdir should be created");
        let path = dir.path().join("config.json");
        fs::write(
            &path,
            r#"{
                "active_profile": "work",
                "profiles": {
                    "work": {
                        "domain": "cycraft-corp.atlassian.net",
                        "auth_type": "basic",
                        "email": "from-file@example.com",
                        "api_token": "token-file",
                        "read_only": false
                    }
                }
            }"#,
        )
        .expect("config should be written");

        let _set = EnvGuard::set(&[
            ("CONFLUENCE_DOMAIN", "custom.example.internal"),
            ("CONFLUENCE_EMAIL", "from-env@example.com"),
            ("CONFLUENCE_API_TOKEN", "token-env"),
            ("CONFLUENCE_READ_ONLY", "true"),
        ]);

        let runtime =
            load_runtime(&ResolveOptions::new(Some(path), None)).expect("runtime should load");
        let profile = runtime
            .resolved_profile
            .expect("resolved profile should exist");

        assert_eq!(profile.domain, "custom.example.internal");
        assert_eq!(profile.api_path, "/rest/api");
        assert_eq!(profile.email.as_deref(), Some("from-env@example.com"));
        assert_eq!(profile.api_token.as_deref(), Some("token-env"));
        assert!(profile.read_only);
    }

    #[test]
    fn fails_on_missing_basic_auth_secret() {
        let _lock = env_lock().lock().expect("env lock should succeed");
        let profile = Profile {
            domain: Some("cycraft-corp.atlassian.net".to_owned()),
            auth_type: Some(AuthKind::Basic),
            email: Some("oscar@example.com".to_owned()),
            ..Profile::default()
        };

        let error = resolve_from_profile(Some("work".to_owned()), "work", profile, None)
            .expect_err("basic auth without token should fail");

        assert!(matches!(error, ConfluenceCliError::Config(_)));
    }

    #[test]
    fn init_and_upsert_profile_persist_to_disk() {
        let _lock = env_lock().lock().expect("env lock should succeed");
        let dir = tempdir().expect("tempdir should be created");
        let path = dir.path().join("config.json");

        init_config(
            &path,
            "default",
            Profile {
                domain: Some("example.atlassian.net".to_owned()),
                auth_type: Some(AuthKind::Bearer),
                ..Profile::default()
            },
        )
        .expect("config should initialize");

        upsert_profile(
            &path,
            "work",
            Profile {
                domain: Some("work.atlassian.net".to_owned()),
                auth_type: Some(AuthKind::Basic),
                email: Some("oscar@example.com".to_owned()),
                api_token: Some("token".to_owned()),
                ..Profile::default()
            },
            true,
        )
        .expect("profile should upsert");

        let config = load_config(&path).expect("config should reload");
        assert_eq!(config.active_profile.as_deref(), Some("work"));
        assert!(config.profiles.contains_key("default"));
        assert!(config.profiles.contains_key("work"));
    }

    #[test]
    fn set_active_and_remove_profile_update_active_profile() {
        let _lock = env_lock().lock().expect("env lock should succeed");
        let dir = tempdir().expect("tempdir should be created");
        let path = dir.path().join("config.json");

        let mut config = ConfigFile {
            active_profile: Some("default".to_owned()),
            ..ConfigFile::default()
        };
        config.profiles.insert(
            "default".to_owned(),
            Profile {
                domain: Some("example.atlassian.net".to_owned()),
                auth_type: Some(AuthKind::Bearer),
                ..Profile::default()
            },
        );
        config.profiles.insert(
            "work".to_owned(),
            Profile {
                domain: Some("work.atlassian.net".to_owned()),
                auth_type: Some(AuthKind::Bearer),
                ..Profile::default()
            },
        );
        save_config(&path, &config).expect("config should save");

        let updated = set_active_profile(&path, "work").expect("active profile should change");
        assert_eq!(updated.active_profile.as_deref(), Some("work"));

        let updated = remove_profile(&path, "work").expect("profile should be removed");
        assert_eq!(updated.active_profile.as_deref(), Some("default"));
        assert!(!updated.profiles.contains_key("work"));
    }

    #[test]
    fn load_runtime_migrates_plaintext_secret_into_store() {
        let _lock = env_lock().lock().expect("env lock should succeed");
        let _clear = EnvGuard::clear(&[
            "CONFLUENCE_DOMAIN",
            "CONFLUENCE_PROTOCOL",
            "CONFLUENCE_API_PATH",
            "CONFLUENCE_AUTH_TYPE",
            "CONFLUENCE_EMAIL",
            "CONFLUENCE_USERNAME",
            "CONFLUENCE_API_TOKEN",
            "CONFLUENCE_PASSWORD",
            "CONFLUENCE_READ_ONLY",
            "CONFLUENCE_PROFILE",
        ]);

        let dir = tempdir().expect("tempdir should be created");
        let path = dir.path().join("config.json");
        fs::write(
            &path,
            r#"{
                "active_profile": "work",
                "profiles": {
                    "work": {
                        "domain": "cycraft-corp.atlassian.net",
                        "auth_type": "bearer",
                        "api_token": "token-file"
                    }
                }
            }"#,
        )
        .expect("config should be written");

        let store = MemorySecretStore::new();
        let runtime =
            load_runtime_with_store(&ResolveOptions::new(Some(path.clone()), None), Some(&store))
                .expect("runtime should load");
        let profile = runtime
            .resolved_profile
            .expect("resolved profile should exist");

        assert_eq!(profile.api_token.as_deref(), Some("token-file"));
        let persisted = load_config(&path).expect("config should reload");
        let profile_config = persisted
            .profiles
            .get("work")
            .expect("profile should exist");
        assert!(profile_config.id.is_some());
        assert_eq!(profile_config.secret_backend, Some(SecretBackend::Keyring));
        assert!(profile_config.api_token.is_none());
        let profile_id = profile_config
            .id
            .as_deref()
            .expect("profile id should exist");
        assert_eq!(
            store
                .get(profile_id, SecretKind::ApiToken)
                .expect("store read should succeed")
                .as_deref(),
            Some("token-file")
        );
    }

    #[test]
    fn env_secret_overrides_keyring_and_legacy_plaintext() {
        let _lock = env_lock().lock().expect("env lock should succeed");
        let _clear = EnvGuard::clear(&[
            "CONFLUENCE_DOMAIN",
            "CONFLUENCE_PROTOCOL",
            "CONFLUENCE_API_PATH",
            "CONFLUENCE_AUTH_TYPE",
            "CONFLUENCE_EMAIL",
            "CONFLUENCE_USERNAME",
            "CONFLUENCE_API_TOKEN",
            "CONFLUENCE_PASSWORD",
            "CONFLUENCE_READ_ONLY",
            "CONFLUENCE_PROFILE",
        ]);
        let _set = EnvGuard::set(&[("CONFLUENCE_API_TOKEN", "token-env")]);

        let mut config = ConfigFile {
            active_profile: Some("work".to_owned()),
            ..ConfigFile::default()
        };
        config.profiles.insert(
            "work".to_owned(),
            Profile {
                id: Some("profile-1".to_owned()),
                domain: Some("cycraft-corp.atlassian.net".to_owned()),
                auth_type: Some(AuthKind::Bearer),
                api_token: Some("token-file".to_owned()),
                secret_backend: Some(SecretBackend::Keyring),
                ..Profile::default()
            },
        );

        let store = MemorySecretStore::new();
        store
            .set("profile-1", SecretKind::ApiToken, "token-store")
            .expect("store write should succeed");
        let profile = resolve_profile_with_store(&mut config, "work", Some(&store))
            .expect("profile should resolve");
        assert_eq!(profile.api_token.as_deref(), Some("token-env"));
    }
}
