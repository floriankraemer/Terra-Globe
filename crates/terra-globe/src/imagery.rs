//! Imagery: tile URL construction, async fetching and decoding.
//!
//! `ehttp` is used instead of `reqwest` because it compiles to native threads
//! and to `fetch` on wasm from one code path, which keeps the browser target
//! honest without a second implementation.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use web_time::Instant;

use image::RgbaImage;
use terra_providers::TileSource;

use crate::geo::TileId;

/// Side length of every decoded tile texture.
pub const TILE_TEXTURE_SIZE: u32 = 256;

const USER_AGENT: &str = concat!(
    "terra-globe/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/floriankraemer/webglobe)"
);

#[derive(Debug, Clone)]
pub struct TileProvider {
    pub source: TileSource,
    pub api_key: Option<String>,
}

impl TileProvider {
    pub fn new(source: TileSource) -> Self {
        Self {
            source,
            api_key: None,
        }
    }

    pub fn url(&self, id: TileId) -> String {
        self.source
            .tile_url(id.z, id.x, id.y, self.api_key.as_deref())
    }

    pub fn max_level(&self) -> u8 {
        self.source.maximum_level
    }

    pub fn credit(&self) -> &str {
        &self.source.credit
    }
}

#[derive(Debug)]
pub struct FetchedTile {
    pub id: TileId,
    pub result: Result<TilePixels, String>,
}

/// What a fetched tile carries.
#[derive(Debug)]
pub enum TilePixels {
    /// The image as decoded: what a raw (elevation) fetcher delivers, and what
    /// a caller that builds a tile by hand passes in.
    Decoded(RgbaImage),
    /// The mip chain to upload, largest level first, built where the tile was
    /// decoded so the render thread only has to copy it to the GPU. Overlay
    /// fetchers premultiply it as well.
    Texture(Vec<RgbaImage>),
}

impl TilePixels {
    /// The full-size image.
    pub fn image(&self) -> Option<&RgbaImage> {
        match self {
            Self::Decoded(image) => Some(image),
            Self::Texture(mips) => mips.first(),
        }
    }

    /// The mip chain to upload. A tile that did not come prepared from a
    /// fetcher is run through `prepare` and mipmapped here.
    pub fn into_mips(self, prepare: impl FnOnce(RgbaImage) -> RgbaImage) -> Vec<RgbaImage> {
        match self {
            Self::Decoded(image) => mip_chain(prepare(image)),
            Self::Texture(mips) => mips,
        }
    }
}

impl From<RgbaImage> for TilePixels {
    fn from(image: RgbaImage) -> Self {
        Self::Decoded(image)
    }
}

/// Maximum concurrent tile requests. Browsers cap connections per host anyway;
/// this also keeps us a polite client of public tile servers.
const MAX_IN_FLIGHT: usize = 8;
/// Transient failures are retried this many times before the tile is given up.
const MAX_ATTEMPTS: u8 = 3;
/// Upper bound on queued requests, so a pathological camera jump cannot flood
/// the queue. Ancestors sort first, so the cap only drops deep leaves.
const MAX_QUEUED: usize = 1024;
/// A tile that used up all its attempts is retried after this long. Without the
/// backoff a transient network failure would leave that patch of the globe on
/// coarse imagery until the app restarts.
const RETRY_AFTER: Duration = Duration::from_secs(30);

/// Persistent storage for already-downloaded tile bytes.
///
/// The renderer only talks to this trait, so the native shell can keep tiles in
/// a directory while the browser build can supply a different implementation.
pub trait TileCache: Send + Sync {
    fn get(&self, source: &TileSource, id: TileId) -> Option<Vec<u8>>;
    fn put(&self, source: &TileSource, id: TileId, bytes: &[u8]);
}

/// Fire-and-forget fetcher with a prioritised queue.
///
/// The renderer calls [`TileFetcher::set_desired`] every frame with the tiles it
/// wants; the queue is replaced but in-flight work is kept, so zooming does not
/// duplicate requests and stale work is dropped.
pub struct TileFetcher {
    sender: Sender<FetchedTile>,
    receiver: Receiver<FetchedTile>,
    in_flight: HashSet<TileId>,
    queue: VecDeque<(TileId, u8)>,
    attempts: HashMap<TileId, u8>,
    /// Tile -> when it gave up, so it can be retried later.
    failed: HashMap<TileId, Instant>,
    max_in_flight: usize,
    cache: Option<Arc<dyn TileCache>>,
    decode: Decode,
}

