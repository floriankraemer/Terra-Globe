use std::collections::HashMap;
use std::sync::Mutex;

use crate::ports::PreferencesStore;

#[derive(Debug, Default)]
pub struct InMemoryPreferencesStore {
    entries: Mutex<HashMap<String, String>>,
}

impl InMemoryPreferencesStore {
    pub fn with(entries: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>) -> Self {
        let map = entries
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        Self {
            entries: Mutex::new(map),
        }
    }
}

impl PreferencesStore for InMemoryPreferencesStore {
    fn get(&self, key: &str) -> Option<String> {
        self.entries.lock().expect("poisoned").get(key).cloned()
    }

    fn set(&self, key: &str, value: &str) {
        self.entries
            .lock()
            .expect("poisoned")
            .insert(key.to_owned(), value.to_owned());
    }

    fn remove(&self, key: &str) {
        self.entries.lock().expect("poisoned").remove(key);
    }
}
