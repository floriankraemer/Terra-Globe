//! Terra Globe application layer: egui views over the `terra-globe` renderer.
//!
//! Immediate mode keeps the whole UI a function of [`AppData`], which is what
//! makes the async-init state explicit instead of a hidden nullable reference.

pub mod i18n;
pub mod platform;
pub mod state;
pub mod theme;
pub mod tools;
pub mod views;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use eframe::egui;
use terra_core::{
    OverlayLayerSetting, PlacemarkGeometry, RectangleBounds, Settings, is_folder_visible,
};
use terra_globe::imagery::{ImageOverlay, ImageRgba, TileOverlay, TileProvider};
use terra_globe::{Camera, Globe, Scene, SceneEntry, SceneMode};
use terra_providers::{SecretStore, TileSource};
use web_time::Instant;

use crate::i18n::I18n;
use crate::platform::{FileSystem, PersistedState, SettingsStore};
use crate::state::StateStore;
use crate::tools::{ImageSource, JobRunner, RouteState, RulerState};

/// Whether this build can show 3D terrain. The elevation tiles come from a
/// server that sends no CORS headers, so a browser cannot fetch them.
pub const TERRAIN_SUPPORTED: bool = cfg!(not(target_arch = "wasm32"));

/// Everything the UI needs, injected by the shell.
pub struct AppDeps {
    pub repository: Arc<dyn terra_storage::PlacesRepository>,
    pub settings: Arc<dyn SettingsStore>,
    pub files: Arc<dyn FileSystem>,
    pub secrets: Arc<dyn SecretStore>,
    /// Optional persistent tile store; the desktop shell keeps tiles on disk.
    pub tile_cache: Option<Arc<dyn terra_globe::imagery::TileCache>>,
}

