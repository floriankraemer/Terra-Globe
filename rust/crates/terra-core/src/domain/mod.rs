pub mod folder;
pub mod geometry;
pub mod placemark;
pub mod screen_overlay;
pub mod style;

pub use folder::*;
pub use geometry::*;
pub use placemark::*;
pub use screen_overlay::*;
pub use style::*;

/// Validation failure for a domain constructor.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ValidationError(pub String);
