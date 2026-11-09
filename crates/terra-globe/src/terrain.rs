//! Terrain: Terrarium elevation tiles, a height lookup over whatever has
//! loaded, and the fetch queue that feeds it.
//!
//! Everything here is CPU-side and GPU-free. The renderer displaces tile
//! meshes with [`TerrainCache::height_at_level`], and the placemark, picking
//! and camera code share the same lookup through [`crate::geo::GroundFn`].

use std::cell::Cell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use image::RgbaImage;
use terra_providers::TileSource;

use crate::geo::{MERCATOR_MAX_LAT_DEG, TileId, lon_lat_to_mercator_uv, mercator_uv_to_lon_lat};
use crate::imagery::{FetchedTile, TileCache, TileFetcher, TileProvider};

/// Side length of an elevation tile, in samples.
pub const TERRAIN_TILE_SIZE: u32 = 256;
/// Deepest elevation level the cache keeps. Imagery deeper than this samples a
/// sub-rectangle of the level-15 tile.
pub const MAX_TERRAIN_LEVEL: u8 = 15;
/// Above the highest summit, below any orbit: bounds ray marching.
pub const MAX_TERRAIN_HEIGHT_M: f64 = 9_000.0;
/// Decoded tiles kept in memory (256 KiB each).
const MAX_RESIDENT_TILES: usize = 384;

const SAMPLES: usize = (TERRAIN_TILE_SIZE * TERRAIN_TILE_SIZE) as usize;

/// One decoded elevation tile: 256x256 heights in metres above sea level,
/// row-major from the north-west corner.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainTile {
    heights: Vec<f32>,
    min: f32,
    max: f32,
}

impl TerrainTile {
    /// `None` unless there are exactly 256x256 finite samples.
    pub fn from_heights(heights: Vec<f32>) -> Result<Self, String> {
        if heights.len() != SAMPLES {
            return Err(format!(
                "elevation tile has {} samples, expected {SAMPLES}",
                heights.len()
            ));
        }
        if heights.iter().any(|height| !height.is_finite()) {
            return Err("elevation tile has non-finite samples".to_string());
        }
        let min = heights.iter().copied().fold(f32::INFINITY, f32::min);
        let max = heights.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        Ok(Self { heights, min, max })
    }

    /// Terrarium: `R * 256 + G + B / 256 - 32768` metres.
    pub fn terrarium_height(red: u8, green: u8, blue: u8) -> f32 {
        f32::from(red) * 256.0 + f32::from(green) + f32::from(blue) / 256.0 - 32768.0
    }

    /// Fails for anything but a 256x256 image: resampling would blend heights,
    /// and a wrong size means the server did not send an elevation tile.
    pub fn check_rgba(image: &RgbaImage) -> Result<(), String> {
        if image.dimensions() != (TERRAIN_TILE_SIZE, TERRAIN_TILE_SIZE) {
            return Err(format!(
                "elevation tile is {}x{}, expected {TERRAIN_TILE_SIZE}x{TERRAIN_TILE_SIZE}",
                image.width(),
                image.height()
            ));
        }
        Ok(())
    }

    pub fn from_rgba(image: &RgbaImage) -> Result<Self, String> {
        Self::check_rgba(image)?;
        Self::from_heights(
            image
                .pixels()
                .map(|pixel| Self::terrarium_height(pixel[0], pixel[1], pixel[2]))
                .collect(),
        )
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let image = image::load_from_memory(bytes)
            .map_err(|error| format!("decode failed: {error}"))?
            .to_rgba8();
        Self::from_rgba(&image)
    }

    /// A tile filled from a height function `(lon_deg, lat_deg) -> metres`,
    /// sampled at the pixel centres. This is the hook tests use to supply
    /// terrain without a network.
    pub fn synthetic(id: TileId, height: impl Fn(f64, f64) -> f64) -> Self {
        let count = 2f64.powi(i32::from(id.z));
        let size = f64::from(TERRAIN_TILE_SIZE);
        let mut heights = Vec::with_capacity(SAMPLES);
        for row in 0..TERRAIN_TILE_SIZE {
            for column in 0..TERRAIN_TILE_SIZE {
                let u = (f64::from(id.x) + (f64::from(column) + 0.5) / size) / count;
                let v = (f64::from(id.y) + (f64::from(row) + 0.5) / size) / count;
                let (lon, lat) = mercator_uv_to_lon_lat(u, v);
                heights.push(height(lon, lat) as f32);
            }
        }
        Self::from_heights(heights).unwrap_or_else(|_| Self {
            heights: vec![0.0; SAMPLES],
            min: 0.0,
            max: 0.0,
        })
    }

    pub fn min(&self) -> f32 {
        self.min
    }

    pub fn max(&self) -> f32 {
        self.max
    }

    fn at(&self, x: u32, y: u32) -> f32 {
        let index = (y.min(TERRAIN_TILE_SIZE - 1) * TERRAIN_TILE_SIZE
            + x.min(TERRAIN_TILE_SIZE - 1)) as usize;
        self.heights.get(index).copied().unwrap_or(0.0)
    }
}

/// Every elevation tile that has arrived, with the lookups built on them.
#[derive(Debug, Default)]
pub struct TerrainCache {
    tiles: HashMap<TileId, Arc<TerrainTile>>,
    /// Bit `z` is set when some tile of level `z` is resident, so a lookup
    /// skips levels that cannot answer.
    levels: u16,
    generation: u64,
    /// Recent arrivals `(generation, tile)`, oldest first, so a mesh can tell
    /// whether anything that arrived since it was built concerns it.
    arrivals: VecDeque<(u64, TileId)>,
    /// Generation of the last change that concerns every mesh (terrain on or
    /// off, cleared, or an arrival forgotten from the log).
    floor: u64,
}

