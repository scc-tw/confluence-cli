use std::path::PathBuf;

use crate::api::{HttpApiConfig, HttpConfluenceApi};
use crate::application::profiles::{
    ProfileDraft, ProfileSecrets, add_or_update_profile, attach_secret_backend, init_profile,
    list_profiles, remove_profile, use_profile,
};
use crate::application::runtime::{ResolveOptions, ResolvedProfile, RuntimeContext};
use crate::cli::args::CliAuthKind;
use crate::config::default_config_path;
use crate::infrastructure::{profile_manager, runtime_loader};
use crate::secret::{KeyringSecretStore, SecretStore};
use crate::support::{ConfluenceCliError, Result};

use super::output::{print_profiles_human, print_profiles_json};
use super::{GlobalArgs, OutputFormat, ProfileArgs};

pub(super) fn config_init(
    global: &GlobalArgs,
    name: &str,
    profile_args: ProfileArgs,
) -> Result<()> {
    let store = KeyringSecretStore;
    config_init_with_store(global, name, profile_args, &store)
}

pub(super) fn config_init_with_store(
    global: &GlobalArgs,
    name: &str,
    profile_args: ProfileArgs,
    store: &dyn SecretStore,
) -> Result<()> {
    let path = config_path(global);
    let (profile, secrets) = profile_from_args(profile_args, None)?;
    let manager = profile_manager::ProfileManager::new(path.clone(), store);
    let runtime = init_profile(&manager, name, profile, &secrets)?;
    print_profiles(global.output, runtime)?;
    Ok(())
}

pub(super) fn profile_add(
    global: &GlobalArgs,
    name: &str,
    profile_args: ProfileArgs,
    activate: bool,
) -> Result<()> {
    let store = KeyringSecretStore;
    profile_add_with_store(global, name, profile_args, activate, &store)
}

pub(super) fn profile_add_with_store(
    global: &GlobalArgs,
    name: &str,
    profile_args: ProfileArgs,
    activate: bool,
    store: &dyn SecretStore,
) -> Result<()> {
    let path = config_path(global);
    let (profile, secrets) = profile_from_args(profile_args, None)?;
    let manager = profile_manager::ProfileManager::new(path.clone(), store);
    let runtime = add_or_update_profile(&manager, name, profile, &secrets, activate)?;
    print_profiles(global.output, runtime)?;
    Ok(())
}

pub(super) fn profile_use(global: &GlobalArgs, name: &str) -> Result<()> {
    let path = config_path(global);
    let store = KeyringSecretStore;
    let manager = profile_manager::ProfileManager::new(path.clone(), &store);
    let runtime = use_profile(&manager, name)?;
    print_profiles(global.output, runtime)?;
    Ok(())
}

pub(super) fn profile_remove(global: &GlobalArgs, name: &str) -> Result<()> {
    let store = KeyringSecretStore;
    profile_remove_with_store(global, name, &store)
}

pub(super) fn profile_remove_with_store(
    global: &GlobalArgs,
    name: &str,
    store: &dyn SecretStore,
) -> Result<()> {
    let path = config_path(global);
    let manager = profile_manager::ProfileManager::new(path.clone(), store);
    let runtime = remove_profile(&manager, name)?;
    print_profiles(global.output, runtime)?;
    Ok(())
}

pub(super) fn profile_list(global: &GlobalArgs) -> Result<()> {
    let path = config_path(global);
    let store = KeyringSecretStore;
    let manager = profile_manager::ProfileManager::new(path.clone(), &store);
    let runtime = list_profiles(&manager)?;
    match global.output {
        OutputFormat::Human => print_profiles_human(&runtime),
        OutputFormat::Json => super::output::print_profiles_json(&runtime)?,
    }
    Ok(())
}

pub(super) fn load_runtime_context(global: &GlobalArgs) -> Result<RuntimeContext> {
    let options = ResolveOptions::new(global.config_path.clone(), global.profile.clone());
    let store = KeyringSecretStore;
    load_runtime_context_with_store(&options, &store)
}

pub(super) fn load_runtime_context_with_store(
    options: &ResolveOptions,
    store: &dyn SecretStore,
) -> Result<RuntimeContext> {
    runtime_loader::load_runtime_context_with_store(options, store)
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
) -> Result<(ProfileDraft, ProfileSecrets)> {
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
        ProfileDraft {
            id: existing_id,
            domain,
            protocol: args.protocol,
            api_path: args.api_path,
            auth_type: auth_type.map(Into::into),
            email: args.email,
            username: args.username,
            read_only: args.read_only,
            has_secrets: false,
        },
        secrets,
    ))
    .map(|(mut profile, secrets)| {
        attach_secret_backend(
            &mut profile,
            secrets.api_token.is_some() || secrets.password.is_some(),
        );
        (profile, secrets)
    })
}

fn config_path(global: &GlobalArgs) -> PathBuf {
    global
        .config_path
        .clone()
        .unwrap_or_else(default_config_path)
}

fn print_profiles(
    output: OutputFormat,
    runtime: crate::application::runtime::RuntimeConfig,
) -> Result<()> {
    match output {
        OutputFormat::Human => print_profiles_human(&runtime),
        OutputFormat::Json => print_profiles_json(&runtime)?,
    }

    Ok(())
}
