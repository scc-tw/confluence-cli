use crate::application::runtime::{ResolveOptions, ResolvedProfile};
use crate::config::{ConfigFile, ConfigSecretBackend, Profile};
use crate::profile::AuthKind;
use crate::secret::SecretKind;
use crate::secret::SecretStore;
use crate::support::{ConfluenceCliError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfigState {
    pub config_path: std::path::PathBuf,
    pub config: ConfigFile,
    pub resolved_profile: Option<ResolvedProfile>,
    pub migration: Option<ConfigMigration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConfigMigration {
    pub updated_profiles: Vec<(String, Profile)>,
}

impl ConfigMigration {
    pub fn is_empty(&self) -> bool {
        self.updated_profiles.is_empty()
    }
}

pub fn resolve_runtime_state(
    config_path: std::path::PathBuf,
    mut config: ConfigFile,
    options: &ResolveOptions,
    secret_store: Option<&dyn SecretStore>,
) -> Result<RuntimeConfigState> {
    let profile_name = options
        .profile
        .clone()
        .or_else(|| std::env::var("CONFLUENCE_PROFILE").ok())
        .or_else(|| config.active_profile.clone());

    let mut migration = ConfigMigration::default();
    let resolved_profile = if let Some(name) = profile_name.as_ref() {
        let profile = config
            .profiles
            .get(name)
            .cloned()
            .ok_or_else(|| ConfluenceCliError::Config(format!("profile '{name}' not found")))?;

        let (resolved, migrated) =
            resolve_profile_state(Some(name.to_owned()), name, profile, secret_store)?;
        if let Some(profile) = migrated {
            config.profiles.insert(name.to_owned(), profile.clone());
            migration.updated_profiles.push((name.to_owned(), profile));
        }
        Some(resolved)
    } else {
        None
    };

    Ok(RuntimeConfigState {
        config_path,
        config,
        resolved_profile,
        migration: (!migration.is_empty()).then_some(migration),
    })
}

#[cfg(test)]
pub fn resolve_profile_with_store(
    config: &mut ConfigFile,
    profile_name: &str,
    secret_store: Option<&dyn SecretStore>,
) -> Result<ResolvedProfile> {
    let profile =
        config.profiles.get(profile_name).cloned().ok_or_else(|| {
            ConfluenceCliError::Config(format!("profile '{profile_name}' not found"))
        })?;

    let (resolved, migrated_profile) = resolve_profile_state(
        Some(profile_name.to_owned()),
        profile_name,
        profile,
        secret_store,
    )?;

    if let Some(profile) = migrated_profile {
        config.profiles.insert(profile_name.to_owned(), profile);
    }

    Ok(resolved)
}

pub fn resolve_profile_state(
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

    let domain = std::env::var("CONFLUENCE_DOMAIN")
        .ok()
        .or_else(|| profile.domain.clone())
        .ok_or_else(|| ConfluenceCliError::Config("missing Confluence domain".to_owned()))?;

    let protocol = std::env::var("CONFLUENCE_PROTOCOL")
        .ok()
        .or_else(|| profile.protocol.clone())
        .unwrap_or_else(|| "https".to_owned());

    let api_path = std::env::var("CONFLUENCE_API_PATH")
        .ok()
        .or_else(|| profile.api_path.clone())
        .unwrap_or_else(|| infer_api_path(&domain));

    let auth_type = std::env::var("CONFLUENCE_AUTH_TYPE")
        .ok()
        .map(|raw| parse_auth_kind(&raw))
        .transpose()?
        .or_else(|| profile.auth_type.clone())
        .unwrap_or(AuthKind::Basic);

    let email = std::env::var("CONFLUENCE_EMAIL")
        .ok()
        .or_else(|| profile.email.clone());
    let username = std::env::var("CONFLUENCE_USERNAME")
        .ok()
        .or_else(|| profile.username.clone());

    let (api_token, api_token_migrated) = resolve_secret(
        &profile_id,
        profile_name,
        profile.secret_backend.as_ref(),
        SecretKind::ApiToken,
        std::env::var("CONFLUENCE_API_TOKEN").ok(),
        profile.api_token.clone(),
        secret_store,
    )?;

    let (password, password_migrated) = resolve_secret(
        &profile_id,
        profile_name,
        profile.secret_backend.as_ref(),
        SecretKind::Password,
        std::env::var("CONFLUENCE_PASSWORD").ok(),
        profile.password.clone(),
        secret_store,
    )?;

    let read_only = std::env::var("CONFLUENCE_READ_ONLY")
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
        if api_token_migrated || password_migrated {
            profile.secret_backend = Some(ConfigSecretBackend::Keyring);
        }
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
        },
        migrated_profile,
    ))
}

pub fn infer_api_path(domain: &str) -> String {
    if domain.ends_with(".atlassian.net") {
        "/wiki/rest/api".to_owned()
    } else {
        "/rest/api".to_owned()
    }
}

pub fn parse_auth_kind(value: &str) -> Result<AuthKind> {
    match value.to_ascii_lowercase().as_str() {
        "basic" => Ok(AuthKind::Basic),
        "bearer" => Ok(AuthKind::Bearer),
        "mtls" => Ok(AuthKind::Mtls),
        other => Err(ConfluenceCliError::Config(format!(
            "unsupported auth type '{other}'"
        ))),
    }
}

pub fn parse_bool(value: &str) -> Result<bool> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        other => Err(ConfluenceCliError::Config(format!(
            "unsupported boolean value '{other}'"
        ))),
    }
}

pub fn validate_auth(
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

fn resolve_secret(
    profile_id: &str,
    profile_name: &str,
    backend: Option<&ConfigSecretBackend>,
    kind: SecretKind,
    env_value: Option<String>,
    legacy_value: Option<String>,
    secret_store: Option<&dyn SecretStore>,
) -> Result<(Option<String>, bool)> {
    if let Some(value) = env_value {
        return Ok((Some(value), false));
    }

    if matches!(backend, Some(ConfigSecretBackend::Keyring)) {
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
        profile.id = Some(uuid::Uuid::new_v4().to_string());
    }
    profile
}