/// Arrivals remembered; a mesh older than that is rebuilt unconditionally.
const ARRIVAL_LOG: usize = 2048;

impl TerrainCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Bumped whenever the answers of [`Self::height_at`] may have changed.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Marks the answers as changed without touching the tiles (terrain turned
    /// on or off).
    pub fn touch(&mut self) {
        self.generation += 1;
        self.floor = self.generation;
    }

    /// Whether anything that arrived after `generation` can change the heights
    /// a mesh of `mesh` samples: a tile no deeper than the mesh's own sampling
    /// level, near enough to it to be interpolated into its edge.
    pub fn changed_since(&self, generation: u64, mesh: TileId) -> bool {
        if generation < self.floor {
            return true;
        }
        let level = mesh.z.min(MAX_TERRAIN_LEVEL);
        let area = mesh.bounds();
        self.arrivals
            .iter()
            .rev()
            .take_while(|(arrived_at, _)| *arrived_at > generation)
            .any(|(_, tile)| {
                if tile.z > level {
                    return false;
                }
                // Two pixels of margin: bilinear taps reach into neighbours.
                let other = tile.bounds();
                let margin_lon = other.width_deg() * 2.0 / f64::from(TERRAIN_TILE_SIZE);
                let margin_lat = other.height_deg() * 2.0 / f64::from(TERRAIN_TILE_SIZE);
                other.west - margin_lon < area.east
                    && other.east + margin_lon > area.west
                    && other.south - margin_lat < area.north
                    && other.north + margin_lat > area.south
            })
    }

    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    pub fn contains(&self, id: TileId) -> bool {
        self.tiles.contains_key(&id)
    }

    /// Tiles deeper than [`MAX_TERRAIN_LEVEL`] are ignored.
    pub fn insert(&mut self, id: TileId, tile: TerrainTile) {
        if id.z > MAX_TERRAIN_LEVEL {
            return;
        }
        self.levels |= 1 << id.z;
        self.tiles.insert(id, Arc::new(tile));
        self.generation += 1;
        self.arrivals.push_back((self.generation, id));
        if self.arrivals.len() > ARRIVAL_LOG {
            if let Some((forgotten, _)) = self.arrivals.pop_front() {
                self.floor = self.floor.max(forgotten);
            }
        }
    }

    pub fn clear(&mut self) {
        self.tiles.clear();
        self.levels = 0;
        self.generation += 1;
        self.floor = self.generation;
    }

    /// Drops tiles outside `keep` once the cache is over its budget. Heights
    /// the view still needs are in `keep`, so nothing visible changes.
    pub fn prune(&mut self, keep: &HashSet<TileId>) {
        if self.tiles.len() <= MAX_RESIDENT_TILES {
            return;
        }
        self.tiles.retain(|id, _| keep.contains(id));
        self.levels = self.tiles.keys().fold(0, |mask, id| mask | (1 << id.z));
    }

    /// Height in metres at a point from the deepest tile that covers it.
    /// `None` when nothing covering it has loaded, or beyond Web-Mercator's
    /// latitude limit where no elevation tile exists.
    pub fn height_at(&self, lon_deg: f64, lat_deg: f64) -> Option<f64> {
        self.height_at_level(lon_deg, lat_deg, MAX_TERRAIN_LEVEL)
    }

    /// As [`Self::height_at`], but never from a tile deeper than `max_level`.
    /// Coarse meshes sample coarse tiles, which keeps them from aliasing the
    /// detail of the deepest level.
    pub fn height_at_level(&self, lon_deg: f64, lat_deg: f64, max_level: u8) -> Option<f64> {
        let (u, v) = (mercator_u(lon_deg)?, mercator_v(lat_deg)?);
        self.deepest_sample(
            &|id| self.tiles.get(&id).map(|tile| &**tile),
            u,
            v,
            max_level,
        )
    }

    /// [`Self::height_at_level`] at every node of a longitude/latitude grid,
    /// written row-major (one row per latitude) into `out`, with 0 where
    /// nothing answers. The answers are the same; a tile mesh asks for
    /// thousands of nodes that mostly fall in one elevation tile, so the
    /// projection is done once per row and column and the tile lookups are
    /// remembered between nodes instead of being hashed again for each.
    pub fn heights_at_level(
        &self,
        lons_deg: &[f64],
        lats_deg: &[f64],
        max_level: u8,
        out: &mut [f64],
    ) {
        let memo = TileMemo::new(&self.tiles);
        let get = |id: TileId| memo.get(id);
        let top = max_level.min(MAX_TERRAIN_LEVEL);
        let top_resident = self.levels & (1 << top) != 0;
        // A mesh samples its own level, which is the deepest one here as soon
        // as its elevation tile has arrived, so the pixel positions at that
        // level are worked out once per column and row.
        let columns: Vec<Option<(f64, Axis)>> = lons_deg
            .iter()
            .map(|lon| mercator_u(*lon).map(|u| (u, Axis::at(top, u))))
            .collect();
        let mut nodes = out.iter_mut();
        for lat in lats_deg {
            let row = mercator_v(*lat).map(|v| (v, Axis::at(top, v)));
            for (column, node) in columns.iter().zip(&mut nodes) {
                *node = column
                    .zip(row)
                    .and_then(|((u, column), (v, row))| {
                        top_resident
                            .then(|| sample(&get, top, column, row))
                            .flatten()
                            .or_else(|| self.deepest_sample(&get, u, v, top.checked_sub(1)?))
                    })
                    .unwrap_or(0.0);
            }
        }
    }

    /// The sample from the deepest resident level, no deeper than `max_level`,
    /// that covers the point.
    fn deepest_sample<'a>(
        &self,
        get: &impl Fn(TileId) -> Option<&'a TerrainTile>,
        u: f64,
        v: f64,
        max_level: u8,
    ) -> Option<f64> {
        (0..=max_level.min(MAX_TERRAIN_LEVEL))
            .rev()
            .filter(|level| self.levels & (1 << level) != 0)
            .find_map(|level| sample(get, level, Axis::at(level, u), Axis::at(level, v)))
    }

    /// Lowest and highest sample of the nearest resident tile that covers
    /// `id` (the tile itself, or an ancestor, which bounds it from outside).
    /// Used for bounding volumes, so it only ever errs on the wide side.
    pub fn relief(&self, id: TileId) -> Option<(f64, f64)> {
        let mut current = Some(TileId {
            z: id.z.min(MAX_TERRAIN_LEVEL),
            x: id.x >> id.z.saturating_sub(MAX_TERRAIN_LEVEL),
            y: id.y >> id.z.saturating_sub(MAX_TERRAIN_LEVEL),
        });
        while let Some(candidate) = current {
            if let Some(tile) = self.tiles.get(&candidate) {
                return Some((f64::from(tile.min), f64::from(tile.max)));
            }
            current = candidate.parent();
        }
        None
    }
}

