use terra_core::{BoxFuture, TileSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileId {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

impl TileId {
    pub const ROOT: Self = Self { z: 0, x: 0, y: 0 };

    pub fn parent(self) -> Option<Self> {
        (self.z > 0).then(|| Self {
            z: self.z - 1,
            x: self.x / 2,
            y: self.y / 2,
        })
    }

    pub fn children(self) -> [Self; 4] {
        let (z, x, y) = (self.z + 1, self.x * 2, self.y * 2);
        [
            Self { z, x, y },
            Self { z, x: x + 1, y },
            Self { z, x, y: y + 1 },
            Self {
                z,
                x: x + 1,
                y: y + 1,
            },
        ]
    }
}

/// A decoded RGBA8 raster tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedTile {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TileError {
    #[error("tile fetch failed: {0}")]
    Fetch(String),
    #[error("tile decode failed: {0}")]
    Decode(String),
    #[error("tile fetch cancelled")]
    Cancelled,
}

/// Fetches and decodes one tile. Implemented over `HttpClient` + a disk cache
/// in `terra-render`; faked in tests.
pub trait TileFetcher: Send + Sync {
    fn fetch(
        &self,
        source: &TileSource,
        id: TileId,
    ) -> BoxFuture<'_, Result<DecodedTile, TileError>>;
}
