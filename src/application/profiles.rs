use crate::profile::AuthKind;
use crate::support::Result;

use super::ports::ProfilesStore;
use super::runtime::RuntimeConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileDraft {
    pub id: Option<String>,
    pub domain: String,
    pub protocol: Option<String>,
    pub api_path: Option<String>,
    pub auth_type: Option<AuthKind>,
    pub email: Option<String>,
    pub username: Option<String>,
    pub read_only: Option<bool>,
    pub has_secrets: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileSecrets {
    pub api_token: Option<String>,
    pub password: Option<String>,
}

pub fn attach_secret_backend(profile: &mut ProfileDraft, has_secret: bool) {
    profile.has_secrets = has_secret;
}

pub fn init_profile<S: ProfilesStore>(
    store: &S,
    name: &str,
    draft: ProfileDraft,
    secrets: &ProfileSecrets,
) -> Result<RuntimeConfig> {
    store.init_profile(name, draft, secrets)
}

pub fn add_or_update_profile<S: ProfilesStore>(
    store: &S,
    name: &str,
    draft: ProfileDraft,
    secrets: &ProfileSecrets,
    activate: bool,
) -> Result<RuntimeConfig> {
    store.add_or_update_profile(name, draft, secrets, activate)
}

pub fn use_profile<S: ProfilesStore>(store: &S, name: &str) -> Result<RuntimeConfig> {
    store.use_profile(name)
}

pub fn remove_profile<S: ProfilesStore>(store: &S, name: &str) -> Result<RuntimeConfig> {
    store.remove_profile(name)
}

pub fn list_profiles<S: ProfilesStore>(store: &S) -> Result<RuntimeConfig> {
    store.list_profiles()
}
