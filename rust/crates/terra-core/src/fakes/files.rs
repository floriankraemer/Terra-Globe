use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::ports::{FileError, FileIo};

#[derive(Debug, Default)]
pub struct InMemoryFileIo {
    files: Mutex<HashMap<PathBuf, Vec<u8>>>,
}

impl InMemoryFileIo {
    pub fn put(&self, path: impl Into<PathBuf>, bytes: impl Into<Vec<u8>>) {
        self.files
            .lock()
            .expect("poisoned")
            .insert(path.into(), bytes.into());
    }

    pub fn get(&self, path: &Path) -> Option<Vec<u8>> {
        self.files.lock().expect("poisoned").get(path).cloned()
    }
}

impl FileIo for InMemoryFileIo {
    fn read(&self, path: &Path) -> Result<Vec<u8>, FileError> {
        self.get(path)
            .ok_or_else(|| FileError::NotFound(path.display().to_string()))
    }

    fn write(&self, path: &Path, bytes: &[u8]) -> Result<(), FileError> {
        self.put(path.to_path_buf(), bytes.to_vec());
        Ok(())
    }
}
