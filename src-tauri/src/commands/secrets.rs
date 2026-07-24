use keyring::Entry;

const SERVICE: &str = "terra-globe-provider";

#[derive(Debug, serde::Serialize)]
pub enum SecretError {
    Unavailable(String),
}

impl std::fmt::Display for SecretError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecretError::Unavailable(msg) => write!(f, "OS keychain unavailable: {msg}"),
        }
    }
}

fn entry(id: &str) -> Result<Entry, SecretError> {
    Entry::new(SERVICE, id).map_err(|e| SecretError::Unavailable(e.to_string()))
}

#[tauri::command]
pub fn secret_get(id: String) -> Result<Option<String>, SecretError> {
    match entry(&id)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(SecretError::Unavailable(e.to_string())),
    }
}

#[tauri::command]
pub fn secret_set(id: String, value: String) -> Result<(), SecretError> {
    entry(&id)?
        .set_password(&value)
        .map_err(|e| SecretError::Unavailable(e.to_string()))
}

#[tauri::command]
pub fn secret_remove(id: String) -> Result<(), SecretError> {
    match entry(&id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(SecretError::Unavailable(e.to_string())),
    }
}
