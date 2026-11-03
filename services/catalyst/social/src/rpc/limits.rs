//! The abuse throttle of the friendship/block RPC mutations (upstream
//! social-service-ea #456): each step publishes an update into the other
//! account's subscription stream, so an unbounded REQUEST/CANCEL or
//! block/unblock loop is a push-spam primitive. Budgets: 30 actions per actor
//! and 10 per pair per fixed minute by default, shared across replicas in SQL.
//!
//! [`Admission`] holds the per-address socket limits the RPC socket and
//! `/v1/stream` share (upstream #503).

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;

use crate::shared::rpc::ws::SocketClose;

pub use crate::comms::limits::{RateLimitDecision, RateLimiter};

const FRIENDSHIP_RATE_LIMIT_PER_ACTOR: u32 = 30;
const FRIENDSHIP_RATE_LIMIT_PER_PAIR: u32 = 10;

#[derive(Clone, Copy, Debug)]
pub struct FriendshipLimits {
    pub window_seconds: f64,
    pub per_actor: f64,
    pub per_pair: f64,
}

impl Default for FriendshipLimits {
    fn default() -> Self {
        Self {
            window_seconds: 60.0,
            per_actor: FRIENDSHIP_RATE_LIMIT_PER_ACTOR as f64,
            per_pair: FRIENDSHIP_RATE_LIMIT_PER_PAIR as f64,
        }
    }
}

/// Friendship pairs are symmetric; block pairs are directional and keep a
/// distinct key prefix so one account cannot spend the other's block budget.
#[derive(Debug, Clone, Copy)]
pub enum PairScope {
    Symmetric,
    Directional,
}

/// Upstream's Redis INCR/EXPIRE fixed window over the existing shared SQL
/// authority. Each bucket is atomic; actor consumption commits before the pair
/// attempt, including when the pair is denied or its store is unavailable.
pub struct FriendshipMutationLimiter {
    pool: sqlx::PgPool,
    limits: parking_lot::RwLock<FriendshipLimits>,
}

