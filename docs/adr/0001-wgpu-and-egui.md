# ADR 0001 — wgpu renderer with an egui application layer

Status: accepted (2026-10-31)

## Context

The TypeScript app gets its globe from CesiumJS.
Rebuilding in Rust means replacing that renderer, and the choice of graphics layer determines everything downstream: the browser target, the test strategy, and how much of the app we own.

No maintained Rust crate provides a WGS84 globe with imagery LOD.
The survey done for the plan found `maplibre-rs` (vector tiles, no ellipsoid globe), `CesiumRS` and `Penumbra` (tiny single-author projects), and game engines where we would still write the globe ourselves.

## Decision

Use **wgpu 30** directly for the globe (`terra-globe`) and **egui 0.37 via eframe** for the UI (`terra-app`).

- `wgpu` targets Metal, DX12, Vulkan and GLES natively, and WebGPU plus WebGL2 on wasm, from one API.
- We use a deliberately small slice: render pipelines, buffers, textures, samplers, one depth format, no compute shaders (so the WebGL2 path stays viable).
- The renderer draws into an offscreen `Rgba8Unorm` texture; egui registers that texture and paints it. `terra-globe` has no egui dependency, so the UI framework can change (e.g. to `iced`) without touching the renderer.
- Tauri is removed: it exists to host a web UI, and file dialogs, SQLite and the keychain are all direct Rust APIs.

## Consequences

- We own roughly 8k lines of renderer code: tile quadtree, imagery pipeline, camera, picking, tessellation. That is the largest single risk, which is why M1 is a hard gate.
- egui 0.37 uses HarfBuzz (`harfrust`) for text layout, which was the blocker for this app's 13 locales including Hindi, Japanese and Chinese.
- `wgpu` and `egui` both release breaking changes roughly quarterly. Versions are pinned and upgraded between milestones, never mid-milestone.
- The WebGL2 path lacks storage buffers and has MSAA limits, so renderer features are chosen to stay within downlevel limits.
