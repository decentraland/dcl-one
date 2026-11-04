//! The abuse throttle of the friendship/block RPC mutations (upstream
//! social-service-ea #456): each step publishes an update into the other
//! account's subscription stream, so an unbounded REQUEST/CANCEL or
//! block/unblock loop is a push-spam primitive. Budgets: 30 actions per actor
//! and 10 per pair per minute, over the shared token bucket
//! (`crate::comms::limits::RateLimiter`). In-process only, like the rest of
//! this module's state (see `pubsub`).
//!
//! [`Admission`] holds the per-address socket limits the RPC socket and
//! `/v1/stream` share (upstream #503).

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::shared::rpc::ws::SocketClose;

pub use crate::comms::limits::{RateLimitDecision, RateLimiter};

const FRIENDSHIP_RATE_LIMIT_PER_ACTOR: u32 = 30;
const FRIENDSHIP_RATE_LIMIT_PER_PAIR: u32 = 10;
const FRIENDSHIP_RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);

/// A friendship is symmetric while a block is not: a canonical (sorted) key
/// for block/unblock would let the account being blocked spend the budget the
/// victim needs, deciding whether the block lands (upstream #456).
#[derive(Debug, Clone, Copy)]
pub enum PairScope {
    Symmetric,
    Directional,
}

/// The two pair schemes keep distinct key prefixes so they can never merge.
pub struct FriendshipMutationLimiter {
    per_actor: RateLimiter,
    per_pair: RateLimiter,
}

impl Default for FriendshipMutationLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl FriendshipMutationLimiter {
    pub fn new() -> Self {
        Self {
            per_actor: RateLimiter::new(
                FRIENDSHIP_RATE_LIMIT_PER_ACTOR,
                FRIENDSHIP_RATE_LIMIT_WINDOW,
            ),
            per_pair: RateLimiter::new(
                FRIENDSHIP_RATE_LIMIT_PER_PAIR,
                FRIENDSHIP_RATE_LIMIT_WINDOW,
            ),
        }
    }

    pub fn allow(&self, actor: &str, target: &str, scope: PairScope) -> bool {
        if self.per_actor.check(&format!("actor:{actor}")) == RateLimitDecision::Deny {
            return false;
        }
        let pair_key = match scope {
            PairScope::Symmetric => {
                let (a, b) = if actor <= target {
                    (actor, target)
                } else {
                    (target, actor)
                };
                format!("pair:sym:{a}:{b}")
            }
            PairScope::Directional => format!("pair:dir:{actor}:{target}"),
        };
        self.per_pair.check(&pair_key) == RateLimitDecision::Allow
    }
}

/// Which socket: the RPC socket or `/v1/stream` (upstream's `WsProtocol`,
/// the `protocol` label of the socket metrics).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    Rpc,
    Stream,
}

impl Protocol {
    pub fn label(self) -> &'static str {
        match self {
            Protocol::Rpc => "rpc",
            Protocol::Stream => "stream",
        }
    }
}

/// The close a socket displaced by a newer one of its address gets.
pub const REPLACED: (u16, &str) = (3005, "Replaced by a newer connection");

/// The per-address limits both protocols share (upstream
/// `src/logic/ws-admission/component.ts:19-22`, each a positive number).
#[derive(Clone, Copy, Debug)]
pub struct AdmissionLimits {
    /// `WS_MAX_CONNECTIONS_PER_ADDRESS`: authenticated sockets per address.
    pub max_per_address: usize,
    /// `WS_SUBSCRIPTIONS_PER_SECOND`: the subscription bucket's refill.
    pub subscriptions_per_second: f64,
    /// `WS_SUBSCRIPTION_BURST`: the bucket's size.
    pub subscription_burst: usize,
    /// `WS_ADDRESS_BUDGET_CAPACITY`: disconnected addresses whose partly
    /// spent bucket is remembered; forgetting one grants a full bucket.
    pub idle_budget_capacity: usize,
}

/// Upstream's defaults: 8 sockets, 2 subscriptions a second, a burst of 24,
/// 10,000 idle buckets.
pub const DEFAULT_MAX_SOCKETS_PER_ADDRESS: usize = 8;

impl Default for AdmissionLimits {
    fn default() -> Self {
        Self {
            max_per_address: DEFAULT_MAX_SOCKETS_PER_ADDRESS,
            subscriptions_per_second: 2.0,
            subscription_burst: 24,
            idle_budget_capacity: 10_000,
        }
    }
}

#[derive(Clone, Copy)]
struct Budget {
    tokens: f64,
    updated_at: Instant,
}

impl Budget {
    fn full(limits: &AdmissionLimits, now: Instant) -> Self {
        Self {
            tokens: limits.subscription_burst as f64,
            updated_at: now,
        }
    }

    fn replenish(&mut self, limits: &AdmissionLimits, now: Instant) {
        let elapsed = now.saturating_duration_since(self.updated_at).as_secs_f64();
        self.tokens = (self.tokens + elapsed * limits.subscriptions_per_second)
            .min(limits.subscription_burst as f64);
        self.updated_at = now;
    }
}

