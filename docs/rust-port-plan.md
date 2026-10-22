# Port Terra Globe to a pure-Rust desktop app with a custom 3D globe

## Context

Terra Globe today is a React + TypeScript + CesiumJS frontend (~15k lines across `packages/core|map|ui|storage-*`) in a thin Tauri v2 Rust shell. The goal is the same product (same features, same look) as one Rust code base with its own 3D globe engine, shipping native binaries for Windows, macOS and Linux via `make dist` / `make dist-<os>`, developed test-first on branch `claude/quirky-dijkstra-oaby2v`, not merged until approved.

Decisions taken with the user:
- **Desktop only now, WASM-ready later**: no browser-only code; every platform service is a trait; `terra-core`, `terra-geo`, `terra-scene` have no tokio/reqwest/rusqlite/wgpu/egui dependency so they compile to `wasm32` untouched.
- **3D globe only**: 2D / Columbus modes and the View menu are dropped.
- **`make dist-macos` is native-only** (must run on a Mac). Windows is cross-built from Linux with `cargo-xwin` as today.
- **Side by side**: the Rust app lives in a new top-level `rust/` workspace; `packages/`, `src-tauri/`, `e2e/` stay untouched on this branch. Removal of the TS code is a later PR after parity.

Feature inventory to reproduce (from the TS app; see `packages/ui/src/features/*`): drawing (marker, rectangle, circle, polygon, line with live preview, Finish/Cancel), folder tree (expand, visibility checkbox, rename, delete, New Folder, search filter, drag-and-drop with before/inside/after rules), placemark editor (floating draggable/resizable panel: name, description, visible, location/center in decimal or DMS, radius/circumference/area, outline/fill styling, 6 Lucide marker icons, distance rings, Save/Close with unsaved-changes confirm, Delete confirm, live preview with revert), drag-to-move on the map, name labels within 50 km scaled by distance, ruler (segments + total panel), route planner (car/foot/bike via OSRM/GraphHopper/ORS/Mapbox; train/plane straight-line estimates; stops list with reorder; alternatives), address search (Nominatim/LocationIQ/Mapbox/OpenCage), elevation profile chart, undo/redo (20 snapshots, Ctrl/Cmd+Z, Shift+Z, Y), KML/KMZ import/export (ExtendedData round-trip for circle/rectangle, MultiGeometry split/regroup, gx:Track, GroundOverlay, ScreenOverlay, Model, StyleMap, NetworkLink depth 5 with cycle detection, KMZ assets), desktop Load/Save/Save As with autosave (1500 ms) and exit confirm, settings (metric/imperial, decimal/DMS, 12 languages, providers with API keys in the OS keychain, autosave toggle), about modal, notices, basemap menu (OSM, OpenTopoMap, Mapbox/MapTiler/Thunderforest presets with `{api_key}`), saved camera pose, saved sidebar width and panel geometry. No terrain: flat WGS84 ellipsoid.

## Engine choice and why

No mature Rust "globe with imagery tiles" crate exists (Galileo is 2D with a globe on its roadmap; Bevy/rend3 are general engines, and Bevy would force an ECS and a fast-moving API on what is mostly UI + data). Chosen stack, all MIT/Apache/ISC (GPLv3-compatible, enforced by `cargo-deny`):

| Concern | Crate (latest stable, Oct 2026) | Why |
|---|---|---|
| Window/UI | `eframe`/`egui`/`egui-wgpu` 0.37 | Immediate mode, custom-styled from the CSS tokens, AccessKit tree → headless UI tests via `egui_kittest` 0.37 (the Playwright replacement). WASM-capable. |
| 3D | `wgpu` 30 | Vulkan/Metal/DX12/GL + WebGPU. The globe renders into its own offscreen target that egui composites. |
| Math | `glam` 0.34 (`DVec3`/`DMat4`/`DQuat`) | Earth-scale coordinates need f64 on the CPU. |
| Storage | `rusqlite` 0.40 (`bundled`) | Same schema and JSON `data` column as `packages/storage-sqlite/src/schema.ts`, so existing `terra-globe.db` files open unchanged. |
| KML/KMZ | `quick-xml` 0.42, `zip` 9 | streaming parser |
| HTTP | `reqwest` 0.13 with `default-features=false, features=["rustls-no-provider","http2","gzip","json"]` + `rustls` `ring` provider, `tokio` 1 | avoids aws-lc-rs (cmake) which breaks cargo-xwin; `ring` only needs `nasm` on windows-msvc |
| Images | `image` 0.25 (tiles, overlays), `resvg` 0.48 (marker SVGs) | |
| Secrets / dialogs | `keyring` 4 (apple-native / windows-native / zbus-secret-service, pure Rust), `rfd` 0.17 (xdg-portal, no GTK) | same keychain service `terra-globe-provider` as today |
| Misc | `serde`/`serde_json`, `uuid` 1, `thiserror` 2, `directories` 6, `lru` 0.19, `earcutr` 0.5, `gltf` (M11), `tracing`, `winresource` (Windows icon/version), `cargo-xwin` 0.23, `cargo-deb`, `cargo-deny` | |

