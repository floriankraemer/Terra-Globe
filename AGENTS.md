# AGENTS.md

Instructions for AI coding agents working in this repository.
See [`README.md`](README.md) for a project overview and [`docs/architecture.md`](docs/architecture.md) for how the packages fit together - read that before making structural changes.

## Setup

```bash
pnpm install
```

Node 20+, pnpm 9+. Docker is optional but recommended for a clean environment (`make dev`, `make test`, `make build-*`) - see the `Makefile`.

## Commands

```bash
pnpm dev                 # browser dev server (packages/ui), http://localhost:5173
pnpm -r build             # build all TS packages (required before typecheck - core/map/storage-*
                           # have no bundler; downstream packages typecheck against emitted .d.ts
                           # via TS project references)
pnpm -r test               # unit tests (Vitest), all packages
pnpm -r lint                # ESLint + Prettier, all packages
pnpm format                 # prettier --write .

cd e2e/playwright && pnpm exec playwright test --workers=1   # E2E (always --workers=1: parallel
                                                                # WebGL contexts on one machine are
                                                                # flaky, not a real signal)
```

Rust shell (`src-tauri`):

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

These are exactly the checks CI runs (`.github/workflows/ci.yml`), except E2E - which is local-only (WebGL in CI runners is unreliable) - run them all before considering a change done.

## Code Conventions

- **TDD, unit-tests-first.** Write the failing test before the implementation. This isn't a formality here: it's how the port/adapter seams (`IEntityFactory`, `PlacesRepository`) stay honest and testable without a browser.
- **Dependency direction is one-way and enforced by convention, not tooling**: `core` imports nothing from this repo; `map` imports only `core`; `ui` imports `map` and `core`. Never import `cesium` (or any DOM/browser API) into `packages/core`. Never import a concrete storage adapter into `packages/map` or `packages/core` - go through `PlacesRepository`.
- New storage backend behavior must pass `sharedRepositoryContractTests` (`packages/core/src/storage/repositoryContractTests.ts`), not just its own bespoke tests.
- New Cesium-facing behavior belongs behind `IEntityFactory`/`IScreenPicker` if it needs to be unit-tested; `CesiumEntityFactory`/`createViewer` themselves are the accepted "E2E-only tested" boundary - don't try to mock WebGL to cover them at the unit level.
- TypeScript strict mode, `noUncheckedIndexedAccess` - no `any`, no unchecked array/object indexing.
- No comments explaining _what_ code does; a short comment is fine for a _why_ that isn't obvious from the code (a workaround, a non-obvious invariant, a perf trade-off already documented elsewhere in this file's sibling files - see `CesiumEntityFactory.updateEntity`'s in-place-mutation comment for the pattern).
- Match existing formatting: Prettier is authoritative, run `pnpm format` rather than hand-formatting.
- **Never let a mutating action silently no-op while its backing async init hasn't finished.** Any hook/component that depends on an async-created ref (a repo, a synchronizer, a viewer handle) must expose a `ready` boolean and the UI must disable the affected controls until it's true - not just guard the handler body with `if (!repo) return;`. A guard like that hides real bugs (a click during init does nothing, no error, no feedback) and is exactly what caused the CI E2E flakiness fixed in commit `52b93a5`: `useLibrary`'s repo/sync refs were still null when clicks landed, so tests read as flaky timeouts when the actual defect was a race a real user could hit too.

## Testing Expectations

- Every bug fix starts by reproducing the bug in an E2E test (or the most end-to-end unit test available) _before_ touching the fix, so the fix is verified against the real symptom, not a guess at the cause.
- After any change: rebuild affected packages (`pnpm -r build` if `core`/`map` changed), run the full unit suite, run lint, and run the full E2E suite with `--workers=1` - not just the tests for the file you touched. This repo has a working, fully-green baseline; keep it that way.
- If a Playwright test uses `getByRole`/`getByText` and the match is ambiguous (e.g. a toolbar control and an auto-named tree item both match), scope the locator (`.getByRole("toolbar", {...})`) or pass `{ exact: true }` rather than loosening the assertion.
- UI-visible changes (new controls, layout changes): update the visual-regression baseline (`playwright test tests/visual.spec.ts --update-snapshots`) deliberately, and check the diff is the change you intended, not incidental drift.

## Known Environment Gotchas

- Docker-run build/test containers can leave `packages/*/dist` or the `node_modules` volume root-owned, causing `EACCES` on the next local `pnpm build`. Fix: `docker run --rm -v $(pwd)/packages/ui/dist:/w alpine sh -c "rm -rf /w/*"` (or remove the stale volume), then rebuild.
- The sandboxed dev/E2E environment's WebGL and IndexedDB timing is not representative of a real desktop/browser - don't tune application code to sandbox timing; reproduce and fix real bugs, not environment noise. If a Playwright run looks flaky, rerun with `--workers=1` before assuming it's a regression.
- Windows cross-builds must go through `cargo tauri build --target x86_64-pc-windows-msvc --runner cargo-xwin`, not a bare `cargo-xwin` invocation - the latter skips Tauri's production-mode env vars and produces a binary that tries to load a dev server.

## Licensing

This project is GPLv3-licensed (see `LICENSE.md`). Only add dependencies whose license is compatible with GPLv3 (MIT, BSD, Apache-2.0, and other GPLv3-compatible permissive licenses are fine; GPL-incompatible licenses - e.g. some "source available" or non-commercial licenses - are not). If unsure, check before adding.
