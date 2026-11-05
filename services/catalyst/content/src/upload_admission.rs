//! Admission for `POST /entities` before its body is read: a byte budget
//! shared by every in-flight deploy (the forms are held in memory), each
//! client's share of it ([`SourceUploads`]), a
//! per-client burst limit (upstream's `POST_ENTITIES_RATE_LIMIT_MAX` per
//! `POST_ENTITIES_RATE_LIMIT_WINDOW_SECONDS`, every request counted) and a
//! per-client daily deploy quota, checked before the body and counted once
//! the form is read; both client limits are fixed windows keyed by address
//! (IPv6 by /64), as upstream's rate limiter keys them. All of them are
//! in-process state, never the database:
//! each upstream behind the gateway keeps its own, so N upstreams admit N
//! times the limits, and a restart resets them (upstream's limiter is
//! process-local too).
//!
//! The table holds [`MAX_QUOTA_SOURCES`] windows; a new source past it
//! evicts the oldest, whose next deploy starts a fresh window. A client
//! holding more /64s than that (a /48 alone has 65,536) can so cycle its
//! own windows; the per-second gateway limit and the upload budget still
//! bound it. A coarser prefix would put unrelated hosts of one provider
//! under one quota.
//!
//! The source is the address the gateway stamps, so a node forwarding its
//! accepted deploys here (`content::forward`) is one source: every deploy
//! it forwards shares its quota, and past it the forwarder's sends get 429,
//! retry and end `failed`. No forwarder is trusted to name its
//! own clients; such an upstream raises `POST_ENTITIES_DAILY_QUOTA_MAX` or
//! sets it to 0.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

/// The least a request reserves, covering its fields and per-request
/// overhead, so the budget also bounds how many deploys run at once.
pub const MIN_UPLOAD_RESERVATION_BYTES: u64 = 16 * 1024 * 1024;

pub const BUDGET_EXHAUSTED_MESSAGE: &str =
    "Server is handling too many uploads, please retry shortly.";

const DAILY_WINDOW: Duration = Duration::from_secs(24 * 60 * 60);

/// Client sources tracked at once; a new one past it evicts the oldest window.
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

/// Upstream's 429 for a source past its share of uploads in flight.
pub const SOURCE_BUSY_MESSAGE: &str =
    "Too many uploads in progress from this client, please retry shortly.";

/// A slot frees when one of the source's own uploads ends.
pub const SOURCE_BUSY_RETRY_AFTER_SECS: u64 = 5;

/// Bodies and bytes in flight per client source (upstream's
/// `MAX_CONCURRENT_UPLOADS_PER_SOURCE` and
/// `MAX_IN_FLIGHT_UPLOAD_BYTES_PER_SOURCE`), so one source holds only a
/// share of the [`UploadBudget`] however many requests it opens. Only
/// sources with an upload in flight have an entry; a request without a
/// gateway-stamped address is not limited, as the quotas skip it.
pub struct SourceUploads {
    max_uploads: u32,
    max_bytes: u64,
    in_flight: Mutex<HashMap<String, (u32, u64)>>,
}

/// A source with its share in flight.
#[derive(Debug)]
pub struct SourceBusy;

/// One admitted upload's share of its source's allowance, returned on drop.
pub struct SourceLease<'a> {
    limits: &'a SourceUploads,
    key: String,
    bytes: u64,
}

impl SourceUploads {
    pub fn new(max_uploads: u32, max_bytes: u64) -> Self {
        Self {
            max_uploads,
            max_bytes,
            in_flight: Mutex::new(HashMap::new()),
        }
    }

