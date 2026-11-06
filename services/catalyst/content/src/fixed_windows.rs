//! Fixed-window counters per client key, shared by the `/contents` limit
//! ([`crate::content_get_limit`]) and the deploy quota
//! ([`crate::upload_admission`]): a key's window starts at its first count
//! and ends `length` later; at most `max_keys` keys are tracked, a new one
//! past that evicting the oldest window.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

pub(crate) struct Window {
    pub(crate) started: Instant,
    pub(crate) count: u32,
}

impl Window {
    /// Whole seconds until this window ends at `now`, at least 1.
    pub(crate) fn reset_secs(&self, length: Duration, now: Instant) -> u64 {
        let left = length.saturating_sub(now.saturating_duration_since(self.started));
        (left.as_secs() + u64::from(left.subsec_nanos() > 0)).max(1)
    }
}

/// Live windows by key, and their keys oldest first (one entry each).
pub(crate) struct FixedWindows {
    length: Duration,
    max_keys: usize,
    by_key: HashMap<String, Window>,
    by_age: VecDeque<String>,
}

impl FixedWindows {
    pub(crate) fn new(length: Duration, max_keys: usize) -> Self {
        Self {
            length,
            max_keys: max_keys.max(1),
            by_key: HashMap::new(),
            by_age: VecDeque::new(),
        }
    }

    pub(crate) fn length(&self) -> Duration {
        self.length
    }

    /// Drops the windows that ended by `now`, oldest first.
    pub(crate) fn prune(&mut self, now: Instant) {
        while let Some(key) = self.by_age.front() {
            match self.by_key.get(key) {
                Some(w) if now.saturating_duration_since(w.started) < self.length => break,
                _ => {
                    let key = self.by_age.pop_front().expect("front exists");
                    self.by_key.remove(&key);
                }
            }
        }
    }

    /// `key`'s live window; call [`Self::prune`] first.
    pub(crate) fn get(&self, key: &str) -> Option<&Window> {
        self.by_key.get(key)
    }

    pub(crate) fn get_mut(&mut self, key: &str) -> Option<&mut Window> {
        self.by_key.get_mut(key)
    }

    /// `key`'s live window, a new empty one starting at `now` when it has
    /// none (evicting the oldest past `max_keys`); call [`Self::prune`] first.
    pub(crate) fn window(&mut self, key: &str, now: Instant) -> &mut Window {
        if !self.by_key.contains_key(key) {
            if self.by_key.len() >= self.max_keys
                && let Some(oldest) = self.by_age.pop_front()
            {
                self.by_key.remove(&oldest);
            }
            self.by_key.insert(
                key.to_string(),
                Window {
                    started: now,
                    count: 0,
                },
            );
            self.by_age.push_back(key.to_string());
        }
        self.by_key.get_mut(key).expect("inserted above")
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.by_key.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_ends_after_its_length_and_the_oldest_is_evicted_past_the_cap() {
        let start = Instant::now();
        let mut windows = FixedWindows::new(Duration::from_secs(10), 2);
        windows.window("a", start).count += 1;
        windows.window("b", start + Duration::from_secs(1)).count += 1;
        windows.window("c", start + Duration::from_secs(2)).count += 1;
        assert!(windows.get("a").is_none(), "evicted as the oldest");
        assert_eq!(windows.len(), 2);
        assert_eq!(
            windows
                .get("b")
                .unwrap()
                .reset_secs(windows.length(), start + Duration::from_millis(1500)),
            10
        );
        windows.prune(start + Duration::from_secs(11));
        assert!(windows.get("b").is_none(), "its window ended");
        assert_eq!(windows.get("c").unwrap().count, 1);
    }
}