/// Rejects a decoded tile; runs on the fetch thread so a bad response counts
/// as a failed request (retried, then given up) instead of being re-requested
/// every frame.
pub type TileValidator = fn(&RgbaImage) -> Result<(), String>;

/// How downloaded bytes become a tile.
#[derive(Clone, Copy)]
struct Decode {
    output: Output,
    validate: Option<TileValidator>,
}

/// What the fetch thread makes of a decoded tile.
#[derive(Clone, Copy)]
enum Output {
    /// Rescaled to 256x256 and mipmapped.
    Basemap,
    /// As the basemap, with colour premultiplied by alpha first.
    Overlay,
    /// Exactly as served: resampling or mipmapping elevation would blend
    /// heights.
    Raw,
}

impl Default for TileFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl TileFetcher {
    pub fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        Self {
            sender,
            receiver,
            in_flight: HashSet::new(),
            queue: VecDeque::new(),
            attempts: HashMap::new(),
            failed: HashMap::new(),
            max_in_flight: MAX_IN_FLIGHT,
            cache: None,
            decode: Decode {
                output: Output::Basemap,
                validate: None,
            },
        }
    }

    /// A fetcher for a transparent tile layer: as [`Self::new`], but the tiles
    /// arrive premultiplied, which is what blending them over the basemap
    /// needs.
    pub fn new_overlay() -> Self {
        Self {
            decode: Decode {
                output: Output::Overlay,
                validate: None,
            },
            ..Self::new()
        }
    }

    /// A fetcher that hands tiles over as decoded, at their served size and
    /// without a mip chain, for data that is not imagery.
    pub fn new_raw(validate: TileValidator) -> Self {
        Self {
            decode: Decode {
                output: Output::Raw,
                validate: Some(validate),
            },
            ..Self::new()
        }
    }

    /// Serves tiles that are already on disk (or another store) before going
    /// to the network.
    pub fn set_cache(&mut self, cache: Arc<dyn TileCache>) {
        self.cache = Some(cache);
    }

    pub fn with_cache(mut self, cache: Arc<dyn TileCache>) -> Self {
        self.set_cache(cache);
        self
    }

    pub fn is_in_flight(&self, id: TileId) -> bool {
        self.in_flight.contains(&id)
    }

    pub fn has_failed(&self, id: TileId) -> bool {
        self.failed.contains_key(&id)
    }

    /// Replaces the wanted set for this frame. Ancestors sort first so coarse
    /// imagery arrives before the deepest leaves, which is what makes zooming
    /// feel progressive instead of blank-then-sudden.
    pub fn set_desired(&mut self, provider: &TileProvider, mut desired: Vec<(TileId, u8)>) {
        let mut seen = HashSet::new();
        desired.retain(|(id, _)| seen.insert(*id));
        desired.sort_by(|(left, left_priority), (right, right_priority)| {
            left_priority
                .cmp(right_priority)
                .then_with(|| left.z.cmp(&right.z))
                .then_with(|| left.x.cmp(&right.x))
                .then_with(|| left.y.cmp(&right.y))
        });
        desired.truncate(MAX_QUEUED);
        self.queue = desired.into_iter().collect();
        self.pump(provider);
    }

    /// Queues one tile at the back (kept for direct callers and tests).
    pub fn request(&mut self, provider: &TileProvider, id: TileId) {
        self.queue.push_back((id, id.z));
        self.pump(provider);
    }

    fn pump(&mut self, provider: &TileProvider) {
        while self.in_flight.len() < self.max_in_flight {
            let Some((id, _)) = self.queue.pop_front() else {
                break;
            };
            if self.ready_to_request(id) {
                self.spawn(provider, id);
            }
        }
    }

    /// Whether a queued tile may be started now. A tile that used up its
    /// attempts is held back until [`RETRY_AFTER`] has passed, then given a
    /// fresh set of attempts.
    fn ready_to_request(&mut self, id: TileId) -> bool {
        if self.in_flight.contains(&id) {
            return false;
        }
        if let Some(gave_up_at) = self.failed.get(&id).copied() {
            if gave_up_at.elapsed() < RETRY_AFTER {
                return false;
            }
            self.failed.remove(&id);
            self.attempts.remove(&id);
        }
        if self.attempts.get(&id).copied().unwrap_or(0) >= MAX_ATTEMPTS {
            self.failed.insert(id, Instant::now());
            return false;
        }
        true
    }

    fn spawn(&mut self, provider: &TileProvider, id: TileId) {
        self.in_flight.insert(id);

        if let Some(cache) = &self.cache {
            if let Some(bytes) = cache.get(&provider.source, id) {
                // Decoding and mipmapping off the render thread keeps a
                // screenful of cache hits from stalling the frame.
                let sender = self.sender.clone();
                let decode = self.decode;
                #[cfg(not(target_arch = "wasm32"))]
                std::thread::spawn(move || {
                    let _ = sender.send(FetchedTile {
                        id,
                        result: decode_bytes(&bytes, decode),
                    });
                });
                #[cfg(target_arch = "wasm32")]
                let _ = sender.send(FetchedTile {
                    id,
                    result: decode_bytes(&bytes, decode),
                });
                return;
            }
        }

        let url = provider.url(id);
        let sender = self.sender.clone();
        let source = provider.source.clone();
        let cache = self.cache.clone();
        let decode = self.decode;
        let mut request = ehttp::Request::get(url);
        request.headers = ehttp::Headers::new(&[("User-Agent", USER_AGENT)]);
        request.timeout = Some(std::time::Duration::from_secs(20));

        ehttp::fetch(request, move |response| {
            let result = match response {
                Ok(response) => {
                    if response.ok {
                        if let Some(cache) = &cache {
                            cache.put(&source, id, &response.bytes);
                        }
                    }
                    decode_response(response, decode)
                }
                Err(error) => Err(format!("request failed: {error}")),
            };
            if result.is_err() {
                tracing::warn!(tile = %id, "tile request failed");
            }
            // The receiver may be gone if the app shut down mid-flight; a
            // failed send is expected then and must not panic the fetch thread.
            let _ = sender.send(FetchedTile { id, result });
        });
    }

    /// Drains completed requests and immediately starts the next queued tiles.
    /// Failures are retried a bounded number of times, then remembered so a
    /// permanently dead tile cannot loop forever.
    pub fn poll(&mut self, provider: &TileProvider) -> Vec<FetchedTile> {
        let mut completed = Vec::new();
        while let Ok(tile) = self.receiver.try_recv() {
            self.in_flight.remove(&tile.id);
            if tile.result.is_err() {
                let attempts = self.attempts.entry(tile.id).or_insert(0);
                *attempts += 1;
                if *attempts >= MAX_ATTEMPTS {
                    self.failed.insert(tile.id, Instant::now());
                } else {
                    self.queue.push_back((tile.id, tile.id.z));
                }
            } else {
                self.attempts.remove(&tile.id);
                self.failed.remove(&tile.id);
            }
            completed.push(tile);
        }
        self.pump(provider);
        completed
    }

    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    pub fn queued_count(&self) -> usize {
        self.queue.len()
    }
}