    /// Admits an upload of up to `bytes` from `client_ip`: `Err` when the
    /// source already has its share in flight, `Ok(None)` without a source.
    pub fn try_acquire(&self, client_ip: &str, bytes: u64) -> Result<Option<SourceLease<'_>>, SourceBusy> {
        if client_ip.trim().is_empty() {
            return Ok(None);
        }
        let key = source_key(client_ip);
        let mut in_flight = self.in_flight.lock();
        let (uploads, held) = in_flight.get(&key).copied().unwrap_or((0, 0));
        if uploads >= self.max_uploads || held.saturating_add(bytes) > self.max_bytes {
            return Err(SourceBusy);
        }
        in_flight.insert(key.clone(), (uploads + 1, held + bytes));
        Ok(Some(SourceLease {
            limits: self,
            key,
            bytes,
        }))
    }
}

impl Drop for SourceLease<'_> {
    fn drop(&mut self) {
        let mut in_flight = self.limits.in_flight.lock();
        if let Some((uploads, held)) = in_flight.get_mut(&self.key) {
            *uploads -= 1;
            *held -= self.bytes;
            if *uploads == 0 {
                in_flight.remove(&self.key);
            }
        }
    }
}

use crate::shared::client_ip::source_key;

struct Window {
    started: Instant,
    count: u32,
}

/// Live windows by source key, and their keys oldest first (one entry each).
#[derive(Default)]
struct Windows {
    by_source: HashMap<String, Window>,
    by_age: VecDeque<String>,
}

impl Windows {
    /// Drops the windows of length `length` that ended by `now`, oldest first.
    fn prune(&mut self, now: Instant, length: Duration) {
        while let Some(key) = self.by_age.front() {
            match self.by_source.get(key) {
                Some(w) if now.saturating_duration_since(w.started) < length => break,
                _ => {
                    let key = self.by_age.pop_front().expect("front exists");
                    self.by_source.remove(&key);
                }
            }
        }
    }

    fn evict_oldest(&mut self) {
        if let Some(key) = self.by_age.pop_front() {
            self.by_source.remove(&key);
        }
    }
}

/// A fixed window of requests per client source (24 hours for the daily
/// quota); `max` 0 is off. Per process: every upstream and restart counts
/// on its own.
pub struct DailyQuota {
    max: u32,
    window: Duration,
    max_sources: usize,
    windows: Mutex<Windows>,
    warned_no_source: AtomicBool,
}

/// One counted deploy, which [`QuotaCharge::refund`] gives back.
pub struct QuotaCharge<'a> {
    quota: &'a DailyQuota,
    key: String,
    started: Instant,
}

impl QuotaCharge<'_> {
    /// Uncounts the deploy, if its window is still the live one.
    pub fn refund(self) {
        let mut windows = self.quota.windows.lock();
        if let Some(w) = windows.by_source.get_mut(&self.key)
            && w.started == self.started
        {
            w.count = w.count.saturating_sub(1);
        }
    }
}

impl DailyQuota {
    pub fn new(max: u32) -> Self {
        Self::with_window(max, DAILY_WINDOW)
    }

    /// `max` requests per `window` and source: the burst limit's shape.
    pub fn with_window(max: u32, window: Duration) -> Self {
        Self::with_max_sources(max, window, MAX_QUOTA_SOURCES)
    }

    fn with_max_sources(max: u32, window: Duration, max_sources: usize) -> Self {
        Self {
            max,
            window,
            max_sources: max_sources.max(1),
            windows: Mutex::new(Windows::default()),
            warned_no_source: AtomicBool::new(false),
        }
    }