## How the 3D projection works (what gets implemented)

1. **Ellipsoid**: WGS84 (a = 6378137, b = 6356752.314245). Geodetic (lon, lat, h) ↔ ECEF `DVec3`, surface normal, scale-to-unit-sphere helper (used by culling and pan). All f64.
2. **Camera**: Google-Earth-style look-at model `{ target: geodetic, range_m, heading, pitch (−90° = nadir), roll 0 }` → eye position + East-North-Up frame → f64 view matrix with the eye at the origin. Projection: perspective, **reversed-Z with infinite far plane** (`Depth32Float`, clear 0, compare `Greater`), `near = clamp(height·1e-4, 0.5 m, 1 km)`. Persisted in the TS shape `{longitude, latitude, height, heading, pitch, roll}` (`terra-globe:cameraView`).
3. **Per-object relative-to-centre (RTC) origins**: each tile and entity mesh is stored once in f32 relative to its own f64 origin (tile centre / entity centroid). Per draw, `offset = origin − eye` is computed in f64 and cast to f32; the uploaded f32 view-projection holds rotation + projection only. Worst case error ≈ 6 cm at 1000 km, sub-pixel wherever the object is visible.
4. **Globe surface**: a quadtree of **Web Mercator XYZ tiles** (`z/x/y`, the format of every basemap in the app). Each tile is a 16×16 vertex grid on the ellipsoid: lon linear in x, lat from mercator y via the Gudermannian; UVs linear in mercator space so raster tiles drape exactly. Skirts hide cracks between LOD levels; two flat polar caps cover |lat| > 85.05°. Textures live in one `texture_2d_array` atlas (256 px, 512 layers) with LRU eviction.
5. **Level of detail**: refine from z=0 while screen-space error `geometric_error(z)·viewport_h / (2·dist·tan(fov/2))` > 2 px and z < the source's max level. Culling: frustum test on the bounding sphere + **horizon culling** (sample points in unit-sphere space). If a selected tile is not loaded, draw it with the nearest loaded ancestor's texture and a UV sub-rect (no holes); the root tile loads first so the globe is never blank.
6. **Picking**: screen point → f64 ray → ray/ellipsoid intersection in unit-sphere-scaled space. Entity picking: marker screen rects first, then ground point → point-in-polygon / distance-to-segment in metres.
7. **Navigation** (Cesium defaults re-created): left drag pans so the picked ground point stays under the cursor (rotate the camera about the Earth centre by `DQuat::from_rotation_arc(p_now, p_start)` every frame); wheel zooms toward the cursor (range ×0.85 per tick, clamped 20 m…40 000 km, same arc correction); right/Ctrl-drag changes heading/pitch around the target (pitch clamped −89.9°…−5°); double-click and tree double-click/geocode/fly-to-geometry run an eased fly-to (slerp on target, log-lerp on range with a mid-flight bump, nadir pitch, min radius 500 m as in `flyTo.ts`). Camera is persisted 400 ms after the last input.
8. **Vector entities**: fills (circle = 64-gon, rectangle = graticule-aligned grid, polygon with holes) are densified (≤ 1° per edge), triangulated with `earcutr` in a local tangent plane, placed on the ellipsoid and rendered lifted along the normal by `max(1 m, 2e-4·dist)` plus a depth bias so they never z-fight the globe; the depth test still hides them behind the horizon. Outlines and lines are instanced **constant-pixel-width polylines**: the vertex shader projects both segment endpoints, clips at the near plane and extrudes a screen-space quad (the Cesium technique). Markers, name labels and distance-ring labels are projected each frame and drawn with egui's painter (crisp text, free hit-testing, horizon-hidden). GroundOverlay = textured patch; Model = glTF (M11), non-glTF falls back to a marker as today.