/// Horizontal Web-Mercator coordinate of a longitude, wrapped into the world;
/// `None` for garbage.
fn mercator_u(lon_deg: f64) -> Option<f64> {
    lon_deg
        .is_finite()
        .then(|| lon_lat_to_mercator_uv((lon_deg + 180.0).rem_euclid(360.0) - 180.0, 0.0).0)
}

/// Vertical Web-Mercator coordinate of a latitude; `None` for garbage and
/// beyond the latitude limit, where no elevation tile exists.
fn mercator_v(lat_deg: f64) -> Option<f64> {
    (lat_deg.is_finite() && lat_deg.abs() <= MERCATOR_MAX_LAT_DEG)
        .then(|| lon_lat_to_mercator_uv(0.0, lat_deg).1)
}

/// Remembers the last tile looked up at each level. Neighbouring samples
/// nearly always land in the same tile, so a run of them hashes once.
struct TileMemo<'a> {
    tiles: &'a HashMap<TileId, Arc<TerrainTile>>,
    last: [Cell<Option<Remembered<'a>>>; MAX_TERRAIN_LEVEL as usize + 1],
}

/// A tile address and what the cache holds for it.
type Remembered<'a> = (TileId, Option<&'a TerrainTile>);

impl<'a> TileMemo<'a> {
    fn new(tiles: &'a HashMap<TileId, Arc<TerrainTile>>) -> Self {
        Self {
            tiles,
            last: Default::default(),
        }
    }

    fn get(&self, id: TileId) -> Option<&'a TerrainTile> {
        let slot = self.last.get(usize::from(id.z))?;
        match slot.get() {
            Some((known, tile)) if known == id => tile,
            _ => {
                let tile = self.tiles.get(&id).map(|tile| &**tile);
                slot.set(Some((id, tile)));
                tile
            }
        }
    }
}

/// Where a Web-Mercator coordinate falls in the pixel grid of one level,
/// along one axis.
#[derive(Clone, Copy)]
struct Axis {
    /// Index of the tile under the coordinate.
    home: u64,
    /// Global index of the first of the two pixels interpolated between; -1
    /// in the half pixel before the first one.
    first: i64,
    /// Weight of the second pixel.
    blend: f64,
}

impl Axis {
    fn at(level: u8, coordinate: f64) -> Self {
        let tiles_per_side = 1u64 << level;
        let size = u64::from(TERRAIN_TILE_SIZE);
        let global = coordinate * (tiles_per_side * size) as f64;
        let position = global - 0.5;
        Self {
            home: (global / size as f64)
                .floor()
                .clamp(0.0, (tiles_per_side - 1) as f64) as u64,
            first: position.floor() as i64,
            blend: position - position.floor(),
        }
    }

    /// Offset of `first` inside the home tile, when both pixels are in it.
    fn inside_home(self) -> Option<usize> {
        let offset = self.first - (self.home * u64::from(TERRAIN_TILE_SIZE)) as i64;
        (0..i64::from(TERRAIN_TILE_SIZE) - 1)
            .contains(&offset)
            .then_some(offset as usize)
    }
}