    /// Counts one deploy of `client_ip` if it has quota left:
    /// `Err(retry_after_secs)` when spent, `Ok(None)` when nothing is counted
    /// (quota off, or no client address).
    pub fn try_consume(&self, client_ip: &str) -> Result<Option<QuotaCharge<'_>>, u64> {
        self.try_consume_at(client_ip, Instant::now())
    }

    fn try_consume_at(
        &self,
        client_ip: &str,
        now: Instant,
    ) -> Result<Option<QuotaCharge<'_>>, u64> {
        if self.max == 0 {
            return Ok(None);
        }
        if client_ip.trim().is_empty() {
            if !self.warned_no_source.swap(true, Ordering::Relaxed) {
                tracing::warn!(
                    "POST /entities without a gateway-stamped client address; the per-client limits do not apply to such requests"
                );
            }
            return Ok(None);
        }
        let key = source_key(client_ip);
        let mut windows = self.windows.lock();
        windows.prune(now, self.window);
        let started = match windows.by_source.get_mut(&key) {
            Some(w) if w.count >= self.max => return Err(retry_after(w, now, self.window)),
            Some(w) => {
                w.count += 1;
                w.started
            }
            None => {
                if windows.by_source.len() >= self.max_sources {
                    windows.evict_oldest();
                }
                windows.by_source.insert(
                    key.clone(),
                    Window {
                        started: now,
                        count: 1,
                    },
                );
                windows.by_age.push_back(key.clone());
                now
            }
        };
        Ok(Some(QuotaCharge {
            quota: self,
            key,
            started,
        }))
    }
}

impl DailyQuota {
    /// `Some(retry_after_secs)` when `client_ip` has spent its quota; counts nothing.
    pub fn exhausted(&self, client_ip: &str) -> Option<u64> {
        if self.max == 0 || client_ip.trim().is_empty() {
            return None;
        }
        let now = Instant::now();
        let mut windows = self.windows.lock();
        windows.prune(now, self.window);
        windows
            .by_source
            .get(&source_key(client_ip))
            .filter(|w| w.count >= self.max)
            .map(|w| retry_after(w, now, self.window))
    }
}

#[cfg(test)]
impl DailyQuota {
    /// Deploys counted for `client_ip` in its live window.
    pub(crate) fn counted_for(&self, client_ip: &str) -> u32 {
        self.windows
            .lock()
            .by_source
            .get(&source_key(client_ip))
            .map_or(0, |w| w.count)
    }
}

