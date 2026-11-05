//! KMZ asset helpers: data URLs, MIME types and KMZ-internal paths.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

/// MIME type guessed from a file extension, matching the web app's table.
pub fn guess_mime_type(path: &str) -> &'static str {
    match extension_of(path).as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "dae" => "model/vnd.collada+xml",
        "gltf" => "model/gltf+json",
        "glb" => "model/gltf-binary",
        "bin" => "application/octet-stream",
        _ => "application/octet-stream",
    }
}

pub fn bytes_to_data_url(bytes: &[u8], path: &str) -> String {
    format!(
        "data:{};base64,{}",
        guess_mime_type(path),
        STANDARD.encode(bytes)
    )
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecodedDataUrl {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

pub fn decode_data_url(url: &str) -> Option<DecodedDataUrl> {
    let (header, payload) = url.split_once(";base64,")?;
    let mime_type = header.strip_prefix("data:")?;
    let bytes = STANDARD.decode(payload).ok()?;
    Some(DecodedDataUrl {
        mime_type: mime_type.to_string(),
        bytes,
    })
}

pub fn extension_for_mime_type(mime_type: &str) -> &'static str {
    match mime_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        "model/vnd.collada+xml" => "dae",
        "model/gltf+json" => "gltf",
        "model/gltf-binary" => "glb",
        _ => "bin",
    }
}

/// Strips a leading `./` or `/` from a KMZ-internal path.
pub fn normalize_asset_path(path: &str) -> String {
    path.trim_start_matches("./")
        .trim_start_matches('/')
        .to_string()
}

fn extension_of(path: &str) -> String {
    path.rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_urls_round_trip() {
        let bytes = [0x89, b'P', b'N', b'G'];
        let url = bytes_to_data_url(&bytes, "images/icon.png");
        assert!(url.starts_with("data:image/png;base64,"));
        let decoded = decode_data_url(&url).expect("decodes");
        assert_eq!(decoded.mime_type, "image/png");
        assert_eq!(decoded.bytes, bytes);
    }

    #[test]
    fn non_data_urls_are_rejected() {
        assert!(decode_data_url("https://example.test/icon.png").is_none());
    }

    #[test]
    fn mime_guessing_and_reverse_mapping_agree() {
        for path in ["a.png", "a.JPG", "a.gltf", "a.unknown"] {
            let mime = guess_mime_type(path);
            assert_eq!(extension_for_mime_type(mime), {
                match extension_of(path).as_str() {
                    "jpg" | "jpeg" => "jpg",
                    "png" => "png",
                    "gltf" => "gltf",
                    _ => "bin",
                }
            });
        }
    }

    #[test]
    fn asset_paths_are_normalized() {
        assert_eq!(normalize_asset_path("./images/a.png"), "images/a.png");
        assert_eq!(normalize_asset_path("/images/a.png"), "images/a.png");
        assert_eq!(normalize_asset_path("images/a.png"), "images/a.png");
    }
}
