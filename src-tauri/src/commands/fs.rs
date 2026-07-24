use std::fs;
use std::path::Path;

#[derive(Debug, serde::Serialize)]
pub enum FsError {
    NotFound(String),
    ReadFailed(String),
}

impl std::fmt::Display for FsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FsError::NotFound(path) => write!(f, "file not found: {path}"),
            FsError::ReadFailed(msg) => write!(f, "failed to read file: {msg}"),
        }
    }
}

/// Pure, testable core of the `read_text_file` Tauri command.
pub fn read_file_contents(path: &str) -> Result<String, FsError> {
    if !Path::new(path).exists() {
        return Err(FsError::NotFound(path.to_string()));
    }
    fs::read_to_string(path).map_err(|e| FsError::ReadFailed(e.to_string()))
}

#[tauri::command]
pub fn read_text_file(path: String) -> Result<String, FsError> {
    read_file_contents(&path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_file(contents: &str) -> String {
        let path = std::env::temp_dir().join(format!(
            "terra-globe-test-{}.txt",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn returns_contents_for_an_existing_file() {
        let path = write_temp_file("hello world");

        let result = read_file_contents(&path);

        assert_eq!(result.unwrap(), "hello world");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn returns_not_found_for_a_missing_path() {
        let result = read_file_contents("/nonexistent/path/does-not-exist.kml");

        assert!(matches!(result, Err(FsError::NotFound(_))));
    }
}
