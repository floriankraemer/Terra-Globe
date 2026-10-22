//! Pure f64 geospatial math for the globe: no GPU, no I/O. See
//! `docs/rust-port-plan.md` ("How the 3D projection works").

pub mod ellipsoid;

pub use ellipsoid::Ellipsoid;
