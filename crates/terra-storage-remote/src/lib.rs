//! Placeholder remote storage adapter.
//!
//! Keeping this crate in the workspace means the sync seam is compiled and
//! tested from day one, so it cannot quietly rot (same intent as
//! `packages/storage-remote`).

use terra_core::DomainError;

#[derive(Debug, thiserror::Error)]
pub enum RemoteError {
    #[error("remote storage is not configured")]
    NotConfigured,
    #[error("remote storage is implemented after desktop parity")]
    NotImplemented,
    #[error(transparent)]
    Invalid(#[from] DomainError),
}

/// A remote endpoint the user owns (self-hosted sync). No first-party server
/// exists in the default path; that is a product constraint, not a TODO.
#[derive(Debug, Clone)]
pub struct RemotePlacesRepository {
    base_url: String,
}

impl RemotePlacesRepository {
    pub fn new(base_url: impl Into<String>) -> Result<Self, RemoteError> {
        let base_url = base_url.into();
        if base_url.trim().is_empty() {
            return Err(RemoteError::NotConfigured);
        }
        Ok(Self { base_url })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_an_explicit_endpoint() {
        assert!(matches!(
            RemotePlacesRepository::new("   "),
            Err(RemoteError::NotConfigured)
        ));
        let repo =
            RemotePlacesRepository::new("https://sync.example.test").expect("endpoint is valid");
        assert_eq!(repo.base_url(), "https://sync.example.test");
    }
}
