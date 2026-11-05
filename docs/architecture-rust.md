# Rust architecture (M0–M1)

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
- **Imagery**: `TileProvider` builds URLs from the same presets as the web app. `TileFetcher` uses `ehttp` (threads natively, `fetch` on wasm), keeps a priority queue rebuilt from the current selection every frame, runs at most 8 requests at a time, retries transient failures up to three times and holds a given-up tile back for 30 s before trying again, so a flaky network does not permanently punch a hole in the imagery. Decoded tiles are normalised to 256×256.
- **Colour management**: tile textures are `Rgba8UnormSrgb`, shading happens in linear space, and the fragment shader encodes to sRGB before writing the `Rgba8Unorm` target, because egui samples user textures as sRGB.
- **Overlays**: markers and labels are projected by `Globe::project` (with an analytic horizon test) and drawn by egui in screen space, so text is crisp and shaped by HarfBuzz.

## Data layer

- **KML** is parsed into a small XML DOM (`xml.rs`), then mapped with the same rules as the web app: folders and styles round-trip, Circle and Rectangle export as polygon plus `<ExtendedData>` markers so they re-import exactly, MultiGeometry groups are preserved, and unsupported features are skipped with warnings.
- **KMZ** read/write, data URLs and MIME guessing live in `kmz_assets.rs`; NetworkLink resolution takes an injected `KmlFetcher` so it is testable and portable.
- **Storage** keeps the exact schema and JSON-in-row layout, so the old app reads new files and vice versa (`tools/parity/verify-rust-db.mjs` checks the former against a real database). `collect_all` is the shared "everything" view used by export, the CLI and (later) undo snapshots.
- **Providers** are pure request-building plus response-parsing over an `HttpClient` port, so every preset is tested against recorded payloads without touching the network. `EhttpClient` is the native transport; the browser transport arrives with M7.
- **Settings and secrets** are ports in `terra-core`/`terra-providers`; the shells decide where they persist.
- **i18n** catalogues are generated from the web app (`tools/parity/extract-i18n.mjs`) into flat JSON with a key-parity test per locale and plural selection for the locales that need it (Polish `few`/`many`).

## Application layer

`terra-app` owns one `AppData` value: the library projection, the editor buffers, drawing/ruler/route state, settings, notices and the globe.
Every mutation goes through `StateStore`, which snapshots the library for undo (capped at 20), writes to the repository first and only then updates the in-memory copy; a failed write restores the snapshot so the UI never shows state the database rejected.

- **Scene**: the UI derives a `Scene` from the library each frame (visible entries, selection, preview, ruler, route, distance rings) and hands it to the renderer with a revision number, so meshes are re-tessellated only when something changed.
- **Input**: pointer handling lives in `views::central_globe`; drawing uses `DrawingSession`, Shift+drag moves a shape, double-click flies to a picked geometry.
- **Ports**: `platform::{SettingsStore, FileSystem}` and `terra_providers::SecretStore` are injected by the shell, so the entire state layer runs headless in tests (`crates/terra-app/tests/app_flow.rs`).
- **Shells**: `terra-desktop` composes SQLite, a JSON settings file, rfd dialogs and the OS keychain (with an in-memory fallback). `terra-web` currently uses in-memory adapters until IndexedDB lands.

## Scene modes

`terra_globe::SceneMode` selects between the globe and two flat map views:

- **3D** uses the WGS84 ellipsoid, orbit camera, reverse-Z perspective and the quadtree described above.
- **2D** is an orthographic Mercator map: tiles become flat quads (one per tile) and overlays are built on the same plane, with pan/zoom and picking in plane coordinates.
- **Columbus** is a perspective camera over that same plane with heading and tilt.

Camera and projection live in `flat.rs`; the renderer picks flat tile meshes and flat overlay meshes when the mode is flat, and skips the polar caps (which only exist because the globe is a sphere).
Switching modes keeps the geographic centre and an approximate zoom, and the mode is persisted in settings.

## Vector rendering

Overlays are drawn after the globe into the same depth buffer:
- filled polygons/circles/rectangles are triangulated with `earcutr` in an ENU frame at the geometry centre, with rings densified for context and holed polygons supported;
- polylines are quads expanded in screen space so their width is constant at every zoom, with no CPU re-projection;
- geometry is lifted 3 m along the ellipsoid normal so it wins the depth test against the globe's own coarse tile chords without z-fighting.

## Browser build (M7)

- **Storage**: `terra-storage-indexeddb` implements `PlacesRepository` against the same Dexie schema the web app used (database `terra-globe`, stores `folders`/`placemarks`/`styles`/`screenOverlays`, inline key `id`), so an existing browser profile keeps its data.
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

See [`parity-notes.md`](parity-notes.md) for the running list (no MSAA yet, no tile mipmaps, camera gestures are orbit + zoom only, and so on).
