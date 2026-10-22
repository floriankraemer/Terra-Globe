use std::sync::atomic::{AtomicU64, Ordering};

use crate::ports::IdGenerator;

/// Produces `id-1`, `id-2`, ... for deterministic tests.
#[derive(Debug, Default)]
pub struct SequentialIdGenerator {
    next: AtomicU64,
}

impl IdGenerator for SequentialIdGenerator {
    fn next_id(&self) -> String {
        let n = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        format!("id-{n}")
    }
}
