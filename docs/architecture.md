# Architecture

WebGlobe is a pnpm-workspace monorepo built around one rule: **domain logic never imports a rendering engine, a browser API, or a database driver directly**.
Every cross-cutting concern (rendering, storage, platform) is expressed as a TypeScript interface ("port") in `packages/core` or `packages/map`, with concrete implementations ("adapters") swapped in at a single composition root.
This is what lets the same `packages/ui` React app run unmodified in a browser tab and inside a Tauri desktop window, and what lets drawing logic and KML parsing be unit-tested without a WebGL context.

## Package Map

```
packages/
├── core/               domain model, storage port, KML - zero Cesium/DOM/Node deps
├── map/                Cesium wrapper: viewer bootstrap, drawing tools, entity sync
├── storage-indexeddb/  PlacesRepository adapter for the browser (Dexie/IndexedDB)
├── storage-sqlite/     PlacesRepository adapter for desktop (Tauri SQL plugin / better-sqlite3)
├── storage-remote/     PlacesRepository adapter stub for a future remote backend
└── ui/                 React app - the one frontend used by both browser and desktop builds
src-tauri/               thin Rust shell: window chrome, SQL plugin wiring, packaging only
e2e/playwright/           end-to-end tests against a built browser bundle
```

Dependency direction is strictly one-way: `ui` depends on `map` and `core`; `map` depends on `core`; `core` depends on nothing in this repo.
`storage-*` packages depend only on `core` (the `PlacesRepository` interface they implement).

## Domain Model (`packages/core/src/domain`)

```
GeoPoint        { lon, lat, altitude? }
PlacemarkGeometry = Point | Circle | Rectangle | Polygon | LineString
Style           { outlineEnabled, outlineColor, outlineWidth, filled, fillColor, fillOpacity, ... }
Placemark       { id, folderId, name, description?, geometry, styleId, visibility, ... }
Folder          { id, parentId, name, visibility, order, ... }
```