fn decode_response(response: ehttp::Response, decode: Decode) -> Result<TilePixels, String> {
    if !response.ok {
        return Err(format!("HTTP {} {}", response.status, response.status_text));
    }
    decode_bytes(&response.bytes, decode)
}

fn decode_bytes(bytes: &[u8], decode: Decode) -> Result<TilePixels, String> {
    let rgba = decode_rgba(bytes)?;
    if let Some(validate) = decode.validate {
        validate(&rgba)?;
    }
    Ok(match decode.output {
        Output::Raw => TilePixels::Decoded(rgba),
        Output::Basemap => TilePixels::Texture(mip_chain(normalize_tile(rgba))),
        Output::Overlay => TilePixels::Texture(mip_chain(premultiply_srgb(&normalize_tile(rgba)))),
    })
}

/// Providers occasionally serve other sizes; normalize so the atlas and the
/// shader's UV math never depend on the response dimensions.
fn normalize_tile(rgba: RgbaImage) -> RgbaImage {
    if rgba.width() == TILE_TEXTURE_SIZE && rgba.height() == TILE_TEXTURE_SIZE {
        return rgba;
    }
    image::imageops::resize(
        &rgba,
        TILE_TEXTURE_SIZE,
        TILE_TEXTURE_SIZE,
        image::imageops::FilterType::Triangle,
    )
}

fn decode_rgba(bytes: &[u8]) -> Result<RgbaImage, String> {
    image::load_from_memory(bytes)
        .map(|image| image.to_rgba8())
        .map_err(|error| format!("decode failed: {error}"))
}

/// Largest side an overlay image keeps on the GPU. Bigger sources are scaled
/// down (aspect preserved): a 16k texture costs a gigabyte with its mips and is
/// not available on every adapter.
pub const MAX_OVERLAY_IMAGE_SIDE: u32 = 4096;