#[derive(Debug, Clone)]
pub struct Notice {
    pub message: String,
    pub error: bool,
    pub created: Instant,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmAction {
    DeletePlacemark(String),
    DeleteFolder(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalKind {
    Settings,
    About,
    HeightProfile,
}

#[derive(Debug, Clone)]
pub struct DragMove {
    pub id: String,
    pub start: terra_core::GeoPoint,
    pub original: terra_core::PlacemarkGeometry,
    pub delta_lon: f64,
    pub delta_lat: f64,
}

#[derive(Debug, Clone, Default)]
pub struct GeocodeState {
    pub query: String,
    pub results: Vec<terra_providers::GeocodeResult>,
    pub pending: bool,
    pub error: Option<String>,
}

/// Text buffers for a ground overlay, so half-typed or invalid input never
/// reaches the library; validated and committed with Save.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayEditor {
    pub north: String,
    pub south: String,
    pub east: String,
    pub west: String,
    pub rotation: String,
    pub opacity: f64,
    pub href: String,
    pub error: Option<String>,
}

impl OverlayEditor {
    pub fn from_geometry(geometry: &PlacemarkGeometry) -> Option<Self> {
        let PlacemarkGeometry::GroundOverlay {
            bounds,
            image_url,
            rotation,
            opacity,
        } = geometry
        else {
            return None;
        };
        Some(Self {
            north: bounds.north.to_string(),
            south: bounds.south.to_string(),
            east: bounds.east.to_string(),
            west: bounds.west.to_string(),
            rotation: rotation.unwrap_or(0.0).to_string(),
            opacity: opacity.unwrap_or(1.0),
            href: image_url.clone(),
            error: None,
        })
    }

    /// The geometry these buffers describe, or why they are not valid.
    pub fn to_geometry(&self) -> Result<PlacemarkGeometry, String> {
        let number = |label: &str, text: &str| {
            text.trim()
                .parse::<f64>()
                .map_err(|_| format!("{label} must be a number"))
        };
        let bounds = RectangleBounds::new(
            number("north", &self.north)?,
            number("south", &self.south)?,
            number("east", &self.east)?,
            number("west", &self.west)?,
        )
        .map_err(|error| error.to_string())?;
        let rotation = number("rotation", &self.rotation)?;
        PlacemarkGeometry::ground_overlay(
            bounds,
            self.href.trim().to_string(),
            (rotation != 0.0).then_some(rotation),
            (self.opacity < 1.0).then_some(self.opacity),
        )
        .map_err(|error| error.to_string())
    }
}

/// Edit buffers for the selected placemark; committed with Save.
#[derive(Debug, Clone)]
pub struct EditorState {
    pub id: String,
    pub name: String,
    pub description: String,
    pub visible: bool,
    pub style: terra_core::Style,
    pub rings_enabled: bool,
    pub ring_spacing_meters: f64,
    pub overlay: Option<OverlayEditor>,
}

#[derive(Clone)]
pub enum ImageState {
    Loading,
    Ready(Arc<ImageRgba>),
    Failed,
}

pub struct AppData {
    pub deps: AppDeps,
    pub store: StateStore,
    pub settings: Settings,
    pub globe: Globe,
    pub texture: Option<egui::TextureId>,
    pub registered_generation: u64,
    pub selected: Option<String>,
    pub hovered: Option<String>,
    pub editor: Option<EditorState>,
    pub drawing: terra_globe::DrawingSession,
    pub ruler: RulerState,
    pub ruler_adding: bool,
    pub route: RouteState,
    pub route_adding: bool,
    pub jobs: JobRunner,
    pub geocode: GeocodeState,
    pub notice: Option<Notice>,
    pub confirm: Option<ConfirmAction>,
    pub modal: Option<ModalKind>,
    pub search: String,
    pub selected_folder: Option<String>,
    pub renaming: Option<(String, String)>,
    /// The name prompt was opened for a folder that was just created, not for a rename.
    pub renaming_new_folder: bool,
    pub file_path: Option<PathBuf>,
    pub scene_revision: u64,
    pub last_preview: Option<terra_core::PlacemarkGeometry>,
    pub cursor_world: Option<terra_core::GeoPoint>,
    pub drag_move: Option<DragMove>,
    pub frame_ms: f32,
    /// How long the globe arrow keys have been held, for the speed ramp.
    pub globe_key_hold: f32,
    pub show_stats: bool,
    pub start: Instant,
    pub last_camera_save: Instant,
    pub error_banner: Option<String>,
    pub settings_draft: SettingsDraft,
    /// Layer row whose opacity slider is open in the Layers panel.
    pub selected_layer: Option<String>,
    /// Read-through cache of the repository's asset store (the source of truth),
    /// keyed by normalized path.
    pub assets: HashMap<String, Vec<u8>>,
    /// Ground-overlay pictures by href, so overlays sharing a picture share one load.
    pub images: HashMap<String, ImageState>,
    applied_images: Vec<ImageOverlay>,
}

#[derive(Debug, Clone, Default)]
pub struct SettingsDraft {
    pub api_key: String,
    pub provider_preset: String,
}

impl AppData {
    pub fn new(cc: &eframe::CreationContext<'_>, deps: AppDeps) -> Result<Self, String> {
        let render_state = cc
            .wgpu_render_state
            .as_ref()
            .ok_or_else(|| "eframe was started without the wgpu backend".to_string())?;
        let persisted = deps.settings.load();
        let provider = tile_provider(&persisted.settings);
        let globe = Globe::new(
            render_state.device.clone(),
            render_state.queue.clone(),
            provider,
            (1280, 800),
        );
        Self::from_parts(deps, globe, persisted)
    }

    /// Builds the app around an already created globe. Split out from [`Self::new`]
    /// so integration tests can run the whole state layer with a headless device.
    pub fn from_parts(
        deps: AppDeps,
        mut globe: Globe,
        persisted: PersistedState,
    ) -> Result<Self, String> {
        if let Some(cache) = &deps.tile_cache {
            globe.set_tile_cache(cache.clone());
        }
        if let Some(camera) = persisted.camera {
            let mut restored = Camera::default();
            restored.target_lon_deg = camera.lon_deg;
            restored.target_lat_deg = camera.lat_deg;
            restored.distance = camera.distance;
            restored.heading = camera.heading;
            restored.pitch = camera.pitch;
            globe.camera = restored;
        }
        globe.set_mode(scene_mode_from_setting(&persisted.settings.scene_mode));
        let store = StateStore::load(deps.repository.clone()).map_err(|error| error.to_string())?;
        let settings = persisted.settings.clone().normalized();

        let mut data = Self {
            deps,
            store,
            settings,
            globe,
            texture: None,
            registered_generation: u64::MAX,
            selected: None,
            hovered: None,
            editor: None,
            drawing: terra_globe::DrawingSession::new(),
            ruler: RulerState::default(),
            ruler_adding: false,
            route: RouteState::default(),
            route_adding: false,
            jobs: JobRunner::new(),
            geocode: GeocodeState::default(),
            notice: None,
            confirm: None,
            modal: None,
            search: String::new(),
            selected_folder: None,
            renaming: None,
            renaming_new_folder: false,
            file_path: None,
            scene_revision: 0,
            last_preview: None,
            cursor_world: None,
            drag_move: None,
            frame_ms: 0.0,
            globe_key_hold: 0.0,
            show_stats: false,
            start: Instant::now(),
            last_camera_save: Instant::now(),
            error_banner: None,
            settings_draft: SettingsDraft::default(),
            selected_layer: None,
            assets: HashMap::new(),
            images: HashMap::new(),
            applied_images: Vec::new(),
        };
        data.apply_layers();
        data.apply_terrain();
        data.collect_unreferenced_assets();
        Ok(data)
    }

    pub fn now_seconds(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    pub fn notify(&mut self, message: impl Into<String>) {
        self.notice = Some(Notice {
            message: message.into(),
            error: false,
            created: Instant::now(),
        });
    }

    pub fn notify_error(&mut self, message: impl Into<String>) {
        self.notice = Some(Notice {
            message: message.into(),
            error: true,
            created: Instant::now(),
        });
    }

    pub fn select(&mut self, id: Option<String>) {
        self.selected = id;
        self.sync_editor();
    }

    /// Reloads the editor buffers from the library.
    pub fn sync_editor(&mut self) {
        let Some(id) = self.selected.clone() else {
            self.editor = None;
            return;
        };
        let Some(placemark) = self.store.library().placemark(&id) else {
            self.editor = None;
            self.selected = None;
            return;
        };
        let style = self.store.library().style_for(placemark);
        self.editor = Some(EditorState {
            id: id.clone(),
            name: placemark.name.clone(),
            description: placemark.description.clone().unwrap_or_default(),
            visible: placemark.visibility,
            rings_enabled: false,
            ring_spacing_meters: 100.0,
            overlay: OverlayEditor::from_geometry(&placemark.geometry),
            style,
        });
    }

    pub fn close_editor(&mut self) {
        self.selected = None;
        self.editor = None;
    }

    pub fn visible_scene_entries(&self) -> Vec<SceneEntry> {
        self.store
            .library()
            .placemarks
            .iter()
            .map(|placemark| {
                let moving = self
                    .drag_move
                    .as_ref()
                    .filter(|drag| drag.id == placemark.id);
                let geometry = match moving {
                    Some(drag) => terra_core::translate_geometry(
                        &drag.original,
                        drag.delta_lon,
                        drag.delta_lat,
                    ),
                    None => placemark.geometry.clone(),
                };
                SceneEntry {
                    id: placemark.id.clone(),
                    visible: self.store.library().is_placemark_visible(placemark),
                    selected: self.selected.as_deref() == Some(placemark.id.as_str()),
                    geometry,
                    style: self.store.library().style_for(placemark),
                }
            })
            .collect()
    }

    /// Commits a Shift+drag move as one undoable edit.
    pub fn commit_drag_move(&mut self) {
        let Some(drag) = self.drag_move.take() else {
            return;
        };
        let geometry =
            terra_core::translate_geometry(&drag.original, drag.delta_lon, drag.delta_lat);
        let patch = terra_core::PlacemarkPatch {
            geometry: terra_core::Patch::Set(geometry),
            ..Default::default()
        };
        match self.store.update_placemark(&drag.id, patch) {
            Ok(()) => {
                self.autosave();
                self.scene_revision = self.scene_revision.wrapping_add(1);
            }
            Err(error) => self.notify_error(error.to_string()),
        }
    }

    /// Hash of every input that changes overlay geometry. The renderer rebuilds
    /// meshes only when this changes, so selecting a placemark or toggling rings
    /// takes effect without re-tessellating on every camera move.
    fn scene_fingerprint(&self, preview: Option<&terra_core::PlacemarkGeometry>) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.store.library().revision.hash(&mut hasher);
        self.selected.hash(&mut hasher);
        self.ruler.points.len().hash(&mut hasher);
        for point in &self.ruler.points {
            point.lon.to_bits().hash(&mut hasher);
            point.lat.to_bits().hash(&mut hasher);
        }
        self.route.legs.len().hash(&mut hasher);
        for leg in &self.route.legs {
            leg.path.len().hash(&mut hasher);
            leg.distance_meters.to_bits().hash(&mut hasher);
        }
        if let Some(editor) = &self.editor {
            editor.rings_enabled.hash(&mut hasher);
            editor.ring_spacing_meters.to_bits().hash(&mut hasher);
        }
        if let Some(drag) = &self.drag_move {
            drag.id.hash(&mut hasher);
            drag.delta_lon.to_bits().hash(&mut hasher);
            drag.delta_lat.to_bits().hash(&mut hasher);
        }
        if let Some(preview) = preview {
            let center = terra_core::geometry_center(preview);
            center.lon.to_bits().hash(&mut hasher);
            center.lat.to_bits().hash(&mut hasher);
            preview.type_name().hash(&mut hasher);
            match preview {
                terra_core::PlacemarkGeometry::LineString { path, .. } => {
                    path.len().hash(&mut hasher)
                }
                terra_core::PlacemarkGeometry::Polygon { outer_ring, .. } => {
                    outer_ring.len().hash(&mut hasher)
                }
                terra_core::PlacemarkGeometry::Circle { radius_meters, .. } => {
                    radius_meters.to_bits().hash(&mut hasher)
                }
                terra_core::PlacemarkGeometry::Rectangle {
                    north,
                    south,
                    east,
                    west,
                } => {
                    north.to_bits().hash(&mut hasher);
                    south.to_bits().hash(&mut hasher);
                    east.to_bits().hash(&mut hasher);
                    west.to_bits().hash(&mut hasher);
                }
                _ => {}
            }
        }
        hasher.finish()
    }

    /// Scene plus the revision the renderer uses to decide when to re-tessellate.
    pub fn build_scene(&mut self) -> (u64, Scene) {
        let cursor = self.cursor_world;
        let preview = self.drawing.preview(cursor);
        let rings = self
            .selected
            .as_deref()
            .and_then(|id| self.store.library().placemark(id))
            .and_then(|placemark| {
                let editor = self.editor.as_ref()?;
                if !editor.rings_enabled {
                    return None;
                }
                match &placemark.geometry {
                    terra_core::PlacemarkGeometry::Point { coordinates } => {
                        Some(terra_globe::DistanceRings {
                            center: *coordinates,
                            spacing_meters: editor.ring_spacing_meters.max(1.0),
                            count: 5,
                        })
                    }
                    _ => None,
                }
            });

        self.last_preview = preview.clone();
        self.scene_revision = self.scene_fingerprint(preview.as_ref());

        let scene = Scene {
            entries: self.visible_scene_entries(),
            ruler_path: (!self.ruler.points.is_empty()).then(|| self.ruler.points.clone()),
            route_path: self.route.route_path(),
            preview,
            rings,
            ..Default::default()
        };
        (self.scene_revision, scene)
    }

    pub fn open_file(&mut self) {
        let Some(path) = self.deps.files.pick_open() else {
            return;
        };
        self.open_path(path);
    }

    pub fn open_path(&mut self, path: PathBuf) {
        let bytes = match self.deps.files.read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.notify_error(error);
                return;
            }
        };
        let mut entries = Vec::new();
        let document = if path.extension().and_then(|value| value.to_str()) == Some("kmz") {
            match terra_kml::read_kmz(&bytes) {
                Ok(read) => {
                    let document = read
                        .iter()
                        .find(|entry| entry.name.ends_with(".kml"))
                        .and_then(|entry| String::from_utf8(entry.bytes.clone()).ok());
                    entries = read;
                    document
                }
                Err(error) => {
                    self.notify_error(error.to_string());
                    return;
                }
            }
        } else {
            String::from_utf8(bytes).ok()
        };
        let Some(document) = document else {
            self.notify_error("the file is not a readable KML/KMZ document");
            return;
        };
        match terra_kml::parse_kml(&document) {
            Ok(parsed) => {
                let warnings = parsed.warnings.len();
                if let Err(error) = self.keep_assets(&parsed, entries) {
                    self.notify_error(format!("could not store the document's images: {error}"));
                    return;
                }
                match self.store.import(parsed) {
                    Ok(count) => {
                        self.file_path = Some(path);
                        self.notify(format!(
                            "imported {count} placemarks{}",
                            if warnings > 0 {
                                format!(" ({warnings} warnings)")
                            } else {
                                String::new()
                            }
                        ));
                        self.sync_editor();
                    }
                    Err(error) => self.notify_error(error.to_string()),
                }
            }
            Err(error) => self.notify_error(error.to_string()),
        }
    }

