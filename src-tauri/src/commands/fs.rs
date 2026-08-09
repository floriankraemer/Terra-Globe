use std::fs;
use std::path::Path;

#[derive(Debug, serde::Serialize)]
pub enum FsError {
    NotFound(String),
    ReadFailed(String),
    WriteFailed(String),
}

impl std::fmt::Display for FsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FsError::NotFound(path) => write!(f, "file not found: {path}"),
            FsError::ReadFailed(msg) => write!(f, "failed to read file: {msg}"),
            FsError::WriteFailed(msg) => write!(f, "failed to write file: {msg}"),
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

/// Pure, testable core of the `read_binary_file` Tauri command.
pub fn read_binary_file_contents(path: &str) -> Result<Vec<u8>, FsError> {
    if !Path::new(path).exists() {
        return Err(FsError::NotFound(path.to_string()));
    }
    fs::read(path).map_err(|e| FsError::ReadFailed(e.to_string()))
}

#[tauri::command]
pub fn read_binary_file(path: String) -> Result<Vec<u8>, FsError> {
    read_binary_file_contents(&path)
}

/// Pure, testable core of the `write_binary_file` Tauri command.
pub fn write_binary_file_contents(path: &str, contents: &[u8]) -> Result<(), FsError> {
    fs::write(path, contents).map_err(|e| FsError::WriteFailed(e.to_string()))
}

#[tauri::command]
pub fn write_binary_file(path: String, contents: Vec<u8>) -> Result<(), FsError> {
    write_binary_file_contents(&path, &contents)
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

    #[test]
    fn round_trips_binary_contents_through_write_and_read() {
        let path = std::env::temp_dir().join(format!(
            "terra-globe-test-{}.kmz",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = path.to_string_lossy().into_owned();
        let bytes: Vec<u8> = vec![0x50, 0x4b, 0x03, 0x04, 0x00, 0xff];

        write_binary_file_contents(&path, &bytes).unwrap();
        let result = read_binary_file_contents(&path).unwrap();

        assert_eq!(result, bytes);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn write_binary_file_fails_when_parent_dir_is_missing() {
        let result = write_binary_file_contents("/nonexistent/parent/dir/file.kmz", &[1, 2, 3]);

        assert!(matches!(result, Err(FsError::WriteFailed(_))));
    }

    #[test]
    fn read_binary_file_returns_not_found_for_a_missing_path() {
        let result = read_binary_file_contents("/nonexistent/path/does-not-exist.kmz");

        assert!(matches!(result, Err(FsError::NotFound(_))));
    }
}
