#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SecretError {
    #[error("OS keychain unavailable: {0}")]
    Unavailable(String),
}

/// Secret storage port. Holds API keys addressed by provider config id and is
/// never persisted alongside plain settings data.
pub trait SecretStore: Send + Sync {
    fn get(&self, id: &str) -> Result<Option<String>, SecretError>;
    fn set(&self, id: &str, value: &str) -> Result<(), SecretError>;
    /// Removing a missing entry succeeds.
    fn remove(&self, id: &str) -> Result<(), SecretError>;
}