## Rust workspace layout (`rust/`)

Dependency direction is one-way, mirroring today's `core → map → ui` rule, enforced by Cargo manifests: `terra-core ← terra-geo ← terra-scene ← terra-render ← terra-app`; `terra-platform` depends only on `terra-core`; only `terra-app` depends on `terra-platform`, `eframe`, `egui_kittest`.

```
rust/
├── Cargo.toml                 workspace: [workspace.dependencies] pins, [workspace.lints] (clippy::all, unsafe_code forbid
│                              except terra-render), release profile (lto, codegen-units=1, opt-level s, strip, panic abort)
├── deny.toml                  license allow-list (MIT/Apache-2.0/ISC/BSD/Zlib/OFL/Unicode) + bans
├── crates/
│   ├── terra-core/            domain (Folder, Placemark, Style, ScreenOverlay, Geometry enum tagged "type"),
│   │                          validation rules, measurements/units/coordinate formatting, visibility, translate,
│   │                          track profile, route estimates, KML/KMZ, provider catalog + Geocoding/Routing
│   │                          providers generic over HttpClient, ALL ports, In-memory doubles, contract tests
│   ├── terra-geo/             f64 WGS84 math, mercator tiling, camera model + projection, ray picking,
│   │                          frustum/horizon culling, SSE LOD selection, ellipsoid tessellation, fly-to curves
│   ├── terra-scene/           CPU scene layer (the TS `map` package): EntityLayer/ScreenPicker ports,
│   │                          DrawingController, RulerController, RouteController, EntitySynchronizer,
│   │                          LibraryService, History (undo/redo), TileScheduler, CameraController,
│   │                          LabelPlacer, Picker, DistanceRings, FileSyncState, tree drag-drop rules; Fake* doubles
│   ├── terra-render/          wgpu GlobeRenderer: pipelines + WGSL (globe_tile, polygon_fill, polyline, model),
│   │                          tile atlas, GPU meshes, offscreen target + readback, TileLoader (tokio); implements EntityLayer
│   ├── terra-platform/        desktop adapters: SqlitePlacesRepository, KeyringSecretStore, RfdFileDialogs,
│   │                          StdFileIo, ReqwestHttpClient, JsonPreferencesStore (replaces localStorage),
│   │                          DiskTileCache, SystemClock, UuidIdGenerator
│   └── terra-app/             eframe binary `terra-globe`: composition root, theme, i18n, features/*, egui_kittest tests
├── packaging/                 linux/ (desktop file, icons), windows/ (ico, installer.nsi), macos/ (Info.plist.in, icns)
├── scripts/                   dist-linux.sh, dist-windows.sh, dist-macos.sh, set-version.sh, export-locales.mjs
└── fixtures/                  JSON/KML goldens and a schema-v2 terra-globe.db written by the TS app
```

Key ports (object-safe, `Send + Sync`):
- `terra_core::ports::PlacesRepository` — 1:1 with `PlacesRepository.ts`, **sync** (rusqlite and the in-memory repo are sync); `RepositoryError::{NotFound{kind,id}, NotEmpty, Backend}`. Run on a worker thread in the app; controls stay disabled until `LibraryService::ready()` (AGENTS.md rule).
- `SecretStore`, `Clock`, `IdGenerator`, `HttpClient` (returns `BoxFuture`, no tokio in core), `GeocodingProvider`, `RoutingProvider`, `TileSource`, `FileDialogs`, `FileIo`, `PreferencesStore`.
- `terra_scene::EntityLayer` (`create/update/remove/set_visible/begin_live_geometry_edit/set_live_geometry`) and `ScreenPicker` — the `IEntityFactory`/`IScreenPicker` seams; `FakeEntityLayer` for tests.
- `terra_app::GlobeView` (`pick_ground`, `pick_entity`, `fly_to`, `camera_view`, `set_base_layer`, `entity_layer_mut`) implemented by the wgpu view and by `FakeGlobeView` so every UI test runs without a GPU.

Serde compatibility is a hard requirement: `#[serde(rename_all = "camelCase")]`, `Option` fields `skip_serializing_if = "Option::is_none"` (TS omits `undefined`), geometry `#[serde(tag = "type")]`, numeric `order` as `f64`.

## Globe ↔ egui integration (important detail)