/// Whole seconds until a live `window` of length `length` ends, at least 1.
fn retry_after(window: &Window, now: Instant, length: Duration) -> u64 {
    length
        .saturating_sub(now.saturating_duration_since(window.started))
        .as_secs()
        .max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_source_holds_at_most_its_uploads_and_bytes_in_flight() {
        let limits = SourceUploads::new(2, 100);
        let first = limits.try_acquire("203.0.113.1", 60).unwrap();
        assert!(
            limits.try_acquire("203.0.113.1", 41).is_err(),
            "past the byte share"
        );
        let second = limits.try_acquire("203.0.113.1", 40).unwrap();
        assert!(
            limits.try_acquire("203.0.113.1", 0).is_err(),
            "past the upload count"
        );
        assert!(
            limits.try_acquire("203.0.113.2", 100).unwrap().is_some(),
            "another source has its own share"
        );
        assert!(limits.try_acquire("", 1000).unwrap().is_none());
        drop(first);
        let third = limits.try_acquire("203.0.113.1", 60).unwrap();
        drop((second, third));
        assert!(limits.in_flight.lock().is_empty());
    }

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

    fn counted(quota: &DailyQuota, ip: &str, at: Instant) -> bool {
        quota.try_consume_at(ip, at).expect("quota left").is_some()
    }

    #[test]
    fn the_quota_counts_per_source_and_reports_the_window_left() {
        let quota = DailyQuota::new(2);
        let now = Instant::now();
        assert!(counted(&quota, "203.0.113.1", now));
        assert!(counted(&quota, "203.0.113.1", now));
        let retry = quota
            .try_consume_at("203.0.113.1", now + Duration::from_secs(60))
            .err()
            .expect("spent");
        assert_eq!(retry, 24 * 60 * 60 - 60);
        assert!(
            counted(&quota, "203.0.113.2", now),
            "another source has its own"
        );
    }

    #[test]
    fn an_ended_window_starts_over_and_zero_is_off() {
        let quota = DailyQuota::new(1);
        let now = Instant::now();
        assert!(counted(&quota, "203.0.113.1", now));
        assert!(quota.try_consume_at("203.0.113.1", now).is_err());
        assert!(counted(&quota, "203.0.113.1", now + DAILY_WINDOW));
        assert!(
            quota
                .try_consume_at("203.0.113.1", now + DAILY_WINDOW)
                .is_err()
        );

        let off = DailyQuota::new(0);
        for _ in 0..10 {
            assert!(off.try_consume("203.0.113.1").unwrap().is_none());
        }
    }

    #[test]
    fn a_refund_gives_the_deploy_back() {
        let quota = DailyQuota::new(1);
        let charge = quota.try_consume("203.0.113.1").unwrap().unwrap();
        assert!(quota.try_consume("203.0.113.1").is_err());
        charge.refund();
        assert!(quota.try_consume("203.0.113.1").unwrap().is_some());
    }

    #[test]
    fn a_refund_after_its_window_ended_is_ignored() {
        let quota = DailyQuota::new(1);
        let now = Instant::now();
        let charge = quota.try_consume_at("203.0.113.1", now).unwrap().unwrap();
        let next_day = now + DAILY_WINDOW;
        assert!(counted(&quota, "203.0.113.1", next_day));
        charge.refund();
        assert!(
            quota.try_consume_at("203.0.113.1", next_day).is_err(),
            "the new window keeps its count"
        );
    }

    #[test]
    fn a_new_source_past_the_table_evicts_only_the_oldest_window() {
        let quota = DailyQuota::with_max_sources(1, DAILY_WINDOW, 3);
        let now = Instant::now();
        for (i, ip) in ["203.0.113.1", "203.0.113.2", "203.0.113.3"]
            .iter()
            .enumerate()
        {
            assert!(counted(&quota, ip, now + Duration::from_secs(i as u64)));
        }
        assert!(counted(&quota, "203.0.113.4", now + Duration::from_secs(3)));
        let later = now + Duration::from_secs(4);
        assert!(
            quota.try_consume_at("203.0.113.2", later).is_err()
                && quota.try_consume_at("203.0.113.3", later).is_err()
                && quota.try_consume_at("203.0.113.4", later).is_err(),
            "the newer windows keep their counts"
        );
        assert!(
            counted(&quota, "203.0.113.1", later),
            "the oldest was evicted"
        );
        assert_eq!(quota.windows.lock().by_source.len(), 3);
        assert_eq!(quota.windows.lock().by_age.len(), 3);
    }

    #[test]
    fn ipv6_sources_are_grouped_by_their_64() {
        let quota = DailyQuota::new(1);
        assert!(quota.try_consume("2001:db8:1:2::1").unwrap().is_some());
        assert!(
            quota
                .try_consume("2001:db8:1:2:ffff:ffff:ffff:ffff")
                .is_err(),
            "the same /64 shares one quota"
        );
        assert!(quota.try_consume("2001:db8:1:3::1").unwrap().is_some());
        assert_eq!(source_key("2001:db8:1:2:abcd::9"), "2001:db8:1:2::/64");
        assert_eq!(source_key("::ffff:203.0.113.5"), "203.0.113.5");
        assert_eq!(source_key("203.0.113.5"), "203.0.113.5");
    }

    #[test]
    fn a_short_window_limits_bursts_and_reports_its_own_end() {
        let burst = DailyQuota::with_window(2, Duration::from_secs(60));
        let now = Instant::now();
        assert!(counted(&burst, "203.0.113.1", now));
        assert!(counted(&burst, "203.0.113.1", now + Duration::from_secs(1)));
        assert_eq!(
            burst
                .try_consume_at("203.0.113.1", now + Duration::from_secs(20))
                .err(),
            Some(40)
        );
        assert!(
            counted(&burst, "203.0.113.1", now + Duration::from_secs(60)),
            "the next window starts over"
        );
    }

    #[test]
    fn a_request_without_a_client_address_is_not_counted() {
        let quota = DailyQuota::new(1);
        for _ in 0..3 {
            assert!(quota.try_consume("").unwrap().is_none());
        }
        assert!(quota.windows.lock().by_source.is_empty());
    }
}
