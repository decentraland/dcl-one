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
use tokio::sync::Notify;

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

/// Authenticated sockets an address may hold across the RPC socket and
/// `/v1/stream` (upstream `WS_MAX_CONNECTIONS_PER_ADDRESS`, default 8,
/// `src/logic/ws-admission/component.ts:19`).
pub const MAX_SOCKETS_PER_ADDRESS: usize = 8;
/// The per-address subscription-start bucket of `/v1/stream`
/// (`WS_SUBSCRIPTIONS_PER_SECOND` 2, `WS_SUBSCRIPTION_BURST` 24).
const SUBSCRIPTIONS_PER_SECOND: f64 = 2.0;
const SUBSCRIPTION_BURST: f64 = 24.0;
/// Disconnected addresses whose partly spent bucket is remembered
/// (`WS_ADDRESS_BUDGET_CAPACITY` 10,000); forgetting one grants a full bucket.
const IDLE_BUDGET_CAPACITY: usize = 10_000;

/// How a socket displaced by a newer one of its address is ended: an RPC
/// transport through its kill handle, a stream socket by its own loop, which
/// closes it `3005`.
pub enum Eviction {
    Rpc(Arc<Notify>),
    Stream(Arc<Notify>),
}

impl Eviction {
    fn fire(&self) {
        match self {
            Eviction::Rpc(kill) => kill.notify_waiters(),
            // A permit, so a loop between two selects still sees it.
            Eviction::Stream(evicted) => evicted.notify_one(),
        }
    }

    fn is(&self, handle: &Arc<Notify>) -> bool {
        match self {
            Eviction::Rpc(h) | Eviction::Stream(h) => Arc::ptr_eq(h, handle),
        }
    }
}

#[derive(Clone, Copy)]
struct Budget {
    tokens: f64,
    updated_at: Instant,
}

impl Budget {
    fn full(now: Instant) -> Self {
        Self {
            tokens: SUBSCRIPTION_BURST,
            updated_at: now,
        }
    }

    fn replenish(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.updated_at).as_secs_f64();
        self.tokens = (self.tokens + elapsed * SUBSCRIPTIONS_PER_SECOND).min(SUBSCRIPTION_BURST);
        self.updated_at = now;
    }
}

struct AddressSockets {
    /// Oldest first.
    sockets: VecDeque<Eviction>,
    budget: Budget,
}

/// The per-address WebSocket limits both protocols share (upstream
/// `ws-admission`, #503): a socket cap whose overflow closes the address's
/// OLDEST socket instead of refusing the new one, so a half-open socket or a
/// replayed chain cannot lock the owner out; and the subscription-start
/// bucket of `/v1/stream`, which survives reconnects. In-process, like the
/// rest of this module's state.
#[derive(Default)]
pub struct Admission {
    inner: Mutex<AdmissionInner>,
}

#[derive(Default)]
struct AdmissionInner {
    connected: HashMap<String, AddressSockets>,
    idle: HashMap<String, Budget>,
}

impl Admission {
    /// Admits an authenticated socket of `address`; past the cap the oldest
    /// one is told to end. Answers whether one was displaced.
    pub fn claim(&self, address: &str, socket: Eviction) -> bool {
        let address = address.to_lowercase();
        let now = Instant::now();
        let mut inner = self.inner.lock();
        let budget = inner.idle.remove(&address);
        let state = inner
            .connected
            .entry(address)
            .or_insert_with(|| AddressSockets {
                sockets: VecDeque::new(),
                budget: budget.unwrap_or_else(|| Budget::full(now)),
            });
        let mut displaced = false;
        while state.sockets.len() >= MAX_SOCKETS_PER_ADDRESS {
            if let Some(oldest) = state.sockets.pop_front() {
                oldest.fire();
                displaced = true;
            }
        }
        state.sockets.push_back(socket);
        displaced
    }

    /// One subscription start from `address`'s bucket; false when spent.
    pub fn admit_subscription(&self, address: &str) -> bool {
        let now = Instant::now();
        let mut inner = self.inner.lock();
        let Some(state) = inner.connected.get_mut(&address.to_lowercase()) else {
            return false;
        };
        state.budget.replenish(now);
        if state.budget.tokens < 1.0 {
            return false;
        }
        state.budget.tokens -= 1.0;
        true
    }

    /// Whether the socket still holds its slot (a displaced one does not).
    pub fn holds(&self, address: &str, handle: &Arc<Notify>) -> bool {
        self.inner
            .lock()
            .connected
            .get(&address.to_lowercase())
            .is_some_and(|state| state.sockets.iter().any(|s| s.is(handle)))
    }

    /// Frees the socket's slot; the address's last socket leaves its partly
    /// spent bucket behind, the least recently used of those forgotten first.
    pub fn release(&self, address: &str, handle: &Arc<Notify>) {
        let address = address.to_lowercase();
        let now = Instant::now();
        let mut inner = self.inner.lock();
        let Some(state) = inner.connected.get_mut(&address) else {
            return;
        };
        state.sockets.retain(|s| !s.is(handle));
        if !state.sockets.is_empty() {
            return;
        }
        let mut budget = inner.connected.remove(&address).map(|s| s.budget).unwrap();
        budget.replenish(now);
        if budget.tokens >= SUBSCRIPTION_BURST {
            return;
        }
        if inner.idle.len() >= IDLE_BUDGET_CAPACITY
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
        let handles: Vec<Arc<Notify>> = (0..=MAX_SOCKETS_PER_ADDRESS)
            .map(|_| Arc::new(Notify::new()))
            .collect();
        for (i, h) in handles.iter().enumerate() {
            let socket = if i % 2 == 0 {
                Eviction::Stream(h.clone())
            } else {
                Eviction::Rpc(h.clone())
            };
            assert_eq!(
                admission.claim("0xAbC", socket),
                i == MAX_SOCKETS_PER_ADDRESS,
                "socket {i}"
            );
        }
        assert_eq!(admission.sockets_of("0xabc"), MAX_SOCKETS_PER_ADDRESS);
        assert!(!admission.holds("0xabc", &handles[0]));
        assert!(admission.holds("0xabc", &handles[1]));
        // The displaced stream socket holds a permit until its loop reads it.
        assert!(
            futures::FutureExt::now_or_never(handles[0].notified()).is_some(),
            "the oldest was told to end"
        );
        admission.release("0xabc", &handles[0]);
        assert_eq!(admission.sockets_of("0xabc"), MAX_SOCKETS_PER_ADDRESS);
        for h in &handles[1..] {
            admission.release("0xabc", h);
        }
        assert_eq!(admission.sockets_of("0xabc"), 0);
    }

    #[test]
    fn the_subscription_bucket_survives_a_reconnect() {
        let admission = Admission::default();
        let first = Arc::new(Notify::new());
        assert!(!admission.admit_subscription("0xa"), "no socket, no budget");
        admission.claim("0xa", Eviction::Stream(first.clone()));
        let spent = (0..100)
            .take_while(|_| admission.admit_subscription("0xa"))
            .count();
        assert_eq!(spent, SUBSCRIPTION_BURST as usize);
        admission.release("0xa", &first);
        let second = Arc::new(Notify::new());
        admission.claim("0xa", Eviction::Stream(second.clone()));
        assert!(
            !admission.admit_subscription("0xa"),
            "reconnecting does not refill the bucket"
        );
        let other = Arc::new(Notify::new());
        admission.claim("0xb", Eviction::Stream(other));
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
