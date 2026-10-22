use std::sync::Mutex;

use crate::ports::Clock;

/// A clock that only moves when a test says so.
#[derive(Debug)]
pub struct FakeClock {
    now_ms: Mutex<u64>,
}

impl Default for FakeClock {
    fn default() -> Self {
        Self::at_ms(1_700_000_000_000)
    }
}

impl FakeClock {
    pub fn at_ms(epoch_ms: u64) -> Self {
        Self {
            now_ms: Mutex::new(epoch_ms),
        }
    }

    pub fn advance_ms(&self, ms: u64) {
        *self.now_ms.lock().expect("clock poisoned") += ms;
    }

    pub fn epoch_ms(&self) -> u64 {
        *self.now_ms.lock().expect("clock poisoned")
    }
}

impl Clock for FakeClock {
    fn now_iso8601(&self) -> String {
        iso8601_from_epoch_ms(self.epoch_ms())
    }

    fn monotonic_ms(&self) -> u64 {
        self.epoch_ms()
    }
}

/// Formats Unix epoch milliseconds like JavaScript's `Date.toISOString()`.
pub fn iso8601_from_epoch_ms(epoch_ms: u64) -> String {
    let secs = epoch_ms / 1000;
    let millis = epoch_ms % 1000;
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (y, m, d) = civil_from_days(days as i64);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

// Howard Hinnant's days-to-civil algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_javascript_to_iso_string() {
        assert_eq!(iso8601_from_epoch_ms(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(
            iso8601_from_epoch_ms(1_700_000_000_123),
            "2023-11-14T22:13:20.123Z"
        );
        assert_eq!(
            iso8601_from_epoch_ms(951_782_400_000),
            "2000-02-29T00:00:00.000Z"
        );
    }

    #[test]
    fn advancing_changes_both_clocks() {
        let clock = FakeClock::at_ms(1000);
        clock.advance_ms(500);
        assert_eq!(clock.monotonic_ms(), 1500);
        assert_eq!(clock.now_iso8601(), "1970-01-01T00:00:01.500Z");
    }
}