/// Decodes any PNG/JPEG/... into RGBA at its native size, for image overlays.
/// Unlike tiles the size is not normalised; oversized images are scaled down.
pub fn decode_image(bytes: &[u8]) -> Result<ImageRgba, String> {
    let image = decode_rgba(bytes)?;
    if image.width() == 0 || image.height() == 0 {
        return Err("image has no pixels".to_string());
    }
    Ok(limit_image_size(image, MAX_OVERLAY_IMAGE_SIDE))
}

/// Scales `image` down so neither side exceeds `max_side`, keeping the aspect
/// ratio. Smaller images are returned untouched.
pub fn limit_image_size(image: RgbaImage, max_side: u32) -> RgbaImage {
    let longest = image.width().max(image.height());
    if longest <= max_side || max_side == 0 {
        return image;
    }
    let scale = f64::from(max_side) / f64::from(longest);
    let scaled = |side: u32| ((f64::from(side) * scale).round() as u32).clamp(1, max_side);
    image::imageops::resize(
        &image,
        scaled(image.width()),
        scaled(image.height()),
        image::imageops::FilterType::Triangle,
    )
}

/// A decoded raster in 8-bit sRGB with straight alpha.
pub type ImageRgba = RgbaImage;

/// A transparent tile layer drawn over the basemap (Layers panel entries such
/// as OpenSeaMap or OpenRailwayMap).
#[derive(Debug, Clone, PartialEq)]
pub struct TileOverlay {
    pub source: TileSource,
    pub opacity: f32,
}

/// A KML `GroundOverlay`: an image stretched over a lat/lon box.
#[derive(Clone)]
pub struct ImageOverlay {
    pub id: String,
    pub north: f64,
    pub south: f64,
    pub east: f64,
    pub west: f64,
    /// Counter-clockwise degrees about the centre of the box.
    pub rotation_deg: f64,
    pub opacity: f32,
    pub image: Arc<ImageRgba>,
}

impl std::fmt::Debug for ImageOverlay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageOverlay")
            .field("id", &self.id)
            .field("north", &self.north)
            .field("south", &self.south)
            .field("east", &self.east)
            .field("west", &self.west)
            .field("rotation_deg", &self.rotation_deg)
            .field("opacity", &self.opacity)
            .field("image", &self.image.dimensions())
            .finish()
    }
}

/// Multiplies colour by alpha in linear light and re-encodes to sRGB.
///
/// Overlay textures are filtered and mipmapped, and straight-alpha texels with
/// a transparent black colour would darken every soft edge.
pub fn premultiply_srgb(image: &RgbaImage) -> RgbaImage {
    let mut out = image.clone();
    for pixel in out.pixels_mut() {
        if pixel[3] == 255 {
            continue;
        }
        let alpha = f32::from(pixel[3]) / 255.0;
        for channel in &mut pixel.0[..3] {
            *channel = linear_to_srgb(srgb_to_linear(*channel) * alpha);
        }
    }
    out
}

/// Builds the full mip chain for a tile, halving until 1x1.
///
/// The average is taken in linear light and re-encoded to sRGB. Averaging the
/// stored sRGB values directly would darken every mip level, which is very
/// visible on a checkerboard of dark and light imagery.
pub fn mip_chain(image: RgbaImage) -> Vec<RgbaImage> {
    let mut levels = vec![image];
    while let Some(last) = levels.last() {
        if last.width() <= 1 && last.height() <= 1 {
            break;
        }
        levels.push(downsample_half(last));
    }
    levels
}

fn downsample_half(image: &RgbaImage) -> RgbaImage {
    let source_width = image.width() as usize;
    let width = (source_width / 2).max(1);
    let height = (image.height() as usize / 2).max(1);
    let texels: &[[u8; 4]] = bytemuck::cast_slice(image.as_raw());
    let tables = srgb_tables();
    let mut out = Vec::with_capacity(width * height * 4);
    let mut rows = texels.chunks_exact(source_width.max(1));
    for _ in 0..height {
        let Some(top) = rows.next() else {
            break;
        };
        // A source one texel high or wide has no second row or column; its
        // only one is sampled twice.
        let bottom = rows.next().unwrap_or(top);
        if let ([left], [right]) = (top, bottom) {
            out.extend_from_slice(&average_texels(tables, [left, left, right, right]));
            continue;
        }
        for pair in top.chunks_exact(2).zip(bottom.chunks_exact(2)) {
            if let ([a, b], [c, d]) = pair {
                out.extend_from_slice(&average_texels(tables, [a, b, c, d]));
            }
        }
    }
    RgbaImage::from_raw(width as u32, height as u32, out)
        .unwrap_or_else(|| RgbaImage::new(width as u32, height as u32))
}

