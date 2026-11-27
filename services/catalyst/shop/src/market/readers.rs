//! The reader kind of the stage-5 sources: one bounded subgraph call
//! (`first` capped, `skip` capped at the graph's own 5,000) memoized under
//! the route's freshness budget. A page past the graph's skip window is read
//! by keyset: after the first 6,000 rows (`skip` 0..=5,000 in pages of
//! 1,000), each further page asks for the rows at or past the last sort value
//! (`<field>_lte` descending, `_gte` ascending) and skips the ties already
//! read (the graph orders ties by id), up to [`DEEP_WINDOW`] rows. There is
//! no table; a reader whose subgraph is not configured answers the
//! unavailable envelope through [`require_collections`] /
//! [`require_marketplace`], never an empty page.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::shared::cache::TtlMap;

use super::graph::{self, Network};
use super::{MarketplaceState, Source, unavailable};

/// Index snapshot SQL errors use the statement-bound 503 or a sanitized 500.
pub(super) fn index_read_error(e: sqlx::Error) -> Response {
    if super::nft_reads::cancelled(&e) {
        super::nft_reads::timed_out()
    } else {
        super::wire::ApiError::from(e).into_response()
    }
}

pub(super) fn index_source_error(e: super::owned_items::ReadError) -> Response {
    match e {
        super::owned_items::ReadError::Db(e) => index_read_error(e),
        super::owned_items::ReadError::NotReady(source, why) => unavailable(source, &why),
    }
}

/// Require the LAND publication for the served registries on the book's snapshot.
pub(super) async fn require_land_on(
    conn: &mut sqlx::PgConnection,
    state: &MarketplaceState,
    chain_id: i64,
) -> Result<crate::land_authz::indexer::Cursor, Response> {
    let unavailable = super::nft_reads::land_unavailable;
    let served = super::nft_index::deployments::Deployments::for_chain(
        super::chain_walk::ChainNet::Ethereum,
        chain_id,
    )
    .and_then(|deployments| deployments.land);
    let land = state
        .land_network()
        .filter(|land| Some(*land) == served)
        .ok_or_else(unavailable)?;
    let observed = crate::land_authz::indexer::cursor_observed(conn, land.network)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "cannot read the land-authz index for the marketplace book");
            unavailable()
        })?;
    observed
        .filter(|observed| {
            crate::land_authz::indexer::observation_ready(
                observed,
                land.land_registry,
                land.estate_registry,
            )
        })
        .map(|observed| observed.cursor)
        .ok_or_else(unavailable)
}

/// The freshness budgets the route table names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ttl {
    S30,
    S60,
    M5,
    M10,
    M15,
}

impl Ttl {
    pub fn duration(self) -> Duration {
        match self {
            Ttl::S30 => Duration::from_secs(30),
            Ttl::S60 => Duration::from_secs(60),
            Ttl::M5 => Duration::from_secs(300),
            Ttl::M10 => Duration::from_secs(600),
            Ttl::M15 => Duration::from_secs(900),
        }
    }
}

const MAX_ENTRIES: usize = 4_096;

pub struct Caches {
    s30: TtlMap<String, Arc<Value>>,
    s60: TtlMap<String, Arc<Value>>,
    m5: TtlMap<String, Arc<Value>>,
    m10: TtlMap<String, Arc<Value>>,
    m15: TtlMap<String, Arc<Value>>,
    /// The keys a background read is refreshing, or whose last refresh failed
    /// ([`Caches::begin_refresh`]).
    refreshing: parking_lot::Mutex<HashMap<String, Refresh>>,
    /// The next [`ClaimId`] a refresh is given.
    next_claim: AtomicU64,
    /// The last good answer of the keys served while they refresh ([`Caches::keep`]), apart from
    /// the bounded maps, whose eviction would drop an expired one first.
    kept: parking_lot::Mutex<HashMap<String, Kept>>,
}

/// A kept answer: when it was read, plus the age a test adds to it ([`Caches::backdate_kept`]),
/// which never moves an `Instant` back past the clock's start.
struct Kept {
    at: Instant,
    aged: Duration,
    value: Arc<Value>,
}

impl Kept {
    fn age(&self) -> Duration {
        self.at.elapsed() + self.aged
    }
}

