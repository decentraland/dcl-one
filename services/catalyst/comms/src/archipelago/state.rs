use crate::archipelago::auth::ChallengeStore;
use crate::archipelago::ban::BanChecker;
use crate::archipelago::config::Settings;
use crate::archipelago::control_v4::AssignmentAuthority;
use crate::archipelago::enforcement::Enforcement;
use crate::archipelago::feed::{FeedCache, Outbox};
use crate::archipelago::registry::PeersRegistry;
use crate::deny_list::DenyList;
use std::sync::Arc;

crate::shared::send_sync_once! {
    pub struct AppStateInner {
        #[cfg(feature = "test-support")]
        pub ws_ping_gate: super::ws::assignment_test_support::PingGate,
        pub settings: Settings,
        pub sockets: Arc<tokio::sync::Semaphore>,
        /// Sockets that have not completed the handshake; released at the welcome.
        pub pending_handshakes: Arc<PendingHandshakes>,
        /// Pending handshakes per client source, so a few sources cannot hold the whole pending share.
        pub pending_handshakes_per_ip: Arc<IpSlots>,
        pub sockets_per_ip: Arc<IpSlots>,
        pub handlers: Arc<tokio::sync::Semaphore>,
        pub restart: tokio_util::sync::CancellationToken,
        pub enforcement: Arc<Enforcement>,
        pub statistics: Arc<crate::stats::Reader>,
        pub(crate) _statistics_subscription: crate::gatekeeper::bus::NatsSubscription,
        pub observations: super::observations::Observations,
        pub started_at: std::time::Instant,
        pub started: std::sync::atomic::AtomicBool,
        pub registry: Arc<PeersRegistry>,
        pub feed: Arc<FeedCache>,
        pub outbox: Outbox,
        /// The gatekeeper this connector runs beside: its NATS link (`bus`) carries the feed, and its
        /// LiveKit removes a kicked wallet's participant.
        pub gatekeeper: crate::gatekeeper::SharedState,
        pub challenges: Arc<ChallengeStore>,

        pub control_v4: Arc<AssignmentAuthority>,
        pub replica_id: String,

        pub ban_checker: Arc<BanChecker>,
        pub deny_list: Arc<DenyList>,
    }
}

pub type AppState = Arc<AppStateInner>;

impl AppStateInner {
    /// The serve listener has started and has not begun draining.
    pub fn listener_ready(&self) -> bool {
        self.started.load(std::sync::atomic::Ordering::Acquire) && !self.restart.is_cancelled()
    }
}

/// A socket's hold on the pending-handshake share, global and per source; released at the
/// welcome or when the socket ends.
pub(crate) struct PendingHandshake {
    pub(crate) global: PendingSlot,
    pub(crate) _source: Option<IpSlot>,
}

/// Sockets that have not completed the handshake, at most `max`. When the share is full a new
/// socket evicts the oldest one instead of being refused, so handshakes held open cannot refuse
/// new joins: a client that completes its handshake promptly is never the oldest for long.
pub struct PendingHandshakes {
    max: usize,
    inner: parking_lot::Mutex<PendingInner>,
}

#[derive(Default)]
struct PendingInner {
    next: u64,
    held: std::collections::BTreeMap<u64, tokio_util::sync::CancellationToken>,
}

/// One socket's place in the pending share; `evicted` fires when a newer socket takes it.
pub struct PendingSlot {
    set: Arc<PendingHandshakes>,
    id: u64,
    pub evicted: tokio_util::sync::CancellationToken,
}

impl PendingHandshakes {
    pub fn new(max: usize) -> Arc<Self> {
        Arc::new(Self {
            max: max.max(1),
            inner: Default::default(),
        })
    }

    /// Takes a place for a new socket, evicting the oldest pending socket when the share is full.
    pub fn admit(self: &Arc<Self>) -> PendingSlot {
        let mut inner = self.inner.lock();
        if inner.held.len() >= self.max
            && let Some((_, oldest)) = inner.held.pop_first()
        {
            oldest.cancel();
            crate::gatekeeper::admission::cluster::metrics::ws_pending_eviction();
            tracing::debug!("evicting the oldest pending /ws handshake: the pending share is full");
        }
        let id = inner.next;
        inner.next += 1;
        let evicted = tokio_util::sync::CancellationToken::new();
        inner.held.insert(id, evicted.clone());
        crate::gatekeeper::admission::cluster::metrics::ws_pending_handshakes(inner.held.len());
        PendingSlot {
            set: Arc::clone(self),
            id,
            evicted,
        }
    }

