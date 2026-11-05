# ADR 0005 — async without a runtime in the portable crates

Status: accepted (2026-10-31)

## Context

The desktop app wants blocking SQLite and HTTP on worker threads; the browser has no threads (in the default wasm build) and must use `fetch`.
If either shape leaks into `terra-core` or `terra-globe`, the wasm target rots and the core becomes hard to test.

## Decision

- Portable crates (`terra-core`, `terra-kml`, `terra-providers`, `terra-storage`, `terra-globe`) never spawn, never call `block_on`, and never reference `tokio`.
- I/O is expressed as traits (`PlacesRepository`, `TileProvider`, `HttpClient`, `SecretStore`) and the concrete implementation is chosen in the composition root.
- Imagery uses `ehttp`, which is thread-based natively and `fetch`-based on wasm, so the globe has one code path on both targets.
- Time uses `web-time::Instant` in the UI so `Instant::now()` works on wasm.
- SQLite work lands on the desktop binary's runtime/blocking pool in M2; the browser adapter uses IndexedDB futures.

## Consequences

- `cargo check -p terra-web --target wasm32-unknown-unknown` is part of CI from M0, so a blocking call that only compiles natively fails immediately.
- Tile fetching is fire-and-forget with an in-flight set: a camera move re-selecting a tile does not duplicate requests, and a permanently failed tile is not retried in a loop.
- Adding a background job later means adding it to the desktop/web executor seams, not to the portable crates.
