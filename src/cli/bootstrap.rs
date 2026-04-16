use std::path::PathBuf;

use uuid::Uuid;

use crate::api::{HttpApiConfig, HttpConfluenceApi};
use crate::application::runtime::{ResolveOptions, ResolvedProfile, RuntimeContext};
use crate::cli::args::CliAuthKind;
use crate::config::{Profile, default_config_path, load_config};
use crate::infrastructure::{profile_manager, runtime_loader};
use crate::support::{ConfluenceCliError, Result};

use super::output::{print_profiles_human, print_profiles_json};
use super::{GlobalArgs, OutputFormat, ProfileArgs};

type ProfileSecrets = profile_manager::ProfileSecrets;

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

    let (profile, secrets) = profile_from_args(profile_args, None)?;
    let config = profile_manager::init_profile_config(name, profile, &secrets)?;
    print_profiles(global.output, profile_manager::runtime_profiles(config))?;
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
    let existing_id = existing
        .profiles
        .get(name)
        .and_then(|profile| profile.id.clone());
    let (profile, secrets) = profile_from_args(profile_args, existing_id)?;
    let config = profile_manager::add_or_update_profile(&path, name, profile, &secrets, activate)?;
    print_profiles(global.output, profile_manager::runtime_profiles(config))?;
    Ok(())
}

pub(super) fn profile_use(global: &GlobalArgs, name: &str) -> Result<()> {
    let path = config_path(global);
    let config = profile_manager::use_profile(&path, name)?;
    print_profiles(global.output, profile_manager::runtime_profiles(config))?;
    Ok(())
}

pub(super) fn profile_remove(global: &GlobalArgs, name: &str) -> Result<()> {
    let path = config_path(global);
    let config = profile_manager::remove_profile_with_secrets(&path, name)?;
    print_profiles(global.output, profile_manager::runtime_profiles(config))?;
    Ok(())
}

pub(super) fn profile_list(global: &GlobalArgs) -> Result<()> {
    let path = config_path(global);
    let config = load_config(&path)?;
    let runtime = profile_manager::runtime_profiles(config);
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
            secret_backend: None,
        },
        secrets,
    ))
    .map(|(mut profile, secrets)| {
        profile_manager::attach_secret_backend(
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