Do **not** draw the globe inside `egui_wgpu::CallbackTrait::paint`: that pass is egui's own colour target with egui's depth setup, so reversed-Z, custom clears and MSAA are impossible there. Instead the `GlobeRenderer` renders into its own offscreen colour (`Rgba8UnormSrgb`, 4× MSAA resolved) + `Depth32Float` target using the device/queue from `frame.wgpu_render_state()`, submits before egui's pass, and the resolved texture is registered once via `egui_wgpu::Renderer::register_native_texture` (re-registered on resize) and painted full-window in an `egui::Area` with `Order::Background`. Floating panels (`egui::Window`/`Area`) sit on top and take pointer input first; the background `Area` interaction (`Sense::click_and_drag`) plus `!ctx.wants_pointer_input()` gates globe navigation. The same `GlobeRenderer::render(device, queue, target, size)` call is what headless tests call. Tile arrivals, async results and animations call `ctx.request_repaint()` through one `AsyncBridge` (owns the tokio runtime + a `Context` clone) so no feature can forget it.

## Milestones (each ends green: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo deny check && cargo test --workspace`)

Every step is test-first: failing test → implementation → refactor. Test modules named per milestone.

- **M0 Workspace + domain + JSON compat** — `rust/` workspace, `rust-ci.yml`, `terra-core::domain`. Tests: `domain_json_compat.rs` (deserialise JSON fixtures exported from the TS app via `scripts/export-fixtures.mjs`, re-serialise byte-equal), `units.rs`, `measurements.rs`, `visibility.rs`, `geometry_validation.rs` ported 1:1 from the vitest cases.
- **M1 Storage + ports** — `PlacesRepository`, `InMemoryPlacesRepository`, `#[macro_export] repository_contract_tests!(make_repo)` reproducing the 19 cases in `repositoryContractTests.ts`; `SqlitePlacesRepository` passing the macro + `sqlite_compat.rs` opening `fixtures/terra-globe-ts-v2.db`; `SecretStore` (+ in-memory, keyring with `#[ignore]` live test), `Clock`/`IdGenerator` fakes, `JsonPreferencesStore`.
- **M2 KML/KMZ** — `kml_parse.rs`, `kml_serialize.rs` (goldens from the TS serializer), `kml_roundtrip.rs`, `kml_resilience.rs`, `kmz.rs`, `network_link.rs` (`FakeHttpClient`). Fixtures from `e2e/playwright/fixtures`.
- **M3 terra-geo** — `ellipsoid.rs` (round-trip ≤ 1e-9 m), `mercator.rs`, `camera.rs` (look-at ↔ CameraView), `projection.rs` (reversed-Z invariants), `pick.rs`, `culling.rs`, `lod.rs` (expected tile sets for given cameras), `flyto.rs`, `tessellate.rs` (holes, antimeridian, vertices on ellipsoid), `polyline_geometry.rs`.
- **M4 terra-scene** — `drawing_controller.rs` (port of `packages/map/test/drawing/*`), `ruler.rs`, `route.rs`, `entity_synchronizer.rs`, `library.rs` (port of `useLibrary.ts`: ordering, move/reorder, visibility propagation, snapshot export/restore, refuse moving a folder into its own subtree), `tree_drag_drop.rs` (port of `treeDragDrop.test.ts`), `undo_redo.rs` (cap 20), `tile_scheduler.rs` (priority, 2-per-host cap, cancel, parent fallback), `camera_controller.rs` (picked point stays under cursor to 1e-6°, zoom-to-cursor, pitch clamps, 1000-step pan stability), `labels.rs`, `picking.rs`, `distance_rings.rs`, `screen_overlay_layout.rs`, `file_sync.rs` (debounce with fake clock).
- **M5 terra-render MVP** — tiles + caps + skirts + parent fallback + atlas LRU + offscreen target + readback; dev binary showing OSM. Tests: `render_globe.rs` with `TestGpu::try_new()` (skip when no adapter) and `FakeTileFetcher` checkerboards (disc centred, no holes after fallback, lifted fill wins over tile); PNG snapshot with tolerance.
- **M6 App shell** — `theme.rs` (CSS tokens → `egui::Visuals` light/dark: bg `#ffffff`/`#1a1d23`, panel `#ffffff`/`#23262d`, hover `#f1f3f5`/`#2c2f37`, border `#e1e4e9`/`#383c44`, text `#1a1d23`/`#e7e9ec`, muted `#6b7280`/`#9aa1ac`, primary `#2563eb`/`#5b8def`, danger `#dc2626`/`#f87171`, radii 10/6 px, shadow (0,4,16, rgba(15,23,42,.12)), 13 px body, 12 px uppercase headers; embedded Inter + system font fallback for CJK/Devanagari), `i18n.rs` (locales exported to JSON by `scripts/export-locales.mjs`; `i18n_keys.rs` asserts every locale matches `en`'s key set; `{{x}}` interpolation + `_one/_other` plurals), a11y helper for labelled toolbar groups, top bar (File, Map, Undo/Redo, Drawing tools, Ruler, Route, Address search, Settings, About), places panel (resizable 200–600 px, header, search, tree with egui `dnd_*`), notices, confirm modals. First kittest tests `ui_drawing.rs`, `ui_folders.rs`, `ui_placemark_editor.rs` on `FakeGlobeView` + in-memory repo.
- **M7 Tools and panels** — placemark editor (all fields, icons, rings, confirms, live preview/revert), ruler panel, route planner panel, address search, elevation profile (painter chart; `profile_layout.rs` pure), undo/redo toolbar + shortcuts, settings modal (units, format, languages, providers + Test, autosave), about modal, basemap menu. Tests: `ui_ruler.rs`, `ui_route_planner.rs`, `ui_geocoding.rs`, `ui_elevation_profile.rs`, `ui_undo_redo.rs`, `ui_providers.rs`, `ui_shape_styling.rs`, `ui_map_interaction.rs`.
- **M8 Files and persistence** — File menu (Import/Export KML/KMZ, Load/Save/Save As via `FakeFileDialogs` in tests), autosave, exit confirm (`ViewportCommand::CancelClose`), camera/sidebar/panel geometry persistence. Tests: `ui_import_export.rs`, `ui_import_resilience.rs`, `ui_persistence.rs`, `ui_camera_persistence.rs`.
- **M9 Render parity** — fills/outlines/live preview/drag-to-move, lines, labels, markers (resvg-rasterised Lucide icons), ground + screen overlays, basemap switching, disk tile cache with `Cache-Control`, retry/backoff, OSM attribution overlay, `User-Agent: TerraGlobe/<ver> (+repo URL)`.
- **M10 Packaging** — section below; `ui_snapshot_chrome.rs` replaces `visual.spec.ts`.
- **M11 Parity audit + Model (glTF)** — `gltf` + `model.wgsl`; walk the inventory and the spec-mapping table; update `README.md`, `docs/architecture.md`, `AGENTS.md` for the Rust workspace.

## Build, dist and CI

- **`docker/Dockerfile.rust`** (`rust:1-bookworm`): `clang lld llvm nasm pkg-config libxkbcommon-dev libxcb-*-dev mesa-vulkan-drivers nsis squashfs-tools file` + pinned `appimagetool`; `rustup target add x86_64-pc-windows-msvc`; `cargo install cargo-xwin cargo-deb cargo-deny --locked`; `rustup component add clippy rustfmt` last (same ordering caveat as the existing Dockerfile). New `rust-build` compose service with named volumes for `rust/target` and the cargo registry.
- **Makefile** (existing targets untouched; `dist/` is already gitignored):
  - `rust-test`, `rust-lint` (fmt + clippy + deny) via the container.
  - `dist-linux` → `scripts/dist-linux.sh`: `cargo build --release -p terra-app`, `cargo deb --no-build` (`[package.metadata.deb]`: `/usr/bin/terra-globe`, desktop file with KML/KMZ MIME types, hicolor icons), AppDir + `appimagetool --appimage-extract-and-run`, tar.gz → `dist/linux/`.
  - `dist-windows` → `scripts/dist-windows.sh`: `cargo xwin build --release -p terra-app --target x86_64-pc-windows-msvc`; `build.rs` + `winresource` embeds `icon.ico` and version (uses `llvm-rc`); `#![cfg_attr(windows, windows_subsystem = "windows")]`; `makensis packaging/windows/installer.nsi` → `dist/windows/TerraGlobe-Setup-x64.exe` + `TerraGlobe-x86_64.exe` (unsigned, as today).
  - `dist-macos` → guard `uname -s = Darwin` else exit with message; `scripts/dist-macos.sh`: build `aarch64-apple-darwin` + `x86_64-apple-darwin`, `lipo`, assemble `TerraGlobe.app` (`Info.plist.in` with `com.terraglobe.app`, document types, `icon.icns` reused from `src-tauri/icons`), ad-hoc `codesign`, `hdiutil create` → `dist/macos/TerraGlobe-<ver>.dmg`. Only Command Line Tools needed.
  - `dist` = `dist-linux dist-windows` (+ `dist-macos` when the host is macOS, otherwise prints a note).
- **CI**: `.github/workflows/rust-ci.yml` (ubuntu: fmt, clippy, deny, test with `WGPU_BACKEND=vulkan` on lavapipe; `windows-cross` job runs `cargo xwin build`; macos job builds + tests). `.github/workflows/rust-release.yml` on tags `rust-v*` running `make dist-linux dist-windows` on ubuntu and `make dist-macos` on macOS, uploading `dist/**`. Existing `ci.yml`/`release.yml` stay as they are. `scripts/set-version.sh` syncs the workspace version from the tag.

## Playwright specs → egui_kittest tests

Harness: `Harness::builder().with_size((1280., 800.)).build_eframe(|cc| TerraApp::new_with(TestDeps{ in-memory repo, FakeGlobeView, FakeFileDialogs, FakeHttpClient, FakeClock, InMemorySecretStore }))`; queries via `get_by_label` / `get_by_role_and_label` scoped to labelled groups ("Drawing tools", "Undo/redo"), actions via `click`, `type_text`, `drag_at/drop_at`, `key_press_modifiers`. "Reload" = a second `TerraApp` on the same repo/preferences.

| Playwright spec | kittest module | Notes |
|---|---|---|
| globe | `ui_basemap.rs` + `render_globe.rs` | basemap switch recorded by `FakeGlobeView` |
| drawing | `ui_drawing.rs` | toggled state, Finish/Cancel semantics |
| map-interaction | `ui_map_interaction.rs` + `flyto.rs` | editor reopen on pick; fly-to call recorded |
| placemark-editor, shape-styling | `ui_placemark_editor.rs`, `ui_shape_styling.rs` | modals by role Dialog; disabled state via AccessKit |
| tree-drag-drop | `ui_tree_dnd.rs` + `tree_drag_drop.rs` | |
| persistence, cameraPersistence | `ui_persistence.rs`, `ui_camera_persistence.rs`, `sqlite_compat.rs` | |
| undo-redo | `ui_undo_redo.rs` | keyboard shortcuts |
| import-export, import-resilience | `ui_import_export.rs`, `ui_import_resilience.rs` | notices by role Alert, Dismiss |
| elevation-profile, ruler, route-planner, geocoding, providers | one `ui_*.rs` each | fakes for HTTP/providers |
| visual | `ui_snapshot_chrome.rs` | top 210 px snapshot, globe replaced by a flat colour |

## Risks and mitigations

- **Pixel-identical UI is impossible without a web renderer**; egui reproduces layout, sizes, colours, radii, shadows and fonts. Verified by the chrome snapshot test.
- **Old GPUs / no Vulkan**: wgpu falls back to GL; shaders stay within downlevel limits; adapter failure shows an rfd message box.
- **Tile policies**: identifying User-Agent, 2 requests per host, disk cache (500 MB cap), visible attribution, API keys only in `SecretStore` and never in cache keys.
- **Large KML**: tessellation off the UI thread via the same channel pattern as tiles; `importBatch` in one transaction; tree rows virtualised.
- **cargo-xwin**: aws-lc-rs avoided; rusqlite `bundled` builds with clang-cl; the `windows-cross` CI job catches regressions.
- **Keychain unavailable on Linux**: surfaced as a notice with the same warning the TS app shows for localStorage secrets.

## Multi-agent execution

The first execution step commits this plan to the repository as `docs/rust-port-plan.md` (plan mode forbids repo edits until approval). That file is the single brief every sub-agent is pointed at; agents never receive the chat history.

**Principles**
- **Contract first, then fan out.** The coordinator (this session) writes the workspace skeleton, every port trait, the domain types and the `Fake*` doubles in M0 before any agent starts, so parallel agents code against fixed interfaces and never block on each other.
- **One agent = one crate or one feature module, in its own git worktree** (`isolation: "worktree"`), returning a branch that passes `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test -p <crate>`. The coordinator merges worktrees in dependency order and runs the full workspace check after each merge.
- **Cheapest adequate model per task**: `haiku` for mechanical translation and scaffolding, `sonnet` for well-specified crates with a TS reference implementation to port, `opus` only for the renderer, camera/navigation math, egui↔wgpu integration and the final review. Fresh agents (not forks) everywhere: a fork inherits this whole context and is the most expensive option.
- **Every brief contains**: the plan section to implement, the exact TS source and test files to port, the trait signatures to implement, the test module names expected, and the done criterion (tests listed above green). Agents report back only the branch name, test count and open questions.
- **Review gate per wave**: one `opus` code-review agent reads each merged wave's diff adversarially (`/code-review` on the merge commit) before the next wave is briefed.

**Waves** (agents inside a wave run concurrently; a wave starts when its inputs are merged)

| Wave | Agent (model) | Scope | Inputs |
|---|---|---|---|
| 0 | coordinator | `rust/` workspace, `deny.toml`, all port traits, domain types + serde, `Fake*` doubles, `docs/rust-port-plan.md`, `rust-ci.yml` | — |
| 1 | core-domain (sonnet) | M0 tests + impl: units, measurements, validation, visibility, translate, track profile, route estimates | wave 0 |
| 1 | geo-math (opus) | M3 `terra-geo` entire crate | wave 0 |
| 1 | fixtures+locales (haiku) | `scripts/export-fixtures.mjs`, `scripts/export-locales.mjs`, committed JSON/KML goldens, `terra-globe-ts-v2.db` fixture, `i18n_keys.rs` | wave 0 |
| 1 | packaging (sonnet) | `docker/Dockerfile.rust`, compose service, Makefile `dist*`/`rust-*` targets, `scripts/dist-*.sh`, `packaging/*`, `build.rs` + winresource, `rust-release.yml` | wave 0 |
| 2 | storage (sonnet) | M1: in-memory + SQLite repos, contract-test macro, preferences store, keyring adapter | wave 1 core |
| 2 | kml (sonnet) | M2: parser, serializer, KMZ, NetworkLink resolver | wave 1 core + fixtures |
| 2 | providers (sonnet) | geocoding/routing/tile-test adapters over `HttpClient`, provider catalog, reqwest adapter | wave 1 core |
| 2 | scene-tools (sonnet) | M4: DrawingController, Ruler/RouteController, EntitySynchronizer, LibraryService, History, tree DnD rules, FileSyncState, distance rings | wave 1 core + geo |
| 2 | scene-camera (opus) | M4: CameraController, TileScheduler, LabelPlacer, Picker, screen-overlay layout | wave 1 geo |
| 2 | renderer (opus) | M5: `terra-render` tiles/caps/skirts/atlas/offscreen target, dev binary; then M9 vector/polyline/overlay passes | wave 1 geo |
| 3 | app-shell (opus) | M6 core: eframe app, theme, i18n runtime, a11y helpers, globe↔egui compositing, AsyncBridge, `GlobeView` + `FakeGlobeView`, composition root, test harness builder | wave 2 all |
| 4 | feature agents (sonnet ×5) | places panel + tree; drawing toolbar + placemark editor + drag-to-move; ruler + route planner + geocoding + elevation profile; settings + providers tab + about + basemap menu; file menu + import/export + autosave + exit confirm + persistence (M7/M8 split by feature, each with its `ui_*.rs` tests) | wave 3 |
| 4 | render-parity (opus) | M9 remaining: markers/labels via painter, ground/screen overlays, disk cache, attribution | wave 3 |
| 5 | model-gltf (sonnet) | M11 glTF pass + fallback | wave 4 |
| 5 | docs (haiku) | README, `docs/architecture.md`, AGENTS.md updates for the Rust workspace | wave 4 |
| 5 | parity-audit + review (opus) | walk the feature inventory and spec table, run the full suite, `make dist-linux dist-windows`, report gaps; final adversarial review | wave 4 |

Expected critical path: wave 0 → geo-math → renderer → app-shell → feature agents → audit. Everything else overlaps with it. Coordinator work between waves is limited to merging, running the workspace check, and briefing the next wave.

## Verification

1. `cd rust && cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo deny check && cargo test --workspace` (unit, contract, kittest UI tests; render snapshots skip without an adapter).
2. `make dist-linux` and `make dist-windows` (Docker) produce `dist/linux/*` and `dist/windows/*`; run the Linux binary: draw each shape, save/reload, import `e2e/playwright/fixtures/*.kml`, switch basemap, open a `terra-globe.db` written by the Tauri app.
3. Existing TS checks remain green and untouched: `pnpm -r test`, `pnpm -r lint`.
