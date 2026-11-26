//! A budget of contract-wallet checks per network. A forged contract-wallet
//! chain stores nothing, so the pending caps never see it, while each of its
//! `isValidSignature` calls costs an RPC request: every call is charged to the
//! caller's network, a network that spent its budget is refused before the
//! RPC until the window ends, and the calls behind a request that is stored
//! are given back, so honest contract wallets never run it down. A replica
//! also caps the calls it makes in all; forged calls are never given back, so
//! enough networks together can spend it and refuse every contract wallet on
//! the replica until the window ends (logged once per window). An IPv6 caller
//! is keyed by its /48, so one allocation is one network. The budget is per
//! process: each replica keeps its own.

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
/// The prefix an IPv6 source key is widened to.
const IPV6_NETWORK_BITS: u8 = 48;
/// The error a [`Budgeted`] call refused for a spent budget carries.
pub(super) const BUDGET_SPENT: &str = "contract check budget spent";

/// Whose budget refused a call: the caller's network, or the replica's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Spent {
    Network,
    Replica,
}

/// `source` as a budget key: an IPv6 key (a /64) widened to its /48.
pub(super) fn network_key(source: &str) -> String {
    match source.parse::<ipnet::Ipv6Net>() {
        Ok(net) if net.prefix_len() > IPV6_NETWORK_BITS => {
            ipnet::Ipv6Net::new(net.network(), IPV6_NETWORK_BITS)
                .map_or_else(|_| source.to_string(), |n| n.trunc().to_string())
        }
        _ => source.to_string(),
    }
}

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
    /// Whether the replica cap was logged in this global window.
    global_tripped: bool,
    /// While the table is full, no tracked window can end before this.
    next_prune: Option<Instant>,
    /// Scans of the full table.
    #[cfg(test)]
    prunes: u64,
}

impl Windows {
    /// The budget `source` is charged to: its network's, or, while the table
    /// is full once the ended windows are dropped, the overflow one.
    fn key_for(&mut self, source: Option<&str>, now: Instant) -> Key {
        let key = source.map_or(Key::NoSource, |s| Key::Source(network_key(s)));
        if !self.by_key.contains_key(&key)
            && self.by_key.len() >= MAX_SOURCES
            && self.next_prune.is_none_or(|at| now >= at)
        {
            self.by_key
                .retain(|_, w| now.saturating_duration_since(w.started) < WINDOW);
            #[cfg(test)]
            {
                self.prunes += 1;
            }
            self.next_prune = self.by_key.values().map(|w| w.started + WINDOW).min();
        }
        if !self.by_key.contains_key(&key)
            && self.by_key.len() >= MAX_SOURCES
            && matches!(key, Key::Source(_))
        {
            return Key::Overflow;
        }
        key
    }
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
                global_tripped: false,
                next_prune: None,
                #[cfg(test)]
                prunes: 0,
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
    /// Whether `source` could be charged a call now, without charging it: a
    /// spent budget is refused before any database count.
    pub(super) fn has_room(&self, source: Option<&str>, now: Instant) -> Result<(), Spent> {
        let mut windows = self.windows.lock();
        let live = |w: &Window| now.saturating_duration_since(w.started) < WINDOW;
        if live(&windows.global) && windows.global.count >= GLOBAL {
            return Err(Spent::Replica);
        }
        let key = windows.key_for(source, now);
        match windows.by_key.get(&key) {
            Some(w) if live(w) && w.count >= key.max() => Err(Spent::Network),
            _ => Ok(()),
        }
    }

