# Parity notes

Differences between the Rust rewrite and the frozen TypeScript app, kept as an explicit list rather than discovered by users.
Each entry says whether it is intentional, temporary, or pending a decision.

## Renderer (M1)

- **MSAA is 4× natively and 1× in the browser** (WebGL2 has no multisampled textures), so the wasm build still shows aliased silhouettes.
- **No tile mipmaps.** Tiles are sampled linearly without mipmaps. LOD targets one texel per pixel, so this only shows up where imagery is minified (grazing angles at the limb, and coarse ancestor fallbacks); anisotropic filtering is the follow-up if that becomes visible.
- **Navigation follows Google Earth.** Left-drag moves the globe (the ground follows the cursor), the wheel zooms towards the cursor without rotating, right-drag zooms vertically and rotates horizontally, middle-drag and shift+left-drag tilt and rotate, and the arrow keys move along the current heading with shift+arrows for rotation and tilt; `n`/`u`/`r` reset north/tilt, space cancels a fly-to. Not implemented: Google Earth's Alt-to-slow modifier, the Ctrl+drag first-person look, and inertial easing.
- **Zoom-at-cursor approximates the target move** by interpolating the target toward the picked point. It reads correctly at normal zoom levels but is not the exact "keep the point under the cursor" solution.
- **No terrain.** The surface is the WGS84 ellipsoid, which matches the current app. `HeightProvider` is designed for later.
- **No 3D models.** KML `<Model>` placemarks are stored and exported but not rendered yet (pending the M3 decision).
- **Polar caps are a flat colour.** Beyond ±85.05° there is no Web-Mercator imagery; the cap is painted with the fallback colour, matching what Cesium shows over OSM. The same colour is what a tile shows before its imagery arrives.
- **Tile LOD is best-first and always complete.** The selector refines the tile whose texels are largest on screen until one texel covers at most one pixel, then stops at the provider's maximum level or when the 160-tile budget is spent. It never stops mid-traversal, so the visible surface is always fully covered: a spent budget only leaves coarser imagery in the lowest-error region (the far limb), never a hole. Tiles without imagery yet are drawn with their nearest loaded ancestor's sub-rectangle, which is why zooming shows a coarse version of the area immediately and sharpens as tiles arrive.
- **LOD seams are hidden by skirts** rather than by geomorphing, and neighbours can differ by one level next to the horizon. There is no terrain, so no height-based stitching is needed.
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
- **Ground overlays** render their extent as a translucent white shape; the referenced image is not downloaded or textured yet.
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
- **Tile mipmaps** are still not generated (see the renderer section).
- **Browser build**: IndexedDB storage, localStorage settings/secrets and download-based exports work; file import via a picker and network providers (geocoding/routing) are not wired yet.
- **`egui_kittest`** covers the toolbar, tree, editor, settings modal and undo; it does not yet assert pixel output (renderer snapshots cover that).

## Tooling

- **Playwright** still targets only the TypeScript app. The Rust UI is tested with `egui_kittest` plus headless renderer snapshots; browser E2E for the wasm build arrives with M7.
- **`cargo deny`** runs with an allow-list of GPLv3-compatible licences. Font licences (OFL) are included for the bundled subsets that will arrive with the UI work.
