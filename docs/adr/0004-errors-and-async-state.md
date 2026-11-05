# ADR 0004 — errors, and never silently doing nothing

Status: accepted (2026-10-31)

## Context

`AGENTS.md` records a real defect caused by a handler guard (`if (!repo) return;`) that made clicks during async initialisation do nothing: no error, no feedback, and it read as flaky E2E tests.
The Rust rewrite should make that class of bug structurally hard, not just avoid it once.

## Decision

- Every fallible operation returns a typed error (`thiserror`), with no `todo!()` or `unimplemented!()` in shipped code. `clippy::todo` and `clippy::unimplemented` are denied workspace-wide.
- Async-backed state is modelled as `Initializing | Ready | Failed`, and the UI disables the affected controls until it is `Ready`. There is no nullable handle to guard against.
- Every mutation goes through a single `AppCommand` handler, which is also the single place undo/redo snapshots are taken.
- Stubs are explicit errors, not empty results: for example `parse_kml` returns `KmlError::NotImplemented` until M2 rather than an empty document that would look like a successful empty import.

## Consequences

- Error messages are part of the UI contract and are tested where they are user-visible.
- Libraries stay free of `anyhow`; adapters convert to `StorageError`/`ProviderError` at the boundary so callers can match on causes.
- The renderer reports tile failures through the notice channel; a failed tile never blanks the globe because the ancestor texture and the fallback colour keep the surface covered.
