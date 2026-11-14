//! A per-process fixed-window counter for user writes that have no natural
//! bound (reports, poster uploads): each key may act its limit times per
//! `window`. A call names every key it counts against (a wallet and a
//! network) and is counted against all of them or none. The map is bounded:
//! once full it drops closed windows, then the oldest ones, and never
//! refuses a key for lack of room.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

type Windows = HashMap<String, (Instant, u32)>;

pub struct WindowLimiter {
    windows: Mutex<Windows>,
    window: Duration,
    max_keys: usize,
}

impl WindowLimiter {
    pub fn new(window: Duration, max_keys: usize) -> Self {
        Self {
            windows: Mutex::new(HashMap::new()),
            window,
            max_keys: max_keys.max(1),
        }
    }

    /// Counts one action against every `(key, limit)`; `false`, with nothing
    /// counted, when any key has used its limit in the current window.
    pub fn try_acquire(&self, keys: &[(&str, u32)]) -> bool {
        self.try_acquire_at(keys, Instant::now())
    }

    fn try_acquire_at(&self, keys: &[(&str, u32)], now: Instant) -> bool {
        let mut windows = self.windows.lock();
        let window = self.window;
        let open = |at: &Instant| now.saturating_duration_since(*at) < window;
        let over = keys.iter().any(|(key, limit)| {
            windows
                .get(*key)
                .is_some_and(|(at, count)| open(at) && *count >= *limit)
        });
        if over {
            return false;
        }
        let new_keys = keys
            .iter()
            .filter(|(k, _)| !windows.contains_key(*k))
            .count();
        if windows.len() + new_keys > self.max_keys {
            make_room(&mut windows, self.max_keys, new_keys, open);
        }
        for (key, _) in keys {
            let entry = windows.entry((*key).to_string()).or_insert((now, 0));
            if !open(&entry.0) {
                *entry = (now, 0);
            }
            entry.1 += 1;
        }
        true
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.windows.lock().len()
    }
}

/// Drops closed windows, then the oldest open ones down to ~90% of the cap,
/// so a full map pays one scan per tenth of its size.
fn make_room(
    windows: &mut Windows,
    max_keys: usize,
    incoming: usize,
    open: impl Fn(&Instant) -> bool,
) {
    windows.retain(|_, (at, _)| open(at));
    let low_water = max_keys.saturating_sub(max_keys / 10 + 1).max(1);
    let len = windows.len() + incoming;
    if len <= max_keys {
        return;
    }
    let excess = len.saturating_sub(low_water.max(incoming)).max(1);
    let mut starts: Vec<(Instant, String)> = windows
        .iter()
        .map(|(k, (at, _))| (*at, k.clone()))
        .collect();
    if starts.len() > excess {
        starts.select_nth_unstable_by_key(excess - 1, |(at, _)| *at);
        starts.truncate(excess);
    }
    for (_, key) in starts {
        windows.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_refused_past_its_limit_until_the_window_closes() {
        let limiter = WindowLimiter::new(Duration::from_secs(60), 100);
        let t0 = Instant::now();
        assert!(limiter.try_acquire_at(&[("a", 2)], t0));
        assert!(limiter.try_acquire_at(&[("a", 2)], t0));
        assert!(!limiter.try_acquire_at(&[("a", 2)], t0));
        assert!(limiter.try_acquire_at(&[("b", 2)], t0), "keys count apart");
        assert!(limiter.try_acquire_at(&[("a", 2)], t0 + Duration::from_secs(61)));
    }

    #[test]
    fn a_refused_call_counts_against_no_key() {
        let limiter = WindowLimiter::new(Duration::from_secs(60), 100);
        let t0 = Instant::now();
        assert!(limiter.try_acquire_at(&[("ip:1", 1), ("w:a", 5)], t0));
        assert!(!limiter.try_acquire_at(&[("ip:1", 1), ("w:b", 5)], t0));
        assert!(
            limiter.try_acquire_at(&[("ip:2", 1), ("w:b", 1)], t0),
            "the refused call did not count w:b"
        );
        assert_eq!(limiter.len(), 4, "the refused call added no key");
    }

    #[test]
    fn a_full_map_evicts_the_oldest_and_keeps_admitting_new_keys() {
        let limiter = WindowLimiter::new(Duration::from_secs(60), 10);
        let t0 = Instant::now();
        for i in 0..1_000u32 {
            let key = format!("k{i}");
            assert!(
                limiter.try_acquire_at(&[(key.as_str(), 1)], t0 + Duration::from_millis(i.into())),
                "a new key is never refused for room"
            );
            assert!(limiter.len() <= 10, "{}", limiter.len());
        }
        assert!(
            !limiter.try_acquire_at(&[("k999", 1)], t0 + Duration::from_secs(1)),
            "the newest window survives eviction"
        );
    }
}
