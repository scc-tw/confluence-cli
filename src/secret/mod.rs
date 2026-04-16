use keyring::Entry;
#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::Mutex;

use crate::support::{ConfluenceCliError, Result};

const SERVICE_NAME: &str = "confluence-cli";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretKind {
    ApiToken,
    Password,
}

impl SecretKind {
    pub fn as_key(self) -> &'static str {
        match self {
            SecretKind::ApiToken => "api_token",
            SecretKind::Password => "password",
        }
    }
}

pub trait SecretStore {
    fn get(&self, profile_id: &str, kind: SecretKind) -> Result<Option<String>>;
    fn set(&self, profile_id: &str, kind: SecretKind, value: &str) -> Result<()>;
    fn delete(&self, profile_id: &str, kind: SecretKind) -> Result<()>;
}

#[derive(Debug, Default)]
pub struct KeyringSecretStore;

impl KeyringSecretStore {
    fn entry(&self, profile_id: &str, kind: SecretKind) -> Result<Entry> {
        Entry::new(SERVICE_NAME, &format!("{profile_id}:{}", kind.as_key())).map_err(|error| {
            ConfluenceCliError::SecretStore(format!("failed to construct keyring entry: {error}"))
        })
    }
}

impl SecretStore for KeyringSecretStore {
    fn get(&self, profile_id: &str, kind: SecretKind) -> Result<Option<String>> {
        let entry = self.entry(profile_id, kind)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(ConfluenceCliError::SecretStore(format!(
                "failed to read secret from keyring: {error}"
            ))),
        }
    }

    fn set(&self, profile_id: &str, kind: SecretKind, value: &str) -> Result<()> {
        let entry = self.entry(profile_id, kind)?;
        entry.set_password(value).map_err(|error| {
            ConfluenceCliError::SecretStore(format!("failed to write secret to keyring: {error}"))
        })
    }

    fn delete(&self, profile_id: &str, kind: SecretKind) -> Result<()> {
        let entry = self.entry(profile_id, kind)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(ConfluenceCliError::SecretStore(format!(
                "failed to delete secret from keyring: {error}"
            ))),
        }
    }
}

#[cfg(test)]
#[derive(Debug, Default)]
pub struct MemorySecretStore {
    values: Mutex<HashMap<(String, SecretKind), String>>,
}

#[cfg(test)]
impl MemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
impl SecretStore for MemorySecretStore {
    fn get(&self, profile_id: &str, kind: SecretKind) -> Result<Option<String>> {
        Ok(self
            .values
            .lock()
            .expect("secret store mutex should not be poisoned")
            .get(&(profile_id.to_owned(), kind))
            .cloned())
    }

    fn set(&self, profile_id: &str, kind: SecretKind, value: &str) -> Result<()> {
        self.values
            .lock()
            .expect("secret store mutex should not be poisoned")
            .insert((profile_id.to_owned(), kind), value.to_owned());
        Ok(())
    }

    fn delete(&self, profile_id: &str, kind: SecretKind) -> Result<()> {
        self.values
            .lock()
            .expect("secret store mutex should not be poisoned")
            .remove(&(profile_id.to_owned(), kind));
        Ok(())
    }
}
