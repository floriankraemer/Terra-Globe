//! Timestamps shared by every entity.
//!
//! The TypeScript app uses `new Date().toISOString()`, which is always UTC with
//! millisecond precision and a `Z` suffix. Matching that exactly keeps row data
//! interchangeable between the two implementations.

use chrono::{SecondsFormat, Utc};

pub fn now_iso8601() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}
