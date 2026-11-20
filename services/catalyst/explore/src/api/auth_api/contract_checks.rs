//! A budget of contract-wallet checks per network. A forged contract-wallet
//! chain stores nothing, so the pending caps never see it, while each of its
//! `isValidSignature` calls costs an RPC request: every call is charged to the
//! caller's network, a network that spent its budget is refused before the
//! RPC until the window ends, and the calls behind a request that is stored
//! are given back, so honest contract wallets never run it down. A replica
//! also caps the calls it makes in all. The budget is per process: each
//! replica keeps its own.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::auth::AuthError;
use crate::auth::eip1654::Eip1654Validator;

/// Calls one network may have charged per window.
pub(super) const PER_SOURCE: u32 = 20;
/// Calls charged by requests with no network (no socket peer, an untrusted
/// proxy), together.
const NO_SOURCE: u32 = 200;
/// Calls charged by networks past `MAX_SOURCES`, together.
const OVERFLOW: u32 = 200;
/// Calls this replica may have charged per window, every network together.
const GLOBAL: u32 = 1000;
const WINDOW: Duration = Duration::from_secs(60);
const MAX_SOURCES: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Key {
    Source(String),
    NoSource,
    Overflow,
}

impl Key {
    fn max(&self) -> u32 {
        match self {
            Key::Source(_) => PER_SOURCE,
            Key::NoSource => NO_SOURCE,
            Key::Overflow => OVERFLOW,
        }
    }
}

#[derive(Clone, Copy)]
struct Window {
    started: Instant,
    count: u32,
}

impl Window {
    fn new(now: Instant) -> Self {
        Self {
            started: now,
            count: 0,
        }
    }

    /// This window, or a new one once it ended by `now`.
    fn current(&mut self, now: Instant) -> &mut Self {
        if now.saturating_duration_since(self.started) >= WINDOW {
            *self = Self::new(now);
        }
        self
    }
}

struct Windows {
    by_key: HashMap<Key, Window>,
    global: Window,
}

pub struct ContractChecks {
    windows: Mutex<Windows>,
}

impl Default for ContractChecks {
    fn default() -> Self {
        Self {
            windows: Mutex::new(Windows {
                by_key: HashMap::new(),
                global: Window::new(Instant::now()),
            }),
        }
    }
}

/// A charged call, given back with [`ContractChecks::refund`] while its
/// windows last.
pub(super) struct Charge {
    key: Key,
    started: Instant,
    global_started: Instant,
}

impl ContractChecks {
    /// Charges one call to `source`; `None` when its budget or the replica's is spent.
    pub(super) fn charge(&self, source: Option<&str>, now: Instant) -> Option<Charge> {
        let mut windows = self.windows.lock();
        let global = *windows.global.current(now);
        if global.count >= GLOBAL {
            return None;
        }
        let mut key = source.map_or(Key::NoSource, |s| Key::Source(s.to_string()));
        if !windows.by_key.contains_key(&key) && windows.by_key.len() >= MAX_SOURCES {
            windows
                .by_key
                .retain(|_, w| now.saturating_duration_since(w.started) < WINDOW);
            if windows.by_key.len() >= MAX_SOURCES && matches!(key, Key::Source(_)) {
                key = Key::Overflow;
            }
        }
        let max = key.max();
        let w = windows
            .by_key
            .entry(key.clone())
            .or_insert(Window::new(now))
            .current(now);
        if w.count >= max {
            return None;
        }
        w.count += 1;
        let started = w.started;
        windows.global.count += 1;
        Some(Charge {
            key,
            started,
            global_started: global.started,
        })
    }

    /// Gives back a call, unless its window already ended.
    pub(super) fn refund(&self, charge: Charge) {
        let mut windows = self.windows.lock();
        if let Some(w) = windows.by_key.get_mut(&charge.key)
            && w.started == charge.started
        {
            w.count = w.count.saturating_sub(1);
        }
        if windows.global.started == charge.global_started {
            windows.global.count = windows.global.count.saturating_sub(1);
        }
    }
}

/// `inner` with every call charged to `source`'s budget.
pub(super) struct Budgeted<'a> {
    inner: &'a dyn Eip1654Validator,
    checks: &'a ContractChecks,
    source: Option<&'a str>,
    charges: Mutex<Vec<Charge>>,
    spent: AtomicBool,
}

impl<'a> Budgeted<'a> {
    pub(super) fn new(
        inner: &'a dyn Eip1654Validator,
        checks: &'a ContractChecks,
        source: Option<&'a str>,
    ) -> Self {
        Self {
            inner,
            checks,
            source,
            charges: Mutex::new(Vec::new()),
            spent: AtomicBool::new(false),
        }
    }

    /// Whether a call was refused because the budget was spent.
    pub(super) fn spent(&self) -> bool {
        self.spent.load(Ordering::Relaxed)
    }