    pub fn export_to(&mut self, path: PathBuf, kmz: bool) {
        let mut batch = self.store.export_batch();
        let assets = if kmz {
            self.embed_overlay_images(&mut batch.placemarks)
        } else {
            Vec::new()
        };
        let kml = match terra_kml::serialize_kml(&terra_kml::SerializeKmlInput {
            folders: &batch.folders,
            placemarks: &batch.placemarks,
            styles: &batch.styles,
            screen_overlays: &batch.screen_overlays,
        }) {
            Ok(kml) => kml,
            Err(error) => {
                self.notify_error(error.to_string());
                return;
            }
        };
        let bytes = if kmz {
            match terra_kml::write_kmz(&kml, &assets) {
                Ok(bytes) => bytes,
                Err(error) => {
                    self.notify_error(error.to_string());
                    return;
                }
            }
        } else {
            kml.into_bytes()
        };
        match self.deps.files.write(&path, &bytes) {
            Ok(()) => {
                self.file_path = Some(path.clone());
                self.notify(format!("exported to {}", path.display()));
            }
            Err(error) => self.notify_error(error),
        }
    }

    pub fn save(&mut self, force_save_as: bool) {
        let path = if force_save_as || self.file_path.is_none() {
            self.deps.files.pick_save("terra-globe.kml")
        } else {
            self.file_path.clone()
        };
        let Some(path) = path else {
            return;
        };
        let kmz = path.extension().and_then(|value| value.to_str()) == Some("kmz");
        self.export_to(path, kmz);
    }

