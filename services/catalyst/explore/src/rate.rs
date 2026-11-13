//! A per-process fixed-window counter for user writes that have no natural
//! bound (reports, poster uploads): each key may act `limit` times per
//! `window`. The map is pruned of closed windows once it reaches `max_keys`,
//! and refuses unknown keys while it is still full.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

type Windows = HashMap<String, (Instant, u32)>;

pub struct WindowLimiter {
    windows: Mutex<Windows>,
    limit: u32,
    window: Duration,
    max_keys: usize,
}

impl WindowLimiter {
    pub fn new(limit: u32, window: Duration, max_keys: usize) -> Self {
        Self {
            windows: Mutex::new(HashMap::new()),
            limit,
            window,
            max_keys,
        }
    }

    /// Counts one action for every key; `false` (and nothing counted) when
    /// any key is over its limit.
    pub fn try_acquire(&self, keys: &[&str]) -> bool {
        self.try_acquire_at(keys, Instant::now())
    }

    fn try_acquire_at(&self, keys: &[&str], now: Instant) -> bool {
        let mut windows = self.windows.lock();
        let open = |at: &Instant| now.saturating_duration_since(*at) < self.window;
        let new_keys = keys.iter().filter(|k| !windows.contains_key(**k)).count();
        if windows.len() + new_keys > self.max_keys {
            windows.retain(|_, (at, _)| open(at));
            if windows.len() + new_keys > self.max_keys {
                return false;
            }
        }
        let over = keys.iter().any(|key| {
            windows
                .get(*key)
                .is_some_and(|(at, count)| open(at) && *count >= self.limit)
        });
        if over {
            return false;
        }
        for key in keys {
            let entry = windows.entry((*key).to_string()).or_insert((now, 0));
            if !open(&entry.0) {
                *entry = (now, 0);
            }
            entry.1 += 1;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_refused_past_its_limit_until_the_window_closes() {
        let limiter = WindowLimiter::new(2, Duration::from_secs(60), 100);
        let t0 = Instant::now();
        assert!(limiter.try_acquire_at(&["a"], t0));
        assert!(limiter.try_acquire_at(&["a"], t0));
        assert!(!limiter.try_acquire_at(&["a"], t0));
        assert!(limiter.try_acquire_at(&["b"], t0), "keys count apart");
        assert!(limiter.try_acquire_at(&["a"], t0 + Duration::from_secs(61)));
    }

    #[test]
    fn any_key_over_its_limit_refuses_without_counting_the_others() {
        let limiter = WindowLimiter::new(1, Duration::from_secs(60), 100);
        let t0 = Instant::now();
        assert!(limiter.try_acquire_at(&["wallet:a", "ip:1"], t0));
        assert!(!limiter.try_acquire_at(&["wallet:b", "ip:1"], t0));
        assert!(
            limiter.try_acquire_at(&["wallet:b", "ip:2"], t0),
            "the refused call did not count wallet:b"
        );
    }

    #[test]
    fn a_full_map_drops_closed_windows_before_refusing_new_keys() {
        let limiter = WindowLimiter::new(5, Duration::from_secs(60), 2);
        let t0 = Instant::now();
        assert!(limiter.try_acquire_at(&["a"], t0));
        assert!(limiter.try_acquire_at(&["b"], t0));
        assert!(!limiter.try_acquire_at(&["c"], t0), "full of open windows");
        assert!(limiter.try_acquire_at(&["c"], t0 + Duration::from_secs(61)));
    }
}