/// A key's background refresh: running under the claim it was given, or failed at an instant.
enum Refresh {
    Running(ClaimId),
    FailedAt(Instant),
}

/// One claim on a key's refresh ([`Caches::begin_refresh`]). Its release acts only while the key
/// still runs under it, so a claim that outlives a reset ([`Caches::clear`]) never releases or
/// fails a newer one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClaimId(u64);

impl Default for Caches {
    fn default() -> Self {
        Self {
            s30: TtlMap::bounded(Ttl::S30.duration(), MAX_ENTRIES),
            s60: TtlMap::bounded(Ttl::S60.duration(), MAX_ENTRIES),
            m5: TtlMap::bounded(Ttl::M5.duration(), MAX_ENTRIES),
            m10: TtlMap::bounded(Ttl::M10.duration(), MAX_ENTRIES),
            m15: TtlMap::bounded(Ttl::M15.duration(), MAX_ENTRIES),
            refreshing: parking_lot::Mutex::default(),
            next_claim: AtomicU64::new(0),
            kept: parking_lot::Mutex::default(),
        }
    }
}

impl Caches {
    fn map(&self, ttl: Ttl) -> &TtlMap<String, Arc<Value>> {
        match ttl {
            Ttl::S30 => &self.s30,
            Ttl::S60 => &self.s60,
            Ttl::M5 => &self.m5,
            Ttl::M10 => &self.m10,
            Ttl::M15 => &self.m15,
        }
    }

    /// `key`'s answer under `ttl` while it is fresh.
    pub fn fresh(&self, ttl: Ttl, key: &str) -> Option<Arc<Value>> {
        self.map(ttl).get_fresh(&key.to_string())
    }

    /// Keeps `value` as `key`'s last good answer, for [`Caches::kept_within`], aged from the read
    /// that produced it: keeping the very answer already kept (a cache hit handing it out again)
    /// leaves its age alone, and only a new answer starts it over.
    pub fn keep(&self, key: &str, value: Arc<Value>) {
        let mut kept = self.kept.lock();
        if kept
            .get(key)
            .is_some_and(|held| Arc::ptr_eq(&held.value, &value))
        {
            return;
        }
        let fresh = Kept {
            at: Instant::now(),
            aged: Duration::ZERO,
            value,
        };
        kept.insert(key.to_string(), fresh);
    }

    /// Ages `key`'s kept answer by `by`.
    #[cfg(any(test, feature = "test-support"))]
    pub fn backdate_kept(&self, key: &str, by: Duration) {
        if let Some(held) = self.kept.lock().get_mut(key) {
            held.aged += by;
        }
    }

    /// `key`'s last good answer while it was kept less than `max_age` ago.
    pub fn kept_within(&self, key: &str, max_age: Duration) -> Option<Arc<Value>> {
        self.kept
            .lock()
            .get(key)
            .filter(|held| held.age() < max_age)
            .map(|held| Arc::clone(&held.value))
    }

    /// Claims `key`'s background refresh: the claim for the one caller that is to run it and
    /// release it with [`Caches::end_refresh`]. None while another runs, nor within `backoff` of a
    /// failed one, so a failing source is retried at most once per `backoff`.
    pub fn begin_refresh(&self, key: &str, backoff: Duration) -> Option<ClaimId> {
        let mut claims = self.refreshing.lock();
        let blocked = match claims.get(key) {
            Some(Refresh::Running(_)) => true,
            Some(Refresh::FailedAt(at)) => at.elapsed() < backoff,
            None => false,
        };
        if blocked {
            return None;
        }
        let claim = ClaimId(self.next_claim.fetch_add(1, Ordering::Relaxed));
        claims.insert(key.to_string(), Refresh::Running(claim));
        Some(claim)
    }

    /// Releases `claim` on `key`: cleared when the refresh succeeded, else marked failed now. A
    /// claim the key no longer runs under (a reset dropped it, a newer one took the key) changes
    /// nothing.
    pub fn end_refresh(&self, key: &str, claim: ClaimId, succeeded: bool) {
        let mut claims = self.refreshing.lock();
        if !matches!(claims.get(key), Some(Refresh::Running(held)) if *held == claim) {
            return;
        }
        if succeeded {
            claims.remove(key);
        } else {
            claims.insert(key.to_string(), Refresh::FailedAt(Instant::now()));
        }
    }

