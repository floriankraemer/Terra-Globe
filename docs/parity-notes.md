# Parity notes

Differences between the Rust rewrite and the frozen TypeScript app, kept as an explicit list rather than discovered by users.
Each entry says whether it is intentional, temporary, or pending a decision.

## Renderer (M1)

- **MSAA is 4× natively and 1× in the browser** (WebGL2 has no multisampled textures), so the wasm build still shows aliased silhouettes.
- **Navigation follows Google Earth.** Left-drag moves the globe (the ground follows the cursor), the wheel zooms towards the cursor without rotating, right-drag zooms vertically and rotates horizontally, middle-drag and shift+left-drag tilt and rotate, and the arrow keys move along the current heading with shift+arrows for rotation and tilt; `n`/`u`/`r` reset north/tilt, space cancels a fly-to and Alt slows every keyboard move. Not implemented: the Ctrl+drag first-person look (the on-map look joystick covers that job) and inertial easing.
- **Zoom-at-cursor approximates the target move** by interpolating the target toward the picked point. It reads correctly at normal zoom levels but is not the exact "keep the point under the cursor" solution.
- **Terrain is new and desktop-only.** The TS app draws the bare ellipsoid; the Rust desktop app displaces the globe by free elevation data (AWS Terrain Tiles, Terrarium encoding, zoom 0 to 15).
  A "Terrain (3D)" row at the top of the Layers list switches it, and `Settings.terrain_enabled` (default on) remembers the choice.
  The elevation server sends no CORS headers, so the browser build keeps the ellipsoid and hides the row.
  The 2D and Columbus modes never use terrain.
- **Terrain details that differ from a full terrain engine.**
  Heights are clamped to sea level, so oceans stay flat; the sea floor and the geoid are not modelled, and Terrarium heights are used as ellipsoid heights.
  The hillshade is a fixed north-west light derived per pixel from the mesh, so it is faceted at coarse levels and does not use the elevation tile's own gradient.
  Markers and labels are placed at terrain height but are not hidden by a mountain in front of them, because they are drawn by egui without a depth test; only the horizon of the ellipsoid occludes them.
  Ray picking steps through the terrain shell in 512 steps, so a summit narrower than a step can be missed at grazing angles.
- **No 3D models.** KML `<Model>` placemarks are stored and exported but not rendered yet (pending the M3 decision).
- **Polar caps are a flat colour.** Beyond ±85.05° there is no Web-Mercator imagery; the cap is painted with the fallback colour, matching what Cesium shows over OSM. The same colour is what a tile shows before its imagery arrives.
- **Tile LOD is best-first and always complete.** The selector refines the tile whose texels are largest on screen until one texel covers at most one pixel, then stops at the provider's maximum level or when the 160-tile budget is spent. It never stops mid-traversal, so the visible surface is always fully covered: a spent budget only leaves coarser imagery in the lowest-error region (the far limb), never a hole. Tiles without imagery yet are drawn with their nearest loaded ancestor's sub-rectangle, which is why zooming shows a coarse version of the area immediately and sharpens as tiles arrive.
- **LOD seams are hidden by skirts** rather than by geomorphing, and neighbours can differ by one level next to the horizon. With terrain on, edges are stitched to coarser neighbours (up to six levels) so the seam is one line from both sides, and skirts only cover what stitching cannot (a neighbour not yet rebuilt, or grids more than six levels apart); there is no geomorphing.
- **Screen-space markers/labels** are drawn by egui rather than as GPU billboards. This is intentional (crisp text, no glyph atlas), but it means labels do not participate in depth testing; the analytic horizon test handles occlusion by the globe.

## Data (M2)

- **KML/KMZ import and export are implemented** and validated against the TypeScript implementation over a corpus (`crates/terra-testkit/fixtures/kml-parity.json`): parsed structures match exactly, and serialized XML matches after masking generated ids/timestamps.
- **StyleMap re-export duplicates component styles.** A KML `StyleMap` parses into the map entry plus both component `<Style>` elements, and export writes both the components and a new StyleMap with derived ids, so the `<Style>` count grows by two on every export (verified identical in the web app: 2 → 4 → 6 tags). Files stay valid and readable; the follow-up is to skip components referenced by a map. Tracked for M4.
- **Numeric-looking text is kept as text.** The web app's XML parser coerces element text that looks like a number, so a phone number `+49` becomes `49`. The Rust parser preserves `"+49"`. Deliberate improvement; the differential fixture uses a non-numeric phone value so it does not encode the quirk as expected behaviour.
- **Circle export can differ in the last 1–2 ULP** from the web app: V8 and the platform `libm` are not bit-identical for `sin`/`cos`/`asin`. The differential test rounds coordinates to nine decimals, far below anything the domain or a consumer uses.
- **ScreenOverlay ids are preserved** on import when the element has an `id`; the web app ignores it and generates a new one on every parse. Deliberate improvement.
- **Id-less MultiGeometry documents change member ids once.** The group marker falls back to `multigeom-<name>`, so the first export writes that as the Placemark id and the next import derives `<id>-0`, `<id>-1`. Content is unaffected and the document is a fixed point afterwards; the web app behaves the same. The CLI round-trip check compares content with generated ids masked.
- **KMZ assets**: the archive layer and data-URL helpers are implemented (read/write, MIME guessing, base64). Import currently keeps image `href`s instead of inlining them; extraction/inlining lands with the renderer's image handling (M3).
- **NetworkLink resolution** is implemented behind an injected `KmlFetcher` and covered by tests, but the CLI and app do not call it yet, so imports keep the references and surface warnings.
- **IndexedDB adapter** is wasm-only and not implemented (M7).
- **Settings and secrets ports** exist (`terra_core::Settings`, `terraform_providers::SecretStore`); persistence to the OS keychain and config directory lands with the desktop shell work (M6), and the provider UI is M4.
- **Number formatting** is validated against TypeScript goldens, including `toFixed` tie behaviour.

