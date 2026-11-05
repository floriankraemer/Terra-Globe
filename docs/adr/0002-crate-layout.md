# ADR 0002 — crate layout and the one-way dependency rule

Status: accepted (2026-10-31)

## Context

The TypeScript monorepo's central rule is that domain logic never imports a rendering engine, a browser API or a database driver, with concrete adapters injected at one composition root.
That rule is what let the drawing and KML logic be unit-tested without a browser, and what lets the same UI run in a tab and on the desktop.

A Rust rewrite could easily lose that property by putting everything in one binary crate.

## Decision

Split the workspace into twelve crates with a checked dependency direction:

```
terra-core        domain, units, geometry math. No other workspace crate.
├── terra-kml     quick-xml + zip
├── terra-providers  provider catalog and traits
├── terra-storage PlacesRepository port, contract suite, in-memory adapter
│   ├── terra-storage-sqlite
│   ├── terra-storage-indexeddb (wasm only)
│   └── terra-storage-remote (placeholder)
├── terra-globe   wgpu renderer. No egui, no filesystem, no SQL.
└── terra-testkit fixtures and image helpers

terra-app         egui state, views, orchestration
├── terra-desktop native binary
└── terra-web     wasm entry point
```

`scripts/check-rust-deps.py` reads `cargo metadata` and fails when a crate gains an internal dependency it is not allowed to have, or when a portable crate gains a forbidden external dependency (`egui`, `wgpu`, `rusqlite`, `tokio`, ...).
CI runs it on every push and pull request.

## Consequences

- The rule is enforced mechanically rather than by review discipline.
- Adding a new adapter means adding it to the checker, which forces a deliberate decision.
- `terra-globe` stays reusable and testable without a UI; its snapshot tests need only a wgpu device.
- The wasm build check (`cargo check -p terra-web --target wasm32-unknown-unknown`) keeps the browser target honest from M0, even though it only ships at M7.