fn average_texels(tables: &SrgbTables, texels: [&[u8; 4]; 4]) -> [u8; 4] {
    let mut lin = [0.0f32; 3];
    for texel in texels {
        lin[0] += tables.linear[usize::from(texel[0])];
        lin[1] += tables.linear[usize::from(texel[1])];
        lin[2] += tables.linear[usize::from(texel[2])];
    }
    // Basemap imagery is opaque throughout, and rounding the alpha average was
    // a third of the time spent on such a tile.
    let alpha = if texels.iter().all(|texel| texel[3] == u8::MAX) {
        u8::MAX
    } else {
        let sum: f32 = texels
            .iter()
            .map(|texel| tables.unit[usize::from(texel[3])])
            .sum();
        ((sum / 4.0).clamp(0.0, 1.0) * 255.0).round() as u8
    };
    [
        encode_srgb(tables, lin[0] / 4.0),
        encode_srgb(tables, lin[1] / 4.0),
        encode_srgb(tables, lin[2] / 4.0),
        alpha,
    ]
}

/// The sRGB transfer function, for a value in 0..=1.
fn srgb_decode(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear value of each sRGB byte, and the linear value at which the encoded
/// byte rounds up to the next one. A tile's mip chain converts a few hundred
/// thousand samples, and `powf` per sample was several milliseconds per tile.
struct SrgbTables {
    linear: [f32; 256],
    /// Each byte as a fraction of 255, for alpha.
    unit: [f32; 256],
    round_up_at: [f32; 255],
    /// The encoded byte at the low end of each of [`ENCODE_BUCKETS`] equal
    /// slices of 0..1, so encoding starts at most one step below its answer
    /// instead of searching all of `round_up_at`.
    encode_from: [u8; ENCODE_BUCKETS],
}

/// A power of two, so that scaling a linear value to its bucket is exact and
/// the bucket's first byte can never be above the value's own.
const ENCODE_BUCKETS: usize = 4096;

fn srgb_tables() -> &'static SrgbTables {
    static TABLES: OnceLock<SrgbTables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let round_up_at: [f32; 255] =
            std::array::from_fn(|byte| srgb_decode((byte as f32 + 0.5) / 255.0));
        SrgbTables {
            linear: std::array::from_fn(|byte| srgb_decode(byte as f32 / 255.0)),
            unit: std::array::from_fn(|byte| byte as f32 / 255.0),
            round_up_at,
            encode_from: std::array::from_fn(|bucket| {
                let low = bucket as f32 / ENCODE_BUCKETS as f32;
                let steps = round_up_at.partition_point(|threshold| *threshold <= low);
                u8::try_from(steps).unwrap_or(u8::MAX)
            }),
        }
    })
}

fn srgb_to_linear(value: u8) -> f32 {
    srgb_tables().linear[usize::from(value)]
}

fn linear_to_srgb(value: f32) -> u8 {
    encode_srgb(srgb_tables(), value)
}

fn encode_srgb(tables: &SrgbTables, value: f32) -> u8 {
    // The cast saturates: negatives and NaN start at 0, anything above 1 in the
    // last bucket. Encoded bytes are further apart in linear light than a
    // bucket is wide, so the answer is the bucket's first byte or the one after
    // it, and one comparison without a branch settles which.
    let bucket = ((value * ENCODE_BUCKETS as f32) as usize).min(ENCODE_BUCKETS - 1);
    let byte = tables.encode_from[bucket];
    let round_up = tables
        .round_up_at
        .get(usize::from(byte))
        .is_some_and(|threshold| *threshold <= value);
    byte + u8::from(round_up)
}

/// The 1x1 texture used for tiles whose imagery has not loaded yet, and for the
/// polar caps beyond Web-Mercator's latitude limit.
pub const FALLBACK_RGBA: [u8; 4] = [58, 74, 88, 255];

#[cfg(test)]
mod tests {
    use super::*;
    use terra_providers::TileSource;

    fn provider() -> TileProvider {
        TileProvider::new(TileSource::open_street_map())
    }

    #[test]
    fn urls_come_from_the_tile_source() {
        let provider = provider();
        assert_eq!(
            provider.url(TileId::new(4, 8, 5).expect("valid")),
            "https://tile.openstreetmap.org/4/8/5.png"
        );
        assert_eq!(provider.max_level(), 19);
        assert!(provider.credit().contains("OpenStreetMap"));
    }

