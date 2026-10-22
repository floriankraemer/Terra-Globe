use std::collections::HashMap;
use std::sync::Mutex;

use crate::ports::{SecretError, SecretStore};

#[derive(Debug, Default)]
pub struct InMemorySecretStore {
    entries: Mutex<HashMap<String, String>>,
}

impl SecretStore for InMemorySecretStore {
    fn get(&self, id: &str) -> Result<Option<String>, SecretError> {
        Ok(self.entries.lock().expect("poisoned").get(id).cloned())
    }

    fn set(&self, id: &str, value: &str) -> Result<(), SecretError> {
        self.entries
            .lock()
            .expect("poisoned")
            .insert(id.to_owned(), value.to_owned());
        Ok(())
    }

    fn remove(&self, id: &str) -> Result<(), SecretError> {
        self.entries.lock().expect("poisoned").remove(id);
        Ok(())
    }
}