struct AddressSockets {
    /// Oldest first.
    sockets: VecDeque<(Arc<SocketClose>, Protocol)>,
    budget: Budget,
}

/// The per-address WebSocket limits both protocols share (upstream
/// `ws-admission`, #503): a socket cap whose overflow closes the address's
/// OLDEST socket (3005) instead of refusing the new one, so a half-open
/// socket or a replayed chain cannot lock the owner out; and the
/// subscription-start bucket of `/v1/stream`, which survives reconnects.
/// In-process, like the rest of this module's state.
#[derive(Default)]
pub struct Admission {
    inner: Mutex<AdmissionInner>,
}

#[derive(Default)]
struct AdmissionInner {
    limits: AdmissionLimits,
    connected: HashMap<String, AddressSockets>,
    idle: HashMap<String, Budget>,
}

impl Admission {
    /// The process's limits (`Settings::admission`), set before any socket opens.
    pub fn configure(&self, limits: AdmissionLimits) {
        self.inner.lock().limits = limits;
    }

    /// Admits an authenticated socket of `address`; past the cap the oldest
    /// one is ended with [`REPLACED`]. Answers whether one was displaced.
    pub fn claim(&self, address: &str, socket: Arc<SocketClose>, protocol: Protocol) -> bool {
        let address = address.to_lowercase();
        let now = Instant::now();
        let mut inner = self.inner.lock();
        let limits = inner.limits;
        let budget = inner.idle.remove(&address);
        let state = inner
            .connected
            .entry(address)
            .or_insert_with(|| AddressSockets {
                sockets: VecDeque::new(),
                budget: budget.unwrap_or_else(|| Budget::full(&limits, now)),
            });
        let mut displaced = false;
        while state.sockets.len() >= limits.max_per_address {
            if let Some((oldest, protocol)) = state.sockets.pop_front() {
                let (code, reason) = REPLACED;
                oldest.end(code, reason);
                metrics::counter!("ws_admission_evictions", "protocol" => protocol.label())
                    .increment(1);
                displaced = true;
            }
        }
        state.sockets.push_back((socket, protocol));
        displaced
    }

    /// One subscription start from `address`'s bucket; false when spent.
    pub fn admit_subscription(&self, address: &str) -> bool {
        let now = Instant::now();
        let mut inner = self.inner.lock();
        let limits = inner.limits;
        let Some(state) = inner.connected.get_mut(&address.to_lowercase()) else {
            return false;
        };
        state.budget.replenish(&limits, now);
        if state.budget.tokens < 1.0 {
            metrics::counter!(
                "ws_admission_rejections",
                "protocol" => Protocol::Stream.label(),
                "reason" => "subscriptions"
            )
            .increment(1);
            return false;
        }
        state.budget.tokens -= 1.0;
        true
    }

    /// Whether the socket still holds its slot (a displaced one does not).
    pub fn holds(&self, address: &str, handle: &Arc<SocketClose>) -> bool {
        self.inner
            .lock()
            .connected
            .get(&address.to_lowercase())
            .is_some_and(|state| state.sockets.iter().any(|(s, _)| Arc::ptr_eq(s, handle)))
    }

    /// Frees the socket's slot; the address's last socket leaves its partly
    /// spent bucket behind, the least recently used of those forgotten first.
    pub fn release(&self, address: &str, handle: &Arc<SocketClose>) {
        let address = address.to_lowercase();
        let now = Instant::now();
        let mut inner = self.inner.lock();
        let limits = inner.limits;
        let Some(state) = inner.connected.get_mut(&address) else {
            return;
        };
        state.sockets.retain(|(s, _)| !Arc::ptr_eq(s, handle));
        if !state.sockets.is_empty() {
            return;
        }
        let mut budget = inner.connected.remove(&address).map(|s| s.budget).unwrap();
        budget.replenish(&limits, now);
        if budget.tokens >= limits.subscription_burst as f64 {
            return;
        }
        if inner.idle.len() >= limits.idle_budget_capacity
            && let Some(oldest) = inner
                .idle
                .iter()
                .min_by_key(|(_, b)| b.updated_at)
                .map(|(a, _)| a.clone())
        {
            inner.idle.remove(&oldest);
        }
        inner.idle.insert(address, budget);
    }

