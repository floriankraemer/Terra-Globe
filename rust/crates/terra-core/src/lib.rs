//! Domain model, ports (dependency-inversion boundaries) and pure domain
//! logic. This crate must never depend on a rendering engine, a database
//! driver, an HTTP client, the OS or a UI toolkit, so that it compiles
//! unchanged for desktop and `wasm32`.

pub mod domain;
pub mod fakes;
pub mod ports;

pub use domain::*;
pub use ports::*;

/// A boxed, sendable future, so async ports do not depend on an executor.
pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;