Circle and Rectangle are modeled as first-class geometry types (not generic polygons), even though KML has no native representation for either - see [KML/KMZ](#kmlkmz-packagescoresrckml) below for how that's reconciled on import/export.

`geometryCenter.ts` and `geometryMeasurements.ts` are pure functions over these types (spherical-geometry math: haversine distance, sum-over-edges polygon area, exact graticule-rectangle area) - no rendering engine needed to compute them, so they're plain Vitest-testable.

## Storage Layer (dependency inversion)

`PlacesRepository` (`packages/core/src/storage/PlacesRepository.ts`) is the single port every storage backend implements: CRUD for folders, placemarks, and styles, plus batch import.

```
InMemoryPlacesRepository    packages/core           - test double, zero I/O
IndexedDbPlacesRepository   packages/storage-indexeddb  - Dexie, used in the browser
SqlitePlacesRepository      packages/storage-sqlite     - Tauri SQL plugin (prod) / better-sqlite3 (tests),
                                                           used on desktop
RemotePlacesRepository      packages/storage-remote     - stub, keeps the seam exercised for future sync
```

All four are exercised against one shared contract test suite, `sharedRepositoryContractTests` (`packages/core/src/storage/repositoryContractTests.ts`), so the backends can't silently diverge in behavior.

Runtime backend selection happens exactly once, at the composition root: `packages/ui/src/platform/createRepository.ts` checks `isTauri()` and picks IndexedDB or SQLite accordingly. Nothing downstream of that branches on platform again.

## Map Layer (`packages/map`)

```
viewer/createViewer.ts    Cesium.Viewer bootstrap, OSM/OpenTopoMap imagery providers, base-layer switching
viewer/flyTo.ts            camera fly-to sized to a geometry's extent (used by "double-click to fly")
entities/IEntityFactory.ts  port: createEntity/updateEntity/removeEntity
entities/CesiumEntityFactory.ts  real implementation - geometry+style -> Cesium entity graphics,
                                   including the zoom-scaled name label (distanceDisplayCondition +
                                   NearFarScalar)
entities/FakeEntityFactory.ts    in-memory test double, used by DrawingController/EntitySynchronizer tests
drawing/DrawingController.ts     per-tool state machine (point/rectangle/circle/polygon), drives live
                                   preview via IEntityFactory as the pointer moves, commits on click
drawing/geometryMath.ts          pure geometry construction (rectangle from two corners, circle radius
                                   via great-circle distance, polygon vertex accumulation)
drawing/IScreenPicker.ts          port for screen-to-globe coordinate picking; CesiumScreenPicker is the
                                   real implementation
sync/EntitySynchronizer.ts        bridges PlacesRepository <-> live Cesium entities (load-all-on-start,
                                   create/update/delete mirroring, combined style+field edits to avoid
                                   lost-update races)
```

`IEntityFactory` is the key testability seam: `DrawingController` and `EntitySynchronizer` are unit-tested end-to-end using `FakeEntityFactory`, with zero WebGL involved.
Only `CesiumEntityFactory` and `createViewer` touch real Cesium, and are covered by Playwright E2E instead - a deliberate, documented boundary rather than an attempt to mock WebGL.

`CesiumEntityFactory.updateEntity` mutates an existing entity's graphics in place rather than remove-and-re-add on every call.
Removing and re-adding forces Cesium to rebuild GPU primitives (expensive tessellation for ellipses/rectangles/polygons); since `updateEntity` fires on every pointer-move during a shape drag-preview, the naive approach made drawing visibly laggy.

## KML/KMZ (`packages/core/src/kml`)

Custom parser (`parseKml.ts`, via `fast-xml-parser`) and serializer (`serializeKml.ts`, template strings) rather than a general-purpose library - needed control over folder-hierarchy and style mapping that off-the-shelf tools flatten away.

- `mapping/geometryKmlMapping.ts` - domain geometry <-> KML geometry elements (Point/Polygon/LineString are native; Circle/Rectangle have no KML equivalent, so they're serialized as a Polygon approximation plus an `<ExtendedData>` block recording the original shape, enabling lossless round-trip within this app while degrading gracefully to a plain polygon in any other KML consumer, matching Google Earth's own behavior).
- `mapping/styleKmlMapping.ts`, `mapping/colorMapping.ts` - `Style` <-> KML `<Style>`/color string conversion.
- `kmz.ts` - zip/unzip via `fflate`.
- `parseKml` is deliberately resilient: an unsupported or malformed placemark is skipped with a warning pushed onto `ParseKmlResult.warnings`, rather than aborting the whole import - a bad shape in a large KML file shouldn't silently drop everything else in it.

## Frontend (`packages/ui`)

React + Vite. Cesium is imperative regardless of framework, so `CesiumViewerHost` is a thin component that owns the imperative `Cesium.Viewer` in a ref; all reactive UI state (selected placemark, drawing mode, settings) lives in React state/hooks outside Cesium's own object graph.

`src/features/*` groups UI by feature (drawing-toolbar, folders, import-export, placemark-editor, settings, notice), each typically pairing a component with a `use*` hook that wires it to `core`/`map` ports. `src/platform/` is the composition root described above.

## Testing Strategy

- **Vitest** for all TypeScript unit tests, mirroring `test/` to `src/`. Business logic (drawing math, KML mapping, storage, unit/coordinate formatting) never imports real `cesium` - only ports/fakes - so it runs fast, with no browser.
- **React Testing Library** for `ui` components, with `core`/`map` ports mocked or faked.
- **Playwright** for E2E, driving a real built browser bundle end to end (drawing, styling, import/export, persistence, visual regression of the toolbar/panel chrome). Run with `--workers=1`: multiple parallel WebGL contexts on one machine cause GPU/CPU contention flakiness that isn't representative of real usage.
- **Rust**: `cargo test` for the (intentionally thin) Tauri shell, `clippy -D warnings` + `rustfmt` in CI.

## Desktop Packaging (Tauri)

`src-tauri` is kept deliberately small: window configuration, the SQL plugin wire-up, and OS bundling only.
All application logic lives in `packages/ui` and its dependencies, loaded as the Tauri WebView's frontend (`packages/ui/dist` in production, the Vite dev server in `cargo tauri dev`).

Cross-compiling the Windows build from Linux goes through `cargo tauri build --target x86_64-pc-windows-msvc --runner cargo-xwin` (see `Makefile`/`docker/`) rather than invoking `cargo-xwin` directly - going through `tauri-cli` is what sets Tauri's production-mode environment variables (asset bundling vs. pointing at a dev server URL).
macOS builds require a real macOS runner (Apple SDK/code-signing constraints) and are not attempted in the Linux container.
