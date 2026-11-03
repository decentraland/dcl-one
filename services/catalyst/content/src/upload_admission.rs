//! Admission for `POST /entities` before its body is read: a byte budget
//! shared by every in-flight deploy (the forms are held in memory) and a
//! per-client daily deploy quota. In-process state, never the database.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

/// The least a request reserves, covering its fields and per-request
/// overhead, so the budget also bounds how many deploys run at once.
pub const MIN_UPLOAD_RESERVATION_BYTES: u64 = 16 * 1024 * 1024;

pub const BUDGET_EXHAUSTED_MESSAGE: &str =
    "Server is handling too many uploads, please retry shortly.";

const DAILY_WINDOW: Duration = Duration::from_secs(24 * 60 * 60);

/// Client addresses tracked at once; past it the expired windows are
/// dropped, and all of them if none had expired.
const MAX_QUOTA_SOURCES: usize = 100_000;

/// Bytes reserved by in-flight deploys out of a fixed capacity.
pub struct UploadBudget {
    capacity: u64,
    reserved: Mutex<u64>,
}

/// One request's share of an [`UploadBudget`], returned on drop.
pub struct Reservation<'a> {
    budget: &'a UploadBudget,
    bytes: u64,
}

impl UploadBudget {
    pub fn new(capacity: u64) -> Self {
        Self {
            capacity,
            reserved: Mutex::new(0),
        }
    }

    /// Reserves `bytes` (at least [`MIN_UPLOAD_RESERVATION_BYTES`]); `None`
    /// when they do not fit beside what is already reserved.
    pub fn try_reserve(&self, bytes: u64) -> Option<Reservation<'_>> {
        let bytes = bytes.max(MIN_UPLOAD_RESERVATION_BYTES);
        let mut reserved = self.reserved.lock();
        if reserved.saturating_add(bytes) > self.capacity {
            return None;
        }
        *reserved += bytes;
        Some(Reservation {
            budget: self,
            bytes,
        })
    }
}

impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        *self.budget.reserved.lock() -= self.bytes;
    }
}

struct Window {
    started: Instant,
    count: u32,
}

/// A fixed 24-hour window of deploys per client address; `max` 0 is off.
pub struct DailyQuota {
    max: u32,
    windows: Mutex<HashMap<String, Window>>,
}

impl DailyQuota {
    pub fn new(max: u32) -> Self {
        Self {
            max,
            windows: Mutex::new(HashMap::new()),
        }
    }

    /// `Some(retry_after_secs)` while `source` has spent its quota; counts nothing.
    pub fn exhausted(&self, source: &str) -> Option<u64> {
        if self.max == 0 {
            return None;
        }
        let now = Instant::now();
        let windows = self.windows.lock();
        let window = windows.get(source)?;
        (window.count >= self.max)
            .then(|| retry_after(window, now))
            .flatten()
    }

    /// Counts one deploy of `source`; `Err(retry_after_secs)` past the quota.
    pub fn consume(&self, source: &str) -> Result<(), u64> {
        if self.max == 0 {
            return Ok(());
        }
        let now = Instant::now();
        let mut windows = self.windows.lock();
        if windows.len() >= MAX_QUOTA_SOURCES && !windows.contains_key(source) {
            windows.retain(|_, w| now.duration_since(w.started) < DAILY_WINDOW);
            if windows.len() >= MAX_QUOTA_SOURCES {
                windows.clear();
            }
        }
        let window = windows.entry(source.to_string()).or_insert(Window {
            started: now,
            count: 0,
        });
        if now.duration_since(window.started) >= DAILY_WINDOW {
            *window = Window {
                started: now,
                count: 0,
            };
        }
        if window.count >= self.max {
            return Err(retry_after(window, now).unwrap_or(1));
        }
        window.count += 1;
        Ok(())
    }
}

/// Whole seconds until `window` ends, at least 1; `None` once it has ended.
fn retry_after(window: &Window, now: Instant) -> Option<u64> {
    let left = DAILY_WINDOW.checked_sub(now.duration_since(window.started))?;
    (!left.is_zero()).then(|| left.as_secs().max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_budget_admits_until_full_and_frees_on_drop() {
        let budget = UploadBudget::new(3 * MIN_UPLOAD_RESERVATION_BYTES);
        let small = budget
            .try_reserve(1)
            .expect("a small body reserves the minimum");
        let large = budget
            .try_reserve(2 * MIN_UPLOAD_RESERVATION_BYTES)
            .expect("fits beside the first");
        assert!(budget.try_reserve(0).is_none(), "the budget is full");
        drop(small);
        assert!(budget.try_reserve(0).is_some());
        drop(large);
        assert!(
            budget
                .try_reserve(3 * MIN_UPLOAD_RESERVATION_BYTES)
                .is_some()
        );
        assert!(
            budget
                .try_reserve(3 * MIN_UPLOAD_RESERVATION_BYTES + 1)
                .is_none()
        );
    }

    #[test]
    fn the_quota_counts_per_source_and_reports_the_window_left() {
        let quota = DailyQuota::new(2);
        assert_eq!(quota.exhausted("a"), None);
        assert_eq!(quota.consume("a"), Ok(()));
        assert_eq!(quota.exhausted("a"), None);
        assert_eq!(quota.consume("a"), Ok(()));
        let retry = quota.exhausted("a").expect("spent");
        assert!(
            retry > 24 * 60 * 60 - 60 && retry <= 24 * 60 * 60,
            "{retry}"
        );
        assert!(quota.consume("a").is_err());
        assert_eq!(quota.consume("b"), Ok(()), "another source has its own");
    }

    #[test]
    fn an_ended_window_starts_over_and_zero_is_off() {
        let quota = DailyQuota::new(1);
        quota.consume("a").unwrap();
        quota.windows.lock().get_mut("a").unwrap().started = Instant::now() - DAILY_WINDOW;
        assert_eq!(quota.exhausted("a"), None);
        assert_eq!(quota.consume("a"), Ok(()));
        assert!(quota.exhausted("a").is_some());

        let off = DailyQuota::new(0);
        for _ in 0..10 {
            assert_eq!(off.consume("a"), Ok(()));
        }
        assert_eq!(off.exhausted("a"), None);
    }
}
