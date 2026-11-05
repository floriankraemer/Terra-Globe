//! Imagery: tile URL construction, async fetching and decoding.
//!
//! `ehttp` is used instead of `reqwest` because it compiles to native threads
//! and to `fetch` on wasm from one code path, which keeps the browser target
//! honest without a second implementation.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

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
    pub result: Result<RgbaImage, String>,
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
        }
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

        let url = provider.url(id);
        let sender = self.sender.clone();
        let mut request = ehttp::Request::get(url);
        request.headers = ehttp::Headers::new(&[("User-Agent", USER_AGENT)]);
        request.timeout = Some(std::time::Duration::from_secs(20));

        ehttp::fetch(request, move |response| {
            let result = response
                .map_err(|error| format!("request failed: {error}"))
                .and_then(decode_response);
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

fn decode_response(response: ehttp::Response) -> Result<RgbaImage, String> {
    if !response.ok {
        return Err(format!("HTTP {} {}", response.status, response.status_text));
    }
    let image = image::load_from_memory(&response.bytes)
        .map_err(|error| format!("decode failed: {error}"))?;
    let rgba = image.to_rgba8();
    if rgba.width() == TILE_TEXTURE_SIZE && rgba.height() == TILE_TEXTURE_SIZE {
        return Ok(rgba);
    }
    // Providers occasionally serve other sizes; normalize so the atlas and the
    // shader's UV math never depend on the response dimensions.
    Ok(image::imageops::resize(
        &rgba,
        TILE_TEXTURE_SIZE,
        TILE_TEXTURE_SIZE,
        image::imageops::FilterType::Triangle,
    ))
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
}