    /// Drops `key`'s answer under `ttl`, as an eviction would, without moving the clock (a
    /// backdate of minutes can pass the clock's start on a host just booted).
    #[cfg(any(test, feature = "test-support"))]
    pub fn forget(&self, ttl: Ttl, key: &str) {
        self.map(ttl).remove(&key.to_string());
    }

    /// Whether `key` holds a fresh answer under `ttl`.
    #[cfg(any(test, feature = "test-support"))]
    pub fn holds(&self, ttl: Ttl, key: &str) -> bool {
        self.map(ttl).get_fresh(&key.to_string()).is_some()
    }

    /// Drops every answer under `ttl`, as their expiry would.
    #[cfg(any(test, feature = "test-support"))]
    pub fn forget_all(&self, ttl: Ttl) {
        self.map(ttl).clear();
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn clear(&self) {
        for m in [&self.s30, &self.s60, &self.m5, &self.m10, &self.m15] {
            m.clear();
        }
        self.refreshing.lock().clear();
        self.kept.lock().clear();
    }
}

/// A claimed refresh ([`Caches::begin_refresh`]), released when dropped however the refresh
/// ended, a panic included: as failed unless [`RefreshClaim::succeeded`] was called.
pub struct RefreshClaim {
    state: super::SharedState,
    key: String,
    claim: ClaimId,
    succeeded: bool,
}

impl RefreshClaim {
    pub fn new(state: super::SharedState, key: String, claim: ClaimId) -> Self {
        Self {
            state,
            key,
            claim,
            succeeded: false,
        }
    }

