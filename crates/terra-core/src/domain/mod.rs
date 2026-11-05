//! Domain modules and the shared validation error type.

pub mod circle;
pub mod folder;
pub mod geometry;
pub mod ids;
pub mod measurements;
pub mod placemark;
pub mod route_estimate;
pub mod screen_overlay;
pub mod style;
pub mod time;
pub mod translate;
pub mod visibility;

use thiserror::Error;

/// Validation failures mirror the TypeScript implementation's error messages so
/// that parity tests can compare behaviour, not just success.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum DomainError {
    #[error("{label} must be in range [{min}, {max}], got {value}")]
    OutOfRange {
        label: &'static str,
        min: f64,
        max: f64,
        value: f64,
    },
    #[error("radius must be a positive number, got {0}")]
    InvalidRadius(f64),
    #[error("south ({south}) must be less than north ({north})")]
    SouthNotBelowNorth { south: f64, north: f64 },
    #[error("west ({west}) must be less than east ({east})")]
    WestNotBelowEast { west: f64, east: f64 },
    #[error("{label} must have at least {minimum} points, got {actual}")]
    TooFewPoints {
        label: &'static str,
        minimum: usize,
        actual: usize,
    },
    #[error("timestamps length ({timestamps}) must match path length ({path})")]
    TimestampLengthMismatch { timestamps: usize, path: usize },
    #[error("name must not be empty")]
    EmptyName,
    #[error("imageUrl must not be empty")]
    EmptyImageUrl,
    #[error("modelUri must not be empty")]
    EmptyModelUri,
    #[error("outlineWidth must be a non-negative number, got {0}")]
    InvalidOutlineWidth(f64),
    #[error("fillOpacity must be in range [0, 1], got {0}")]
    InvalidFillOpacity(f64),
    #[error("placemark not found: {0}")]
    PlacemarkNotFound(String),
    #[error("folder not found: {0}")]
    FolderNotFound(String),
    #[error("style not found: {0}")]
    StyleNotFound(String),
    #[error("screen overlay not found: {0}")]
    ScreenOverlayNotFound(String),
    #[error("folder {0} is not empty; pass recursive = true to delete it")]
    FolderNotEmpty(String),
}

/// Distinguishes "field not mentioned in the update" from "field set to null".
///
/// The TypeScript API uses `Partial<T>` where `undefined` means unchanged and
/// `null` means cleared. A plain `Option<T>` cannot express both, and getting
/// this wrong silently erases fields on update.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Patch<T> {
    #[default]
    Unset,
    Set(T),
}

impl<T> Patch<T> {
    pub fn is_unset(&self) -> bool {
        matches!(self, Self::Unset)
    }
}

impl<T> Patch<T>
where
    T: Clone,
{
    /// Applies the patch to a field, leaving it untouched when unset.
    ///
    /// For optional fields the patch is `Patch<Option<T>>`, so `Set(None)`
    /// assigns `None` (clear) while `Unset` keeps the current value.
    pub fn apply(&self, slot: &mut T) {
        match self {
            Self::Unset => {}
            Self::Set(value) => *slot = value.clone(),
        }
    }
}