    /// Charges one call to `source`; refused when its budget or the replica's is spent.
    pub(super) fn charge(&self, source: Option<&str>, now: Instant) -> Result<Charge, Spent> {
        let mut windows = self.windows.lock();
        let before = windows.global.started;
        let global = *windows.global.current(now);
        if global.started != before {
            windows.global_tripped = false;
        }
        if global.count >= GLOBAL {
            if !windows.global_tripped {
                windows.global_tripped = true;
                tracing::warn!(
                    "auth-api: the replica's contract-wallet check budget is spent; contract wallets get 429 until the window ends"
                );
            }
            return Err(Spent::Replica);
        }
        let key = windows.key_for(source, now);
        let max = key.max();
        let w = windows
            .by_key
            .entry(key.clone())
            .or_insert(Window::new(now))
            .current(now);
        if w.count >= max {
            return Err(Spent::Network);
        }
        w.count += 1;
        let started = w.started;
        windows.global.count += 1;
        Ok(Charge {
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
    spent: Mutex<Option<Spent>>,
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
            spent: Mutex::new(None),
        }
    }

    /// Whose spent budget refused a call, if one was.
    pub(super) fn spent(&self) -> Option<Spent> {
        *self.spent.lock()
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
        let charge = match self.checks.charge(self.source, Instant::now()) {
            Ok(charge) => charge,
            Err(spent) => {
                *self.spent.lock() = Some(spent);
                return Err(AuthError::Eip1654ValidationFailed(BUDGET_SPENT.into()));
            }
        };
        self.charges.lock().push(charge);
        self.inner
            .validate_signature(contract_address, hash, signature)
            .await
    }
}

/// Takes every contract-wallet link as signed, noting that one was asked: a
/// chain's checks that need no RPC, before any budgeted call.
#[derive(Default)]
pub(super) struct Offline(AtomicBool);

impl Offline {
    /// Whether the chain had a contract-wallet link.
    pub(super) fn asked(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[async_trait::async_trait]
impl Eip1654Validator for Offline {
    async fn validate_signature(&self, _: &str, _: &[u8], _: &[u8]) -> Result<bool, AuthError> {
        self.0.store(true, Ordering::Relaxed);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spend(checks: &ContractChecks, source: Option<&str>, n: u32, now: Instant) {
        for _ in 0..n {
            assert!(checks.charge(source, now).is_ok());
        }
    }

    #[test]
    fn a_network_spends_its_budget_until_the_window_ends() {
        let checks = ContractChecks::default();
        let now = Instant::now();
        spend(&checks, Some("a"), PER_SOURCE, now);
        assert!(checks.charge(Some("a"), now).is_err());
        assert!(
            checks.charge(Some("b"), now).is_ok(),
            "another network has its own"
        );
        assert!(checks.charge(Some("a"), now + WINDOW).is_ok());
    }

    #[test]
    fn an_ipv6_allocation_is_one_network() {
        let checks = ContractChecks::default();
        let now = Instant::now();
        spend(&checks, Some("2001:db8:1:2::/64"), PER_SOURCE, now);
        assert!(matches!(
            checks.charge(Some("2001:db8:1:3::/64"), now),
            Err(Spent::Network)
        ));
        assert!(checks.charge(Some("2001:db8:2::/64"), now).is_ok());
        assert_eq!(network_key("203.0.113.5"), "203.0.113.5");
    }

    #[test]
    fn a_full_table_is_scanned_again_only_once_a_window_can_have_ended() {
        let checks = ContractChecks::default();
        let now = Instant::now();
        for i in 0..MAX_SOURCES {
            let charge = checks.charge(Some(&i.to_string()), now).unwrap();
            checks.refund(charge);
        }
        for i in 0..50 {
            let later = now + Duration::from_secs(i);
            assert!(checks.charge(Some(&format!("new-{i}")), later).is_ok());
        }
        assert_eq!(checks.windows.lock().prunes, 1);
        assert!(checks.charge(Some("newest"), now + WINDOW).is_ok());
        let windows = checks.windows.lock();
        assert_eq!(windows.prunes, 2);
        assert!(windows.by_key.len() <= 2, "the ended windows were dropped");
    }

    #[test]
    fn room_is_checked_without_charging() {
        let checks = ContractChecks::default();
        let now = Instant::now();
        for _ in 0..PER_SOURCE {
            assert!(checks.has_room(Some("a"), now).is_ok());
            assert!(checks.charge(Some("a"), now).is_ok());
        }
        assert!(matches!(
            checks.has_room(Some("a"), now),
            Err(Spent::Network)
        ));
        assert!(checks.has_room(Some("b"), now).is_ok());
        assert!(checks.has_room(Some("a"), now + WINDOW).is_ok());
        assert_eq!(checks.windows.lock().global.count, PER_SOURCE);
    }

    #[test]
    fn room_follows_the_replica_cap() {
        let checks = ContractChecks::default();
        let now = Instant::now();
        for i in 0..GLOBAL / PER_SOURCE {
            spend(&checks, Some(&i.to_string()), PER_SOURCE, now);
        }
        assert!(matches!(
            checks.has_room(Some("fresh"), now),
            Err(Spent::Replica)
        ));
        assert!(checks.has_room(Some("fresh"), now + WINDOW).is_ok());
    }

    #[test]
    fn room_files_a_network_where_a_charge_would() {
        let checks = ContractChecks::default();
        let t0 = Instant::now();
        for i in 0..MAX_SOURCES {
            let charge = checks.charge(Some(&i.to_string()), t0).unwrap();
            checks.refund(charge);
        }
        let t1 = t0 + Duration::from_secs(30);
        spend(&checks, Some("new"), OVERFLOW, t1);
        assert!(
            matches!(checks.has_room(Some("newer"), t1), Err(Spent::Network)),
            "a full table files a new network under the spent overflow budget"
        );
        let t2 = t0 + WINDOW;
        assert!(
            checks.has_room(Some("fresh"), t2).is_ok(),
            "once the tracked windows ended, a new network has its own"
        );
        assert!(checks.charge(Some("fresh"), t2).is_ok());
    }

    #[test]
    fn a_call_given_back_frees_its_slot() {
        let checks = ContractChecks::default();
        let now = Instant::now();
        for _ in 0..PER_SOURCE * 3 {
            let charge = checks.charge(Some("a"), now).unwrap();
            checks.refund(charge);
        }
        assert!(checks.charge(Some("a"), now).is_ok());
    }

    #[test]
    fn a_refund_never_reaches_a_later_window() {
        let checks = ContractChecks::default();
        let now = Instant::now();
        let stale = checks.charge(Some("a"), now).unwrap();
        let later = now + WINDOW;
        spend(&checks, Some("a"), PER_SOURCE, later);
        checks.refund(stale);
        assert!(checks.charge(Some("a"), later).is_err());
    }

    #[test]
    fn requests_without_a_network_have_their_own_budget() {
        let checks = ContractChecks::default();
        let now = Instant::now();
        spend(&checks, None, NO_SOURCE, now);
        assert!(checks.charge(None, now).is_err());
        assert!(checks.charge(Some("a"), now).is_ok());
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
        assert!(checks.charge(Some("newer"), now).is_err());
        assert!(
            checks.charge(None, now).is_ok(),
            "requests without a network keep theirs"
        );
        assert!(checks.windows.lock().by_key.len() <= MAX_SOURCES + 2);
        assert!(
            checks.charge(Some("new"), now + WINDOW).is_ok(),
            "ended windows are dropped first"
        );
        assert!(checks.windows.lock().by_key.len() <= 2);
    }

    #[tokio::test]
    async fn calls_are_given_back_only_when_the_request_is_stored() {
        let checks = ContractChecks::default();
        let source = Some("a");
        let accept = Offline::default();
        async fn ask(budget: Budgeted<'_>) -> Budgeted<'_> {
            for _ in 0..PER_SOURCE / 2 {
                assert!(budget.validate_signature("0x", b"h", b"s").await.unwrap());
            }
            budget
        }
        ask(Budgeted::new(&accept, &checks, source)).await.refund();
        let dropped = ask(Budgeted::new(&accept, &checks, source)).await;
        drop(dropped);
        let spent = ask(Budgeted::new(&accept, &checks, source)).await;
        assert_eq!(spent.spent(), None);
        assert!(
            spent.validate_signature("0x", b"h", b"s").await.is_err(),
            "the stored request's calls came back, the refused one's did not"
        );
        assert_eq!(spent.spent(), Some(Spent::Network));
    }

    #[test]
    fn a_replica_caps_every_network_together() {
        let checks = ContractChecks::default();
        let now = Instant::now();
        for i in 0..GLOBAL / PER_SOURCE {
            spend(&checks, Some(&i.to_string()), PER_SOURCE, now);
        }
        assert!(matches!(
            checks.charge(Some("fresh"), now),
            Err(Spent::Replica)
        ));
        assert!(checks.charge(Some("fresh"), now + WINDOW).is_ok());
    }
}
