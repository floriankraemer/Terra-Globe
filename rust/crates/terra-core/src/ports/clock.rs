/// Wall-clock and monotonic time, injected so domain logic is testable.
pub trait Clock: Send + Sync {
    /// Current time as an ISO 8601 / RFC 3339 UTC string with millisecond
    /// precision (`2024-01-31T12:00:00.000Z`), the format `Date.toISOString()` produced.
    fn now_iso8601(&self) -> String;
    /// Milliseconds since an arbitrary fixed origin; only differences matter.
    fn monotonic_ms(&self) -> u64;
}