    /// Writes the current document when auto-save is on and a file is open.
    pub fn autosave(&mut self) {
        if self.settings.auto_save {
            if let Some(path) = self.file_path.clone() {
                let kmz = path.extension().and_then(|value| value.to_str()) == Some("kmz");
                self.export_to(path, kmz);
            }
        }
    }

    pub fn persist_settings(&mut self) {
        self.deps.settings.save(&PersistedState {
            settings: self.settings.clone(),
            camera: Some(self.globe.camera.pose()),
        });
    }

    /// Saves the camera at most every few seconds while it moves.
    pub fn maybe_save_camera(&mut self) {
        if self.last_camera_save.elapsed() > Duration::from_secs(3) {
            self.last_camera_save = Instant::now();
            self.persist_settings();
        }
    }

    pub fn secret_for(&self, preset: &str) -> Option<String> {
        self.deps
            .secrets
            .get(&format!("provider:{preset}"))
            .ok()
            .flatten()
    }

    pub fn handle_jobs(&mut self) {
        for result in self.jobs.poll() {
            match result {
                crate::tools::JobResult::Image { href, result } => match result {
                    Ok(image) => {
                        self.images.insert(href, ImageState::Ready(image));
                    }
                    Err(error) => {
                        self.notify_error(format!("could not load image {href}: {error}"));
                        self.images.insert(href, ImageState::Failed);
                    }
                },
                crate::tools::JobResult::Geocode(result) => {
                    self.geocode.pending = false;
                    match result {
                        Ok(results) => {
                            self.geocode.error =
                                results.is_empty().then(|| "no results".to_string());
                            self.geocode.results = results;
                        }
                        Err(error) => {
                            self.geocode.error = Some(error.clone());
                            self.geocode.results.clear();
                            let _ = error;
                        }
                    }
                }
                crate::tools::JobResult::Route(result) => {
                    self.route.pending = false;
                    match result {
                        Ok(legs) => {
                            self.route.error = legs.is_empty().then(|| "no route".to_string());
                            self.route.legs = legs;
                        }
                        Err(error) => {
                            self.route.error = Some(error);
                            self.route.legs.clear();
                        }
                    }
                }
            }
        }
    }

    pub fn start_geocode_search(&mut self) {
        let query = self.geocode.query.trim().to_string();
        if query.is_empty() {
            return;
        }
        let preset = self.settings.geocoding_preset.clone();
        let api_key = self.secret_for(&preset);
        self.geocode.pending = true;
        self.geocode.error = None;
        self.jobs.spawn(crate::tools::JobRequest::Geocode {
            preset,
            api_key,
            query,
        });
    }

    pub fn start_route(&mut self) {
        if self.route.waypoints.len() < 2 {
            return;
        }
        let preset = self.settings.routing_preset.clone();
        let api_key = self.secret_for(&preset);
        let mode = self.route.mode.unwrap_or(crate::tools::RouteMode::Car);
        self.route.pending = true;
        self.route.error = None;
        self.jobs.spawn(crate::tools::JobRequest::Route {
            preset,
            api_key,
            waypoints: self.route.waypoints.clone(),
            mode,
        });
    }