    pub fn succeeded(&mut self) {
        self.succeeded = true;
    }
}

impl Drop for RefreshClaim {
    fn drop(&mut self) {
        self.state
            .readers
            .end_refresh(&self.key, self.claim, self.succeeded);
    }
}

/// The graph's own `skip` ceiling; a page past it is read by keyset.
pub const GRAPH_MAX_SKIP: i64 = 5_000;
/// The deepest row a reader page reaches (`first + skip`): 25 pages of
/// 1,000 per network, one upstream round trip each, memoized like any page.
pub const DEEP_WINDOW: i64 = 25_000;
/// The list routes' page cap on a reader.
pub const READER_MAX_FIRST: i64 = 100;

/// Memoized under `ttl` by `key` (the route and its normalized query), one
/// upstream call per key per budget; concurrent misses wait for the leader.
pub async fn cached<F, Fut>(
    state: &MarketplaceState,
    ttl: Ttl,
    key: String,
    fetch: F,
) -> Result<Arc<Value>, String>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<Value, String>>,
{
    memoized(state, ttl, key, fetch).await
}

/// [`cached`] for a fetch whose failure is the route's own answer; a failure is not kept.
pub async fn memoized<F, Fut, E>(
    state: &MarketplaceState,
    ttl: Ttl,
    key: String,
    fetch: F,
) -> Result<Arc<Value>, E>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<Value, E>>,
{
    state
        .readers
        .map(ttl)
        .get_or_fetch(key, || async { fetch().await.map(Arc::new) })
        .await
}

/// [`memoized`] for a fetch that reads through what the caller holds (its
/// catalogue read), which only the fetching request keeps
/// ([`crate::shared::cache::TtlMap::get_or_fetch_holding`]): a request that
/// waits on another's fetch drops it first. `Ok(None)`: that fetch failed
/// and the caller, its read gone, opens a new one and asks again.
pub async fn memoized_holding<G, F, E>(
    state: &MarketplaceState,
    ttl: Ttl,
    key: String,
    held: &mut Option<G>,
    fetch: F,
) -> Result<Option<Arc<Value>>, E>
where
    F: AsyncFnOnce(&mut G) -> Result<Value, E>,
{
    state
        .readers
        .map(ttl)
        .get_or_fetch_holding(key, held, async move |g: &mut G| {
            fetch(g).await.map(Arc::new)
        })
        .await
}

/// The collections subgraph of `network`, or the envelope naming its variable.
pub fn require_collections(state: &MarketplaceState, network: Network) -> Result<&str, Response> {
    network.url(&state.index).ok_or_else(|| {
        unavailable(
            Source::CollectionsSubgraph(network),
            &format!(
                "the {} collections subgraph reader is not configured on this node",
                network.db_name()
            ),
        )
    })
}

/// Both collections subgraphs, for a route without a network filter: it
/// answers for the whole marketplace, so one missing subgraph is the
/// envelope naming its variable (matic first, the L2 holds the catalogue),
/// never a page that silently omits a network (an eth-only node once
/// presented empty wardrobes and a near-empty marketplace as complete).
pub fn require_both_collections(
    state: &MarketplaceState,
) -> Result<Vec<(Network, &str)>, Response> {
    for network in [Network::Matic, Network::Ethereum] {
        if network.url(&state.index).is_none() {
            return Err(unavailable(
                Source::CollectionsSubgraph(network),
                &format!(
                    "the {} collections subgraph reader is not configured on this node, and a route without a network filter answers for both networks",
                    network.db_name()
                ),
            ));
        }
    }
    Ok([Network::Ethereum, Network::Matic]
        .into_iter()
        .filter_map(|n| n.url(&state.index).map(|u| (n, u)))
        .collect())
}

/// The readers a request needs: the named network's, or both.
pub fn require_networks(
    state: &MarketplaceState,
    network: Option<Network>,
) -> Result<Vec<(Network, &str)>, Response> {
    match network {
        Some(n) => Ok(vec![(n, require_collections(state, n)?)]),
        None => require_both_collections(state),
    }
}

pub fn require_marketplace(state: &MarketplaceState) -> Result<&str, Response> {
    state
        .settings
        .marketplace_subgraph_url
        .as_deref()
        .ok_or_else(|| unavailable(Source::MarketplaceSubgraph, "the marketplace subgraph reader (LAND, estates, names) is not configured on this node"))
}

/// A reader that failed upstream: the lambdas' 503, never a 200 with nothing in it.
pub fn upstream_failed(route: &str, err: &str) -> Response {
    tracing::warn!(route, error = %err, "marketplace reader failed");
    super::service_unavailable("upstream reader unavailable")
}

/// One GraphQL document against `url`; the body's `data` on success.
pub async fn query(
    state: &MarketplaceState,
    url: &str,
    document: &str,
    variables: Value,
) -> Result<Value, String> {
    graph::graph_query(&state.index, url, document, variables).await
}

/// The rows under `field` of a subgraph answer, as owned values.
pub fn rows(data: &Value, field: &str) -> Vec<Value> {
    data.get(field)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

pub fn str_of(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

pub fn opt_str_of(v: &Value, key: &str) -> Option<String> {
    match v.get(key) {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

pub fn int_of(v: &Value, key: &str) -> i64 {
    match v.get(key) {
        Some(Value::String(s)) => s.trim().parse().unwrap_or(0),
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
        _ => 0,
    }
}

pub fn opt_int_of(v: &Value, key: &str) -> Option<i64> {
    match v.get(key) {
        Some(Value::String(s)) => s.trim().parse().ok(),
        Some(Value::Number(n)) => n.as_i64(),
        _ => None,
    }
}

pub fn bool_of(v: &Value, key: &str) -> bool {
    match v.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s == "true",
        _ => false,
    }
}

pub fn opt_bool_of(v: &Value, key: &str) -> Option<bool> {
    match v.get(key) {
        Some(Value::Bool(b)) => Some(*b),
        _ => None,
    }
}

pub fn strings_of(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// The nested object at `path` (`a/b`), when present and not null.
pub fn at<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = v;
    for part in path.split('/') {
        cur = cur.get(part)?;
        if cur.is_null() {
            return None;
        }
    }
    Some(cur)
}

/// The subgraph's unix seconds as the wire's milliseconds.
pub fn seconds_to_ms(seconds: i64) -> i64 {
    seconds.saturating_mul(1000)
}

/// A `first`/`skip` pair inside the reader bounds, or `None` when the page
/// is past the graph's cap (the caller answers the deferred envelope).
pub fn reader_page(
    first: Option<i64>,
    skip: Option<i64>,
    default_first: i64,
) -> Option<(i64, i64)> {
    let first = first.unwrap_or(default_first).clamp(0, READER_MAX_FIRST);
    let skip = skip.unwrap_or(0).max(0);
    (skip <= GRAPH_MAX_SKIP).then_some((first, skip))
}

/// A list route's page, deep pages included: `first` capped at
/// [`READER_MAX_FIRST`], a window past [`DEEP_WINDOW`] refused with 400 (the
/// nft index path's rule for its own window).
pub fn deep_page(
    first: Option<i64>,
    skip: Option<i64>,
    default_first: i64,
) -> Result<(i64, i64), Response> {
    let first = first.unwrap_or(default_first).clamp(0, READER_MAX_FIRST);
    let skip = skip.unwrap_or(0).max(0);
    check_window(first, skip).map(|_| (first, skip))
}

/// 400 unless `first + skip` is inside [`DEEP_WINDOW`].
pub fn check_window(first: i64, skip: i64) -> Result<(), Response> {
    if first.saturating_add(skip) > DEEP_WINDOW {
        return Err(super::wire::ApiError::bad_request(format!(
            "first + skip may not exceed {DEEP_WINDOW}"
        ))
        .into_response());
    }
    Ok(())
}

/// The count a page's total reports: capped at the graph's window for a
/// page inside it, at [`DEEP_WINDOW`] for a deeper one.
pub fn count_cap(first: i64, skip: i64) -> i64 {
    if first + skip > GRAPH_MAX_SKIP {
        DEEP_WINDOW
    } else {
        GRAPH_MAX_SKIP
    }
}

/// The envelope for a page a reader cannot serve (past the graph's cap or a
/// shape only an indexer answers).
pub fn deferred(label: &'static str, message: &str) -> Response {
    unavailable(Source::Deferred(label), message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reader_pages_stay_inside_the_graph_bounds() {
        assert_eq!(reader_page(None, None, 20), Some((20, 0)));
        assert_eq!(reader_page(Some(500), Some(10), 20), Some((100, 10)));
        assert_eq!(reader_page(Some(-3), Some(-1), 20), Some((0, 0)));
        assert_eq!(reader_page(Some(10), Some(5_000), 20), Some((10, 5_000)));
        assert_eq!(reader_page(Some(10), Some(5_001), 20), None);
    }

    #[test]
    fn value_readers_tolerate_the_graphs_stringly_numbers() {
        let v = serde_json::json!({ "a": "12", "b": 7, "c": null, "d": "true", "e": ["x", 1, "y"], "n": { "m": { "k": "v" } } });
        assert_eq!(int_of(&v, "a"), 12);
        assert_eq!(int_of(&v, "b"), 7);
        assert_eq!(int_of(&v, "c"), 0);
        assert_eq!(opt_int_of(&v, "c"), None);
        assert!(bool_of(&v, "d"));
        assert_eq!(opt_bool_of(&v, "d"), None);
        assert_eq!(strings_of(&v, "e"), vec!["x", "y"]);
        assert_eq!(at(&v, "n/m/k").and_then(Value::as_str), Some("v"));
        assert_eq!(at(&v, "n/c"), None);
        assert_eq!(str_of(&v, "b"), "7");
        assert_eq!(seconds_to_ms(1_700_000_000), 1_700_000_000_000);
    }

    #[tokio::test]
    async fn the_memo_answers_the_second_call_from_the_first() {
        let state = crate::market::tests::support::state_without_db();
        let calls = std::sync::atomic::AtomicUsize::new(0);
        for _ in 0..2 {
            let v = cached(&state, Ttl::M5, "k".into(), || async {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(serde_json::json!(1))
            })
            .await
            .unwrap();
            assert_eq!(*v, serde_json::json!(1));
        }
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(require_marketplace(&state).is_err());
        assert!(require_both_collections(&state).is_err());
    }

    #[tokio::test]
    async fn a_route_without_a_network_filter_needs_both_subgraphs() {
        let eth_only =
            crate::market::tests::support::state_without_db_settings(crate::market::Settings {
                eth_collections_subgraph_url: Some("https://subgraph.example/eth".into()),
                ..crate::market::Settings::for_tests()
            });
        let err = require_both_collections(&eth_only).unwrap_err();
        assert_eq!(
            err.status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "matic is missing: the eth-only node does not answer for the whole marketplace"
        );
        assert!(
            require_networks(&eth_only, Some(Network::Ethereum)).is_ok(),
            "the named network alone is enough"
        );
        assert!(require_networks(&eth_only, Some(Network::Matic)).is_err());
        assert!(require_networks(&eth_only, None).is_err());
        let both =
            crate::market::tests::support::state_without_db_settings(crate::market::Settings {
                eth_collections_subgraph_url: Some("https://subgraph.example/eth".into()),
                matic_collections_subgraph_url: Some("https://subgraph.example/matic".into()),
                ..crate::market::Settings::for_tests()
            });
        assert_eq!(require_both_collections(&both).unwrap().len(), 2);
    }
}

/// A `first`/`skip` page of one subgraph entity. `order_by` and `direction`
/// are interpolated into the document and must come from code, never from a
/// request; the filter travels as a variable.
#[derive(Clone)]
pub struct GraphPage<'a> {
    pub entity: &'a str,
    pub filter_type: &'a str,
    pub fields: &'a str,
    pub where_: Value,
    pub order_by: &'a str,
    pub direction: &'a str,
    pub first: i64,
    pub skip: i64,
}

pub fn page_document(p: &GraphPage<'_>) -> String {
    format!(
        "query Page($first: Int!, $skip: Int!, $where: {filter}) {{ {entity}(first: $first, skip: $skip, orderBy: {order_by}, orderDirection: {direction}, where: $where) {{ {fields} }} }}",
        filter = p.filter_type,
        entity = p.entity,
        order_by = p.order_by,
        direction = p.direction,
        fields = p.fields,
    )
}

/// The page's rows, memoized under `ttl` by network, document and filter.
pub async fn page(
    state: &MarketplaceState,
    network: Network,
    url: &str,
    ttl: Ttl,
    p: &GraphPage<'_>,
) -> Result<Vec<Value>, String> {
    let document = page_document(p);
    let key = format!(
        "page:{}:{}:{}:{}:{}",
        network.db_name(),
        document,
        p.where_,
        p.first,
        p.skip
    );
    let entity = p.entity.to_string();
    let vars = json!({ "first": p.first, "skip": p.skip, "where": p.where_ });
    let data = cached(state, ttl, key, || async {
        query(state, url, &document, vars).await
    })
    .await?;
    Ok(rows(&data, &entity))
}

/// The first `want` rows of `p`'s order (its `first`/`skip` ignored): what one
/// network contributes to a page merged across sources, which needs every
/// source's top `first + skip` rows. The graph refuses a `first` above
/// [`super::graph::THE_GRAPH_PAGE_SIZE`] and a `skip` above [`GRAPH_MAX_SKIP`],
/// so the window is read in pages of at most 1000 (six at most for the
/// 6,000 rows `skip` reaches), then by keyset on the sort field past them
/// (see the module doc), each page memoized like any page, stopping at a
/// short one.
pub async fn top_rows(
    state: &MarketplaceState,
    network: Network,
    url: &str,
    ttl: Ttl,
    p: &GraphPage<'_>,
    want: i64,
) -> Result<Vec<Value>, String> {
    read_top(state, network, url, ttl, p, want.min(DEEP_WINDOW)).await
}

async fn read_top(
    state: &MarketplaceState,
    network: Network,
    url: &str,
    ttl: Ttl,
    p: &GraphPage<'_>,
    want: i64,
) -> Result<Vec<Value>, String> {
    let size = super::graph::THE_GRAPH_PAGE_SIZE;
    let mut out: Vec<Value> = Vec::new();
    let mut skip = 0;
    let mut exhausted = false;
    while skip < want && skip <= GRAPH_MAX_SKIP {
        let first = (want - skip).min(size);
        let got = page(
            state,
            network,
            url,
            ttl,
            &GraphPage {
                first,
                skip,
                ..p.clone()
            },
        )
        .await?;
        let n = got.len() as i64;
        out.extend(got);
        if n < first {
            exhausted = true;
            break;
        }
        skip += first;
    }
    while !exhausted && (out.len() as i64) < want {
        let Some(cursor) = out.last().and_then(|r| r.get(p.order_by)).cloned() else {
            return Err(format!("a keyset page needs `{}` in every row", p.order_by));
        };
        let ties = out
            .iter()
            .rev()
            .take_while(|r| r.get(p.order_by) == Some(&cursor))
            .count() as i64;
        if ties > GRAPH_MAX_SKIP {
            return Err(format!(
                "more than {GRAPH_MAX_SKIP} rows share one `{}`",
                p.order_by
            ));
        }
        let first = (want - out.len() as i64).min(size);
        let where_ = keyset_where(&p.where_, p.order_by, p.direction, &cursor);
        let got = page(
            state,
            network,
            url,
            ttl,
            &GraphPage {
                first,
                skip: ties,
                where_,
                ..p.clone()
            },
        )
        .await?;
        let n = got.len() as i64;
        out.extend(got);
        exhausted = n < first;
    }
    Ok(out)
}

fn as_number(v: &Value) -> Option<i128> {
    v.as_i64()
        .map(i128::from)
        .or_else(|| v.as_str().and_then(|s| s.parse::<i128>().ok()))
}

/// `where_` bounded at the cursor: rows at or past it in `direction`, the
/// tighter of the cursor and a bound the filter already sets on the field.
pub fn keyset_where(where_: &Value, field: &str, direction: &str, cursor: &Value) -> Value {
    let mut w = where_.clone();
    let (key, keep_existing): (String, fn(i128, i128) -> bool) = if direction == "asc" {
        (format!("{field}_gte"), |existing, cursor| existing > cursor)
    } else {
        (format!("{field}_lte"), |existing, cursor| existing < cursor)
    };
    let tighter = match (w.get(&key).and_then(as_number), as_number(cursor)) {
        (Some(existing), Some(c)) => keep_existing(existing, c),
        _ => false,
    };
    if !tighter {
        w[key] = cursor.clone();
    }
    w
}

/// One source's rows of a page over `sources` sources: with one source the
/// graph pages it itself (`first`, `skip`) and the caller skips nothing;
/// with several each gives its top `first + skip` rows ([`top_rows`]) and
/// the caller skips `skip` of the merge. Returns the rows and the offset the
/// caller applies after merging.
#[allow(
    clippy::too_many_arguments,
    reason = "the subgraph source, its cache TTL and the merge window"
)]
pub async fn source_rows(
    state: &MarketplaceState,
    network: Network,
    url: &str,
    ttl: Ttl,
    p: &GraphPage<'_>,
    first: i64,
    skip: i64,
    sources: usize,
) -> Result<(Vec<Value>, i64), String> {
    if sources == 1 && skip <= GRAPH_MAX_SKIP {
        Ok((
            page(
                state,
                network,
                url,
                ttl,
                &GraphPage {
                    first,
                    skip,
                    ..p.clone()
                },
            )
            .await?,
            0,
        ))
    } else {
        Ok((
            top_rows(state, network, url, ttl, p, first + skip).await?,
            skip,
        ))
    }
}

/// How many entities match `where_`: id-keyset pages of 1000, at most
/// [`GRAPH_MAX_SKIP`] rows, so a bigger set answers the cap. Memoized 5 min.
pub async fn count_capped(
    state: &MarketplaceState,
    network: Network,
    url: &str,
    entity: &str,
    filter_type: &str,
    where_: Value,
) -> Result<i64, String> {
    count_to(
        state,
        network,
        url,
        entity,
        filter_type,
        where_,
        GRAPH_MAX_SKIP,
    )
    .await
}

/// As [`count_capped`], up to `cap` rows (a deep page's total).
pub async fn count_to(
    state: &MarketplaceState,
    network: Network,
    url: &str,
    entity: &str,
    filter_type: &str,
    where_: Value,
    cap: i64,
) -> Result<i64, String> {
    let key = format!("count:{}:{entity}:{cap}:{where_}", network.db_name());
    let document = format!(
        "query Count($first: Int!, $where: {filter_type}) {{ {entity}(first: $first, orderBy: id, orderDirection: asc, where: $where) {{ id }} }}"
    );
    let value = cached(state, Ttl::M5, key, || async {
        let mut total: i64 = 0;
        let mut last_id: Option<String> = None;
        let page_size = super::graph::THE_GRAPH_PAGE_SIZE;
        while total < cap {
            let mut w = where_.clone();
            if let Some(id) = &last_id {
                w["id_gt"] = Value::String(id.clone());
            }
            let data = query(
                state,
                url,
                &document,
                json!({ "first": page_size, "where": w }),
            )
            .await?;
            let ids = rows(&data, entity);
            let n = ids.len() as i64;
            total += n;
            last_id = ids.last().map(|r| str_of(r, "id"));
            if n < page_size {
                break;
            }
        }
        Ok(Value::from(total.min(cap)))
    })
    .await?;
    Ok(value.as_i64().unwrap_or(0))
}

/// `first`/`skip` clamped to the reader budget, or `None` when the page lies
/// past what a subgraph serves (the caller answers the deferred envelope).
pub fn reader_window(
    first: Option<i64>,
    skip: Option<i64>,
    default_first: i64,
) -> Option<(i64, i64)> {
    reader_page(first, skip, default_first)
}

/// The unix milliseconds the readers compare an expiry against.
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(test)]
mod refresh_tests {
    use super::*;

