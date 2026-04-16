use std::io::{BufRead, Write};
use std::path::PathBuf;

use crate::api::{HttpApiConfig, HttpConfluenceApi};
use crate::application::profiles::{
    add_or_update_profile, attach_secret_backend, init_profile, list_profiles, remove_profile,
    use_profile, ProfileDraft, ProfileSecrets,
};
use crate::application::runtime::{ResolveOptions, ResolvedProfile, RuntimeContext};
use crate::application::vfs::VirtualFileSystem;
use crate::cli::args::CliAuthKind;
use crate::config::default_config_path;
use crate::infrastructure::vfs::ConfluenceVfs;
use crate::infrastructure::{profile_manager, runtime_loader};
use crate::secret::{KeyringSecretStore, SecretStore};
use crate::support::{ConfluenceCliError, Result};

use super::output::{write_profiles, write_resolved_profile};
use super::{prompt, GlobalArgs, ProfileArgs};

pub(super) fn config_init(
    global: &GlobalArgs,
    name: &str,
    profile_args: ProfileArgs,
) -> Result<()> {
    let store = KeyringSecretStore;
    let mut stdout = std::io::stdout();
    config_init_with_writer(global, name, profile_args, &store, &mut stdout)
}

pub(super) fn config_init_with_writer<W: Write>(
    global: &GlobalArgs,
    name: &str,
    profile_args: ProfileArgs,
    store: &dyn SecretStore,
    writer: &mut W,
) -> Result<()> {
    let runtime = config_init_runtime_with_store(global, name, profile_args, store)?;
    write_profiles(global.output, &runtime, writer)?;
    Ok(())
}

pub(super) fn profile_add(
    global: &GlobalArgs,
    name: &str,
    profile_args: ProfileArgs,
    activate: bool,
) -> Result<()> {
    let store = KeyringSecretStore;
    let mut stdout = std::io::stdout();
    profile_add_with_writer(global, name, profile_args, activate, &store, &mut stdout)
}

pub(super) fn profile_add_with_writer<W: Write>(
    global: &GlobalArgs,
    name: &str,
    profile_args: ProfileArgs,
    activate: bool,
    store: &dyn SecretStore,
    writer: &mut W,
) -> Result<()> {
    let runtime = profile_add_runtime_with_store(global, name, profile_args, activate, store)?;
    write_profiles(global.output, &runtime, writer)?;
    Ok(())
}

pub(super) fn profile_show(global: &GlobalArgs) -> Result<()> {
    let store = KeyringSecretStore;
    let mut stdout = std::io::stdout();
    profile_show_with_store(global, &store, &mut stdout)
}

pub(super) fn profile_show_with_store<W: Write>(
    global: &GlobalArgs,
    store: &dyn SecretStore,
    writer: &mut W,
) -> Result<()> {
    let runtime = load_runtime_context_with_store(
        &ResolveOptions::new(global.config_path.clone(), global.profile.clone()),
        store,
    )?;
    let profile = runtime
        .runtime_config
        .resolved_profile
        .as_ref()
        .ok_or_else(|| ConfluenceCliError::Config(no_active_profile_message()))?;
    write_resolved_profile(global.output, profile, writer)
}

pub(super) fn login(global: &GlobalArgs) -> Result<()> {
    let store = KeyringSecretStore;
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();
    login_with_store_and_io(global, &mut reader, &mut writer, &store)
}

pub(super) fn login_with_store_and_io<R: BufRead, W: Write>(
    global: &GlobalArgs,
    reader: &mut R,
    writer: &mut W,
    store: &dyn SecretStore,
) -> Result<()> {
    let name = prompt::prompt_required(reader, writer, "Profile name: ")?;
    let domain = prompt::prompt_required(reader, writer, "Domain: ")?;
    let auth_type = prompt::prompt_auth_kind(reader, writer)?;

    let mut profile = ProfileArgs {
        domain: Some(domain),
        auth_type: Some(auth_type),
        ..ProfileArgs::default()
    };

    match auth_type {
        CliAuthKind::Basic => {
            profile.email =
                prompt::prompt_optional(reader, writer, "Email (leave blank to use username): ")?;
            if profile.email.is_none() {
                profile.username = Some(prompt::prompt_required(reader, writer, "Username: ")?);
            }

            profile.api_token = prompt::prompt_optional(
                reader,
                writer,
                "API token (leave blank to use password): ",
            )?;
            if profile.api_token.is_none() {
                profile.password = Some(prompt::prompt_required(reader, writer, "Password: ")?);
            }
        }
        CliAuthKind::Bearer => {
            profile.api_token = Some(prompt::prompt_required(reader, writer, "API token: ")?);
        }
        CliAuthKind::Mtls => {}
    }

    profile.read_only = Some(prompt::prompt_bool(reader, writer, "Read-only", false)?);

    let runtime = profile_add_runtime_with_store(global, &name, profile, true, store)?;
    write_profiles(global.output, &runtime, writer)
}