    pub fn set_scene_mode(&mut self, mode: SceneMode) {
        self.globe.set_mode(mode);
        self.settings.scene_mode = scene_mode_setting(mode).to_string();
        self.scene_revision = self.scene_revision.wrapping_add(1);
        self.persist_settings();
    }

    pub fn tile_source(&self) -> TileSource {
        tile_source_for(&self.settings.tile_source_id)
    }

    pub fn set_basemap(&mut self, id: &str) {
        if self.settings.tile_source_id == id
            || !terra_providers::builtin_tile_sources()
                .iter()
                .any(|source| source.id == id)
        {
            return;
        }
        self.settings.tile_source_id = id.to_string();
        self.globe.set_provider(tile_provider(&self.settings));
        self.persist_settings();
    }

    /// Enabled flag and opacity of an overlay preset; presets the user never
    /// touched are off and fully opaque.
    pub fn layer_state(&self, id: &str) -> (bool, f32) {
        self.settings
            .overlay_layers
            .iter()
            .find(|layer| layer.id == id)
            .map_or((false, 1.0), |layer| (layer.enabled, layer.opacity))
    }

    pub fn set_layer_enabled(&mut self, id: &str, enabled: bool) {
        let (_, opacity) = self.layer_state(id);
        self.update_layer(id, enabled, opacity);
    }

    pub fn set_layer_opacity(&mut self, id: &str, opacity: f32) {
        let (enabled, _) = self.layer_state(id);
        self.update_layer(id, enabled, opacity);
    }

    fn update_layer(&mut self, id: &str, enabled: bool, opacity: f32) {
        if terra_providers::overlay_preset(id).is_none() {
            return;
        }
        let entry = OverlayLayerSetting {
            id: id.to_string(),
            enabled,
            opacity,
        };
        match self
            .settings
            .overlay_layers
            .iter_mut()
            .find(|layer| layer.id == id)
        {
            Some(layer) => *layer = entry,
            None => self.settings.overlay_layers.push(entry),
        }
        self.settings = std::mem::take(&mut self.settings).normalized();
        self.apply_layers();
        self.persist_settings();
    }

    /// Switches the 3D terrain on or off and remembers the choice.
    pub fn set_terrain_enabled(&mut self, enabled: bool) {
        if self.settings.terrain_enabled == enabled {
            return;
        }
        self.settings.terrain_enabled = enabled;
        self.apply_terrain();
        self.persist_settings();
    }

    /// Hands the terrain setting to the globe. The browser build never turns it
    /// on, whatever the setting says: its elevation source is not reachable.
    pub fn apply_terrain(&mut self) {
        self.globe
            .set_terrain_enabled(self.settings.terrain_enabled && TERRAIN_SUPPORTED);
    }

    /// Pushes the enabled overlay presets to the globe in catalog order.
    pub fn apply_layers(&mut self) {
        self.globe.set_tile_overlays(tile_overlays(&self.settings));
    }

    /// Persists the archive entries that ground overlays of `parsed` point at.
    /// Other entries (icons, models, stray files) are not stored. Runs before
    /// the import so a storage failure leaves no half-imported document.
    fn keep_assets(
        &mut self,
        parsed: &terra_kml::ParseKmlResult,
        entries: Vec<terra_kml::KmzEntry>,
    ) -> Result<(), String> {
        let referenced = overlay_hrefs(parsed.placemarks.iter());
        for entry in entries {
            let path = terra_kml::normalize_asset_path(&entry.name);
            if !referenced.contains(&path) {
                continue;
            }
            pollster::block_on(
                self.store
                    .repository()
                    .put_asset(&path, entry.bytes.clone()),
            )
            .map_err(|error| error.to_string())?;
            self.assets.insert(path, entry.bytes);
        }
        self.images
            .retain(|_, state| !matches!(state, ImageState::Failed));
        Ok(())
    }

    /// Deletes stored assets no placemark or screen overlay points at. Runs
    /// once at startup, when no undo history can still need them.
    fn collect_unreferenced_assets(&mut self) {
        let library = self.store.library();
        let mut referenced = overlay_hrefs(library.placemarks.iter());
        referenced.extend(
            library
                .overlays
                .iter()
                .map(|overlay| terra_kml::normalize_asset_path(&overlay.image_url)),
        );
        let repository = self.store.repository().clone();
        let result = pollster::block_on(async {
            for path in repository.list_assets().await? {
                if !referenced.contains(&path) {
                    repository.delete_asset(&path).await?;
                }
            }
            Ok::<(), terra_storage::StorageError>(())
        });
        if let Err(error) = result {
            self.notify_error(format!("could not clean up stored images: {error}"));
        }
    }

    /// Where the picture for `href` comes from: the asset store (through the
    /// cache), then next to the open document, then an absolute path, URL or
    /// data URL.
    fn image_source(&mut self, href: &str) -> Result<ImageSource, String> {
        if let Some(decoded) = terra_kml::decode_data_url(href) {
            return Ok(ImageSource::Bytes(decoded.bytes));
        }
        if href.starts_with("http://") || href.starts_with("https://") {
            return Ok(ImageSource::Url(href.to_string()));
        }
        let key = terra_kml::normalize_asset_path(href);
        if let Some(bytes) = self.assets.get(&key) {
            return Ok(ImageSource::Bytes(bytes.clone()));
        }
        // ponytail: blocking read on the UI thread (the repo future is not Send);
        // fine for SQLite, move into the job if a slow backend needs it.
        let stored = pollster::block_on(self.store.repository().get_asset(&key))
            .map_err(|error| error.to_string())?;
        if let Some(bytes) = stored {
            self.assets.insert(key, bytes.clone());
            return Ok(ImageSource::Bytes(bytes));
        }
        let path = PathBuf::from(href);
        if path.is_relative() {
            if let Some(parent) = self.file_path.as_deref().and_then(Path::parent) {
                return Ok(ImageSource::Path(parent.join(path)));
            }
        }
        Ok(ImageSource::Path(path))
    }