/// Bilinear sample at one level, or `None` when the tile under the point
/// is not resident. Neighbouring tiles supply the interpolation taps
/// across a tile edge; where the neighbour is missing the edge sample is
/// repeated.
fn sample<'a>(
    get: &impl Fn(TileId) -> Option<&'a TerrainTile>,
    level: u8,
    column: Axis,
    row: Axis,
) -> Option<f64> {
    let size = u64::from(TERRAIN_TILE_SIZE);
    let pixels = (1u64 << level) * size;
    let (x0, y0) = (column.first, row.first);
    let (fx, fy) = (column.blend, row.blend);
    let tile_at = |tx: u64, ty: u64| {
        Some(TileId {
            z: level,
            x: u32::try_from(tx).ok()?,
            y: u32::try_from(ty).ok()?,
        })
    };
    let (mut home_x, mut home_y) = (column.home, row.home);
    let resident = tile_at(home_x, home_y).and_then(get);
    // Away from the tile's border all four taps are its own samples; this is
    // nearly every lookup, so it skips the neighbour search below.
    if let (Some(home), Some(left), Some(top)) = (resident, column.inside_home(), row.inside_home())
    {
        let upper = top * TERRAIN_TILE_SIZE as usize + left;
        let lower = upper + TERRAIN_TILE_SIZE as usize;
        if let (Some(upper), Some(lower)) = (
            home.heights.get(upper..upper + 2),
            home.heights.get(lower..lower + 2),
        ) {
            let top = f64::from(upper[0]) * (1.0 - fx) + f64::from(upper[1]) * fx;
            let bottom = f64::from(lower[0]) * (1.0 - fx) + f64::from(lower[1]) * fx;
            return Some(top * (1.0 - fy) + bottom * fy);
        }
    }
    // The tile the taps fall back on when a neighbour is missing. Normally
    // the one under the point; but a point on the boundary between two
    // tiles belongs to either, and when only one is loaded it has to be
    // that one. Otherwise every vertex on the shared edge would fall back
    // to a coarser level than the vertices beside it, which is a ridge one
    // cell wide along the whole edge of the tile.
    if resident.is_none() {
        let across = |offset: i64| (x0 + offset).rem_euclid(pixels as i64) as u64 / size;
        let down = |offset: i64| (y0 + offset).clamp(0, pixels as i64 - 1) as u64 / size;
        let (lo_x, hi_x, lo_y, hi_y) = (across(0), across(1), down(0), down(1));
        let mut found = None;
        for ty in [lo_y, hi_y] {
            for tx in [lo_x, hi_x] {
                if tile_at(tx, ty).is_some_and(|id| get(id).is_some()) {
                    found.get_or_insert((tx, ty));
                }
            }
        }
        (home_x, home_y) = found?;
    }
    let home = get(tile_at(home_x, home_y)?)?;
    let tap = |dx: i64, dy: i64| -> f64 {
        let px = (x0 + dx).rem_euclid(pixels as i64) as u64;
        let py = (y0 + dy).clamp(0, pixels as i64 - 1) as u64;
        let neighbour = u32::try_from(px / size)
            .ok()
            .zip(u32::try_from(py / size).ok())
            .and_then(|(tx, ty)| {
                get(TileId {
                    z: level,
                    x: tx,
                    y: ty,
                })
            });
        let (tile, local_x, local_y) = match neighbour {
            Some(tile) => (tile, px % size, py % size),
            None => (
                home,
                px.clamp(home_x * size, home_x * size + size - 1) - home_x * size,
                py.clamp(home_y * size, home_y * size + size - 1) - home_y * size,
            ),
        };
        f64::from(tile.at(local_x as u32, local_y as u32))
    };
    let top = tap(0, 0) * (1.0 - fx) + tap(1, 0) * fx;
    let bottom = tap(0, 1) * (1.0 - fx) + tap(1, 1) * fx;
    Some(top * (1.0 - fy) + bottom * fy)
}

/// The terrain model of a globe: whether it is on, where its tiles come from,
/// and what has arrived.
pub struct Terrain {
    enabled: bool,
    provider: TileProvider,
    fetcher: TileFetcher,
    cache: TerrainCache,
}