## Application (M3/M4)

- **2D is an orthographic Mercator map** with pan and zoom, north-up only (no rotation), and **Columbus** is a perspective view over the same plane with heading/tilt. Both match the web app's interaction model closely but are not pixel-identical.
- **Flat modes stop at the Mercator limit** (+/-85.05 degrees); the polar caps that cover the globe in 3D have no counterpart on the plain map.
- **Mode switches keep the centre and an approximate zoom**, but camera animation is instant in the flat modes (the globe eases between poses).
- **Ground overlays are textured.** Pictures come from the asset store, from an import, or from a relative or absolute path, and the rotation and KML alpha are applied.
  The TS app also shows the image, through a Cesium rectangle.
  The depth handling is Rust-specific (see `architecture-rust.md`).
- **On-map navigation controls are new.** The compass ring, look and move joysticks and zoom slider are not in the TS app.
  Intentional. The look joystick is 3D only, and the compass does not rotate in 2D.
- **Transparent tile overlays are new.** The TS app only switches base maps; the Rust app adds OpenRailwayMap, OpenSeaMap, two Waymarked Trails layers and Esri places and boundaries, each with its own opacity.
  Intentional, and the layer state lives in `Settings.overlay_layers`.
- **Image assets are stored in the repository.** Imported and picked pictures are kept through the new `put_asset`/`get_asset`/`delete_asset`/`list_assets` methods, so an image overlay survives a reload without its original file.
  Unreferenced pictures are removed once at startup. Intentional.
- **SQLite schema diverges at v3.** The Rust migrator adds the `assets` table and records version 3, while the TS app's `SCHEMA_VERSION` is still 2.
  The TS migrator checks only for its own version, so it opens a v3 file and ignores `assets`.
  Pending: whether the TS app should learn the table, or the change stays Rust-only until M6 removes the TS app.
- **IndexedDB is at version 3** with an `assets` store; the web app's Dexie schema stops at version 2. Same decision as the SQLite divergence above.
- **Screen overlays** render as a labelled placeholder box at the correct KML anchor; the image is not fetched.
- **3D models** still render as markers (unchanged from M1).
- **Drag-to-move uses Shift+drag.** Plain drag still orbits the camera, unlike the web app where dragging the selected shape moves it without a modifier.
- **Provider "test connection"** is not implemented; a provider is exercised the first time you search or calculate a route, and errors surface in the result panel.
- **Tree drag-and-drop** moves both folders and placemarks, but there is no drop indicator between siblings (only onto folders/root), so reordering within a folder is not possible yet.
- **Search** filters the tree but does not auto-expand collapsed folders that contain matches.
- **Style editing** edits the selected placemark's own style; the shared-style library UI from the web app is not exposed.
- **Ruler, route and height-profile panels** are floating windows rather than docked panels.
- **UI automation**: flows are covered by the headless `app_flow` integration tests and renderer snapshots; `egui_kittest` widget-level automation is scheduled for the M6 hardening pass.
- **MSAA**: 4x natively; the browser build renders at 1x because WebGL2 has no multisampled textures.
- **Browser build**: IndexedDB storage, localStorage settings/secrets and download-based exports work; file import via a picker and network providers (geocoding/routing) are not wired yet.
- **Tile cache**: the desktop shell persists downloaded tiles in the data directory; the browser build has no persistent tile cache yet, so a reload re-fetches through the normal HTTP cache.
- **`egui_kittest`** covers the toolbar, tree, editor, settings modal and undo; it does not yet assert pixel output (renderer snapshots cover that).

## Tooling

- **Playwright** still targets only the TypeScript app. The Rust UI is tested with `egui_kittest` plus headless renderer snapshots; browser E2E for the wasm build arrives with M7.
- **`cargo deny`** runs with an allow-list of GPLv3-compatible licences. Font licences (OFL) are included for the bundled subsets that will arrive with the UI work.