    /// Starts loading pictures of visible ground overlays and hands the loaded
    /// ones to the globe whenever the set changes.
    pub fn sync_images(&mut self) {
        let library = self.store.library();
        let wanted: Vec<(String, PlacemarkGeometry)> = library
            .placemarks
            .iter()
            .filter(|placemark| library.is_placemark_visible(placemark))
            .filter(|placemark| {
                matches!(placemark.geometry, PlacemarkGeometry::GroundOverlay { .. })
            })
            .map(|placemark| (placemark.id.clone(), placemark.geometry.clone()))
            .collect();

        let mut overlays = Vec::new();
        for (id, geometry) in wanted {
            let PlacemarkGeometry::GroundOverlay {
                bounds,
                image_url,
                rotation,
                opacity,
            } = geometry
            else {
                continue;
            };
            match self.images.get(&image_url) {
                None => self.start_image_load(image_url),
                Some(ImageState::Ready(image)) => overlays.push(ImageOverlay {
                    id,
                    north: bounds.north,
                    south: bounds.south,
                    east: bounds.east,
                    west: bounds.west,
                    rotation_deg: rotation.unwrap_or(0.0),
                    opacity: opacity.unwrap_or(1.0) as f32,
                    image: image.clone(),
                }),
                Some(ImageState::Loading | ImageState::Failed) => {}
            }
        }
        if !same_overlays(&self.applied_images, &overlays) {
            self.globe.set_image_overlays(overlays.clone());
            self.applied_images = overlays;
        }
    }

    fn start_image_load(&mut self, href: String) {
        let source = match self.image_source(&href) {
            Ok(source) => source,
            Err(error) => {
                self.notify_error(format!("could not load image {href}: {error}"));
                self.images.insert(href, ImageState::Failed);
                return;
            }
        };
        self.images.insert(href.clone(), ImageState::Loading);
        self.jobs.spawn(crate::tools::JobRequest::LoadImage {
            href,
            source,
            files: self.deps.files.clone(),
        });
    }

    pub fn images_loading(&self) -> bool {
        self.images
            .values()
            .any(|state| matches!(state, ImageState::Loading))
    }

    /// Ground overlays currently handed to the globe.
    pub fn image_overlays(&self) -> &[ImageOverlay] {
        &self.applied_images
    }

