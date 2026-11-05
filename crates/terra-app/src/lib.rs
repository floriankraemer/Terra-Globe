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

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use eframe::egui;
use terra_core::{Settings, is_folder_visible};
use terra_globe::imagery::TileProvider;
use terra_globe::{Camera, Globe, Scene, SceneEntry, SceneMode};
use terra_providers::{SecretStore, TileSource};
use web_time::Instant;

use crate::i18n::I18n;
use crate::platform::{FileSystem, PersistedState, SettingsStore};
use crate::state::StateStore;
use crate::tools::{JobRunner, RouteState, RulerState};

/// Everything the UI needs, injected by the shell.
pub struct AppDeps {
    pub repository: Arc<dyn terra_storage::PlacesRepository>,
    pub settings: Arc<dyn SettingsStore>,
    pub files: Arc<dyn FileSystem>,
    pub secrets: Arc<dyn SecretStore>,
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
        let settings = persisted.settings.clone();

        Ok(Self {
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
        })
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
        let document = if path.extension().and_then(|value| value.to_str()) == Some("kmz") {
            match terra_kml::read_kmz(&bytes) {
                Ok(entries) => entries
                    .iter()
                    .find(|entry| entry.name.ends_with(".kml"))
                    .and_then(|entry| String::from_utf8(entry.bytes.clone()).ok()),
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
        let batch = self.store.export_batch();
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
            match terra_kml::write_kmz(&kml, &[]) {
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
        if self.settings.tile_source_id == "opentopomap" {
            TileSource::open_topo_map()
        } else {
            TileSource::open_street_map()
        }
    }
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

pub fn tile_provider(settings: &Settings) -> TileProvider {
    let source = if settings.tile_source_id == "opentopomap" {
        TileSource::open_topo_map()
    } else {
        TileSource::open_street_map()
    };
    TileProvider::new(source)
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
pub fn run_native(deps: AppDeps) -> eframe::Result<()> {
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
                .map(|app| Box::new(app) as Box<dyn eframe::App>)
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
}