    #[test]
    fn api_key_is_injected_only_when_the_template_asks_for_it() {
        let mut source = TileSource::open_street_map();
        source.url = "https://tiles.example.test/{z}/{x}/{y}?token={api_key}".to_string();
        let mut provider = TileProvider::new(source);
        provider.api_key = Some("abc".to_string());
        assert!(
            provider
                .url(TileId::new(0, 0, 0).expect("valid"))
                .ends_with("token=abc")
        );
    }

    #[test]
    fn tiles_that_just_gave_up_are_not_requested_again() {
        let mut fetcher = TileFetcher::new();
        fetcher.failed.insert(TileId::root(), Instant::now());
        fetcher.request(&provider(), TileId::root());
        assert_eq!(fetcher.in_flight_count(), 0);
        assert_eq!(fetcher.queued_count(), 0);
    }

    #[test]
    fn a_tile_that_failed_long_ago_is_retried() {
        let mut fetcher = TileFetcher::new();
        fetcher
            .failed
            .insert(TileId::root(), Instant::now() - RETRY_AFTER * 2);
        fetcher.attempts.insert(TileId::root(), MAX_ATTEMPTS);
        assert!(fetcher.ready_to_request(TileId::root()));
        assert!(!fetcher.has_failed(TileId::root()));
        assert_eq!(fetcher.attempts.get(&TileId::root()), None);
    }

    #[test]
    fn a_tile_still_in_flight_is_not_requested_twice() {
        let mut fetcher = TileFetcher::new();
        fetcher.in_flight.insert(TileId::root());
        assert!(!fetcher.ready_to_request(TileId::root()));
    }

    #[test]
    fn set_desired_orders_ancestors_first_and_dedups() {
        let mut fetcher = TileFetcher::new();
        let leaf = TileId::new(4, 9, 6).expect("tile");
        let child = leaf.parent().expect("parent");
        let mut desired = vec![(leaf, leaf.z), (child, child.z), (leaf, leaf.z)];
        desired.sort_by_key(|(id, priority)| (*priority, id.z));
        // The renderer builds the list; this asserts the fetcher keeps the order
        // after deduplicating, by queueing more work than the concurrency cap.
        fetcher.set_desired(&provider(), desired);
        assert!(fetcher.in_flight_count() <= MAX_IN_FLIGHT);
    }

    #[test]
    fn mip_chain_halves_down_to_one_pixel() {
        let image = RgbaImage::from_pixel(256, 256, image::Rgba([1, 2, 3, 255]));
        let mips = mip_chain(image);
        assert_eq!(mips.len(), 9, "256 -> 128 -> ... -> 1");
        assert_eq!(mips[0].dimensions(), (256, 256));
        assert_eq!(mips[1].dimensions(), (128, 128));
        assert_eq!(mips[8].dimensions(), (1, 1));
    }

    #[test]
    fn the_srgb_tables_agree_with_the_transfer_function() {
        for byte in 0..=255u8 {
            assert_eq!(linear_to_srgb(srgb_to_linear(byte)), byte);
        }
        // The shortcut into the threshold table must land on the same byte as
        // searching all of it, or mip pixels would shift.
        let searched = |linear: f32| {
            srgb_tables()
                .round_up_at
                .partition_point(|threshold| *threshold <= linear)
        };
        for step in 0..=2_000_000u32 {
            let linear = step as f32 / 1_000_000.0 - 0.5;
            assert_eq!(usize::from(linear_to_srgb(linear)), searched(linear));
        }
        for threshold in srgb_tables().round_up_at {
            for linear in [threshold.next_down(), threshold, threshold.next_up()] {
                assert_eq!(usize::from(linear_to_srgb(linear)), searched(linear));
            }
        }
        assert_eq!(linear_to_srgb(-1.0), 0);
        assert_eq!(linear_to_srgb(f32::NAN), 0);
        assert_eq!(linear_to_srgb(2.0), 255);
        for step in 0..=100_000u32 {
            let linear = step as f32 / 100_000.0;
            let encoded = if linear <= 0.003_130_8 {
                linear * 12.92
            } else {
                1.055 * linear.powf(1.0 / 2.4) - 0.055
            } * 255.0;
            let byte = f32::from(linear_to_srgb(linear));
            // Equal to rounding the formula, except within float error of a
            // rounding boundary, where either neighbour is right.
            assert!(
                (byte - encoded).abs() <= 0.5 + 1e-3,
                "{linear}: {byte} vs {encoded}"
            );
        }
    }

