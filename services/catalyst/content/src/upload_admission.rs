//! Admission for `POST /entities` before its body is read: a byte budget
//! shared by every in-flight deploy (the forms are held in memory) and a
//! per-client daily deploy quota (IPv6 by /64), counted at admission. Both
//! are in-process state, never the database: each upstream behind the
//! gateway keeps its own, so N upstreams admit N times the quota, and a
//! restart resets it.
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
use std::net::{IpAddr, Ipv6Addr};
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

/// Bits of an IPv6 address that name one client, as for content downloads
/// (`CONTENT_GET_RATE_LIMIT_IPV6_PREFIX_LENGTH`): a host routed a /64 is one source.
const IPV6_SOURCE_PREFIX_BITS: u32 = 64;

/// The quota key of a client address: IPv4 as is, IPv6 by its /64 network.
fn source_key(client_ip: &str) -> String {
    let Ok(IpAddr::V6(v6)) = client_ip.parse::<IpAddr>() else {
        return client_ip.to_string();
    };
    if let Some(v4) = v6.to_ipv4_mapped() {
        return v4.to_string();
    }
    let mask = u128::MAX << (128 - IPV6_SOURCE_PREFIX_BITS);
    let network = Ipv6Addr::from(u128::from(v6) & mask);
    format!("{network}/{IPV6_SOURCE_PREFIX_BITS}")
}

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
    /// Drops the windows that ended by `now`, oldest first.
    fn prune(&mut self, now: Instant) {
        while let Some(key) = self.by_age.front() {
            match self.by_source.get(key) {
                Some(w) if now.saturating_duration_since(w.started) < DAILY_WINDOW => break,
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

/// A fixed 24-hour window of deploys per client source; `max` 0 is off.
/// Per process: every upstream and restart counts on its own.
pub struct DailyQuota {
    max: u32,
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
        Self::with_max_sources(max, MAX_QUOTA_SOURCES)
    }

    fn with_max_sources(max: u32, max_sources: usize) -> Self {
        Self {
            max,
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
                    "POST /entities without a gateway-stamped client address; the daily quota does not apply to such requests"
                );
            }
            return Ok(None);
        }
        let key = source_key(client_ip);
        let mut windows = self.windows.lock();
        windows.prune(now);
        let started = match windows.by_source.get_mut(&key) {
            Some(w) if w.count >= self.max => return Err(retry_after(w, now)),
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
        windows.prune(now);
        windows
            .by_source
            .get(&source_key(client_ip))
            .filter(|w| w.count >= self.max)
            .map(|w| retry_after(w, now))
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

/// Whole seconds until a live `window` ends, at least 1.
fn retry_after(window: &Window, now: Instant) -> u64 {
    DAILY_WINDOW
        .saturating_sub(now.saturating_duration_since(window.started))
        .as_secs()
        .max(1)
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
        let quota = DailyQuota::with_max_sources(1, 3);
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
    fn a_request_without_a_client_address_is_not_counted() {
        let quota = DailyQuota::new(1);
        for _ in 0..3 {
            assert!(quota.try_consume("").unwrap().is_none());
        }
        assert!(quota.windows.lock().by_source.is_empty());
    }
}