    /// Gives back every call charged, once the request they verified is stored.
    pub(super) fn refund(self) {
        for charge in self.charges.into_inner() {
            self.checks.refund(charge);
        }
    }
}

#[async_trait::async_trait]
impl Eip1654Validator for Budgeted<'_> {
    async fn validate_signature(
        &self,
        contract_address: &str,
        hash: &[u8],
        signature: &[u8],
    ) -> Result<bool, AuthError> {
        let Some(charge) = self.checks.charge(self.source, Instant::now()) else {
            self.spent.store(true, Ordering::Relaxed);
            return Err(AuthError::Eip1654ValidationFailed(
                "contract check budget spent".into(),
            ));
        };
        self.charges.lock().push(charge);
        self.inner
            .validate_signature(contract_address, hash, signature)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spend(checks: &ContractChecks, source: Option<&str>, n: u32, now: Instant) {
        for _ in 0..n {
            assert!(checks.charge(source, now).is_some());
        }
    }

    #[test]
    fn a_network_spends_its_budget_until_the_window_ends() {
        let checks = ContractChecks::default();
        let now = Instant::now();
        spend(&checks, Some("a"), PER_SOURCE, now);
        assert!(checks.charge(Some("a"), now).is_none());
        assert!(
            checks.charge(Some("b"), now).is_some(),
            "another network has its own"
        );
        assert!(checks.charge(Some("a"), now + WINDOW).is_some());
    }

    #[test]
    fn a_call_given_back_frees_its_slot() {
        let checks = ContractChecks::default();
        let now = Instant::now();
        for _ in 0..PER_SOURCE * 3 {
            let charge = checks.charge(Some("a"), now).unwrap();
            checks.refund(charge);
        }
        assert!(checks.charge(Some("a"), now).is_some());
    }

    #[test]
    fn a_refund_never_reaches_a_later_window() {
        let checks = ContractChecks::default();
        let now = Instant::now();
        let stale = checks.charge(Some("a"), now).unwrap();
        let later = now + WINDOW;
        spend(&checks, Some("a"), PER_SOURCE, later);
        checks.refund(stale);
        assert!(checks.charge(Some("a"), later).is_none());
    }

    #[test]
    fn requests_without_a_network_have_their_own_budget() {
        let checks = ContractChecks::default();
        let now = Instant::now();
        spend(&checks, None, NO_SOURCE, now);
        assert!(checks.charge(None, now).is_none());
        assert!(checks.charge(Some("a"), now).is_some());
    }

    #[test]
    fn networks_past_the_key_cap_share_the_overflow_budget_only() {
        let checks = ContractChecks::default();
        let now = Instant::now();
        for i in 0..MAX_SOURCES {
            // Given back, so the replica's cap stays clear.
            let key = i.to_string();
            let charge = checks.charge(Some(&key), now).unwrap();
            checks.refund(charge);
        }
        spend(&checks, Some("new"), OVERFLOW, now);
        assert!(checks.charge(Some("newer"), now).is_none());
        assert!(
            checks.charge(None, now).is_some(),
            "requests without a network keep theirs"
        );
        assert!(checks.windows.lock().by_key.len() <= MAX_SOURCES + 2);
        assert!(
            checks.charge(Some("new"), now + WINDOW).is_some(),
            "ended windows are dropped first"
        );
        assert!(checks.windows.lock().by_key.len() <= 2);
    }

    #[tokio::test]
    async fn calls_are_given_back_only_when_the_request_is_stored() {
        let checks = ContractChecks::default();
        let source = Some("a");
        async fn ask(budget: Budgeted<'_>) -> Budgeted<'_> {
            for _ in 0..PER_SOURCE / 2 {
                assert!(budget.validate_signature("0x", b"h", b"s").await.unwrap());
            }
            budget
        }
        ask(Budgeted::new(&crate::auth::PeerValidated, &checks, source))
            .await
            .refund();
        let dropped = ask(Budgeted::new(&crate::auth::PeerValidated, &checks, source)).await;
        drop(dropped);
        let spent = ask(Budgeted::new(&crate::auth::PeerValidated, &checks, source)).await;
        assert!(!spent.spent());
        assert!(
            spent.validate_signature("0x", b"h", b"s").await.is_err(),
            "the stored request's calls came back, the refused one's did not"
        );
        assert!(spent.spent());
    }

    #[test]
    fn a_replica_caps_every_network_together() {
        let checks = ContractChecks::default();
        let now = Instant::now();
        for i in 0..GLOBAL / PER_SOURCE {
            spend(&checks, Some(&i.to_string()), PER_SOURCE, now);
        }
        assert!(checks.charge(Some("fresh"), now).is_none());
        assert!(checks.charge(Some("fresh"), now + WINDOW).is_some());
    }
}