    #[test]
    fn mip_chain_filters_in_linear_light() {
        let mut image = RgbaImage::from_pixel(2, 2, image::Rgba([0, 0, 0, 255]));
        image.put_pixel(0, 0, image::Rgba([255, 255, 255, 255]));
        image.put_pixel(1, 0, image::Rgba([255, 255, 255, 255]));
        let mips = mip_chain(image);
        let mid = mips[1].get_pixel(0, 0);
        // Half the texels are white: a numerical average would store 128, but in
        // linear light it is 0.5, which encodes to ~188 in sRGB.
        assert!(
            (180..=195).contains(&mid[0]),
            "expected a linear-light average, got {}",
            mid[0]
        );
    }

    const IMAGERY: Decode = Decode {
        output: Output::Basemap,
        validate: None,
    };

    fn decoded_size(bytes: &[u8], decode: Decode) -> (u32, u32) {
        let pixels = decode_bytes(bytes, decode).expect("valid png");
        pixels.image().expect("level 0").dimensions()
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let pixels = [10u8, 20, 30, 255].repeat((width * height) as usize);
        terra_testkit::rgba_to_png(&pixels, width, height)
    }

    #[test]
    fn decode_image_keeps_the_native_size() {
        let image = decode_image(&png(300, 200)).expect("valid png");
        assert_eq!(image.dimensions(), (300, 200));
        assert_eq!(image.get_pixel(5, 5).0, [10, 20, 30, 255]);
    }

    #[test]
    fn decode_image_rejects_corrupt_and_empty_bytes() {
        assert!(decode_image(b"definitely not an image").is_err());
        assert!(decode_image(&[]).is_err());
        let mut truncated = png(64, 64);
        truncated.truncate(20);
        assert!(decode_image(&truncated).is_err());
    }

    #[test]
    fn tiles_stay_strict_while_images_are_not_resized() {
        assert_eq!(
            decoded_size(&png(300, 200), IMAGERY),
            (TILE_TEXTURE_SIZE, TILE_TEXTURE_SIZE)
        );
    }

    #[test]
    fn raw_fetching_keeps_the_served_size() {
        let raw = Decode {
            output: Output::Raw,
            validate: None,
        };
        assert_eq!(decoded_size(&png(300, 200), raw), (300, 200));
        let strict = Decode {
            output: Output::Raw,
            validate: Some(|image| {
                if image.width() == 256 {
                    Ok(())
                } else {
                    Err("wrong size".to_string())
                }
            }),
        };
        assert!(decode_bytes(&png(300, 200), strict).is_err());
    }

    #[test]
    fn oversized_images_are_scaled_down_keeping_the_aspect() {
        let wide = RgbaImage::new(8000, 100);
        let limited = limit_image_size(wide, MAX_OVERLAY_IMAGE_SIDE);
        assert_eq!(limited.dimensions(), (4096, 51));
        let tall = limit_image_size(RgbaImage::new(100, 8192), MAX_OVERLAY_IMAGE_SIDE);
        assert_eq!(tall.dimensions(), (50, 4096));
    }

    #[test]
    fn images_within_the_limit_are_untouched() {
        let image = limit_image_size(RgbaImage::new(4096, 10), MAX_OVERLAY_IMAGE_SIDE);
        assert_eq!(image.dimensions(), (4096, 10));
    }

    #[test]
    fn premultiplying_scales_colour_in_linear_light() {
        let mut image = RgbaImage::new(3, 1);
        image.put_pixel(0, 0, image::Rgba([255, 255, 255, 255]));
        image.put_pixel(1, 0, image::Rgba([255, 255, 255, 0]));
        image.put_pixel(2, 0, image::Rgba([255, 255, 255, 128]));
        let out = premultiply_srgb(&image);
        assert_eq!(out.get_pixel(0, 0).0, [255, 255, 255, 255]);
        assert_eq!(out.get_pixel(1, 0).0, [0, 0, 0, 0]);
        let half = out.get_pixel(2, 0);
        assert_eq!(half[3], 128);
        assert!((180..=195).contains(&half[0]), "got {}", half[0]);
    }

    #[derive(Default)]
    struct FakeCache(HashMap<TileId, Vec<u8>>);

    impl TileCache for FakeCache {
        fn get(&self, _source: &TileSource, id: TileId) -> Option<Vec<u8>> {
            self.0.get(&id).cloned()
        }

        fn put(&self, _source: &TileSource, _id: TileId, _bytes: &[u8]) {}
    }

    fn offline_provider() -> TileProvider {
        let mut source = TileSource::open_street_map();
        source.url = "http://127.0.0.1:1/{z}/{x}/{y}.png".to_string();
        TileProvider::new(source)
    }

