//! Secret-store port for provider API keys (BYOK).
//!
//! Keys never leave the user's machine: the desktop shell backs this with the OS
//! keychain, the browser with local storage, and tests with memory.

use std::collections::BTreeMap;
use std::sync::Mutex;

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("secret store error: {0}")]
    Backend(String),
}

pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>, SecretError>;
    fn set(&self, key: &str, value: &str) -> Result<(), SecretError>;
    fn remove(&self, key: &str) -> Result<(), SecretError>;
}

#[derive(Debug, Default)]
pub struct InMemorySecretStore {
    entries: Mutex<BTreeMap<String, String>>,
}

impl InMemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for InMemorySecretStore {
    fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
        Ok(self
            .entries
            .lock()
            .map_err(|_| SecretError::Backend("lock poisoned".to_string()))?
            .get(key)
            .cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        self.entries
            .lock()
            .map_err(|_| SecretError::Backend("lock poisoned".to_string()))?
            .insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn remove(&self, key: &str) -> Result<(), SecretError> {
        self.entries
            .lock()
            .map_err(|_| SecretError::Backend("lock poisoned".to_string()))?
            .remove(key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_round_trip_and_are_removable() {
        let store = InMemorySecretStore::new();
        assert_eq!(store.get("mapbox").expect("get"), None);
        store.set("mapbox", "pk.secret").expect("set");
        assert_eq!(
            store.get("mapbox").expect("get").as_deref(),
            Some("pk.secret")
        );
        store.remove("mapbox").expect("remove");
        assert_eq!(store.get("mapbox").expect("get"), None);
    }

    #[test]
    fn removing_a_missing_secret_is_not_an_error() {
        let store = InMemorySecretStore::new();
        store.remove("absent").expect("remove is idempotent");
    }
}
