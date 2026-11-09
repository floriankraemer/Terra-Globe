//! Persistent tile cache for the desktop shell.
//!
//! Tiles are stored as the bytes the provider served, under a namespace derived
//! from the provider id and URL template, so switching base layers or a preset's
//! URL never serves another layer's imagery.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use terra_globe::TileId;
use terra_globe::imagery::TileCache;
use terra_providers::TileSource;

pub struct DiskTileCache {
    root: PathBuf,
}

impl DiskTileCache {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Deletes the oldest tiles until the cache is back under `max_bytes`.
    /// Best effort: the cache is still correct if a file cannot be removed.
    pub fn prune(&self, max_bytes: u64) {
        let mut files = Vec::new();
        let mut total = 0u64;
        collect_files(&self.root, &mut files, &mut total);
        if total <= max_bytes {
            return;
        }
        files.sort_by_key(|(_, _, modified)| *modified);
        for (path, len, _) in files {
            if total <= max_bytes {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                total = total.saturating_sub(len);
            }
        }
    }

    fn path(&self, source: &TileSource, id: TileId) -> PathBuf {
        self.root
            .join(namespace(source))
            .join(id.z.to_string())
            .join(id.x.to_string())
            .join(format!("{}.tile", id.y))
    }
}

impl TileCache for DiskTileCache {
    fn get(&self, source: &TileSource, id: TileId) -> Option<Vec<u8>> {
        let bytes = std::fs::read(self.path(source, id)).ok()?;
        // A zero-length file is a write that was interrupted before the atomic
        // rename; treat it as a miss so the tile can be fetched again.
        (!bytes.is_empty()).then_some(bytes)
    }

    fn put(&self, source: &TileSource, id: TileId, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let path = self.path(source, id);
        let Some(parent) = path.parent() else {
            return;
        };
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }

        // Write beside the target and rename, so a crash cannot leave a partial
        // tile that would be served as valid imagery on the next run.
        static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
        let temp = path.with_extension(format!(
            "tmp{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        if std::fs::write(&temp, bytes).is_err() {
            let _ = std::fs::remove_file(&temp);
            return;
        }
        if std::fs::rename(&temp, &path).is_err() {
            // Windows refuses to replace an existing file; a concurrent writer
            // may have created it since the cache miss.
            let _ = std::fs::remove_file(&path);
            if std::fs::rename(&temp, &path).is_err() {
                let _ = std::fs::remove_file(&temp);
            }
        }
    }
}

fn namespace(source: &TileSource) -> String {
    let id: String = source
        .id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    format!("{id}-{:016x}", fnv1a(source.url.as_bytes()))
}

fn collect_files(
    directory: &Path,
    files: &mut Vec<(PathBuf, u64, std::time::SystemTime)>,
    total: &mut u64,
) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_dir() {
            collect_files(&path, files, total);
        } else if metadata.is_file() && metadata.len() > 0 {
            *total += metadata.len();
            files.push((
                path,
                metadata.len(),
                metadata.modified().unwrap_or(std::time::UNIX_EPOCH),
            ));
        }
    }
}

/// FNV-1a, kept local so the cache layout is stable across Rust versions.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("terra-tile-cache-{}-{name}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        root
    }

    #[test]
    fn round_trips_tile_bytes() {
        let root = temp_root("roundtrip");
        let cache = DiskTileCache::new(root.clone());
        let source = TileSource::open_street_map();
        let id = TileId::new(3, 4, 2).expect("tile");

        assert!(cache.get(&source, id).is_none());
        cache.put(&source, id, &[1, 2, 3, 4]);
        assert_eq!(cache.get(&source, id), Some(vec![1, 2, 3, 4]));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn different_provider_urls_do_not_share_namespaces() {
        let root = temp_root("namespace");
        let cache = DiskTileCache::new(root.clone());
        let source = TileSource::open_street_map();
        let id = TileId::new(3, 4, 2).expect("tile");
        cache.put(&source, id, &[9]);

        let mut other = source.clone();
        other.url.push_str("?style=other");
        assert!(cache.get(&other, id).is_none());

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn empty_files_are_treated_as_misses() {
        let root = temp_root("empty");
        let cache = DiskTileCache::new(root.clone());
        let source = TileSource::open_street_map();
        let id = TileId::new(1, 0, 0).expect("tile");
        let path = cache.path(&source, id);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, []).expect("write");
        assert!(cache.get(&source, id).is_none());

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn prune_keeps_the_cache_under_its_budget() {
        let root = temp_root("prune");
        let cache = DiskTileCache::new(root.clone());
        let source = TileSource::open_street_map();
        for y in 0..4u32 {
            let id = TileId::new(4, 1, y).expect("tile");
            cache.put(&source, id, &[0u8; 10]);
        }

        cache.prune(25);

        let mut files = Vec::new();
        let mut total = 0;
        collect_files(&root, &mut files, &mut total);
        assert!(total <= 25, "cache still holds {total} bytes");
        assert!(files.len() <= 2, "cache still holds {} files", files.len());

        std::fs::remove_dir_all(root).ok();
    }
}