pub(super) fn profile_use(global: &GlobalArgs, name: &str) -> Result<()> {
    let path = config_path(global);
    let store = KeyringSecretStore;
    let manager = profile_manager::ProfileManager::new(path.clone(), &store);
    let runtime = use_profile(&manager, name)?;
    let mut stdout = std::io::stdout();
    write_profiles(global.output, &runtime, &mut stdout)?;
    Ok(())
}

pub(super) fn profile_remove(global: &GlobalArgs, name: &str) -> Result<()> {
    let store = KeyringSecretStore;
    let mut stdout = std::io::stdout();
    profile_remove_with_writer(global, name, &store, &mut stdout)
}

pub(super) fn profile_remove_with_writer<W: Write>(
    global: &GlobalArgs,
    name: &str,
    store: &dyn SecretStore,
    writer: &mut W,
) -> Result<()> {
    let path = config_path(global);
    let manager = profile_manager::ProfileManager::new(path.clone(), store);
    let runtime = remove_profile(&manager, name)?;
    write_profiles(global.output, &runtime, writer)?;
    Ok(())
}

pub(super) fn profile_list(global: &GlobalArgs) -> Result<()> {
    let mut stdout = std::io::stdout();
    profile_list_with_store_and_writer(global, &KeyringSecretStore, &mut stdout)
}

pub(super) fn profile_list_with_store_and_writer<W: Write>(
    global: &GlobalArgs,
    store: &dyn SecretStore,
    writer: &mut W,
) -> Result<()> {
    let path = config_path(global);
    let manager = profile_manager::ProfileManager::new(path.clone(), store);
    let runtime = list_profiles(&manager)?;
    write_profiles(global.output, &runtime, writer)
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

fn config_init_runtime_with_store(
    global: &GlobalArgs,
    name: &str,
    profile_args: ProfileArgs,
    store: &dyn SecretStore,
) -> Result<crate::application::runtime::RuntimeConfig> {
    let path = config_path(global);
    let (profile, secrets) = profile_from_args(profile_args, None)?;
    let manager = profile_manager::ProfileManager::new(path.clone(), store);
    init_profile(&manager, name, profile, &secrets)
}

fn profile_add_runtime_with_store(
    global: &GlobalArgs,
    name: &str,
    profile_args: ProfileArgs,
    activate: bool,
    store: &dyn SecretStore,
) -> Result<crate::application::runtime::RuntimeConfig> {
    let path = config_path(global);
    let (profile, secrets) = profile_from_args(profile_args, None)?;
    let manager = profile_manager::ProfileManager::new(path.clone(), store);
    add_or_update_profile(&manager, name, profile, &secrets, activate)
}

pub(super) fn load_runtime_and_api(
    global: &GlobalArgs,
) -> Result<(RuntimeContext, HttpConfluenceApi)> {
    let runtime = load_runtime_context(global)?;
    let profile = runtime
        .runtime_config
        .resolved_profile
        .clone()
        .ok_or_else(|| ConfluenceCliError::Config(no_active_profile_message()))?;
    Ok((runtime, HttpConfluenceApi::new(http_api_config(profile))?))
}

pub(super) fn load_runtime_and_vfs(
    global: &GlobalArgs,
) -> Result<(RuntimeContext, Box<dyn VirtualFileSystem>)> {
    let (runtime, api) = load_runtime_and_api(global)?;
    Ok((runtime, Box::new(ConfluenceVfs::new(api))))
}

fn no_active_profile_message() -> String {
    "no active or selected profile. Run `confluence login` to create or repair one, use `confluence config init --domain <domain> ...` for first-time setup, or switch profiles with `confluence profile use <name>`. Use `confluence profile list` to inspect configured profiles".to_owned()
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
