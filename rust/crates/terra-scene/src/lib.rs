//! Scene logic that sits between the domain and the renderer (the role of the
//! TypeScript `packages/map`). Everything here is unit-testable with the
//! fakes in [`fakes`]; nothing here touches wgpu, egui or I/O.

pub mod fakes;
pub mod ports;

pub use ports::*;
