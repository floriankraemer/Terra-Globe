use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FileError {
    #[error("file not found: {0}")]
    NotFound(String),
    #[error("failed to read file: {0}")]
    ReadFailed(String),
    #[error("failed to write file: {0}")]
    WriteFailed(String),
}

/// Whole-file read/write, abstracted so file sync is testable without a disk.
pub trait FileIo: Send + Sync {
    fn read(&self, path: &Path) -> Result<Vec<u8>, FileError>;
    fn write(&self, path: &Path, bytes: &[u8]) -> Result<(), FileError>;
}
