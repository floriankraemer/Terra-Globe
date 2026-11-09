# Rust architecture

This document describes the Rust rewrite that lives in `crates/`.
The TypeScript app in `packages/` is frozen and is removed at milestone M6; until then it is the behavioural reference for parity work.

Status: M0–M7 are implemented: foundations, globe rendering, data layer, map editing, the application UI, the 2D/Columbus scene modes, release packaging configuration, and the browser storage adapter.
See `docs/release.md` for the release process and remaining release-engineering steps.

## The one rule

**Domain logic never imports a rendering engine, a browser API, a database driver or an OS shell.**
Every such concern is a trait ("port") implemented by an adapter and selected once in a composition root (`terra-desktop` / `terra-web`).

`scripts/check-rust-deps.py` enforces this against `cargo metadata`.
It fails on an illegal internal dependency (for example `terra-core` → `terra-globe`) or a forbidden external one (`terra-globe` → `egui`), and it fails when a new crate is not registered in the checker.

```
terra-core          domain model, units, settings, ids, timestamps, Patch semantics
├── terra-kml       KML/KMZ parser, serializer, mappings, NetworkLink resolution
├── terra-providers tile sources, provider configs, geocode/route types
├── terra-storage   PlacesRepository port, contract suite, in-memory adapter, collect_all
│   ├── terra-storage-sqlite      schema + migration + full repository
│   ├── terra-storage-indexeddb   wasm-only adapter (M7)
│   └── terra-storage-remote      placeholder keeping the sync seam alive
├── terra-globe     wgpu renderer: geodesy, camera, tiles, imagery, picking
└── terra-testkit   differential fixtures and image-snapshot comparison

terra-app           egui state and views (consumes terra-globe + ports)
├── terra-desktop   native binary `terra-globe`
├── terra-web       wasm entry point
└── terra-cli       `terra-kml` import/export/round-trip tool
```

## Renderer

`terra-globe` has no UI dependency; it renders into an `Rgba8Unorm` texture that the UI registers and paints.

- **Coordinates**: all geodesy and camera math is `f64` WGS84. Per frame the eye is computed in `f64`, and the GPU receives eye-relative `f32` (`f32(tile_center - eye)`) plus `f32` vertices local to each tile centre.
- **Depth**: reverse-Z with an infinite far plane (`Depth32Float`, compare `Greater`, clear `0.0`, near `1 m`). Near maps to 1.0, infinity to 0.0.
- **Surface**: a Web-Mercator quadtree. Tiles are ellipsoid meshes with skirts; there is no separate sphere underneath, so nothing z-fights. The poles (beyond ±85.05°) are two static cap meshes drawn in the fallback colour.
- **LOD**: best-first screen-space refinement with a tile budget (160 by default). The traversal always emits a complete cover of the visible surface - it refines the tile whose texels are largest on screen (target: one texel per pixel) and turns everything it cannot refine into a leaf - so exhausting the budget only coarsens the lowest-error part of the view, never leaves a hole. Frustum culling plus a horizon test widened by each tile's angular size keep the back of the globe out without cutting a notch at the limb. When a tile's texture has not arrived, the nearest ready ancestor's texture is sampled through a UV sub-rectangle; with no ancestor the fallback colour keeps the surface solid instead of showing a hole.
- **Imagery**: `TileProvider` builds URLs from the same presets as the web app. `TileFetcher` uses `ehttp` (threads natively, `fetch` on wasm), keeps a priority queue rebuilt from the current selection every frame, runs at most 8 requests at a time, retries transient failures up to three times and holds a given-up tile back for 30 s before trying again, so a flaky network does not permanently punch a hole in the imagery. Decoded tiles are normalised to 256×256 and their mip chain is built in linear light, through lookup tables for the sRGB transfer function; the sampler uses trilinear filtering with anisotropic filtering, so minified imagery does not shimmer at the horizon. The native shell also injects a `TileCache` that keeps downloaded tile bytes on disk (512 MB budget, pruned at startup), so revisiting an area does not hit the network again.
- **Colour management**: tile textures are `Rgba8UnormSrgb`, shading happens in linear space, and the fragment shader encodes to sRGB before writing the `Rgba8Unorm` target, because egui samples user textures as sRGB.
- **Overlays**: markers and labels are projected by `Globe::project` (with an analytic horizon test) and drawn by egui in screen space, so text is crisp and shaped by HarfBuzz.