    /// One refresh runs at a time; a success clears its claim, a failure holds the next one off
    /// for the backoff, and a refresh that panics still releases its claim, as failed.
    #[tokio::test]
    async fn a_refresh_claim_is_released_however_the_refresh_ends() {
        let state = crate::market::tests::support::state_with_empty_readers(None).await;
        let readers = &state.readers;
        const KEY: &str = "refresh-claim";
        let hour = Duration::from_secs(3600);
        let first = readers.begin_refresh(KEY, hour).expect("claimed");
        assert!(
            readers.begin_refresh(KEY, Duration::ZERO).is_none(),
            "one refresh at a time"
        );
        readers.end_refresh(KEY, first, true);
        let second = readers
            .begin_refresh(KEY, hour)
            .expect("a success clears the claim");
        readers.end_refresh(KEY, second, false);
        assert!(
            readers.begin_refresh(KEY, hour).is_none(),
            "a failure holds the next one off"
        );
        let third = readers.begin_refresh(KEY, Duration::ZERO).expect("claimed");
        let held = Arc::clone(&state);
        let refresh = tokio::spawn(async move {
            let _claim = RefreshClaim::new(held, KEY.to_string(), third);
            panic!("a row that does not decode");
        });
        assert!(refresh.await.is_err());
        assert!(
            readers.begin_refresh(KEY, hour).is_none(),
            "released as failed, not held for good"
        );
        assert!(readers.begin_refresh(KEY, Duration::ZERO).is_some());
    }

