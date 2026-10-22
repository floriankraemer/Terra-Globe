//! wgpu globe renderer. Renders into its own offscreen colour + depth target
//! (reversed-Z) which the app composites as an egui texture; the same
//! `render` call is used by headless tests. See `docs/rust-port-plan.md`.
