# Terra Globe

A Google Earth-style 3D globe application.
Runs in the browser and as a native desktop app (Windows, macOS, Linux), sharing a single frontend codebase.

Draw markers, circles, rectangles, polygons, and paths on a 3D globe.
Organize them into folders.
Import and export Google Earth KML/KMZ files.
Store data locally (IndexedDB in the browser, SQLite on desktop) with a storage interface designed for future remote sync.

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
