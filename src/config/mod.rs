use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::support::{ConfluenceCliError, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthKind {
    Basic,
    Bearer,
    Mtls,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Profile {
    pub domain: Option<String>,
    pub protocol: Option<String>,
    pub api_path: Option<String>,
    pub auth_type: Option<AuthKind>,
    pub email: Option<String>,
    pub username: Option<String>,
    pub api_token: Option<String>,
    pub password: Option<String>,
    pub read_only: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ConfigFile {
    pub active_profile: Option<String>,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProfile {
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
    let path = options
        .config_path
        .clone()
        .unwrap_or_else(default_config_path);

    let config = load_config(&path)?;
    let profile_name = options
        .profile
        .clone()
        .or_else(|| env::var("CONFLUENCE_PROFILE").ok())
        .or_else(|| config.active_profile.clone());

    let resolved_profile = profile_name
        .as_ref()
        .map(|name| resolve_profile(&config, name))
        .transpose()?;

    Ok(RuntimeConfig {
        config,
        resolved_profile,
    })
}

pub fn resolve_profile(config: &ConfigFile, profile_name: &str) -> Result<ResolvedProfile> {
    let profile = config
        .profiles
        .get(profile_name)
        .ok_or_else(|| ConfluenceCliError::Config(format!("profile '{profile_name}' not found")))?;

    resolve_from_profile(Some(profile_name.to_owned()), profile)
}

fn resolve_from_profile(name: Option<String>, profile: &Profile) -> Result<ResolvedProfile> {
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

    let api_token = env::var("CONFLUENCE_API_TOKEN")
        .ok()
        .or_else(|| profile.api_token.clone());

    let password = env::var("CONFLUENCE_PASSWORD")
        .ok()
        .or_else(|| profile.password.clone());

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

    Ok(ResolvedProfile {
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
    })
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

        let error = resolve_from_profile(Some("work".to_owned()), &profile)
            .expect_err("basic auth without token should fail");

        assert!(matches!(error, ConfluenceCliError::Config(_)));
    }
}