    /// Asks for a picture and copies it into the asset store; returns its file
    /// stem and the relative href that references the stored copy.
    pub fn pick_image(&mut self) -> Option<(String, String)> {
        let path = self.deps.files.pick_open_image()?;
        let bytes = match self.deps.files.read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.notify_error(error);
                return None;
            }
        };
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let href = asset_href(&path, &bytes);
        if let Err(error) =
            pollster::block_on(self.store.repository().put_asset(&href, bytes.clone()))
        {
            self.notify_error(format!("could not store the image: {error}"));
            return None;
        }
        self.assets.insert(href.clone(), bytes);
        Some((name, href))
    }

    /// Asks for a picture and adds it as a ground overlay covering the middle of the view.
    pub fn add_image_overlay(&mut self) {
        let Some((name, href)) = self.pick_image() else {
            return;
        };
        let geometry = self
            .view_bounds()
            .and_then(|bounds| {
                PlacemarkGeometry::ground_overlay(bounds, href, None, None)
                    .map_err(|error| error.to_string())
            })
            .map_err(|error| self.notify_error(error))
            .ok();
        let Some(geometry) = geometry else {
            return;
        };
        let folder = self.selected_folder.clone();
        match self
            .store
            .create_named_placemark(geometry, folder, Some(name))
        {
            Ok(placemark) => {
                self.select(Some(placemark.id.clone()));
                self.autosave();
                self.notify(format!("created {}", placemark.name));
            }
            Err(error) => self.notify_error(error.to_string()),
        }
    }

    /// The central half of what the camera shows, falling back to a box around
    /// the camera target when the view corners miss the globe.
    pub fn view_bounds(&self) -> Result<RectangleBounds, String> {
        let (width, height) = self.globe.viewport();
        let (width, height) = (f64::from(width), f64::from(height));
        let corners = [(0.25, 0.25), (0.75, 0.25), (0.75, 0.75), (0.25, 0.75)]
            .map(|(x, y)| self.globe.pick((width * x, height * y)));
        let picked: Vec<_> = corners.into_iter().flatten().collect();
        let fold = |value: fn(&terra_globe::GlobePick) -> f64, pick: fn(f64, f64) -> f64| {
            picked.iter().map(value).reduce(pick)
        };
        if picked.len() == 4 {
            if let (Some(north), Some(south), Some(east), Some(west)) = (
                fold(|p| p.lat_deg, f64::max),
                fold(|p| p.lat_deg, f64::min),
                fold(|p| p.lon_deg, f64::max),
                fold(|p| p.lon_deg, f64::min),
            ) {
                if let Ok(bounds) = RectangleBounds::new(north, south, east, west) {
                    return Ok(bounds);
                }
            }
        }
        let center = self.globe.pose();
        let half = (self.globe.view_distance() / 6_371_000.0).to_degrees() * 0.25;
        let half = half.clamp(0.001, 40.0);
        let (lon, lat) = (center.lon_deg, center.lat_deg);
        RectangleBounds::new(
            (lat + half).min(90.0),
            (lat - half).max(-90.0),
            (lon + half).min(180.0),
            (lon - half).max(-180.0),
        )
        .map_err(|error| error.to_string())
    }

    /// Replaces local picture references by archive paths for a KMZ export and
    /// returns the files to embed. Pictures that cannot be read keep their href.
    fn embed_overlay_images(
        &mut self,
        placemarks: &mut [terra_core::Placemark],
    ) -> Vec<terra_kml::KmzEntry> {
        let mut entries: Vec<terra_kml::KmzEntry> = Vec::new();
        let mut embedded: BTreeMap<String, String> = BTreeMap::new();
        let mut failures = Vec::new();
        for placemark in placemarks {
            let PlacemarkGeometry::GroundOverlay { image_url, .. } = &mut placemark.geometry else {
                continue;
            };
            if let Some(name) = embedded.get(image_url.as_str()) {
                *image_url = name.clone();
                continue;
            }
            let source = match self.image_source(image_url) {
                Ok(source) => source,
                Err(error) => {
                    failures.push(error);
                    continue;
                }
            };
            let (bytes, preferred) = match source {
                ImageSource::Url(_) => continue,
                ImageSource::Bytes(bytes) => {
                    let extension = terra_kml::decode_data_url(image_url)
                        .map_or("png", |decoded| {
                            terra_kml::extension_for_mime_type(&decoded.mime_type)
                        });
                    let name = if terra_kml::decode_data_url(image_url).is_some() {
                        format!("images/{}.{extension}", placemark.id)
                    } else {
                        terra_kml::normalize_asset_path(image_url)
                    };
                    (bytes, name)
                }
                ImageSource::Path(path) => match self.deps.files.read(&path) {
                    Ok(bytes) => {
                        let file = path
                            .file_name()
                            .map_or_else(|| "image".into(), |name| name.to_string_lossy());
                        (bytes, format!("images/{file}"))
                    }
                    Err(error) => {
                        failures.push(error);
                        continue;
                    }
                },
            };
            let mut name = preferred.clone();
            let mut counter = 1;
            while entries.iter().any(|entry| entry.name == name) {
                counter += 1;
                name = format!(
                    "images/{counter}-{}",
                    preferred.trim_start_matches("images/")
                );
            }
            entries.push(terra_kml::KmzEntry {
                name: name.clone(),
                bytes,
            });
            embedded.insert(image_url.clone(), name.clone());
            *image_url = name;
        }
        for failure in failures {
            self.notify_error(format!("could not embed image: {failure}"));
        }
        entries
    }
}

fn same_overlays(a: &[ImageOverlay], b: &[ImageOverlay]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| {
            x.id == y.id
                && x.north == y.north
                && x.south == y.south
                && x.east == y.east
                && x.west == y.west
                && x.rotation_deg == y.rotation_deg
                && x.opacity == y.opacity
                && Arc::ptr_eq(&x.image, &y.image)
        })
}

/// Asset-store path for a picked picture: `images/<stem>-<content hash>.<ext>`.
/// The hash keeps different pictures with the same file name apart and makes
/// picking the same picture twice share one stored copy.
fn asset_href(path: &Path, bytes: &[u8]) -> String {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let stem: String = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '-'
            }
        })
        .take(40)
        .collect();
    let stem = if stem.trim_matches('-').is_empty() {
        "image".to_string()
    } else {
        stem
    };
    let extension: String = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(8)
        .collect();
    let extension = if extension.is_empty() {
        "bin"
    } else {
        &extension
    };
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("images/{stem}-{:08x}.{extension}", hasher.finish() as u32)
}

/// Normalized hrefs of the ground overlays among `placemarks`.
fn overlay_hrefs<'a>(
    placemarks: impl Iterator<Item = &'a terra_core::Placemark>,
) -> std::collections::HashSet<String> {
    placemarks
        .filter_map(|placemark| match &placemark.geometry {
            PlacemarkGeometry::GroundOverlay { image_url, .. } => {
                Some(terra_kml::normalize_asset_path(image_url))
            }
            _ => None,
        })
        .collect()
}

pub fn scene_mode_from_setting(value: &str) -> SceneMode {
    match value {
        "2d" => SceneMode::Scene2D,
        "columbus" => SceneMode::Columbus,
        _ => SceneMode::Scene3D,
    }
}

pub fn scene_mode_setting(mode: SceneMode) -> &'static str {
    match mode {
        SceneMode::Scene3D => "3d",
        SceneMode::Scene2D => "2d",
        SceneMode::Columbus => "columbus",
    }
}

/// The built-in base map with this id, OpenStreetMap when the id is unknown.
pub fn tile_source_for(id: &str) -> TileSource {
    terra_providers::builtin_tile_sources()
        .into_iter()
        .find(|source| source.id == id)
        .unwrap_or_else(TileSource::open_street_map)
}

pub fn tile_provider(settings: &Settings) -> TileProvider {
    TileProvider::new(tile_source_for(&settings.tile_source_id))
}