## Terrain

Terrain is desktop-only and optional (`Settings.terrain_enabled`, default on; the browser build cannot reach the elevation server and never enables it).
`terra_globe::terrain` holds everything CPU-side, and the renderer, picking, camera and app code share one lookup, `GroundFn` (`lon, lat -> Option<height>`).

- **Source**: AWS Terrain Tiles in Terrarium encoding (`TileSource::terrain_terrarium()`, zoom 0 to 15, 256 by 256 PNG, `R * 256 + G + B / 256 - 32768` metres).
  The tiles come through the same `TileFetcher` and `TileCache` as imagery, so the desktop disk cache covers them.
  The fetcher runs in raw mode: it never rescales a tile and rejects any size but 256 by 256 on the fetch thread, so a bad response is retried and then given up rather than re-requested every frame.
- **`TerrainCache`**: decoded tiles, a bilinear `height_at(lon, lat)` from the deepest loaded tile covering the point (taps across a tile edge come from the neighbouring tile, or repeat the edge when it is missing), `None` where nothing is loaded or beyond ±85.05°, and a generation counter that every arrival and every on/off switch bumps.
  `Terrain::ground` is what the rest of the code calls: it is `None` while terrain is off and never below sea level.
- **Tile surface**: with terrain on, each tile mesh is built on a finer grid (64 cells per side up to level 15, fewer beyond) and every vertex is lifted along the ellipsoid normal by the height sampled from the tile's own level (at most 15; levels without elevation yet use the nearest loaded ancestor).
  The skirt depth grows with the tile's relief.
  Edges are stitched so that LOD seams are watertight: an edge whose neighbour is coarser (`coarser_neighbours`, up to six levels) takes its heights from the neighbour, sampling the nodes of the neighbour's grid at the neighbour's level and interpolating linearly between them, which is exactly the line the neighbour's mesh draws there.
  A corner is shared by up to four tiles, so each samples it at the coarsest level among the drawn tiles that touch it (`corner_level`), and so does the end of a neighbour's edge that a stitched edge follows; every tile then puts that vertex at one height.
  A lookup on the boundary between two elevation tiles uses whichever is loaded (a missing neighbour repeats the edge sample), and the elevation tiles around the visible ones are requested after the visible ones, so the vertices along a tile edge are not sampled at a coarser level than the vertices beside them, which would show as a one-cell ridge along the edge.
  The skirt is only the fallback for what stitching cannot cover, such as a neighbour that has not been rebuilt yet, and the shader does not hillshade near-vertical faces, so a visible skirt is not drawn as a dark line.
  Overlay tile layers draw the same meshes, so they follow the terrain without extra work.
  Tiles keep the generation they were built for; when elevation arrives, only the meshes it can change (a tile no deeper than the mesh's sampling level that overlaps it) are rebuilt, up to 12 per frame, and the rest keep their old shape until their turn.
  A mesh is also rebuilt when the levels of its neighbours change.
- **Mesh cost**: mesh detail follows the quadtree, so it already falls with distance: every tile has the same grid, and a tile far from the camera covers more ground.
  What limits the frame rate is the CPU, because a moving camera changes the selection every frame and each change re-stitches the neighbours.
  A mesh therefore asks for its whole height grid in one call (`TerrainSampler::heights`, `TerrainCache::heights_at_level`): the projection runs once per row and column, tile lookups are remembered between nodes, and the ECEF trigonometry is separable in the same way.
  Tiles of one grid resolution share one index buffer (`tile_mesh_indices`), and a rebuild rewrites the existing vertex buffer in place, keeping the uniform and the texture binding.
  Meshes that leave the view are kept and dropped least recently used first, at twice the tile budget.
  `cargo run --release -p terra-globe --example terrain_bench` flies a scripted route over synthetic terrain on wgpu's no-op backend and prints the CPU time per frame (`RenderStats::prepare_micros`) and the meshes built (`RenderStats::meshes_built`).
- **Selection**: `select_tiles_with_relief` grows each tile's bounding sphere by its lowest and highest height and widens the horizon test by the angle a summit sees past the sea level horizon, so a mountain near the limb is not culled with the sea level surface it stands on.
  The elevation tiles for the visible imagery are requested coarse first, like imagery.
- **Shading**: the base tile shader multiplies the imagery by a north-west hillshade taken from the screen-space derivatives of the tile's position; it is off (strength 0) when terrain is off, so terrain-off frames are identical to before.
- **Placemarks**: `build_meshes`, picking and the marker projection receive the lookup. Over loaded terrain lines and fills are densified to 200 m edges (a path longer than 20,000 vertices stretches that), lifted 6 m, and pulled towards the eye by 0.3% of their distance in the vertex shader (at most 2 km), which leaves their pixel unchanged and only wins them the depth test against the coarser tile surface.
  Image overlays are draped the same way on a 250 m grid, capped at 128 cells per side.
  The scene is rebuilt when the terrain generation changes, at most once per 30 frames, and at once when terrain is switched.
- **Picking**: `ray_terrain` steps through the shell between sea level and 9 km, then bisects the crossing. `Globe::pick` (marker placement, ruler) uses it when terrain is on; it also reports the height.
- **Camera**: the orbit target sits on the ground (`Camera::target_height`), so tilting and zooming pivot on the surface in view.
  `Globe::settle_camera` re-seats it after every input and frame and keeps the eye 3 m above the ground under it, moving the eye away from the target and, as a last resort, towards top-down.
  Close to the ground, wheel zoom moves the eye along the cursor ray and re-seats the pivot on the terrain, so zooming onto a summit closes in on the summit.
- **Tests**: `crates/terra-globe/tests/terrain.rs` renders a synthetic mountain injected as elevation tiles (`Globe::inject_terrain_tile`, `TerrainTile::synthetic`) with an elevation source that never answers.

## Data layer

- **KML** is parsed into a small XML DOM (`xml.rs`), then mapped with the same rules as the web app: folders and styles round-trip, Circle and Rectangle export as polygon plus `<ExtendedData>` markers so they re-import exactly, MultiGeometry groups are preserved, and unsupported features are skipped with warnings.
- **KMZ** read/write, data URLs and MIME guessing live in `kmz_assets.rs`; NetworkLink resolution takes an injected `KmlFetcher` so it is testable and portable.
- **Storage** keeps the web app's JSON-in-row layout for folders, placemarks, styles and screen overlays.
  Schema v3 adds a binary `assets` table, so the SQLite schema no longer matches the web app's v2 (see `parity-notes.md`).
  `collect_all` is the shared "everything" view used by export, the CLI and (later) undo snapshots.
- **Providers** are pure request-building plus response-parsing over an `HttpClient` port, so every preset is tested against recorded payloads without touching the network. `EhttpClient` is the native transport; the browser transport arrives with M7.
- **Settings and secrets** are ports in `terra-core`/`terra-providers`; the shells decide where they persist.
- **i18n** catalogues are generated from the web app (`tools/parity/extract-i18n.mjs`) into flat JSON with a key-parity test per locale and plural selection for the locales that need it (Polish `few`/`many`).

## Application layer

`terra-app` owns one `AppData` value: the library projection, the editor buffers, drawing/ruler/route state, settings, notices and the globe.
Every mutation goes through `StateStore`, which snapshots the library for undo (capped at 20), writes to the repository first and only then updates the in-memory copy; a failed write restores the snapshot so the UI never shows state the database rejected.

- **Scene**: the UI derives a `Scene` from the library each frame (visible entries, selection, preview, ruler, route, distance rings) and hands it to the renderer with a revision number, so meshes are re-tessellated only when something changed.
- **Views**: `crates/terra-app/src/views/` is split by screen region, and `views/mod.rs` assembles the frame from the modules below.
  - `mod.rs`: frame assembly, the keyboard shortcuts (undo, redo, Escape) and shared helpers.
  - `top_bar.rs`: the single top bar row with the File, View and Geometry menus, the ruler and route toggles, search and Settings/About.
  - `library.rs`: the left panel's Places tree, with search, drag-and-drop and keyboard navigation.
  - `layers.rs`: the Layers section, with the terrain switch, the base map choice and the transparent tile overlays.
  - `tree.rs`: the reusable IDE-style tree, with the row model (`visible_rows`), keyboard navigation (`next_selection`) and the row painter (`tree_row`).
  - `editor.rs`: the right panel placemark editor.
  - `globe_view.rs`: the central globe viewport, with input, keyboard navigation, overlays and the HUD.
  - `nav_controls.rs`: the on-map navigation controls (see below).
  - `modals.rs`: modal dialogs, the height profile chart and notice toasts.
- **Top bar**: the drawing tools sit under a Geometry menu, which also holds "Image Overlay…".
  The basemap choice is in the Layers panel, not the top bar.
- **Input**: pointer handling lives in `views::globe_view`; drawing uses `DrawingSession`, Shift+drag moves a shape, double-click flies to a picked geometry.
- **Ports**: `platform::{SettingsStore, FileSystem}` and `terra_providers::SecretStore` are injected by the shell, so the entire state layer runs headless in tests (`crates/terra-app/tests/app_flow.rs`).
- **Shells**: `terra-desktop` composes SQLite, a JSON settings file, rfd dialogs and the OS keychain (with an in-memory fallback). `terra-web` composes the IndexedDB repository and refuses to start if IndexedDB cannot be opened.

## Scene modes

`terra_globe::SceneMode` selects between the globe and two flat map views:

- **3D** uses the WGS84 ellipsoid, orbit camera, reverse-Z perspective and the quadtree described above.
- **2D** is an orthographic Mercator map: tiles become flat quads (one per tile) and overlays are built on the same plane, with pan/zoom and picking in plane coordinates.
- **Columbus** is a perspective camera over that same plane with heading and tilt.

Camera and projection live in `flat.rs`; the renderer picks flat tile meshes and flat overlay meshes when the mode is flat, and skips the polar caps (which only exist because the globe is a sphere).
Switching modes keeps the geographic centre and an approximate zoom, and the mode is persisted in settings.

## Navigation controls

`views/nav_controls.rs` paints the on-map controls at the top right of the globe; they take the pointer while it is over them, so the globe underneath does not see those clicks.

- **Compass ring**: drag to rotate the heading, and click the N marker to return to north-up.
  Rotation is disabled in 2D, which is north-up only.
- **Look joystick**: calls `Globe::look_by`, which reaches `Camera::look_by`.
  That turns the view about the eye, which stays put, like Google Earth's look around.
  It is 3D only.
- **Move joystick**: pans with `Globe::move_view`, scaled by the view distance.
- **Zoom slider**: the +/- buttons step the zoom, and dragging the track zooms continuously.

## Vector rendering

Overlays are drawn after the globe into the same depth buffer:
- filled polygons/circles/rectangles are triangulated with `earcutr` in an ENU frame at the geometry centre, then conformingly subdivided until every edge is short enough to follow the ellipsoid, with holed polygons supported;
- polylines are quads expanded in screen space so their width is constant at every zoom, with no CPU re-projection;
- fills are lifted along the ellipsoid normal by 3 m plus the residual chord sag `1.25 · L² / (6R)`, where `L` is the longest edge left after refinement and `R` is the WGS84 equatorial radius.
  The faceted globe therefore never wins the depth test, and fills do not z-fight with the imagery.
- image overlays are gridded into cells of at most 5 km, with at most 128 cells per side, and lifted by 1 m plus the same residual sag term.
- Trade-off: a shape that hits the triangle cap (60,000 fill triangles) keeps its long edges, so its lift grows with their sag.
  A continent-sized capped shape can float up to roughly 2–3 km above the ellipsoid.
  The cap is deliberate, because refining such a shape further would cost hundreds of thousands of triangles.

## Layers and image overlays

Transparent tile overlays and image overlays share one overlay pass.

- **Tile overlays**: `terra_providers::overlay_presets()` lists OpenRailwayMap, OpenSeaMap and the Waymarked Trails hiking and cycling layers.
  `Settings.overlay_layers` stores the enabled flag and opacity of each one in stacking order, and `Globe::set_tile_overlays` receives the enabled ones.
  Layers that stay present keep their imagery when their opacity changes.
- **Image overlays**: KML `GroundOverlay` pictures are passed to `Globe::set_image_overlays`, with rotation applied.
  Opacity is the alpha byte of the KML colour.
- **Asset store**: pictures from imports and from "Image Overlay…" are stored through `PlacesRepository` (`put_asset`, `get_asset`, `delete_asset`, `list_assets`), keyed by their normalised path.
  The SQLite schema v3 adds an `assets` table, and the IndexedDB database moves to version 3 with an `assets` store.
  Unreferenced assets are deleted once at startup, when no undo history can still need them.
- **Pipeline**: the base pass writes opaque imagery and depth.
  The overlay pass uses alpha blending, depth compare `GreaterEqual` and no depth write, so overlays blend at the globe's depth without hiding what is behind them.
  It runs after the base tiles and before the scene fills, so fills still draw over overlays.

## Browser build (M7)

## Browser build (M7)

- **Storage**: `terra-storage-indexeddb` implements `PlacesRepository` against the same Dexie schema the web app used (database `terra-globe`, stores `folders`/`placemarks`/`styles`/`screenOverlays`, inline key `id`), so an existing browser profile keeps its data.
  The Rust adapter then upgrades the database to version 3, which adds the `assets` store; the web app stops at version 2.
- **Settings and secrets**: `localStorage` adapters in `terra-web` (the desktop build uses the OS keychain).
- **Files**: exports trigger a browser download; path-based reads and the file picker are not available yet.
- **Providers**: browser network access is not wired (the blocking provider clients need an async `fetch` transport), so geocoding and routing report "not available" instead of panicking.
- **Rendering**: WebGPU with WebGL2 fallback; MSAA is 4x natively and 1x on wasm because WebGL2 has no multisampled textures.

## Command-line tool

`terra-kml import|export|roundtrip|counts` is the M2 acceptance tool:
importing a KML/KMZ into SQLite and exporting it back must reproduce the same document.

## Testing

| Layer | Where | Notes |
|---|---|---|
| Geodesy, camera, tiles, picking | `crates/terra-globe/src/**` unit tests | Pure math, no GPU |
| Rendered output | `crates/terra-globe/tests/render.rs` | Headless wgpu on a software adapter (lavapipe) in CI; synthetic checkerboard tiles, fixed camera, PNG snapshots |
| Domain + units parity | `crates/terra-testkit/tests/parity.rs` | Expectations generated by running the TypeScript implementation |
| Storage behaviour | `terra-storage::contract` | Shared suite, run by every adapter |
| KML parity | `crates/terra-testkit/tests/kml_parity.rs` | Parse/serialize compared with TypeScript goldens |
| Providers | `FakeHttpClient` in `terra-providers` | URL, headers, body and response mapping per preset |
| Database compatibility | `tools/parity/verify-rust-db.mjs` | The old app's row format parses a Rust-written file |
| Application flow | `crates/terra-app/tests/app_flow.rs` | Import, edit, draw, undo, autosave and export through the real `AppData` with a headless GPU |
| Widgets | `crates/terra-app/tests/ui_flow.rs` (`egui_kittest`) | Clicking real widgets by accessible label and asserting on state |
| Dependency rule | `scripts/check-rust-deps.py` | Runs in CI |

Regenerate snapshots deliberately with `make rust-snapshot` and check the diff is the change you intended.

## Commands

```bash
make rust-run        # run the desktop app
make rust-check      # fmt, clippy, dependency rule, tests, wasm check
make rust-test       # tests only
make rust-snapshot   # regenerate globe snapshots
make rust-deny       # licence and advisory check
```

## Known deviations from the web app

See [`parity-notes.md`](parity-notes.md) for the running list (no MSAA in the browser build, two scene modes are approximations, and so on).
