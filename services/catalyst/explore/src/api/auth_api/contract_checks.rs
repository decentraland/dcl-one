//! A budget of refused contract-wallet checks per network. A forged
//! contract-wallet chain stores nothing, so the pending caps never see it,
//! while each one costs an `isValidSignature` call: a network that spends its
//! budget is refused before the RPC until the window ends. A check that
//! verifies is given back, so honest contract wallets never run it down.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

/// Refused checks one network may cause per window.
pub(super) const PER_SOURCE: u32 = 10;
/// Refused checks shared by requests with no network (no socket peer, an
/// untrusted proxy) and by networks past `MAX_SOURCES`.
const SHARED: u32 = 100;
const WINDOW: Duration = Duration::from_secs(60);
const MAX_SOURCES: usize = 4096;
const SHARED_KEY: &str = "";

struct Window {
    started: Instant,
    count: u32,
}

#[derive(Default)]
pub struct ContractChecks {
    windows: Mutex<HashMap<String, Window>>,
}

/// A charged check, given back with [`ContractChecks::refund`].
pub(super) struct Charge(String);

impl ContractChecks {
    /// Charges one check to `source`; `None` when its budget is spent.
    pub(super) fn charge(&self, source: Option<&str>, now: Instant) -> Option<Charge> {
        let mut windows = self.windows.lock();
        let mut key = source.unwrap_or(SHARED_KEY);
        if !windows.contains_key(key) && windows.len() >= MAX_SOURCES {
            windows.retain(|_, w| now.saturating_duration_since(w.started) < WINDOW);
            if windows.len() >= MAX_SOURCES {
                key = SHARED_KEY;
            }
        }
        let max = if key == SHARED_KEY {
            SHARED
        } else {
            PER_SOURCE
        };
        let w = windows.entry(key.to_string()).or_insert(Window {
            started: now,
            count: 0,
        });
        if now.saturating_duration_since(w.started) >= WINDOW {
            *w = Window {
                started: now,
                count: 0,
            };
        }
        if w.count >= max {
            return None;
        }
        w.count += 1;
        Some(Charge(key.to_string()))
    }

    /// Gives back a check that verified.
    pub(super) fn refund(&self, charge: Charge) {
        if let Some(w) = self.windows.lock().get_mut(&charge.0) {
            w.count = w.count.saturating_sub(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_network_spends_its_budget_until_the_window_ends() {
        let checks = ContractChecks::default();
        let now = Instant::now();
        for _ in 0..PER_SOURCE {
            assert!(checks.charge(Some("a"), now).is_some());
        }
        assert!(checks.charge(Some("a"), now).is_none());
        assert!(
            checks.charge(Some("b"), now).is_some(),
            "another network has its own"
        );
        assert!(checks.charge(Some("a"), now + WINDOW).is_some());
    }

    #[test]
    fn a_check_that_verifies_is_given_back() {
        let checks = ContractChecks::default();
        let now = Instant::now();
        for _ in 0..PER_SOURCE * 3 {
            let charge = checks.charge(Some("a"), now).unwrap();
            checks.refund(charge);
        }
        assert!(checks.charge(Some("a"), now).is_some());
    }

    #[test]
    fn requests_without_a_network_share_one_budget() {
        let checks = ContractChecks::default();
        let now = Instant::now();
        for _ in 0..SHARED {
            assert!(checks.charge(None, now).is_some());
        }
        assert!(checks.charge(None, now).is_none());
    }

    #[test]
    fn networks_past_the_key_cap_fall_into_the_shared_budget() {
        let checks = ContractChecks::default();
        let now = Instant::now();
        for i in 0..MAX_SOURCES {
            assert!(checks.charge(Some(&i.to_string()), now).is_some());
        }
        for _ in 0..SHARED {
            assert!(checks.charge(Some("new"), now).is_some());
        }
        assert!(checks.charge(Some("newer"), now).is_none());
        assert_eq!(checks.windows.lock().len(), MAX_SOURCES + 1);
        assert!(
            checks.charge(Some("new"), now + WINDOW).is_some(),
            "ended windows are dropped first"
        );
        assert!(checks.windows.lock().len() <= 2);
    }
}
