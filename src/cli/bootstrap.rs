use std::path::PathBuf;

use uuid::Uuid;

use crate::api::{HttpApiConfig, HttpConfluenceApi};
use crate::application::runtime::{
    ResolveOptions, ResolvedProfile, RuntimeConfig, RuntimeContext, RuntimeProfiles,
};
use crate::cli::args::CliAuthKind;
use crate::config::{
    ConfigFile, ConfigSecretBackend, Profile, default_config_path, init_config, load_config,
    remove_profile, set_active_profile, upsert_profile,
};
use crate::infrastructure::runtime_loader;
use crate::secret::{KeyringSecretStore, SecretKind, SecretStore};
use crate::support::{ConfluenceCliError, Result};

use super::output::{print_profiles_human, print_profiles_json};
use super::{GlobalArgs, OutputFormat, ProfileArgs};

#[derive(Debug, Clone, Default)]
pub(super) struct ProfileSecrets {
    api_token: Option<String>,
    password: Option<String>,
}

pub(super) fn config_init(
    global: &GlobalArgs,
    name: &str,
    profile_args: ProfileArgs,
) -> Result<()> {
    let path = config_path(global);
    let existing = load_config(&path)?;
    if !existing.profiles.is_empty() {
        return Err(ConfluenceCliError::Config(
            "config already exists; use profile add or profile use instead".to_owned(),
        ));
    }

    let store = KeyringSecretStore;
    let (profile, secrets) = profile_from_args(profile_args, None)?;
    write_profile_secrets(&store, &profile, &secrets)?;
    let config = init_config(&path, name, profile)?;
    print_profiles(global.output, config)?;
    Ok(())
}

pub(super) fn profile_add(
    global: &GlobalArgs,
    name: &str,
    profile_args: ProfileArgs,
    activate: bool,
) -> Result<()> {
    let path = config_path(global);
    let existing = load_config(&path)?;
    let store = KeyringSecretStore;
    let existing_id = existing
        .profiles
        .get(name)
        .and_then(|profile| profile.id.clone());
    let (profile, secrets) = profile_from_args(profile_args, existing_id)?;
    write_profile_secrets(&store, &profile, &secrets)?;
    let config = upsert_profile(&path, name, profile, activate)?;
    print_profiles(global.output, config)?;
    Ok(())
}

pub(super) fn profile_use(global: &GlobalArgs, name: &str) -> Result<()> {
    let path = config_path(global);
    let config = set_active_profile(&path, name)?;
    print_profiles(global.output, config)?;
    Ok(())
}

pub(super) fn profile_remove(global: &GlobalArgs, name: &str) -> Result<()> {
    let path = config_path(global);
    if let Some(profile) = load_config(&path)?.profiles.get(name).cloned()
        && profile.secret_backend.is_some()
    {
        let store = KeyringSecretStore;
        let profile_id = profile.id.as_deref().unwrap_or(name);
        store.delete(profile_id, SecretKind::ApiToken)?;
        store.delete(profile_id, SecretKind::Password)?;
    }
    let config = remove_profile(&path, name)?;
    print_profiles(global.output, config)?;
    Ok(())
}

pub(super) fn profile_list(global: &GlobalArgs) -> Result<()> {
    let path = config_path(global);
    let config = load_config(&path)?;
    let runtime = RuntimeConfig {
        profiles: RuntimeProfiles {
            active_profile: config.active_profile,
            profiles: config.profiles.keys().cloned().collect(),
        },
        resolved_profile: None,
    };
    match global.output {
        OutputFormat::Human => print_profiles_human(&runtime),
        OutputFormat::Json => super::output::print_profiles_json(&runtime)?,
    }
    Ok(())
}

pub(super) fn load_runtime_context(global: &GlobalArgs) -> Result<RuntimeContext> {
    let options = ResolveOptions::new(global.config_path.clone(), global.profile.clone());
    runtime_loader::load_runtime_context(&options)
}

pub(super) fn load_runtime_and_api(
    global: &GlobalArgs,
) -> Result<(RuntimeContext, HttpConfluenceApi)> {
    let runtime = load_runtime_context(global)?;
    let profile = runtime
        .runtime_config
        .resolved_profile
        .clone()
        .ok_or_else(|| ConfluenceCliError::Config("no active or selected profile".to_owned()))?;
    Ok((runtime, HttpConfluenceApi::new(http_api_config(profile))?))
}

fn http_api_config(profile: ResolvedProfile) -> HttpApiConfig {
    HttpApiConfig {
        domain: profile.domain,
        protocol: profile.protocol,
        api_path: profile.api_path,
        auth_type: profile.auth_type,
        email: profile.email,
        username: profile.username,
        api_token: profile.api_token,
        password: profile.password,
    }
}

fn profile_from_args(
    args: ProfileArgs,
    existing_id: Option<String>,
) -> Result<(Profile, ProfileSecrets)> {
    let domain = args.domain.ok_or_else(|| {
        ConfluenceCliError::Config("profile configuration requires --domain".to_owned())
    })?;

    let auth_type = args.auth_type.or_else(|| {
        if args.email.is_some() {
            Some(CliAuthKind::Basic)
        } else if args.api_token.is_some() {
            Some(CliAuthKind::Bearer)
        } else {
            None
        }
    });

    let secrets = ProfileSecrets {
        api_token: args.api_token,
        password: args.password,
    };

    Ok((
        Profile {
            id: Some(existing_id.unwrap_or_else(|| Uuid::new_v4().to_string())),
            domain: Some(domain),
            protocol: args.protocol,
            api_path: args.api_path,
            auth_type: auth_type.map(Into::into),
            email: args.email,
            username: args.username,
            api_token: None,
            password: None,
            read_only: args.read_only.then_some(true),
            secret_backend: if secrets.api_token.is_some() || secrets.password.is_some() {
                Some(ConfigSecretBackend::Keyring)
            } else {
                None
            },
        },
        secrets,
    ))
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

fn config_path(global: &GlobalArgs) -> PathBuf {
    global
        .config_path
        .clone()
        .unwrap_or_else(default_config_path)
}

fn print_profiles(output: OutputFormat, config: ConfigFile) -> Result<()> {
    let runtime = RuntimeConfig {
        profiles: RuntimeProfiles {
            active_profile: config.active_profile,
            profiles: config.profiles.keys().cloned().collect(),
        },
        resolved_profile: None,
    };

    match output {
        OutputFormat::Human => print_profiles_human(&runtime),
        OutputFormat::Json => print_profiles_json(&runtime)?,
    }

    Ok(())
}
