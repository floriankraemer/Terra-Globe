//! KML/KMZ import and export.
//!
//! Behaviour matches `packages/core/src/kml`: folders and styles round-trip,
//! Circle and Rectangle survive via `<ExtendedData>`, unsupported features are
//! skipped with warnings instead of aborting the import.

pub mod color;
pub mod kmz_assets;
pub mod mapping;
pub mod network_links;
pub mod parse;
pub mod serialize;
pub mod xml;

use std::io::{Cursor, Read, Write};

pub use kmz_assets::{
    DecodedDataUrl, bytes_to_data_url, decode_data_url, extension_for_mime_type, guess_mime_type,
    normalize_asset_path,
};
pub use mapping::geometry::{KmlGeometryElement, KmlGeometryResult};
pub use network_links::{DEFAULT_MAX_DEPTH, KmlFetcher, resolve_network_links};
pub use parse::{KmlNetworkLinkRef, ParseKmlResult, RESERVED_EXTENDED_DATA_PREFIX, parse_kml};
pub use serialize::{SerializeKmlInput, escape_xml, serialize_kml};

#[derive(Debug, thiserror::Error)]
pub enum KmlError {
    #[error("invalid KMZ archive: {0}")]
    Zip(String),
    #[error("invalid KML XML: {0}")]
    Xml(String),
    #[error("invalid geometry: {0}")]
    InvalidGeometry(String),
    #[error("{0} cannot be written as a KML shape geometry")]
    UnsupportedGeometry(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

/// One file inside a KMZ archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KmzEntry {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Extracts every file from a KMZ archive. Entries are returned in archive
/// order so callers can pick `doc.kml` deterministically.
pub fn read_kmz(bytes: &[u8]) -> Result<Vec<KmzEntry>, KmlError> {
    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| KmlError::Zip(e.to_string()))?;
    let mut entries = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|e| KmlError::Zip(e.to_string()))?;
        if file.is_dir() {
            continue;
        }
        let mut content = Vec::new();
        file.read_to_end(&mut content)
            .map_err(|e| KmlError::Zip(e.to_string()))?;
        let name = file
            .name()
            .map_err(|error| KmlError::Zip(error.to_string()))?
            .to_string();
        entries.push(KmzEntry {
            name,
            bytes: content,
        });
    }
    if entries.is_empty() {
        return Err(KmlError::Zip("archive contains no files".to_string()));
    }
    Ok(entries)
}

/// Builds a KMZ archive from the KML document plus referenced assets.
pub fn write_kmz(kml: &str, assets: &[KmzEntry]) -> Result<Vec<u8>, KmlError> {
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buffer);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        writer
            .start_file("doc.kml", options)
            .map_err(|e| KmlError::Zip(e.to_string()))?;
        writer
            .write_all(kml.as_bytes())
            .map_err(|e| KmlError::Zip(e.to_string()))?;
        for asset in assets {
            writer
                .start_file(asset.name.clone(), options)
                .map_err(|e| KmlError::Zip(e.to_string()))?;
            writer
                .write_all(&asset.bytes)
                .map_err(|e| KmlError::Zip(e.to_string()))?;
        }
        writer.finish().map_err(|e| KmlError::Zip(e.to_string()))?;
    }
    Ok(buffer.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kmz_round_trips_doc_and_assets() {
        let assets = vec![KmzEntry {
            name: "images/icon.png".to_string(),
            bytes: vec![0x89, b'P', b'N', b'G'],
        }];
        let archive = write_kmz("<kml/>", &assets).expect("write kmz");
        let entries = read_kmz(&archive).expect("read kmz");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "doc.kml");
        assert_eq!(entries[0].bytes, b"<kml/>");
        assert_eq!(entries[1], assets[0]);
    }

    #[test]
    fn invalid_kmz_is_an_explicit_error() {
        let error = read_kmz(b"not a zip").expect_err("must fail");
        assert!(matches!(error, KmlError::Zip(_)));
    }
}
