/// Generates unique entity ids (UUID v4 in production, deterministic in tests).
pub trait IdGenerator: Send + Sync {
    fn next_id(&self) -> String;
}