    #[cfg(test)]
    pub fn sockets_of(&self, address: &str) -> usize {
        self.inner
            .lock()
            .connected
            .get(&address.to_lowercase())
            .map_or(0, |s| s.sockets.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ninth_socket_displaces_the_oldest() {
        let admission = Admission::default();
        let handles: Vec<Arc<SocketClose>> = (0..=DEFAULT_MAX_SOCKETS_PER_ADDRESS)
            .map(|_| Arc::new(SocketClose::default()))
            .collect();
        for (i, h) in handles.iter().enumerate() {
            let protocol = if i % 2 == 0 {
                Protocol::Stream
            } else {
                Protocol::Rpc
            };
            assert_eq!(
                admission.claim("0xAbC", h.clone(), protocol),
                i == DEFAULT_MAX_SOCKETS_PER_ADDRESS,
                "socket {i}"
            );
        }
        assert_eq!(
            admission.sockets_of("0xabc"),
            DEFAULT_MAX_SOCKETS_PER_ADDRESS
        );
        assert!(!admission.holds("0xabc", &handles[0]));
        assert!(admission.holds("0xabc", &handles[1]));
        assert_eq!(
            handles[0].frame(),
            Some(REPLACED),
            "the oldest was told to end"
        );
        assert!(!handles[1].is_ended());
        admission.release("0xabc", &handles[0]);
        assert_eq!(
            admission.sockets_of("0xabc"),
            DEFAULT_MAX_SOCKETS_PER_ADDRESS
        );
        for h in &handles[1..] {
            admission.release("0xabc", h);
        }
        assert_eq!(admission.sockets_of("0xabc"), 0);
    }

    /// The limits are configuration (`WS_MAX_CONNECTIONS_PER_ADDRESS`,
    /// `WS_SUBSCRIPTION_BURST`), and a displacement and a refused
    /// subscription are counted as upstream counts them.
    #[test]
    fn configured_limits_apply_and_are_counted() {
        let admission = Admission::default();
        admission.configure(AdmissionLimits {
            max_per_address: 1,
            subscription_burst: 2,
            ..AdmissionLimits::default()
        });
        let recorder = crate::comms::metrics::prometheus_builder()
            .unwrap()
            .build_recorder();
        let handle = recorder.handle();
        let first = Arc::new(SocketClose::default());
        metrics::with_local_recorder(&recorder, || {
            assert!(!admission.claim("0xa", first.clone(), Protocol::Rpc));
            assert!(admission.claim("0xa", Arc::new(SocketClose::default()), Protocol::Stream));
            assert!(admission.admit_subscription("0xa"));
            assert!(admission.admit_subscription("0xa"));
            assert!(!admission.admit_subscription("0xa"));
        });
        assert_eq!(first.frame(), Some(REPLACED));
        let rendered = handle.render();
        assert!(
            rendered.contains("ws_admission_evictions{protocol=\"rpc\"} 1"),
            "{rendered}"
        );
        assert!(
            rendered.contains(
                "ws_admission_rejections{protocol=\"stream\",reason=\"subscriptions\"} 1"
            ),
            "{rendered}"
        );
    }

    #[test]
    fn the_subscription_bucket_survives_a_reconnect() {
        let admission = Admission::default();
        let first = Arc::new(SocketClose::default());
        assert!(!admission.admit_subscription("0xa"), "no socket, no budget");
        admission.claim("0xa", first.clone(), Protocol::Stream);
        let spent = (0..100)
            .take_while(|_| admission.admit_subscription("0xa"))
            .count();
        assert_eq!(spent, AdmissionLimits::default().subscription_burst);
        admission.release("0xa", &first);
        let second = Arc::new(SocketClose::default());
        admission.claim("0xa", second.clone(), Protocol::Stream);
        assert!(
            !admission.admit_subscription("0xa"),
            "reconnecting does not refill the bucket"
        );
        let other = Arc::new(SocketClose::default());
        admission.claim("0xb", other, Protocol::Stream);
        assert!(
            admission.admit_subscription("0xb"),
            "buckets are per address"
        );
    }

    #[test]
    fn actor_budget_spans_targets() {
        let l = FriendshipMutationLimiter::new();
        for i in 0..FRIENDSHIP_RATE_LIMIT_PER_ACTOR {
            assert!(l.allow("0xa", &format!("0xt{i}"), PairScope::Symmetric));
        }
        assert!(!l.allow("0xa", "0xfresh", PairScope::Symmetric));
    }

    #[test]
    fn pair_budget_is_symmetric_for_friendship_actions() {
        let l = FriendshipMutationLimiter::new();
        for i in 0..FRIENDSHIP_RATE_LIMIT_PER_PAIR / 2 {
            assert!(l.allow("0xa", "0xb", PairScope::Symmetric), "a->b {i}");
            assert!(l.allow("0xb", "0xa", PairScope::Symmetric), "b->a {i}");
        }
        assert!(!l.allow("0xa", "0xb", PairScope::Symmetric));
        assert!(!l.allow("0xb", "0xa", PairScope::Symmetric));
    }

    #[test]
    fn block_budget_is_directional_so_the_counterparty_cannot_spend_it() {
        let l = FriendshipMutationLimiter::new();
        for _ in 0..FRIENDSHIP_RATE_LIMIT_PER_PAIR {
            assert!(l.allow("0xspammer", "0xvictim", PairScope::Directional));
        }
        assert!(!l.allow("0xspammer", "0xvictim", PairScope::Directional));
        assert!(l.allow("0xvictim", "0xspammer", PairScope::Directional));
    }

    #[test]
    fn symmetric_and_directional_buckets_never_merge() {
        let l = FriendshipMutationLimiter::new();
        for _ in 0..FRIENDSHIP_RATE_LIMIT_PER_PAIR {
            assert!(l.allow("0xa", "0xb", PairScope::Symmetric));
        }
        assert!(!l.allow("0xa", "0xb", PairScope::Symmetric));
        assert!(l.allow("0xa", "0xb", PairScope::Directional));
    }
}
