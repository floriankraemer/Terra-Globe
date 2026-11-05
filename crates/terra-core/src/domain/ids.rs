//! Identifier generation.
//!
//! `crypto.randomUUID()` in the TypeScript app produces a v4 UUID string; the
//! `js` feature of `uuid` gives the browser target a matching source of
//! randomness.

use uuid::Uuid;

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}
