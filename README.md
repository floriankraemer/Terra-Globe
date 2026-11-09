# Terra Globe

A Google Earth-style 3D globe application.
Runs in the browser and as a native desktop app (Windows, macOS, Linux), sharing a single frontend codebase.

> **Rewrite in progress.** The application is being rebuilt in Rust (`crates/`), with a purpose-built wgpu globe renderer and an egui UI.
> The TypeScript/Tauri app in `packages/` and `src-tauri/` is frozen and remains the behavioural reference until it is removed at milestone M6.
> Status and commands: [`docs/architecture-rust.md`](docs/architecture-rust.md); deviations: [`docs/parity-notes.md`](docs/parity-notes.md).

Draw markers, circles, rectangles, polygons, and paths on a 3D globe.
Organize them into folders.
Import and export Google Earth KML/KMZ files.
Store data locally (IndexedDB in the browser, SQLite on desktop) with a storage interface designed for future remote sync.

## Rust port

```bash
cargo run -p terra-desktop        # or: make rust-run (binary: terra-globe)
make rust-check                   # fmt, clippy, dependency rule, tests, wasm check
cargo run -p terra-cli -- roundtrip path/to/map.kml   # import/export smoke tool
cargo run -p terra-globe --example lod_probe          # print the tile selection per camera pose
make rust-build-windows          # cross-build TerraGlobe-rust-x64.exe in Docker (cargo-xwin)
```

`examples/render_tiles.rs` renders a frame from a local tile cache for eyeballing imagery quality:

```bash
TERRA_TILE_CACHE=/tmp/terra-tiles TERRA_CAM_LAT=50 TERRA_CAM_LON=10 \
  TERRA_CAM_DISTANCE=8000000 TERRA_ZOOM_TO=400000 \
  cargo run -p terra-globe --example render_tiles
```

It serves `$TERRA_TILE_CACHE/<z>/<x>/<y>.png` over loopback (the normal fetch path), prints the tiles it is missing so you can populate the cache, and writes `$TERRA_TILE_OUT` (default `/tmp/terra-render.png`).

### Navigation

Navigation follows Google Earth: the wheel zooms towards the cursor, arrows move the view, shift rotates and tilts.

| Action | Mouse | Keyboard |
| --- | --- | --- |
| Move over the globe | left-drag (the ground follows the cursor) | arrow keys, relative to the current heading |
| Zoom | wheel, right-drag up/down, double-click in, right double-click out | `+` / `-`, Page Up / Page Down |
| Rotate | shift+left-drag, middle-drag left/right, right-drag left/right | shift+left / shift+right |
| Tilt | shift+left-drag up/down, middle-drag up/down | shift+up / shift+down, down = towards the horizon |
| North up | | `n` |
| Straight down | | `u` |
| North up and straight down | | `r` |
| Cancel a fly-to | | space |
| Move more slowly | | hold Alt with any of the above |

The globe view takes the keyboard when the pointer is over it or after a click.
In the flat 2D view the map stays north-up; Columbus view rotates and tilts like
the globe.

The desktop app stores its library in `terra-globe.db` and its settings in `settings.json` under the platform data directory (`app.terraglobe.Terra Globe`), downloaded tiles in `tiles/` (512 MB budget), and provider keys in the OS keychain.
Draw with the toolbar, fill in the editor on the right, and press **File → Save** to export KML/KMZ.
**View** switches between the 3D globe, a 2D map and Columbus view.
Release process: [`docs/release.md`](docs/release.md).

Linux needs a Vulkan driver (Mesa works); the test suite runs headless on lavapipe.


## Features

- 3D globe rendering via [CesiumJS](https://cesium.com/platform/cesiumjs/), with OpenStreetMap and OpenTopoMap base layers
- Draw and edit markers, circles, rectangles, polygons, and lines, with live preview while drawing
- Per-shape styling: outline color/width/enabled, fill color/enabled
- Folder tree for organizing placemarks, with collapsible nodes, visibility toggles, and a resizable, persisted sidebar
- Area, radius, and circumference shown in the placemark editor, in your preferred unit system (metric/imperial) and coordinate format (decimal/DMS), configurable via the Settings modal
- Placemark name labels on the globe, shown once zoomed in and scaled with distance
- KML/KMZ import and export, including lossless round-tripping of circles and rectangles (which have no native KML representation) via `<ExtendedData>`
- Runs identically in a browser tab or as a native desktop app (Tauri)
- Local-first storage: IndexedDB in the browser, SQLite on desktop, with a `PlacesRepository` port designed for a future remote backend

## Tech Stack

| Concern | Choice |
|---|---|
| Globe rendering | CesiumJS |
| Frontend | React + TypeScript + Vite |
| Desktop shell | Tauri v2 (Rust) |
| Local storage (browser) | IndexedDB via Dexie |
| Local storage (desktop) | SQLite via the Tauri SQL plugin |
| KML/KMZ | Custom parser/serializer (`fast-xml-parser`, `fflate`) |
| Unit tests | Vitest + React Testing Library |
| E2E tests | Playwright |
| Monorepo | pnpm workspaces |

See [`docs/architecture.md`](docs/architecture.md) for how these fit together.

## Getting Started

### Prerequisites

- Node.js 20+ and [pnpm](https://pnpm.io/) 9+
- [Docker](https://www.docker.com/) and Docker Compose, for containerized dev/build (recommended, no local toolchain needed)
- Rust toolchain, if building the desktop app outside Docker

### Run the browser app

```bash
pnpm install
pnpm dev
```

Opens at `http://localhost:5173`.

Or via Docker:

```bash
make dev
```

### Run the desktop app

```bash
cd src-tauri
cargo tauri dev
```

### Build

```bash
make build-web       # static browser bundle -> packages/ui/dist
make build-linux      # Linux desktop bundle (.AppImage/.deb), via Docker
make build-windows     # Windows installer + binary, cross-built via Docker (unsigned)
make build-macos       # prints instructions - must run on real macOS/CI
```

## Testing

```bash
pnpm -r test           # unit tests, all packages
pnpm -r lint            # ESLint + Prettier across all packages
cd e2e/playwright && pnpm exec playwright test --workers=1   # E2E
```

Or via Docker: `make test`, `make lint`.

CI runs lint, typecheck, unit tests, Rust checks (`clippy`/`rustfmt`/`cargo test`), and Playwright E2E on every push and pull request (see `.github/workflows/ci.yml`).
Tagged releases (`vX.Y.Z`) trigger native builds for Windows, macOS, and Linux, attached to a GitHub Release (see `.github/workflows/release.yml`).

## Project Structure

```
packages/
├── core/                # domain model, storage port, KML parser/serializer - no Cesium/DOM deps
├── map/                 # Cesium wrapper: viewer, drawing tools, entity sync
├── storage-indexeddb/    # browser storage adapter (Dexie)
├── storage-sqlite/       # desktop storage adapter (Tauri SQL plugin)
├── storage-remote/       # stub remote storage adapter
└── ui/                   # React app - the single frontend used by browser and desktop
src-tauri/                # Tauri desktop shell (Rust)
e2e/playwright/           # end-to-end tests
docs/                     # architecture documentation
```

## License

GNU General Public License v3.0 - see [LICENSE.md](LICENSE.md).
