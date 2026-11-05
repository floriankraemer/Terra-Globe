//! Shared test infrastructure: type-level fixtures extracted from the
//! TypeScript implementation, plus a small PNG snapshot comparer (insta is
//! used for text snapshots; image snapshots need a tolerance and a diff file).

use std::path::Path;

use image::{ImageFormat, RgbaImage};

pub const CORE_PARITY_JSON: &str = include_str!("../fixtures/core-parity.json");

pub fn core_parity() -> serde_json::Value {
    serde_json::from_str(CORE_PARITY_JSON).expect("core parity fixture is valid JSON")
}

#[derive(Debug)]
pub struct ImageMismatch {
    pub message: String,
}

impl std::fmt::Display for ImageMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ImageMismatch {}

/// Compares two PNG byte streams, allowing per-channel differences up to
/// `max_channel_delta` and up to `max_bad_pixels` offending pixels.
pub fn compare_png(
    actual: &[u8],
    expected: &[u8],
    max_channel_delta: u8,
    max_bad_pixels: u64,
) -> Result<(), ImageMismatch> {
    let actual = image::load_from_memory(actual)
        .map_err(|error| ImageMismatch {
            message: format!("could not decode actual image: {error}"),
        })?
        .to_rgba8();
    let expected = image::load_from_memory(expected)
        .map_err(|error| ImageMismatch {
            message: format!("could not decode snapshot image: {error}"),
        })?
        .to_rgba8();

    if actual.dimensions() != expected.dimensions() {
        return Err(ImageMismatch {
            message: format!(
                "image size changed: actual {:?}, snapshot {:?}",
                actual.dimensions(),
                expected.dimensions()
            ),
        });
    }

    let mut bad_pixels = 0u64;
    let mut worst = 0u8;
    for (x, y, actual_pixel) in actual.enumerate_pixels() {
        let expected_pixel = expected.get_pixel(x, y);
        let delta = actual_pixel
            .0
            .iter()
            .zip(expected_pixel.0.iter())
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        if delta > max_channel_delta {
            bad_pixels += 1;
            worst = worst.max(delta);
        }
    }

    if bad_pixels > max_bad_pixels {
        return Err(ImageMismatch {
            message: format!(
                "{bad_pixels} pixels differ (worst channel delta {worst}, allowed {max_bad_pixels} bad pixels)"
            ),
        });
    }
    Ok(())
}

/// Compares `actual` with the snapshot at `path`, or writes it when
/// `UPDATE_SNAPSHOTS=1` is set. A failed comparison also writes
/// `<path>.new.png` so the difference can be inspected.
pub fn check_or_update_snapshot(
    path: &Path,
    actual: &[u8],
    max_channel_delta: u8,
    max_bad_pixels: u64,
) -> Result<(), ImageMismatch> {
    if std::env::var("UPDATE_SNAPSHOTS").as_deref() == Ok("1") {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| ImageMismatch {
                message: format!("could not create snapshot directory: {error}"),
            })?;
        }
        std::fs::write(path, actual).map_err(|error| ImageMismatch {
            message: format!("could not write snapshot {}: {error}", path.display()),
        })?;
        return Ok(());
    }

    let expected = std::fs::read(path).map_err(|error| ImageMismatch {
        message: format!(
            "could not read snapshot {}: {error} (run with UPDATE_SNAPSHOTS=1 to create it)",
            path.display()
        ),
    })?;

    match compare_png(actual, &expected, max_channel_delta, max_bad_pixels) {
        Ok(()) => Ok(()),
        Err(mismatch) => {
            let new_path = path.with_extension("new.png");
            let _ = std::fs::write(&new_path, actual);
            Err(ImageMismatch {
                message: format!(
                    "{} (wrote {} for inspection)",
                    mismatch.message,
                    new_path.display()
                ),
            })
        }
    }
}

/// Saves raw RGBA pixels as PNG, for renderer tests that read back a texture.
pub fn rgba_to_png(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let buffer = RgbaImage::from_raw(width, height, rgba.to_vec()).expect("pixel buffer size");
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(buffer)
        .write_to(&mut bytes, ImageFormat::Png)
        .expect("encode png");
    bytes.into_inner()
}
