use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::profile::AuthKind;
use crate::support::Result;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigSecretBackend {
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
    pub secret_backend: Option<ConfigSecretBackend>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ConfigFile {
    pub active_profile: Option<String>,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConfluenceCliError;
    use crate::application::runtime::ResolveOptions;
    use crate::infrastructure::profile_manager::{
        init_config, remove_profile, set_active_profile, upsert_profile,
    };
    use crate::infrastructure::runtime_loader::{
        load_runtime, load_runtime_with_store, resolve_profile_state, resolve_profile_with_store,
    };
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
                        "domain": "workspace.example.atlassian.net",
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

        assert_eq!(profile.domain, "workspace.example.atlassian.net");
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
                        "domain": "workspace.example.atlassian.net",
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
            domain: Some("workspace.example.atlassian.net".to_owned()),
            auth_type: Some(AuthKind::Basic),
            email: Some("oscar@example.com".to_owned()),
            ..Profile::default()
        };

        let error = resolve_profile_state(Some("work".to_owned()), "work", profile, None)
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
                        "domain": "workspace.example.atlassian.net",
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
        assert_eq!(
            profile_config.secret_backend,
            Some(ConfigSecretBackend::Keyring)
        );
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
                domain: Some("workspace.example.atlassian.net".to_owned()),
                auth_type: Some(AuthKind::Bearer),
                api_token: Some("token-file".to_owned()),
                secret_backend: Some(ConfigSecretBackend::Keyring),
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