impl FriendshipMutationLimiter {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            pool,
            limits: parking_lot::RwLock::new(FriendshipLimits::default()),
        }
    }

    pub fn configure(&self, limits: FriendshipLimits) {
        *self.limits.write() = limits;
    }

    pub async fn allow(&self, actor: &str, target: &str, scope: PairScope) -> bool {
        self.allow_inner(actor, target, scope, None).await
    }

    async fn allow_inner(
        &self,
        actor: &str,
        target: &str,
        scope: PairScope,
        now: Option<chrono::NaiveDateTime>,
    ) -> bool {
        let limits = *self.limits.read();
        let (actor, target) = (actor.to_lowercase(), target.to_lowercase());
        if !self
            .consume(
                &format!("friends:rate:actor:{actor}"),
                limits.per_actor,
                limits.window_seconds,
                now,
            )
            .await
        {
            return false;
        }
        let pair_key = match scope {
            PairScope::Symmetric => {
                let (a, b) = if actor <= target {
                    (&actor, &target)
                } else {
                    (&target, &actor)
                };
                format!("friends:rate:pair:sym:{a}:{b}")
            }
            PairScope::Directional => format!("friends:rate:pair:dir:{actor}:{target}"),
        };
        self.consume(&pair_key, limits.per_pair, limits.window_seconds, now)
            .await
    }

    async fn consume(
        &self,
        key: &str,
        limit: f64,
        window: f64,
        now: Option<chrono::NaiveDateTime>,
    ) -> bool {
        // Redis EXPIRE only accepts a signed integer number of seconds and
        // rejects millisecond/deadline overflow. INCR is already committed
        // when that first EXPIRE fails, leaving a persistent key.
        let window_ms = if window.is_finite()
            && window.fract() == 0.0
            && window >= i64::MIN as f64
            && window < i64::MAX as f64
        {
            (window as i64).checked_mul(1000)
        } else {
            None
        };
        let row: Result<(i64, Option<i64>), sqlx::Error> = sqlx::query_as(
            r#"WITH clock AS (
                 SELECT COALESCE($3::bigint, floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint) AS now_ms
               ), deadline AS (
                 SELECT now_ms, now_ms::numeric + $2::numeric AS expires FROM clock
               )
               INSERT INTO social.friendship_mutation_limits AS counter (key, attempts, expires_at_ms)
               SELECT $1, 1, CASE WHEN expires BETWEEN -9223372036854775808::numeric AND 9223372036854775807::numeric
                                 THEN expires::bigint ELSE NULL END FROM deadline
               ON CONFLICT (key) DO UPDATE SET
                 attempts = CASE WHEN counter.expires_at_ms <= (SELECT now_ms FROM clock) THEN 1 ELSE counter.attempts + 1 END,
                 expires_at_ms = CASE WHEN counter.expires_at_ms <= (SELECT now_ms FROM clock) THEN EXCLUDED.expires_at_ms ELSE counter.expires_at_ms END
               RETURNING attempts, expires_at_ms"#)
            .bind(key).bind(window_ms).bind(now.map(|t|t.and_utc().timestamp_millis())).fetch_one(&self.pool).await;
        match row {
            Ok((1, None)) => {
                tracing::error!(
                    "friendship rate limiter expiration is not representable; allowing first increment"
                );
                metrics::counter!("friendship_rate_limiter_unavailable").increment(1);
                true
            }
            Ok((count, _)) => (count as f64) <= limit,
            Err(error) => {
                tracing::error!(%error, "friendship rate limiter unavailable; allowing action");
                metrics::counter!("friendship_rate_limiter_unavailable").increment(1);
                true
            }
        }
    }

    pub(crate) async fn cleanup_expired(&self) -> Result<u64, sqlx::Error> {
        Ok(sqlx::query(
            "DELETE FROM social.friendship_mutation_limits WHERE ctid = ANY(ARRAY(SELECT ctid FROM social.friendship_mutation_limits WHERE expires_at_ms <= floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint ORDER BY expires_at_ms, key LIMIT 1000)) AND expires_at_ms <= floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint")
            .execute(&self.pool).await?.rows_affected())
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

    async fn mutation_limiter() -> Option<(FriendshipMutationLimiter, String)> {
        let pool = crate::db::scratch::pool(4).await?;
        Some((
            FriendshipMutationLimiter::new(pool),
            uuid::Uuid::new_v4().to_string(),
        ))
    }

    #[tokio::test]
    async fn actor_budget_spans_targets() {
        let Some((l, actor)) = mutation_limiter().await else {
            return;
        };
        for i in 0..FRIENDSHIP_RATE_LIMIT_PER_ACTOR {
            assert!(
                l.allow(&actor, &format!("target{i}"), PairScope::Symmetric)
                    .await
            );
        }
        assert!(!l.allow(&actor, "fresh", PairScope::Symmetric).await);
    }

    #[tokio::test]
    async fn pair_budget_is_symmetric_for_friendship_actions() {
        let Some((l, prefix)) = mutation_limiter().await else {
            return;
        };
        let (a, b) = (format!("{prefix}a"), format!("{prefix}b"));
        for _ in 0..FRIENDSHIP_RATE_LIMIT_PER_PAIR / 2 {
            assert!(l.allow(&a, &b, PairScope::Symmetric).await);
            assert!(l.allow(&b, &a, PairScope::Symmetric).await);
        }
        assert!(!l.allow(&a, &b, PairScope::Symmetric).await);
        assert!(!l.allow(&b, &a, PairScope::Symmetric).await);
    }

    #[tokio::test]
    async fn block_budget_is_directional_so_the_counterparty_cannot_spend_it() {
        let Some((l, prefix)) = mutation_limiter().await else {
            return;
        };
        let (a, b) = (format!("{prefix}a"), format!("{prefix}b"));
        for _ in 0..FRIENDSHIP_RATE_LIMIT_PER_PAIR {
            assert!(l.allow(&a, &b, PairScope::Directional).await);
        }
        assert!(!l.allow(&a, &b, PairScope::Directional).await);
        assert!(l.allow(&b, &a, PairScope::Directional).await);
    }

    #[tokio::test]
    async fn symmetric_and_directional_buckets_never_merge() {
        let Some((l, prefix)) = mutation_limiter().await else {
            return;
        };
        let (a, b) = (format!("{prefix}a"), format!("{prefix}b"));
        for _ in 0..FRIENDSHIP_RATE_LIMIT_PER_PAIR {
            assert!(l.allow(&a, &b, PairScope::Symmetric).await);
        }
        assert!(!l.allow(&a, &b, PairScope::Symmetric).await);
        assert!(l.allow(&a, &b, PairScope::Directional).await);
    }

    #[tokio::test]
    async fn two_instances_share_a_fixed_window_anchored_at_its_first_hit() {
        let Some((first, prefix)) = mutation_limiter().await else {
            return;
        };
        let second = FriendshipMutationLimiter::new(first.pool.clone());
        let (a, b) = (format!("{prefix}a"), format!("{prefix}b"));
        let now = (chrono::Utc::now() + chrono::Duration::seconds(17)).naive_utc();
        for i in 0..10 {
            let limiter = if i % 2 == 0 { &first } else { &second };
            assert!(
                limiter
                    .allow_inner(&a, &b, PairScope::Symmetric, Some(now))
                    .await
            );
        }
        for seconds in [0, 6, 43, 59] {
            assert!(
                !second
                    .allow_inner(
                        &a,
                        &b,
                        PairScope::Symmetric,
                        Some(now + chrono::Duration::seconds(seconds))
                    )
                    .await,
                "no refill or wall-clock bucket at +{seconds}"
            );
        }
        assert!(
            first
                .allow_inner(
                    &a,
                    &b,
                    PairScope::Symmetric,
                    Some(now + chrono::Duration::seconds(60))
                )
                .await
        );
        let expiry: i64 = sqlx::query_scalar(
            "SELECT expires_at_ms FROM social.friendship_mutation_limits WHERE key=$1",
        )
        .bind(format!("friends:rate:pair:sym:{a}:{b}"))
        .fetch_one(&first.pool)
        .await
        .unwrap();
        assert_eq!(
            expiry,
            (now + chrono::Duration::seconds(120))
                .and_utc()
                .timestamp_millis()
        );
    }

    #[tokio::test]
    async fn concurrent_replicas_share_one_atomic_pair_budget() {
        let Some((first, prefix)) = mutation_limiter().await else {
            return;
        };
        let second = FriendshipMutationLimiter::new(first.pool.clone());
        let limits = FriendshipLimits {
            per_actor: 100.0,
            per_pair: 10.0,
            ..Default::default()
        };
        first.configure(limits);
        second.configure(limits);
        let instances = [Arc::new(first), Arc::new(second)];
        let mut attempts = tokio::task::JoinSet::new();
        for i in 0..32 {
            let instance = instances[i % 2].clone();
            let actor = prefix.clone();
            attempts
                .spawn(async move { instance.allow(&actor, "target", PairScope::Symmetric).await });
        }
        let mut allowed = 0;
        while let Some(result) = attempts.join_next().await {
            allowed += usize::from(result.unwrap());
        }
        assert_eq!(allowed, 10);
        let counts: Vec<i64> = sqlx::query_scalar(
            "SELECT attempts FROM social.friendship_mutation_limits WHERE key LIKE $1 ORDER BY key",
        )
        .bind(format!("%{prefix}%"))
        .fetch_all(&instances[0].pool)
        .await
        .unwrap();
        assert_eq!(counts, vec![32, 32]);
    }

    #[tokio::test]
    async fn denied_actor_does_not_consume_pair_but_denied_pair_consumes_actor() {
        let Some((l, prefix)) = mutation_limiter().await else {
            return;
        };
        l.configure(FriendshipLimits {
            per_actor: 2.0,
            per_pair: 1.0,
            ..Default::default()
        });
        assert!(l.allow(&prefix, "pair", PairScope::Symmetric).await);
        assert!(!l.allow(&prefix, "pair", PairScope::Symmetric).await);
        assert!(!l.allow(&prefix, "fresh", PairScope::Symmetric).await);
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT key,attempts FROM social.friendship_mutation_limits WHERE key LIKE $1",
        )
        .bind(format!("%{prefix}%"))
        .fetch_all(&l.pool)
        .await
        .unwrap();
        assert_eq!(rows.len(), 2, "no bucket for the denied actor's fresh pair");
        assert_eq!(
            rows.iter()
                .find(|(key, _)| key.contains("rate:actor:"))
                .unwrap()
                .1,
            3
        );
        assert_eq!(
            rows.iter()
                .find(|(key, _)| key.contains("pair:sym:"))
                .unwrap()
                .1,
            2
        );
    }

    #[tokio::test]
    async fn zero_negative_fractional_configuration_and_store_errors_follow_source_order() {
        let Some((l, prefix)) = mutation_limiter().await else {
            return;
        };
        for value in [0.0, -1.0] {
            l.configure(FriendshipLimits {
                per_actor: value,
                ..Default::default()
            });
            assert!(
                !l.allow(&format!("{prefix}{value}"), "target", PairScope::Symmetric)
                    .await
            );
        }
        for window in [0.0, -1.0, -1e12] {
            l.configure(FriendshipLimits {
                window_seconds: window,
                per_actor: 1.0,
                per_pair: 1.0,
            });
            for _ in 0..3 {
                assert!(
                    l.allow(
                        &format!("{prefix}window{window}"),
                        "target",
                        PairScope::Symmetric
                    )
                    .await
                );
            }
        }
        l.configure(FriendshipLimits {
            window_seconds: 1.5,
            per_actor: 1.0,
            per_pair: 1.0,
        });
        let actor = format!("{prefix}fractional");
        assert!(
            l.allow(&actor, "target", PairScope::Symmetric).await,
            "first INCR survives EXPIRE failure, so it fails open"
        );
        assert!(
            !l.allow(&actor, "target", PairScope::Symmetric).await,
            "second INCR does not retry EXPIRE and exceeds the budget"
        );
        let closed = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://localhost/unused")
            .unwrap();
        closed.close().await;
        assert!(
            FriendshipMutationLimiter::new(closed)
                .allow("actor", "target", PairScope::Symmetric)
                .await
        );
    }

    #[tokio::test]
    async fn migration_preserves_existing_counters_and_cleanup_is_bounded() {
        let Some((l, prefix)) = mutation_limiter().await else {
            return;
        };
        assert!(l.allow(&prefix, "target", PairScope::Symmetric).await);
        for _ in 0..2 {
            sqlx::raw_sql(include_str!(concat!(
                env!("DCL_ONE_ROOT"),
                "/schemas/migrations/0023_social_friendship_limits.sql"
            )))
            .execute(&l.pool)
            .await
            .unwrap();
        }
        let attempts: i64 = sqlx::query_scalar(
            "SELECT attempts FROM social.friendship_mutation_limits WHERE key=$1",
        )
        .bind(format!("friends:rate:actor:{prefix}"))
        .fetch_one(&l.pool)
        .await
        .unwrap();
        assert_eq!(attempts, 1);
        sqlx::query("INSERT INTO social.friendship_mutation_limits(key,attempts,expires_at_ms) SELECT $1 || i,1,floor(extract(epoch FROM now())*1000)::bigint-1000 FROM generate_series(1,1200)i")
            .bind(format!("expired:{prefix}:")).execute(&l.pool).await.unwrap();
        assert_eq!(l.cleanup_expired().await.unwrap(), 1000);
        let remainder = l.cleanup_expired().await.unwrap();
        assert!(
            (200..=1000).contains(&remainder),
            "the shared scratch database may contain other expired test counters"
        );
        let own_remaining: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM social.friendship_mutation_limits WHERE key LIKE $1",
        )
        .bind(format!("expired:{prefix}:%"))
        .fetch_one(&l.pool)
        .await
        .unwrap();
        assert_eq!(own_remaining, 0);
        assert!(l.cleanup_expired().await.unwrap() <= 1000);
    }

    #[tokio::test]
    async fn shared_sql_limiter_matches_executed_upstream_redis_scenarios() {
        let Some((first, _)) = mutation_limiter().await else {
            return;
        };
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/tests/fixtures/conformance/social/limiter-cases.json"
        )))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let configuration = &case["configuration"];
            let limits = FriendshipLimits {
                window_seconds: configuration["FRIENDSHIP_RATE_LIMIT_WINDOW_SECONDS"]
                    .as_f64()
                    .unwrap_or(60.0),
                per_actor: configuration["FRIENDSHIP_RATE_LIMIT_PER_ACTOR"]
                    .as_f64()
                    .unwrap_or(30.0),
                per_pair: configuration["FRIENDSHIP_RATE_LIMIT_PER_PAIR"]
                    .as_f64()
                    .unwrap_or(10.0),
            };
            let instances = [
                FriendshipMutationLimiter::new(first.pool.clone()),
                FriendshipMutationLimiter::new(first.pool.clone()),
            ];
            for instance in &instances {
                instance.configure(limits);
            }
            let prefix = format!("{}:", uuid::Uuid::new_v4());
            let start = (chrono::Utc::now() + chrono::Duration::seconds(17)).naive_utc();
            for attempt in case["attempts"].as_array().unwrap() {
                let actor = format!("{prefix}{}", attempt["actor"].as_str().unwrap());
                let target = format!("{prefix}{}", attempt["target"].as_str().unwrap());
                let now =
                    start + chrono::Duration::milliseconds(attempt["after_ms"].as_i64().unwrap());
                let allowed = instances[attempt["instance"].as_u64().unwrap() as usize]
                    .allow_inner(&actor, &target, PairScope::Directional, Some(now))
                    .await;
                assert_eq!(
                    allowed,
                    attempt["allowed"].as_bool().unwrap(),
                    "{} {attempt}",
                    case["id"]
                );
                let rows: Vec<(String, i64, Option<i64>)> = sqlx::query_as(
                    "SELECT key,attempts,expires_at_ms FROM social.friendship_mutation_limits WHERE key LIKE $1 AND (expires_at_ms IS NULL OR expires_at_ms > $2) ORDER BY key")
                    .bind(format!("%{prefix}%")).bind(now.and_utc().timestamp_millis()).fetch_all(&first.pool).await.unwrap();
                let actual: Vec<serde_json::Value> = rows.into_iter().map(|(key, attempts, expires)| serde_json::json!({
                    "key": key.replace(&prefix, ""), "attempts": attempts, "expiry": if expires.is_some() { "expiring" } else { "persistent" }
                })).collect();
                assert_eq!(
                    serde_json::json!(actual),
                    attempt["counters"],
                    "{} at {}ms",
                    case["id"],
                    attempt["after_ms"]
                );
            }
        }
    }
}