    /// Sockets currently holding a place.
    pub fn held(&self) -> usize {
        self.inner.lock().held.len()
    }
}

impl Drop for PendingSlot {
    fn drop(&mut self) {
        let mut inner = self.set.inner.lock();
        if inner.held.remove(&self.id).is_some() {
            crate::gatekeeper::admission::cluster::metrics::ws_pending_handshakes(inner.held.len());
        }
    }
}

/// Sockets held per client source (an IPv4 address or an IPv6 /64); a request
/// that cannot be attributed to one client is not counted.
pub struct IpSlots {
    max: usize,
    held: parking_lot::Mutex<std::collections::HashMap<String, usize>>,
}

/// One socket's share of its source; released on drop.
pub struct IpSlot {
    slots: Arc<IpSlots>,
    source: String,
}

impl IpSlots {
    pub fn new(max: u32) -> Arc<Self> {
        Arc::new(Self {
            max: max as usize,
            held: Default::default(),
        })
    }

    /// `Err` when the address already holds its limit; `Ok(None)` when it is not counted.
    #[allow(clippy::result_unit_err)]
    pub fn try_acquire(self: &Arc<Self>, client_ip: Option<&str>) -> Result<Option<IpSlot>, ()> {
        let Some(ip) = client_ip.filter(|_| self.max > 0) else {
            return Ok(None);
        };
        let source = crate::shared::client_ip::source_key(ip);
        let mut held = self.held.lock();
        let count = held.entry(source.clone()).or_default();
        if *count >= self.max {
            return Err(());
        }
        *count += 1;
        Ok(Some(IpSlot {
            slots: Arc::clone(self),
            source,
        }))
    }

    /// Whether this cap counts anything at all.
    pub fn enabled(&self) -> bool {
        self.max > 0
    }
}

impl Drop for IpSlot {
    fn drop(&mut self) {
        let mut held = self.slots.held.lock();
        if let Some(count) = held.get_mut(&self.source) {
            *count -= 1;
            if *count == 0 {
                held.remove(&self.source);
            }
        }
    }
}

#[cfg(test)]
mod pending_tests {
    use super::PendingHandshakes;

    #[test]
    fn a_full_share_evicts_its_oldest_socket_and_frees_places_on_drop() {
        let set = PendingHandshakes::new(2);
        let oldest = set.admit();
        let middle = set.admit();
        assert_eq!(set.held(), 2);
        let newest = set.admit();
        assert!(oldest.evicted.is_cancelled());
        assert!(!middle.evicted.is_cancelled());
        assert_eq!(set.held(), 2);
        // An evicted socket ending later does not free a place it no longer holds.
        drop(oldest);
        assert_eq!(set.held(), 2);
        drop(middle);
        assert_eq!(set.held(), 1);
        let _next = set.admit();
        assert!(!newest.evicted.is_cancelled());
    }
}

#[cfg(test)]
mod ip_slot_tests {
    use super::IpSlots;

    #[test]
    fn an_address_holds_at_most_its_limit_and_frees_on_drop() {
        let slots = IpSlots::new(2);
        let a = Some("203.0.113.7");
        let b = Some("203.0.113.8");
        let first = slots.try_acquire(a).unwrap();
        let _second = slots.try_acquire(a).unwrap();
        assert!(slots.try_acquire(a).is_err());
        assert!(slots.try_acquire(b).unwrap().is_some());
        assert!(slots.try_acquire(None).unwrap().is_none());
        drop(first);
        assert!(slots.try_acquire(a).unwrap().is_some());
        let unlimited = IpSlots::new(0);
        for _ in 0..4 {
            assert!(unlimited.try_acquire(a).unwrap().is_none());
        }
    }

    #[test]
    fn an_ipv6_client_is_counted_by_its_64() {
        let slots = IpSlots::new(1);
        let _held = slots.try_acquire(Some("2001:db8:1:2::1")).unwrap();
        assert!(slots.try_acquire(Some("2001:db8:1:2::ffff")).is_err());
        assert!(
            slots
                .try_acquire(Some("2001:db8:1:3::1"))
                .unwrap()
                .is_some()
        );
    }
}
