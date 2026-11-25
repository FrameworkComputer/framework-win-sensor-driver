//! When to fire the polling timer next.
//!
//! Instead of a fixed delay, each wait aims at `start + interval * n` so
//! that jitter doesn't accumulate, and falls behind by at most one beat
//! before catching up.

/// Polling schedule since the last first sample.
#[derive(Debug, Clone, Copy, Default)]
pub struct Schedule {
    /// `GetPerformanceTime` at the first sample, 0 if unknown.
    start_ms: u32,
    sample_count: u64,
}

impl Schedule {
    /// Starts counting from the first sample, taken at `now_ms`.
    pub fn restart(&mut self, now_ms: Option<u32>) {
        self.start_ms = now_ms.unwrap_or(0);
        self.sample_count = 0;
    }

    /// Milliseconds until the next sample is due.
    ///
    /// Falls back to `interval_ms` when the start or current time is
    /// unknown, and returns 0 to catch up after missing two or more beats.
    ///
    /// Same as the C driver, except that time is measured relative to the
    /// start so that the 32-bit millisecond counter can wrap around, and
    /// that the schedule restarts from `now_ms` if it would wait more than
    /// two intervals (the longest the C driver waits). That happens once
    /// the elapsed time itself wraps, after 49.7 days of sampling.
    pub fn next_due_ms(&mut self, interval_ms: u32, now_ms: Option<u32>) -> u64 {
        let interval = u64::from(interval_ms);
        if self.start_ms == 0 {
            return interval;
        }
        let Some(now_ms) = now_ms else {
            return interval;
        };

        self.sample_count += 1;
        let target = interval.saturating_mul(self.sample_count + 1);
        let elapsed = u64::from(now_ms.wrapping_sub(self.start_ms));
        if elapsed > target {
            return 0;
        }
        let wait = target - elapsed;
        if wait > interval.saturating_mul(2) {
            self.restart(Some(now_ms));
            return interval;
        }
        wait
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_times_use_interval() {
        let mut s = Schedule::default();
        assert_eq!(s.next_due_ms(10, Some(1000)), 10);
        s.restart(None);
        assert_eq!(s.next_due_ms(10, Some(1000)), 10);
        s.restart(Some(1000));
        assert_eq!(s.next_due_ms(10, None), 10);
    }

    #[test]
    fn steady_state() {
        let mut s = Schedule::default();
        s.restart(Some(1000));
        // As in the C driver, the second sample comes two intervals after
        // the first.
        assert_eq!(s.next_due_ms(10, Some(1000)), 20);
        assert_eq!(s.next_due_ms(10, Some(1020)), 10);
        // Fired late: shorten the next wait.
        assert_eq!(s.next_due_ms(10, Some(1033)), 7);
        assert_eq!(s.next_due_ms(10, Some(1050)), 0);
        // Behind schedule: fire immediately.
        assert_eq!(s.next_due_ms(10, Some(1071)), 0);
    }

    #[test]
    fn counter_wraps() {
        let mut s = Schedule::default();
        s.restart(Some(u32::MAX - 5));
        assert_eq!(s.next_due_ms(10, Some(u32::MAX - 5)), 20);
        assert_eq!(s.next_due_ms(10, Some(14)), 10);
        assert_eq!(s.next_due_ms(10, Some(24)), 10);
    }

    #[test]
    fn elapsed_time_wraps() {
        // 2^32 ms in, the elapsed time wraps to 0 while the target doesn't.
        let mut s = Schedule {
            start_ms: 1000,
            sample_count: (1 << 32) / 10,
        };
        assert_eq!(s.next_due_ms(10, Some(1005)), 10);
        // Restarted from there.
        assert_eq!(s.next_due_ms(10, Some(1015)), 10);
        assert_eq!(s.next_due_ms(10, Some(1025)), 10);
    }
}