impl Terrain {
    /// Disabled until [`Self::set_enabled`], so a bare globe stays on the
    /// ellipsoid and never touches the network.
    pub fn new(source: TileSource) -> Self {
        Self {
            enabled: false,
            provider: TileProvider::new(source),
            fetcher: TileFetcher::new_raw(TerrainTile::check_rgba),
            cache: TerrainCache::new(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        if enabled != self.enabled {
            self.enabled = enabled;
            self.cache.touch();
        }
    }

    pub fn set_cache(&mut self, cache: Arc<dyn TileCache>) {
        self.fetcher.set_cache(cache);
    }

    /// Switches the elevation source. Heights from the old one are dropped.
    pub fn set_source(&mut self, source: TileSource) {
        if source.id != self.provider.source.id {
            self.cache.clear();
        }
        self.provider = TileProvider::new(source);
    }

    pub fn credit(&self) -> &str {
        self.provider.credit()
    }

    pub fn cache(&self) -> &TerrainCache {
        &self.cache
    }

    pub fn cache_mut(&mut self) -> &mut TerrainCache {
        &mut self.cache
    }

    pub fn generation(&self) -> u64 {
        self.cache.generation()
    }

    pub fn in_flight(&self) -> usize {
        self.fetcher.in_flight_count()
    }

    pub fn queued(&self) -> usize {
        self.fetcher.queued_count()
    }

    /// Ground height for markers, lines, picking and the camera: the model
    /// height, but never below sea level. Elevation data carries bathymetry,
    /// and the sea surface, not the sea floor, is what imagery shows.
    pub fn ground(&self, lon_deg: f64, lat_deg: f64) -> Option<f64> {
        self.enabled
            .then(|| self.cache.height_at(lon_deg, lat_deg))
            .flatten()
            .map(|height| height.max(0.0))
    }

    /// [`Self::ground`], but sampled no deeper than `max_level`; what a mesh
    /// of that level of detail is built from.
    pub fn ground_at_level(&self, lon_deg: f64, lat_deg: f64, max_level: u8) -> Option<f64> {
        self.enabled
            .then(|| self.cache.height_at_level(lon_deg, lat_deg, max_level))
            .flatten()
            .map(|height| height.max(0.0))
    }

    /// [`Self::ground_at_level`] at every node of a grid (see
    /// [`TerrainCache::heights_at_level`]), with 0 where it has no answer.
    pub fn ground_grid_at_level(
        &self,
        lons_deg: &[f64],
        lats_deg: &[f64],
        max_level: u8,
        out: &mut [f64],
    ) {
        if !self.enabled {
            out.fill(0.0);
            return;
        }
        self.cache
            .heights_at_level(lons_deg, lats_deg, max_level, out);
        for height in out {
            *height = height.max(0.0);
        }
    }

    /// Takes in tiles that finished downloading. Returns whether any arrived.
    pub fn poll(&mut self) -> bool {
        if !self.enabled {
            return false;
        }
        let mut changed = false;
        for FetchedTile { id, result } in self.fetcher.poll(&self.provider) {
            let tile = result.and_then(|pixels| {
                let image = pixels.image().ok_or("elevation tile has no pixels")?;
                TerrainTile::from_rgba(image)
            });
            match tile {
                Ok(tile) => {
                    self.cache.insert(id, tile);
                    changed = true;
                }
                Err(error) => tracing::warn!(tile = %id, %error, "elevation tile rejected"),
            }
        }
        changed
    }

    /// Queues the elevation tiles the visible imagery tiles need, coarse
    /// ancestors first when nothing covers an area yet, and trims the cache.
    pub fn request(&mut self, visible: impl Iterator<Item = TileId>) {
        if !self.enabled {
            return;
        }
        let mut wanted: Vec<TileId> = visible.map(elevation_tile_for).collect();
        wanted.sort_unstable();
        wanted.dedup();

        let mut desired: Vec<(TileId, u8)> = Vec::new();
        let mut keep: HashSet<TileId> = HashSet::new();
        let visible_tiles = wanted.clone();
        for id in wanted {
            let mut chain = vec![id];
            while let Some(parent) = chain.last().and_then(|last| last.parent()) {
                chain.push(parent);
            }
            let covered = chain.iter().any(|ancestor| self.cache.contains(*ancestor));
            keep.extend(chain.iter().copied());
            if self.cache.contains(id) {
                continue;
            }
            if covered {
                desired.push((id, id.z));
            } else {
                desired.extend(chain.iter().rev().map(|ancestor| (*ancestor, ancestor.z)));
            }
        }
        // Interpolating near a tile's edge reads its neighbour. Asking for the
        // ring around the visible tiles, after everything the view needs, keeps
        // the vertices along an edge from being sampled at a coarser level than
        // the ones beside them while the neighbour is missing.
        let mut ring: Vec<TileId> = Vec::new();
        for id in visible_tiles.iter().filter(|id| id.z >= 1) {
            let count = 1i64 << id.z;
            // Edge neighbours only: a corner reads one tap from the diagonal
            // tile, which the clamped fallback covers.
            for (dx, dy) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                let y = i64::from(id.y) + dy;
                if !(0..count).contains(&y) {
                    continue;
                }
                let x = (i64::from(id.x) + dx).rem_euclid(count);
                if let Some(neighbour) = TileId::new(id.z, x as u32, y as u32) {
                    if !keep.contains(&neighbour) && !self.cache.contains(neighbour) {
                        ring.push(neighbour);
                    }
                }
            }
        }
        ring.sort_unstable();
        ring.dedup();
        keep.extend(ring.iter().copied());
        desired.extend(ring.into_iter().map(|id| (id, id.z.saturating_add(1))));
        self.cache.prune(&keep);
        self.fetcher.set_desired(&self.provider, desired);
    }
}

/// The elevation tile whose pixels cover a visible imagery tile: the same
/// address up to level 15, the level-15 ancestor beyond.
pub fn elevation_tile_for(imagery: TileId) -> TileId {
    let drop = imagery.z.saturating_sub(MAX_TERRAIN_LEVEL);
    TileId {
        z: imagery.z - drop,
        x: imagery.x >> drop,
        y: imagery.y >> drop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile_id(z: u8, x: u32, y: u32) -> TileId {
        TileId::new(z, x, y).expect("valid tile")
    }

    fn flat_tile(height: f32) -> TerrainTile {
        TerrainTile::from_heights(vec![height; SAMPLES]).expect("valid tile")
    }

    /// Height ramps with the pixel column: `column` metres.
    fn column_ramp() -> TerrainTile {
        let heights = (0..SAMPLES)
            .map(|index| (index % TERRAIN_TILE_SIZE as usize) as f32)
            .collect();
        TerrainTile::from_heights(heights).expect("valid tile")
    }

    fn png_of(width: u32, height: u32, pixel: [u8; 4]) -> Vec<u8> {
        terra_testkit::rgba_to_png(&pixel.repeat((width * height) as usize), width, height)
    }

    #[test]
    fn terrarium_known_pixels() {
        assert_eq!(TerrainTile::terrarium_height(128, 0, 0), 0.0);
        assert_eq!(TerrainTile::terrarium_height(0, 0, 0), -32768.0);
        // 305 m = 128 * 256 + 49 (305 = 256 + 49): R = 129, G = 49.
        assert_eq!(TerrainTile::terrarium_height(129, 49, 0), 305.0);
        assert_eq!(TerrainTile::terrarium_height(128, 0, 128), 0.5);
        assert_eq!(TerrainTile::terrarium_height(127, 255, 0), -1.0);
    }

    #[test]
    fn decoding_a_png_reads_every_pixel() {
        let sea = TerrainTile::decode(&png_of(256, 256, [128, 0, 0, 255])).expect("decodes");
        assert_eq!((sea.min(), sea.max()), (0.0, 0.0));
        let hill = TerrainTile::decode(&png_of(256, 256, [129, 49, 0, 255])).expect("decodes");
        assert_eq!((hill.min(), hill.max()), (305.0, 305.0));
        assert_eq!(hill.at(255, 255), 305.0);
    }

    #[test]
    fn wrong_size_and_corrupt_data_are_rejected() {
        assert!(TerrainTile::decode(&png_of(128, 128, [128, 0, 0, 255])).is_err());
        assert!(TerrainTile::decode(&png_of(256, 255, [128, 0, 0, 255])).is_err());
        assert!(TerrainTile::decode(b"not a png").is_err());
        assert!(TerrainTile::decode(&[]).is_err());
        let mut truncated = png_of(256, 256, [128, 0, 0, 255]);
        truncated.truncate(30);
        assert!(TerrainTile::decode(&truncated).is_err());
        assert!(TerrainTile::from_heights(vec![0.0; 10]).is_err());
        let mut poisoned = vec![0.0; SAMPLES];
        poisoned[7] = f32::NAN;
        assert!(TerrainTile::from_heights(poisoned).is_err());
    }

    #[test]
    fn nothing_loaded_means_no_height() {
        let cache = TerrainCache::new();
        assert_eq!(cache.height_at(10.0, 40.0), None);
        assert_eq!(cache.relief(tile_id(5, 3, 3)), None);
    }

    #[test]
    fn a_flat_tile_answers_everywhere_inside_it_and_nowhere_outside() {
        let mut cache = TerrainCache::new();
        let id = tile_id(4, 8, 5);
        cache.insert(id, flat_tile(250.0));
        let bounds = id.bounds();
        let inside = cache.height_at(
            (bounds.west + bounds.east) / 2.0,
            (bounds.south + bounds.north) / 2.0,
        );
        assert_eq!(inside, Some(250.0));
        assert_eq!(cache.height_at(bounds.east + 1.0, bounds.south + 0.1), None);
        assert_eq!(cache.height_at(bounds.west + 0.1, bounds.north + 5.0), None);
    }

    #[test]
    fn heights_interpolate_bilinearly_inside_a_tile() {
        let mut cache = TerrainCache::new();
        let id = tile_id(0, 0, 0);
        cache.insert(id, column_ramp());
        // Pixel centres sit at (column + 0.5) / 256 of the world width.
        let lon_of = |column: f64| (column + 0.5) / 256.0 * 360.0 - 180.0;
        let at = |column: f64| cache.height_at(lon_of(column), 0.0).expect("loaded");
        assert!((at(100.0) - 100.0).abs() < 1e-3);
        assert!(
            (at(100.5) - 100.5).abs() < 1e-3,
            "halfway between two pixels"
        );
        assert!((at(100.25) - 100.25).abs() < 1e-3);
    }

    #[test]
    fn interpolation_reaches_across_a_tile_edge() {
        let mut cache = TerrainCache::new();
        cache.insert(tile_id(1, 0, 0), flat_tile(100.0));
        cache.insert(tile_id(1, 1, 0), flat_tile(300.0));
        // Exactly on the shared edge (lon 0) the taps are the last column of
        // the west tile and the first of the east tile.
        let edge = cache.height_at(0.0, 40.0).expect("loaded");
        assert!((edge - 200.0).abs() < 1e-3, "edge value {edge}");
        // A quarter pixel into the east tile is still blended with the west.
        let pixel = 360.0 / 512.0;
        let near = cache.height_at(pixel * 0.25, 40.0).expect("loaded");
        assert!(near > 200.0 && near < 300.0, "near-edge value {near}");
    }

    #[test]
    fn a_point_on_a_boundary_uses_the_tile_that_is_loaded() {
        let mut cache = TerrainCache::new();
        cache.insert(tile_id(0, 0, 0), flat_tile(5.0));
        cache.insert(tile_id(1, 0, 0), flat_tile(100.0));
        // Longitude 0 is the boundary between (1, 0, 0) and the missing
        // (1, 1, 0). It must not fall back to the coarse tile: that would make
        // every vertex along the edge sit at another level than its neighbours.
        assert_eq!(cache.height_at(0.0, 40.0), Some(100.0));
        assert_eq!(cache.height_at(-0.0001, 40.0), Some(100.0));
        // Far inside the missing tile the fallback is still the ancestor.
        assert_eq!(cache.height_at(90.0, 40.0), Some(5.0));
        // And with the east tile loaded instead, the same holds mirrored.
        let mut east = TerrainCache::new();
        east.insert(tile_id(0, 0, 0), flat_tile(5.0));
        east.insert(tile_id(1, 1, 0), flat_tile(300.0));
        assert_eq!(east.height_at(0.0, 40.0), Some(300.0));
        assert_eq!(east.height_at(-90.0, 40.0), Some(5.0));
    }

    #[test]
    fn a_missing_neighbour_repeats_the_edge_instead_of_inventing_data() {
        let mut cache = TerrainCache::new();
        cache.insert(tile_id(1, 1, 0), flat_tile(300.0));
        let edge = cache.height_at(0.0, 40.0).expect("home tile is loaded");
        assert!((edge - 300.0).abs() < 1e-3, "{edge}");
    }

    #[test]
    fn longitude_wraps_across_the_antimeridian() {
        let mut cache = TerrainCache::new();
        cache.insert(tile_id(1, 0, 0), flat_tile(100.0));
        cache.insert(tile_id(1, 1, 0), flat_tile(300.0));
        let wrapped = cache.height_at(180.0, 40.0).expect("loaded");
        assert!((wrapped - 200.0).abs() < 1e-3, "{wrapped}");
        assert_eq!(cache.height_at(190.0, 40.0), cache.height_at(-170.0, 40.0));
    }

    #[test]
    fn the_deepest_loaded_tile_wins_and_ancestors_fill_the_rest() {
        let mut cache = TerrainCache::new();
        cache.insert(tile_id(2, 2, 1), flat_tile(10.0));
        let deep = tile_id(4, 8, 5);
        cache.insert(deep, flat_tile(900.0));
        let bounds = deep.bounds();
        let in_deep = (
            (bounds.west + bounds.east) / 2.0,
            (bounds.south + bounds.north) / 2.0,
        );
        assert_eq!(cache.height_at(in_deep.0, in_deep.1), Some(900.0));
        // Same level-2 tile, outside the level-4 one: the ancestor answers.
        assert_eq!(cache.height_at(in_deep.0 + 12.0, in_deep.1), Some(10.0));
        // Capping the level skips the deep tile.
        assert_eq!(cache.height_at_level(in_deep.0, in_deep.1, 3), Some(10.0));
        assert_eq!(cache.height_at_level(in_deep.0, in_deep.1, 4), Some(900.0));
    }

    /// Heights that differ at every pixel and between tiles.
    fn bumpy(seed: f32) -> TerrainTile {
        let heights = (0..SAMPLES)
            .map(|index| ((index as f32 * 0.37 + seed).sin() * 500.0) + seed)
            .collect();
        TerrainTile::from_heights(heights).expect("valid tile")
    }

    #[test]
    fn a_grid_of_heights_is_the_same_as_asking_point_by_point() {
        let mut cache = TerrainCache::new();
        // A coarse tile, two of its children (so a neighbour is missing next
        // to a loaded one), and one grandchild.
        cache.insert(tile_id(3, 4, 2), bumpy(100.0));
        cache.insert(tile_id(4, 8, 4), bumpy(200.0));
        cache.insert(tile_id(4, 9, 5), bumpy(300.0));
        cache.insert(tile_id(5, 16, 8), bumpy(400.0));
        let area = tile_id(3, 4, 2).bounds();
        // Past every edge of the loaded area, and through the tile boundaries.
        let lons: Vec<f64> = (0..=90)
            .map(|step| area.west - 3.0 + (area.width_deg() + 6.0) * f64::from(step) / 90.0)
            .chain([f64::NAN, 180.0, -180.0, 540.0])
            .collect();
        let lats: Vec<f64> = (0..=70)
            .map(|step| area.south - 3.0 + (area.height_deg() + 6.0) * f64::from(step) / 70.0)
            .chain([f64::INFINITY, 89.0, -89.0])
            .collect();
        for max_level in [0, 3, 4, 5, MAX_TERRAIN_LEVEL] {
            let mut grid = vec![f64::NAN; lons.len() * lats.len()];
            cache.heights_at_level(&lons, &lats, max_level, &mut grid);
            let mut answered = 0;
            for (row, lat) in lats.iter().enumerate() {
                for (column, lon) in lons.iter().enumerate() {
                    let single = cache.height_at_level(*lon, *lat, max_level);
                    answered += usize::from(single.is_some());
                    assert_eq!(
                        grid[row * lons.len() + column],
                        single.unwrap_or(0.0),
                        "level {max_level} at ({lon}, {lat})"
                    );
                }
            }
            assert_eq!(answered > 0, max_level >= 3, "level {max_level}");
        }
    }

    #[test]
    fn a_ground_grid_is_flat_while_terrain_is_off_and_never_below_the_sea() {
        let mut terrain = Terrain::new(TileSource::terrain_terrarium());
        terrain.cache_mut().insert(tile_id(0, 0, 0), bumpy(0.0));
        let (lons, lats) = ([-20.0, 0.0, 20.0], [-10.0, 10.0]);
        let mut grid = [f64::NAN; 6];
        terrain.ground_grid_at_level(&lons, &lats, 0, &mut grid);
        assert_eq!(grid, [0.0; 6]);

        terrain.set_enabled(true);
        terrain.ground_grid_at_level(&lons, &lats, 0, &mut grid);
        for (row, lat) in lats.iter().enumerate() {
            for (column, lon) in lons.iter().enumerate() {
                let single = terrain.ground_at_level(*lon, *lat, 0).unwrap_or(0.0);
                assert_eq!(grid[row * 3 + column], single);
                assert!(single >= 0.0);
            }
        }
        assert!(grid.iter().any(|height| *height > 0.0));
    }

    #[test]
    fn nothing_is_answered_beyond_the_mercator_limit_or_for_garbage() {
        let mut cache = TerrainCache::new();
        cache.insert(tile_id(0, 0, 0), flat_tile(50.0));
        assert_eq!(cache.height_at(0.0, 85.0), Some(50.0));
        assert_eq!(cache.height_at(0.0, 85.06), None);
        assert_eq!(cache.height_at(0.0, -89.9), None);
        assert_eq!(cache.height_at(f64::NAN, 10.0), None);
        assert_eq!(cache.height_at(10.0, f64::INFINITY), None);
    }

    #[test]
    fn tiles_deeper_than_level_fifteen_are_ignored() {
        let mut cache = TerrainCache::new();
        cache.insert(tile_id(16, 1, 1), flat_tile(1.0));
        assert!(cache.is_empty());
        assert_eq!(cache.generation(), 0);
    }

    #[test]
    fn generation_counts_arrivals_and_toggles() {
        let mut cache = TerrainCache::new();
        let before = cache.generation();
        cache.insert(tile_id(1, 0, 0), flat_tile(1.0));
        assert!(cache.generation() > before);
        let after = cache.generation();
        cache.touch();
        assert!(cache.generation() > after);
    }

    #[test]
    fn an_arrival_only_concerns_meshes_it_can_change() {
        let mut cache = TerrainCache::new();
        let before = cache.generation();
        let arrived = tile_id(8, 136, 90);
        cache.insert(arrived, flat_tile(10.0));
        // The tile's own mesh and meshes inside it sample it.
        assert!(cache.changed_since(before, arrived));
        assert!(cache.changed_since(before, tile_id(10, 544, 360)));
        // A coarser mesh never samples a deeper tile, and far away is unaffected.
        assert!(!cache.changed_since(before, tile_id(5, 17, 11)));
        assert!(!cache.changed_since(before, tile_id(8, 20, 90)));
        assert!(!cache.changed_since(before, tile_id(8, 200, 40)));
        // A deeper arrival is not sampled by a coarser mesh.
        let mut deep = TerrainCache::new();
        let start = deep.generation();
        deep.insert(tile_id(10, 544, 360), flat_tile(5.0));
        assert!(!deep.changed_since(start, tile_id(8, 136, 90)));
        assert!(deep.changed_since(start, tile_id(10, 544, 360)));
        // Meshes built after the arrival have nothing to catch up on.
        assert!(!cache.changed_since(cache.generation(), arrived));
    }

    #[test]
    fn switching_terrain_or_clearing_concerns_every_mesh() {
        let mut cache = TerrainCache::new();
        let before = cache.generation();
        cache.touch();
        assert!(cache.changed_since(before, tile_id(12, 1, 1)));
        let after = cache.generation();
        assert!(!cache.changed_since(after, tile_id(12, 1, 1)));
        cache.clear();
        assert!(cache.changed_since(after, tile_id(12, 1, 1)));
    }

    #[test]
    fn relief_comes_from_the_tile_or_its_nearest_ancestor() {
        let mut cache = TerrainCache::new();
        let mut heights = vec![100.0; SAMPLES];
        heights[0] = -20.0;
        heights[1] = 2_400.0;
        cache.insert(
            tile_id(5, 3, 3),
            TerrainTile::from_heights(heights).expect("tile"),
        );
        assert_eq!(cache.relief(tile_id(5, 3, 3)), Some((-20.0, 2_400.0)));
        assert_eq!(cache.relief(tile_id(8, 24, 24)), Some((-20.0, 2_400.0)));
        assert_eq!(cache.relief(tile_id(5, 4, 3)), None);
        // Imagery deeper than the elevation data maps onto the level-15 tile.
        let mut deep = TerrainCache::new();
        deep.insert(tile_id(15, 100, 200), flat_tile(77.0));
        assert_eq!(deep.relief(tile_id(18, 800, 1_600)), Some((77.0, 77.0)));
    }

    #[test]
    fn elevation_tiles_cap_at_level_fifteen() {
        assert_eq!(elevation_tile_for(tile_id(6, 5, 9)), tile_id(6, 5, 9));
        assert_eq!(elevation_tile_for(tile_id(17, 9, 13)), tile_id(15, 2, 3));
    }

    #[test]
    fn synthetic_tiles_sample_the_function_at_pixel_centres() {
        let id = tile_id(3, 4, 2);
        let tile = TerrainTile::synthetic(id, |lon, _| lon);
        let mut cache = TerrainCache::new();
        cache.insert(id, tile);
        let bounds = id.bounds();
        let middle = (bounds.west + bounds.east) / 2.0;
        let height = cache
            .height_at(middle, (bounds.south + bounds.north) / 2.0)
            .expect("loaded");
        assert!((height - middle).abs() < 0.5, "{height} vs {middle}");
    }

    #[test]
    fn ground_is_off_until_enabled_and_never_below_the_sea() {
        let mut terrain = Terrain::new(TileSource::terrain_terrarium());
        terrain
            .cache_mut()
            .insert(tile_id(0, 0, 0), flat_tile(-400.0));
        assert_eq!(terrain.ground(0.0, 0.0), None, "disabled");
        terrain.set_enabled(true);
        assert_eq!(
            terrain.ground(0.0, 0.0),
            Some(0.0),
            "sea floor is not drawn"
        );
        terrain
            .cache_mut()
            .insert(tile_id(1, 0, 0), flat_tile(1_200.0));
        assert_eq!(terrain.ground(-10.0, 10.0), Some(1_200.0));
    }

    #[test]
    fn toggling_terrain_bumps_the_generation_once_per_change() {
        let mut terrain = Terrain::new(TileSource::terrain_terrarium());
        let start = terrain.generation();
        terrain.set_enabled(true);
        terrain.set_enabled(true);
        assert_eq!(terrain.generation(), start + 1);
        terrain.set_enabled(false);
        assert_eq!(terrain.generation(), start + 2);
    }

    #[test]
    fn requests_start_with_coarse_ancestors_and_skip_loaded_tiles() {
        let mut terrain = Terrain::new(TileSource {
            url: "http://127.0.0.1:1/{z}/{x}/{y}.png".to_string(),
            ..TileSource::terrain_terrarium()
        });
        terrain.set_enabled(true);
        terrain.request([tile_id(3, 1, 1)].into_iter());
        // z0..z3 chain: nothing was loaded, so everything is wanted, plus the
        // four edge neighbours of the visible tile, which edge interpolation reads.
        assert_eq!(terrain.in_flight() + terrain.queued(), 4 + 4);

        let mut loaded = Terrain::new(TileSource {
            url: "http://127.0.0.1:1/{z}/{x}/{y}.png".to_string(),
            ..TileSource::terrain_terrarium()
        });
        loaded.set_enabled(true);
        loaded.cache_mut().insert(tile_id(2, 0, 0), flat_tile(5.0));
        loaded.request([tile_id(3, 1, 1)].into_iter());
        assert_eq!(
            loaded.in_flight() + loaded.queued(),
            1 + 4,
            "the tile itself and its ring"
        );
    }

    #[test]
    fn a_disabled_terrain_never_requests_anything() {
        let mut terrain = Terrain::new(TileSource::terrain_terrarium());
        terrain.request([tile_id(3, 1, 1)].into_iter());
        assert_eq!(terrain.in_flight() + terrain.queued(), 0);
        assert!(!terrain.poll());
    }
}