/// Enabled overlay presets in catalog order; ids that are not presets are ignored.
pub fn tile_overlays(settings: &Settings) -> Vec<TileOverlay> {
    terra_providers::overlay_presets()
        .into_iter()
        .filter_map(|source| {
            let layer = settings
                .overlay_layers
                .iter()
                .find(|layer| layer.id == source.id && layer.enabled)?;
            Some(TileOverlay {
                source,
                opacity: layer.opacity,
            })
        })
        .collect()
}

pub struct TerraApp {
    pub data: AppData,
    pub i18n: I18n,
}

impl TerraApp {
    pub fn new(cc: &eframe::CreationContext<'_>, deps: AppDeps) -> Result<Self, String> {
        let data = AppData::new(cc, deps)?;
        let i18n = I18n::new(&data.settings.language);
        Ok(Self { data, i18n })
    }

    /// Ephemeral app used by the browser build until IndexedDB lands (M7).
    pub fn new_in_memory(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        let repository: Arc<dyn terra_storage::PlacesRepository> =
            Arc::new(terra_storage::InMemoryPlacesRepository::new());
        Self::new(
            cc,
            AppDeps {
                repository,
                settings: Arc::new(crate::platform::InMemorySettingsStore::default()),
                files: Arc::new(crate::platform::MemoryFileSystem::default()),
                secrets: Arc::new(terra_providers::InMemorySecretStore::new()),
                tile_cache: None,
            },
        )
    }
}

impl eframe::App for TerraApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let TerraApp { data, i18n } = self;
        theme::apply(ui.ctx());
        crate::views::app_ui(data, i18n, ui, frame.wgpu_render_state());
    }
}

/// Runs the native application.
#[cfg(not(target_arch = "wasm32"))]
/// Starts the desktop app; `startup_file` is a KML/KMZ to open once the library is loaded,
/// e.g. from a double-clicked file.
pub fn run_native(deps: AppDeps, startup_file: Option<PathBuf>) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([900.0, 560.0])
            .with_title("Terra Globe"),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "Terra Globe",
        options,
        Box::new(move |cc| {
            TerraApp::new(cc, deps)
                .map(|mut app| {
                    if let Some(path) = startup_file {
                        app.data.open_path(path);
                    }
                    Box::new(app) as Box<dyn eframe::App>
                })
                .map_err(|error| {
                    Box::new(std::io::Error::other(error))
                        as Box<dyn std::error::Error + Send + Sync>
                })
        }),
    )
}

/// Keeps the compiler honest about the visibility helpers the views rely on.
pub(crate) fn folder_is_visible(library: &state::Library, folder: &terra_core::Folder) -> bool {
    is_folder_visible(&library.folders, Some(&folder.id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_mode_settings_round_trip() {
        for mode in [SceneMode::Scene3D, SceneMode::Scene2D, SceneMode::Columbus] {
            assert_eq!(scene_mode_from_setting(scene_mode_setting(mode)), mode);
        }
        assert_eq!(scene_mode_from_setting("unknown"), SceneMode::Scene3D);
    }

    #[test]
    fn tile_provider_follows_the_setting() {
        let mut settings = Settings::default();
        assert_eq!(tile_provider(&settings).source.id, "osm");
        settings.tile_source_id = "opentopomap".to_string();
        assert_eq!(tile_provider(&settings).source.id, "opentopomap");
    }

    fn editor() -> OverlayEditor {
        OverlayEditor {
            north: "10".into(),
            south: "0".into(),
            east: "20".into(),
            west: "5".into(),
            rotation: "0".into(),
            opacity: 1.0,
            href: "a.png".into(),
            error: None,
        }
    }

    #[test]
    fn overlay_editor_builds_a_valid_geometry() {
        let mut fields = editor();
        fields.rotation = " 30 ".into();
        fields.opacity = 0.5;
        let geometry = fields.to_geometry().expect("valid");
        assert_eq!(OverlayEditor::from_geometry(&geometry), {
            let mut expected = fields.clone();
            expected.rotation = "30".into();
            Some(expected)
        });
    }

    #[test]
    fn overlay_editor_rejects_invalid_input() {
        type Case = (&'static str, fn(&mut OverlayEditor));
        let cases: [Case; 7] = [
            ("north below south", |e| e.north = "-5".into()),
            ("latitude beyond the pole", |e| e.north = "91".into()),
            ("longitude beyond the date line", |e| e.east = "181".into()),
            ("west east of east", |e| e.west = "30".into()),
            ("not a number", |e| e.south = "abc".into()),
            ("bad rotation", |e| e.rotation = "spin".into()),
            ("blank image", |e| e.href = "  ".into()),
        ];
        for (name, mutate) in cases {
            let mut fields = editor();
            mutate(&mut fields);
            assert!(fields.to_geometry().is_err(), "{name}");
        }
    }

    #[test]
    fn unknown_basemaps_fall_back_to_openstreetmap() {
        assert_eq!(tile_source_for("nope").id, "osm");
        assert_eq!(tile_source_for("opentopomap").id, "opentopomap");
    }

    #[test]
    fn tile_overlays_skip_disabled_and_unknown_layers() {
        let layer = |id: &str, enabled| OverlayLayerSetting {
            id: id.into(),
            enabled,
            opacity: 0.5,
        };
        let settings = Settings {
            overlay_layers: vec![
                layer("openseamap", true),
                layer("openrailwaymap", false),
                layer("ghost", true),
            ],
            ..Settings::default()
        };
        let ids: Vec<String> = tile_overlays(&settings)
            .into_iter()
            .map(|overlay| overlay.source.id)
            .collect();
        assert_eq!(ids, vec!["openseamap"]);
    }
}