    /// A claim that outlives a reset releases nothing: whether its refresh ends well or not, the
    /// newer claim keeps the key, so two reads never run at once and a healthy one is not failed.
    #[tokio::test]
    async fn a_claim_outliving_a_reset_leaves_the_newer_claim_alone() {
        let state = crate::market::tests::support::state_with_empty_readers(None).await;
        let readers = &state.readers;
        const KEY: &str = "refresh-reset";
        let old = readers.begin_refresh(KEY, Duration::ZERO).expect("claimed");
        readers.clear();
        let new = readers
            .begin_refresh(KEY, Duration::ZERO)
            .expect("a reset frees the key");
        readers.end_refresh(KEY, old, false);
        assert!(
            readers.begin_refresh(KEY, Duration::ZERO).is_none(),
            "the old claim's failure did not release the newer one"
        );
        readers.end_refresh(KEY, old, true);
        assert!(
            readers.begin_refresh(KEY, Duration::ZERO).is_none(),
            "nor did its success"
        );
        readers.end_refresh(KEY, new, false);
        assert!(
            readers
                .begin_refresh(KEY, Duration::from_secs(3600))
                .is_none(),
            "the newer claim's own failure still backs off"
        );
    }

    /// A kept answer ages from the read that produced it: handing the same answer out again (a
    /// cache hit) does not extend its hour, and only a new read starts it over.
    #[test]
    fn a_kept_answer_ages_from_its_read_not_from_its_reuse() {
        let caches = Caches::default();
        const KEY: &str = "kept";
        let hour = Duration::from_secs(3600);
        let read = Arc::new(json!(["read at 12:00"]));
        caches.keep(KEY, Arc::clone(&read));
        caches.backdate_kept(KEY, Duration::from_secs(50 * 60));
        caches.keep(KEY, Arc::clone(&read));
        assert_eq!(caches.kept_within(KEY, hour), Some(Arc::clone(&read)));
        caches.backdate_kept(KEY, Duration::from_secs(11 * 60));
        assert_eq!(
            caches.kept_within(KEY, hour),
            None,
            "reusing the read at 12:50 did not extend it past 13:00"
        );
        let again = Arc::new(json!(["read at 13:01"]));
        caches.keep(KEY, Arc::clone(&again));
        assert_eq!(caches.kept_within(KEY, hour), Some(again));
    }
}