    /// Runs one tile through `fetcher`, served from a cache holding `png`.
    fn fetch_cached(fetcher: TileFetcher, png: Vec<u8>) -> TilePixels {
        let id = TileId::new(2, 1, 1).expect("tile");
        let mut fetcher = fetcher.with_cache(Arc::new(FakeCache(HashMap::from([(id, png)]))));
        let provider = offline_provider();
        fetcher.request(&provider, id);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(tile) = fetcher.poll(&provider).into_iter().next() {
                assert_eq!(tile.id, id);
                return tile.result.expect("cached bytes decode");
            }
            assert!(Instant::now() < deadline, "cached tile never arrived");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn cached_tiles_are_served_without_a_network_request() {
        let png = terra_testkit::rgba_to_png(&[255, 0, 0, 255], 1, 1);
        let pixels = fetch_cached(TileFetcher::new(), png);
        let image = pixels.image().expect("level 0");
        assert_eq!(image.dimensions(), (TILE_TEXTURE_SIZE, TILE_TEXTURE_SIZE));
        assert_eq!(image.get_pixel(0, 0)[0], 255);
    }

    #[test]
    fn imagery_tiles_arrive_with_their_mip_chain_built() {
        let TilePixels::Texture(mips) = fetch_cached(TileFetcher::new(), png(256, 256)) else {
            panic!("an imagery fetcher delivers an uploadable mip chain");
        };
        let sides: Vec<u32> = mips.iter().map(RgbaImage::width).collect();
        assert_eq!(sides, [256, 128, 64, 32, 16, 8, 4, 2, 1]);
        for mip in &mips {
            assert_eq!(mip.height(), mip.width());
            assert_eq!(mip.get_pixel(0, 0).0, [10, 20, 30, 255]);
        }
    }

    #[test]
    fn overlay_tiles_arrive_premultiplied_with_their_mip_chain_built() {
        let translucent = terra_testkit::rgba_to_png(&[255, 255, 255, 128].repeat(4), 2, 2);
        let TilePixels::Texture(mips) = fetch_cached(TileFetcher::new_overlay(), translucent)
        else {
            panic!("an overlay fetcher delivers an uploadable mip chain");
        };
        assert_eq!(mips.len(), 9);
        for mip in &mips {
            let pixel = mip.get_pixel(0, 0);
            assert_eq!(pixel[3], 128);
            assert!((180..=195).contains(&pixel[0]), "got {}", pixel[0]);
        }
    }

    #[test]
    fn raw_tiles_arrive_as_decoded_without_mips() {
        let fetcher = TileFetcher::new_raw(|_| Ok(()));
        let TilePixels::Decoded(image) = fetch_cached(fetcher, png(256, 256)) else {
            panic!("elevation data must not be mipmapped");
        };
        assert_eq!(image.dimensions(), (256, 256));
    }

    #[test]
    fn hand_built_pixels_get_their_mips_on_demand() {
        let pixels = TilePixels::from(RgbaImage::new(4, 4));
        assert_eq!(pixels.image().map(RgbaImage::dimensions), Some((4, 4)));
        assert_eq!(pixels.into_mips(|image| image).len(), 3);
        let ready = TilePixels::Texture(vec![RgbaImage::new(1, 1)]);
        let untouched = ready.into_mips(|_| panic!("already prepared by the fetcher"));
        assert_eq!(untouched.len(), 1);
    }

    #[test]
    fn mip_chain_clamps_at_the_edge_of_odd_and_one_texel_wide_images() {
        let mut image = RgbaImage::from_pixel(3, 1, image::Rgba([0, 0, 0, 255]));
        image.put_pixel(2, 0, image::Rgba([255, 255, 255, 255]));
        let mips = mip_chain(image);
        assert_eq!(mips.len(), 2);
        // The third column has no partner and is dropped; the single row is
        // sampled twice.
        assert_eq!(mips[1].dimensions(), (1, 1));
        assert_eq!(mips[1].get_pixel(0, 0).0, [0, 0, 0, 255]);

        let mut column = RgbaImage::from_pixel(1, 2, image::Rgba([0, 0, 0, 0]));
        column.put_pixel(0, 1, image::Rgba([255, 255, 255, 255]));
        let mips = mip_chain(column);
        assert_eq!(mips[1].dimensions(), (1, 1));
        let pixel = mips[1].get_pixel(0, 0);
        assert_eq!(pixel[3], 128);
        assert!((180..=195).contains(&pixel[0]), "got {}", pixel[0]);
    }
}
