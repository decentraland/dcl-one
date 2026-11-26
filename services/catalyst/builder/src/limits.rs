//! The builder's request budgets, one token bucket per key
//! (`shared::limits::RateLimiter`), kept in process memory and spent by the
//! node's clock (`State.now`).
//!
//! - Every request from a source address ([`source`]) spends from that
//!   source's read (`GET`, `HEAD`) or write budget before anything else
//!   ([`per_source`]); a preflight never reaches it.
//! - A verified signer spends from its wallet's read or write budget right
//!   after its signature verifies, before the body or the database
//!   ([`spend_wallet`]).
//! - A listing whose rows a request can't bound spends one more read token
//!   per [`ROWS_PER_TOKEN`] rows it answered, from both, once answered
//!   ([`charge_rows`]): a bucket may run into debt that later requests wait out.
//!   An item listing pays for its rows and per [`BYTES_PER_TOKEN`] of their
//!   documents before it loads them, and answers at most [`LISTING_BYTES`]
//!   of documents ([`Listing::admit`]).
//!   `GET /v1/addresses` and the public collection search spend
//!   [`ADDRESSES_TOKENS`] more ([`charge`]).
//! - The review routes also spend from a per-wallet budget of their own
//!   ([`spend_review`]); the newsletter from a per-source
//!   ([`spend_newsletter_source`]), per-email and node-wide
//!   ([`spend_newsletter_email`]) one.
//!
//! - An item or collection save spends one more write token per
//!   [`BYTES_PER_WRITE_TOKEN`] of its body and its KiB of the wallet's and
//!   its upload source's day of documents ([`spend_document`]).
//!
//! A spent budget answers 429 with `Retry-After`, the seconds until it
//! holds a token again. Apart from the budgets, a wallet's uploads may hold
//! at most [`WALLET_BYTES`] and [`WALLET_FILES`] that no published item
//! names, all wallets' together [`NODE_BYTES`] and [`NODE_FILES`]; an upload
//! source ([`upload_source`]) may send at most [`SOURCE_UPLOAD_MIB`] and
//! [`SOURCE_UPLOAD_FILES`] a day, at [`NEAR_BOUND_PRICE`] once the node's
//! unpublished uploads hold half its bound, and a wallet
//! [`WALLET_UPLOAD_FILES`] ([`within_storage`], [`charge_upload`]). A
//! wallet's drafts are bounded by [`Documents`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use futures::TryStreamExt;

use crate::SharedState;
use crate::http::ApiError;
use crate::shared::client_ip::{client_ip, is_internal, source_key};
use crate::shared::limits::RateLimiter;

const MINUTE: Duration = Duration::from_secs(60);
const HOUR: Duration = Duration::from_secs(3_600);
const DAY: Duration = Duration::from_secs(86_400);

/// Reads of one source address a minute.
pub const SOURCE_READS: u32 = 3_000;
/// Writes of one source address a minute.
pub const SOURCE_WRITES: u32 = 2_400;
/// Reads of one signer a minute.
pub const WALLET_READS: u32 = 2_400;
/// Writes of one signer a minute.
pub const WALLET_WRITES: u32 = 1_200;
/// Review writes (retry, appeal, comment) of one signer a minute, as builder-server.
pub const REVIEW_WRITES: u32 = 30;
/// Review timeline reads of one signer a minute, as builder-server.
pub const REVIEW_READS: u32 = 120;
/// Newsletter subscriptions from one source address an hour.
pub const NEWSLETTER_PER_SOURCE: u32 = 10;
/// Newsletter subscriptions of one email a day.
pub const NEWSLETTER_PER_EMAIL: u32 = 3;
/// Newsletter subscriptions through the node an hour.
pub const NEWSLETTER_PER_NODE: u32 = 1_000;
/// Read tokens `GET /v1/addresses` and the public collection search spend
/// beyond their first: their query takes some 50 times the database time of
/// a common read.
pub const ADDRESSES_TOKENS: u32 = 50;
/// Rows of an unbounded listing one read token pays for.
pub const ROWS_PER_TOKEN: usize = 20;
/// Bytes of item documents one read token pays for.
pub const BYTES_PER_TOKEN: i64 = 64 * 1024;
/// Bytes of a saved document one more write token pays for.
pub const BYTES_PER_WRITE_TOKEN: usize = 16 * 1024;
/// Bytes of item documents one listing answers: three times builder-server's
/// largest collection (12,665 items, 19.8 MB).
pub const LISTING_BYTES: i64 = 64 * 1024 * 1024;
/// The bytes one wallet's uploads may hold in the store while no published
/// item names them.
pub const WALLET_BYTES: u64 = 5_000_000_000;
/// Upload records one wallet's uploads may hold while no published item
/// names them: three times builder-server's largest creator's files.
pub const WALLET_FILES: u64 = 100_000;
/// The bytes every wallet's uploads together may hold while no published
/// item names them (`BUILDER_UNPUBLISHED_MAX_BYTES`).
pub const NODE_BYTES: u64 = 1_000_000_000_000;
/// The upload records every wallet's uploads together may hold while no
/// published item names them (`BUILDER_UNPUBLISHED_MAX_FILES`): 35 times
/// the files builder-server's items name.
pub const NODE_FILES: u64 = 10_000_000;
/// MiB of item files and videos one source address may send a day: four
/// wallets' unpublished bound.
pub const SOURCE_UPLOAD_MIB: u32 = 20 * 1024;
/// What each byte and file an address uploads costs its day once every
/// wallet's unpublished uploads hold half the node's bound.
pub const NEAR_BOUND_PRICE: u32 = 10;
/// Files one source address may upload a day: twice a wallet's.
pub const SOURCE_UPLOAD_FILES: u32 = 100_000;
/// Files one wallet may upload a day: three times builder-server's busiest
/// wallet-day (16,182 distinct files named by 5,394 items).
pub const WALLET_UPLOAD_FILES: u32 = 50_000;
/// Items one wallet may own that no publication names: twice the most
/// builder-server's busiest wallet ever saved (12,669).
pub const WALLET_ITEMS: u64 = 25_000;
/// Collections one wallet may own unpublished: three times the most
/// builder-server's busiest wallet holds (1,542), and the drafts listing's cap.
pub const WALLET_COLLECTIONS: u64 = 5_000;
/// Stored bytes of one wallet's unpublished item documents: 20 times all the
/// documents of builder-server's largest wallet (11.3 MB).
pub const WALLET_DOCUMENT_BYTES: u64 = 256 * 1024 * 1024;
/// KiB of item and collection saves one source address may send a day.
pub const SOURCE_DOCUMENT_KIB: u32 = 2 * 1024 * 1024;
/// KiB of item and collection saves one wallet may send a day.
pub const WALLET_DOCUMENT_KIB: u32 = 1024 * 1024;
/// The KiB a save costs of its day at the least, however small.
pub const DOCUMENT_MIN_KIB: u32 = 4;
/// How long a count of the unpublished uploads answers.
pub const UNPUBLISHED_KEPT: Duration = MINUTE;
/// The prefix of an IPv6 address that names one upload source.
const UPLOAD_SOURCE_V6_BITS: u32 = 48;
const MIB: u64 = 1024 * 1024;
/// Keys each budget keeps; past it, the least spent are forgotten.
pub(crate) const KEYS: usize = 50_000;
/// The longest the count of the unpublished uploads may take before every
/// record counts.
pub const UNPUBLISHED_COUNT_TIMEOUT: Duration = Duration::from_secs(50);

/// Every budget, each a pub field so a test can set its own.
pub struct Limits {
    epoch: (Instant, DateTime<Utc>),
    pub source_reads: RateLimiter,
    pub source_writes: RateLimiter,
    pub wallet_reads: RateLimiter,
    pub wallet_writes: RateLimiter,
    pub review_writes: RateLimiter,
    pub review_reads: RateLimiter,
    pub newsletter_sources: RateLimiter,
    pub newsletter_emails: RateLimiter,
    pub newsletter_node: RateLimiter,
    /// [`WALLET_BYTES`], or a test's own.
    pub wallet_bytes: u64,
    /// [`WALLET_FILES`], or a test's own.
    pub wallet_files: u64,
    /// [`NODE_BYTES`], or `BUILDER_UNPUBLISHED_MAX_BYTES`.
    pub node_bytes: u64,
    /// [`NODE_FILES`], or `BUILDER_UNPUBLISHED_MAX_FILES`.
    pub node_files: u64,
    /// [`SOURCE_UPLOAD_MIB`] a day, or a test's own.
    pub source_upload_mib: RateLimiter,
    /// [`SOURCE_UPLOAD_FILES`] a day, or a test's own.
    pub source_upload_files: RateLimiter,
    /// [`WALLET_UPLOAD_FILES`] a day, or a test's own.
    pub wallet_upload_files: RateLimiter,
    /// [`SOURCE_DOCUMENT_KIB`] a day, or a test's own.
    pub source_document_kib: RateLimiter,
    /// [`WALLET_DOCUMENT_KIB`] a day, or a test's own.
    pub wallet_document_kib: RateLimiter,
    /// [`LISTING_BYTES`], or a test's own.
    pub listing_bytes: i64,
    tallies: Tallies,
}

/// What one wallet's unpublished drafts may hold: [`WALLET_ITEMS`],
/// [`WALLET_COLLECTIONS`] and [`WALLET_DOCUMENT_BYTES`], or a test's own.
#[derive(Clone, Copy, Debug)]
pub struct Documents {
    pub items: u64,
    pub collections: u64,
    pub bytes: u64,
}

impl Default for Documents {
    fn default() -> Self {
        Self {
            items: WALLET_ITEMS,
            collections: WALLET_COLLECTIONS,
            bytes: WALLET_DOCUMENT_BYTES,
        }
    }
}

/// Bytes and files of upload records.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Held<T> {
    pub bytes: T,
    pub files: T,
}

impl Held<i64> {
    fn plus(self, other: Held<i64>) -> Held<i64> {
        Held {
            bytes: self.bytes.saturating_add(other.bytes),
            files: self.files.saturating_add(other.files),
        }
    }

    fn minus(self, other: Held<i64>) -> Held<i64> {
        Held {
            bytes: self.bytes.saturating_sub(other.bytes).max(0),
            files: self.files.saturating_sub(other.files).max(0),
        }
    }
}

/// What the node's uploads held when last counted ([`Tallies`]).
struct Tally {
    at: Instant,
    generation: u64,
    node: Held<i64>,
    wallets: HashMap<String, (Held<i64>, Held<i64>)>,
}

/// The node's count of unpublished uploads: one count at a time, kept
/// [`UNPUBLISHED_KEPT`], with what this process stored since it began.
#[derive(Default)]
struct Tallies {
    last: Mutex<Option<Arc<Tally>>>,
    added: Mutex<Held<i64>>,
    counting: tokio::sync::Mutex<()>,
}

/// `per` tokens every `window`, refilled continuously.
pub fn budget(per: u32, window: Duration) -> RateLimiter {
    RateLimiter::new(per, window).with_max_buckets(KEYS)
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            epoch: (Instant::now(), Utc::now()),
            source_reads: budget(SOURCE_READS, MINUTE),
            source_writes: budget(SOURCE_WRITES, MINUTE),
            wallet_reads: budget(WALLET_READS, MINUTE),
            wallet_writes: budget(WALLET_WRITES, MINUTE),
            review_writes: budget(REVIEW_WRITES, MINUTE),
            review_reads: budget(REVIEW_READS, MINUTE),
            newsletter_sources: budget(NEWSLETTER_PER_SOURCE, HOUR),
            newsletter_emails: budget(NEWSLETTER_PER_EMAIL, DAY),
            newsletter_node: budget(NEWSLETTER_PER_NODE, HOUR),
            wallet_bytes: WALLET_BYTES,
            wallet_files: WALLET_FILES,
            node_bytes: NODE_BYTES,
            node_files: NODE_FILES,
            source_upload_mib: budget(SOURCE_UPLOAD_MIB, DAY),
            source_upload_files: budget(SOURCE_UPLOAD_FILES, DAY),
            wallet_upload_files: budget(WALLET_UPLOAD_FILES, DAY),
            source_document_kib: budget(SOURCE_DOCUMENT_KIB, DAY),
            wallet_document_kib: budget(WALLET_DOCUMENT_KIB, DAY),
            listing_bytes: LISTING_BYTES,
            tallies: Tallies::default(),
        }
    }
}

impl Limits {
    /// A million tokens a minute in every budget.
    #[cfg(test)]
    pub fn unbounded() -> Self {
        let all = || budget(1_000_000, MINUTE);
        Self {
            epoch: (Instant::now(), Utc::now()),
            source_reads: all(),
            source_writes: all(),
            wallet_reads: all(),
            wallet_writes: all(),
            review_writes: all(),
            review_reads: all(),
            newsletter_sources: all(),
            newsletter_emails: all(),
            newsletter_node: all(),
            wallet_bytes: u64::MAX,
            wallet_files: u64::MAX,
            node_bytes: u64::MAX,
            node_files: u64::MAX,
            source_upload_mib: all(),
            source_upload_files: all(),
            wallet_upload_files: all(),
            source_document_kib: all(),
            wallet_document_kib: all(),
            listing_bytes: LISTING_BYTES,
            tallies: Tallies::default(),
        }
    }

    fn instant(&self, now: DateTime<Utc>) -> Instant {
        let (at, wall) = self.epoch;
        match (now - wall).to_std() {
            Ok(ahead) => at + ahead,
            Err(_) => (wall - now)
                .to_std()
                .ok()
                .and_then(|behind| at.checked_sub(behind))
                .unwrap_or(at),
        }
    }

    fn take(&self, budget: &RateLimiter, key: &str, now: DateTime<Utc>) -> Result<(), ApiError> {
        self.take_cost(budget, key, 1, now)
    }

    fn take_cost(
        &self,
        budget: &RateLimiter,
        key: &str,
        cost: u32,
        now: DateTime<Utc>,
    ) -> Result<(), ApiError> {
        budget
            .take_at(key, cost, self.instant(now))
            .map_err(ApiError::Limited)
    }
}

/// The source address a request came from, by [`source_key`]; `None` when
/// it names no single client: none at all, or an internal address, which is
/// what every request carries behind a gateway the node doesn't trust
/// (`HTTP_TRUSTED_PROXY_CIDRS`).
pub fn source(headers: &HeaderMap) -> Option<String> {
    let ip = client_ip(headers);
    let parsed: std::net::IpAddr = ip.parse().ok()?;
    (!is_internal(parsed)).then(|| source_key(&ip))
}

fn reads(method: &Method) -> bool {
    method == Method::GET || method == Method::HEAD
}

/// The source's read or write budget, before the route runs.
pub async fn per_source(State(state): State<SharedState>, req: Request, next: Next) -> Response {
    if let Some(source) = source(req.headers()) {
        let limits = &state.limits;
        let budget = if reads(req.method()) {
            &limits.source_reads
        } else {
            &limits.source_writes
        };
        if let Err(refused) = limits.take(budget, &source, state.now()) {
            return refused.into_response();
        }
    }
    next.run(req).await
}

/// A verified signer's read or write budget.
pub fn spend_wallet(state: &SharedState, signer: &str, method: &Method) -> Result<(), ApiError> {
    let limits = &state.limits;
    let budget = if reads(method) {
        &limits.wallet_reads
    } else {
        &limits.wallet_writes
    };
    limits.take(budget, signer, state.now())
}

/// The review routes' own budget for `signer`: its timeline reads or its writes.
pub fn spend_review(state: &SharedState, signer: &str, read: bool) -> Result<(), ApiError> {
    let limits = &state.limits;
    let budget = if read {
        &limits.review_reads
    } else {
        &limits.review_writes
    };
    limits.take(budget, signer, state.now())
}

/// A save of `bytes` of item or collection documents by `signer`: one more
/// write token per [`BYTES_PER_WRITE_TOKEN`] from the signer's and the source's
/// write budgets, and its KiB, at least [`DOCUMENT_MIN_KIB`], from the
/// wallet's [`WALLET_DOCUMENT_KIB`] and its upload source's
/// [`SOURCE_DOCUMENT_KIB`] a day; 429 when any can't pay, spending none.
pub fn spend_document(
    state: &SharedState,
    headers: &HeaderMap,
    signer: &str,
    bytes: usize,
) -> Result<(), ApiError> {
    let limits = &state.limits;
    let tokens = u32::try_from(bytes / BYTES_PER_WRITE_TOKEN).unwrap_or(u32::MAX);
    let kib = u32::try_from(bytes.div_ceil(1024))
        .unwrap_or(u32::MAX)
        .max(DOCUMENT_MIN_KIB);
    let (source, site) = (source(headers), upload_source(headers));
    let mut costs = vec![
        (&limits.wallet_writes, signer.to_string(), tokens),
        (&limits.wallet_document_kib, signer.to_string(), kib),
    ];
    if let Some(source) = source {
        costs.push((&limits.source_writes, source, tokens));
    }
    if let Some(site) = site {
        costs.push((&limits.source_document_kib, site, kib));
    }
    let now = state.now();
    for (n, (budget, key, cost)) in costs.iter().enumerate() {
        if let Err(refused) = limits.take_cost(budget, key, *cost, now) {
            for (budget, key, cost) in &costs[..n] {
                budget.refund(key, *cost);
            }
            return Err(refused);
        }
    }
    Ok(())
}

/// A subscription's budget of its source address, before its body is read.
pub fn spend_newsletter_source(state: &SharedState, source: &str) -> Result<(), ApiError> {
    let limits = &state.limits;
    limits.take(&limits.newsletter_sources, source, state.now())
}

/// A valid subscription's budgets of its email and of the node.
pub fn spend_newsletter_email(state: &SharedState, email: &str) -> Result<(), ApiError> {
    let limits = &state.limits;
    let now = state.now();
    limits.take(&limits.newsletter_emails, email, now)?;
    limits.take(&limits.newsletter_node, "node", now)
}

/// The source an upload came from: [`source`], with an IPv6 address
/// widened to its /48, the block one site is given.
pub fn upload_source(headers: &HeaderMap) -> Option<String> {
    use std::net::{IpAddr, Ipv6Addr};
    let ip: IpAddr = client_ip(headers).parse().ok()?;
    if is_internal(ip) {
        return None;
    }
    match ip {
        IpAddr::V6(v6) if v6.to_ipv4_mapped().is_none() => {
            let mask = u128::MAX << (128 - UPLOAD_SOURCE_V6_BITS);
            Some(format!(
                "{}/{UPLOAD_SOURCE_V6_BITS}",
                Ipv6Addr::from(u128::from(v6) & mask)
            ))
        }
        _ => Some(source_key(&ip.to_string())),
    }
}

/// An upload admitted by [`within_storage`]: who pays for it, at what price
/// per byte and file, and what it sent. When it drops, however the request
/// ended, its bytes, in MiB, and its files go to its source address's day
/// at its price, its files to its wallet's day.
pub struct Upload {
    state: SharedState,
    source: Option<String>,
    wallet: String,
    price: u32,
    received: Arc<AtomicU64>,
    files: u32,
}

impl Upload {
    /// `request` with a body whose bytes this upload pays for.
    pub fn counted(&self, request: Request) -> Request {
        let counter = self.received.clone();
        request.map(|body| {
            axum::body::Body::from_stream(body.into_data_stream().inspect_ok(move |chunk| {
                counter.fetch_add(chunk.len() as u64, Ordering::Relaxed);
            }))
        })
    }

    /// The files it carried: one until told.
    pub fn carried(&mut self, files: u32) {
        self.files = files;
    }
}

impl Drop for Upload {
    fn drop(&mut self) {
        let limits = &self.state.limits;
        let now = limits.instant(self.state.now());
        limits
            .wallet_upload_files
            .charge_at(&self.wallet, self.files, now);
        let Some(source) = &self.source else {
            return;
        };
        let mib =
            u32::try_from(self.received.load(Ordering::Relaxed).div_ceil(MIB)).unwrap_or(u32::MAX);
        limits
            .source_upload_mib
            .charge_at(source, mib.saturating_mul(self.price), now);
        limits
            .source_upload_files
            .charge_at(source, self.files.saturating_mul(self.price), now);
    }
}

/// Refuses an upload by `wallet` before its body is read. 429 while its
/// source address ([`upload_source`]) has sent its day of
/// [`SOURCE_UPLOAD_MIB`] or [`SOURCE_UPLOAD_FILES`], or the wallet its
/// [`WALLET_UPLOAD_FILES`] (what it sends is charged when the [`Upload`]
/// drops, at [`NEAR_BOUND_PRICE`] for the source once every
/// wallet's unpublished uploads hold half the node's bound); 413 when the
/// wallet's unpublished uploads hold its bound of files, or could take its
/// bound of bytes with this body ([`WALLET_FILES`], [`WALLET_BYTES`]); 507
/// when every wallet's together hold the node's ([`NODE_FILES`],
/// [`NODE_BYTES`]). The body's declared length, else `most`, the route's
/// largest body, counts as stored; the route refused a longer one first.
/// The unpublished uploads are counted at most once every [`UNPUBLISHED_KEPT`], by one count at a time and never
/// on the request's way unless none is kept or the wallet passes its bound
/// by the sum of its records; meanwhile a wallet's count moves by what its
/// records gained or lost, the node's by what this process stored.
pub async fn within_storage(
    state: &SharedState,
    wallet: &str,
    headers: &HeaderMap,
    most: u64,
) -> Result<Upload, ApiError> {
    let incoming = declared_length(headers).unwrap_or(most);
    let limits = &state.limits;
    let node_bound = Held {
        bytes: limits.node_bytes,
        files: limits.node_files,
    };
    let wallet_bound = Held {
        bytes: limits.wallet_bytes,
        files: limits.wallet_files,
    };
    let tally = limits.tallies.latest(state).await?;
    let node = tally.node.plus(*lock(&limits.tallies.added));
    let price = if near(node, node_bound) {
        NEAR_BOUND_PRICE
    } else {
        1
    };
    let source = upload_source(headers);
    let now = state.now();
    if let Some(source) = &source {
        limits.probe(&limits.source_upload_mib, source, price, now)?;
        limits.probe(&limits.source_upload_files, source, price, now)?;
    }
    limits.probe(&limits.wallet_upload_files, wallet, 1, now)?;
    let records = crate::access::uploaded(state.items.pool(), Some(wallet)).await?;
    if past(records, incoming, wallet_bound).is_some() {
        let mut tally = if limits.fresh(&tally, now) {
            tally
        } else {
            limits.tallies.fresh(state).await?
        };
        if !tally.wallets.contains_key(wallet) {
            tally = limits.tallies.after(state, tally.generation).await?;
        }
        let held = tally
            .wallets
            .get(wallet)
            .map_or(records, |(held, counted)| {
                held.plus(records).minus(*counted)
            });
        if let Some(over) = past(held, incoming, wallet_bound) {
            return Err(
                ApiError::http(413, "The wallet's unpublished files reached their limit")
                    .with_data(over),
            );
        }
    }
    if let Some(over) = past(node, incoming, node_bound) {
        return Err(
            ApiError::http(507, "The node's unpublished files reached their limit").with_data(over),
        );
    }
    Ok(Upload {
        state: state.clone(),
        source,
        wallet: wallet.to_string(),
        price,
        received: Arc::default(),
        files: 1,
    })
}

/// The body length a request declares in `Content-Length`.
pub fn declared_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok()?.parse().ok())
}

/// The bound `held` passes with `incoming` more bytes, as `{maxBytes}` or
/// `{maxFiles}`; `None` within it.
fn past(held: Held<i64>, incoming: u64, bound: Held<u64>) -> Option<serde_json::Value> {
    let (bytes, files) = (
        u64::try_from(held.bytes).unwrap_or(0),
        u64::try_from(held.files).unwrap_or(0),
    );
    if bytes.saturating_add(incoming) > bound.bytes {
        Some(serde_json::json!({ "maxBytes": bound.bytes }))
    } else if files >= bound.files {
        Some(serde_json::json!({ "maxFiles": bound.files }))
    } else {
        None
    }
}

/// Whether `held` reaches half of `bound` in bytes or files.
fn near(held: Held<i64>, bound: Held<u64>) -> bool {
    i128::from(held.bytes) * 2 >= i128::from(bound.bytes)
        || i128::from(held.files) * 2 >= i128::from(bound.files)
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl Limits {
    /// 429 unless `budget` holds `cost` for `key` now; spends nothing.
    fn probe(
        &self,
        budget: &RateLimiter,
        key: &str,
        cost: u32,
        now: DateTime<Utc>,
    ) -> Result<(), ApiError> {
        self.take_cost(budget, key, cost, now)?;
        budget.refund(key, cost);
        Ok(())
    }

    fn fresh(&self, tally: &Tally, now: DateTime<Utc>) -> bool {
        self.instant(now).saturating_duration_since(tally.at) < UNPUBLISHED_KEPT
    }
}

impl Tallies {
    /// The last count, counting when none is kept; a count past
    /// [`UNPUBLISHED_KEPT`] still answers while a new one runs in a task of
    /// its own.
    async fn latest(&self, state: &SharedState) -> Result<Arc<Tally>, ApiError> {
        let Some(last) = lock(&self.last).clone() else {
            return self.fresh(state).await;
        };
        if !state.limits.fresh(&last, state.now()) && self.counting.try_lock().is_ok() {
            let state = state.clone();
            tokio::spawn(async move {
                if let Err(error) = state.limits.tallies.fresh(&state).await {
                    tracing::warn!(%error, "builder: counting the unpublished uploads failed");
                }
            });
        }
        Ok(last)
    }

    /// A count younger than [`UNPUBLISHED_KEPT`], waiting for the one
    /// running or making one.
    async fn fresh(&self, state: &SharedState) -> Result<Arc<Tally>, ApiError> {
        let _counting = self.counting.lock().await;
        if let Some(last) = lock(&self.last).clone()
            && state.limits.fresh(&last, state.now())
        {
            return Ok(last);
        }
        self.count(state).await
    }

    /// A count made after the count `generation`, waiting for the one
    /// running or making one.
    async fn after(&self, state: &SharedState, generation: u64) -> Result<Arc<Tally>, ApiError> {
        let _counting = self.counting.lock().await;
        if let Some(last) = lock(&self.last).clone()
            && last.generation > generation
        {
            return Ok(last);
        }
        self.count(state).await
    }

    async fn count(&self, state: &SharedState) -> Result<Arc<Tally>, ApiError> {
        let limits = &state.limits;
        let generation = lock(&self.last)
            .as_ref()
            .map_or(0, |last| last.generation + 1);
        let at = limits.instant(state.now());
        let before = *lock(&self.added);
        let near_wallet = Held {
            bytes: limits
                .wallet_bytes
                .saturating_sub(crate::items::video::BODY_LIMIT as u64),
            files: limits
                .wallet_files
                .saturating_sub(crate::items::upload::MAX_FILES as u64),
        };
        let half_node = Held {
            bytes: limits.node_bytes / 2,
            files: limits.node_files / 2,
        };
        let (node, wallets) =
            crate::access::tally_uploads(state.items.pool(), near_wallet, half_node).await?;
        let tally = Arc::new(Tally {
            at,
            generation,
            node,
            wallets,
        });
        *lock(&self.last) = Some(tally.clone());
        let mut added = lock(&self.added);
        *added = added.minus(before);
        drop(added);
        Ok(tally)
    }
}

/// `bytes` in `files` this process stored as upload records, which count
/// toward the node's bound until the next count.
pub fn stored(state: &SharedState, bytes: u64, files: u64) {
    let mut added = lock(&state.limits.tallies.added);
    *added = added.plus(Held {
        bytes: i64::try_from(bytes).unwrap_or(i64::MAX),
        files: i64::try_from(files).unwrap_or(i64::MAX),
    });
}

/// The reader of a signed item listing.
pub struct Listing<'a> {
    pub state: &'a SharedState,
    pub headers: &'a HeaderMap,
    pub signer: &'a str,
}

impl Listing<'_> {
    /// Before `rows` holding `bytes` of item documents load: 413 past the
    /// node's [`LISTING_BYTES`], else their read tokens from the signer's and
    /// the source's read budgets, 429 when either can't pay them now.
    pub fn admit(&self, rows: usize, bytes: i64) -> Result<(), ApiError> {
        let limits = &self.state.limits;
        if bytes > limits.listing_bytes {
            return Err(
                ApiError::http(413, "The listing is too large, ask for it in pages")
                    .with_data(serde_json::json!({ "maxBytes": limits.listing_bytes })),
            );
        }
        let tokens = rows / ROWS_PER_TOKEN + usize::try_from(bytes / BYTES_PER_TOKEN).unwrap_or(0);
        let tokens = u32::try_from(tokens).unwrap_or(u32::MAX);
        if tokens == 0 {
            return Ok(());
        }
        let now = self.state.now();
        limits.take_cost(&limits.wallet_reads, self.signer, tokens, now)?;
        if let Some(source) = source(self.headers)
            && let Err(refused) = limits.take_cost(&limits.source_reads, &source, tokens, now)
        {
            limits.wallet_reads.refund(self.signer, tokens);
            return Err(refused);
        }
        Ok(())
    }
}

/// `rows` answered by an unbounded listing, charged to the signer's and the
/// source's read budgets.
pub fn charge_rows(state: &SharedState, headers: &HeaderMap, signer: Option<&str>, rows: usize) {
    charge(
        state,
        headers,
        signer,
        u32::try_from(rows / ROWS_PER_TOKEN).unwrap_or(u32::MAX),
    );
}

/// `tokens` more spent by a read already answered, from the signer's and
/// the source's read budgets.
pub fn charge(state: &SharedState, headers: &HeaderMap, signer: Option<&str>, tokens: u32) {
    if tokens == 0 {
        return;
    }
    let limits = &state.limits;
    let now = limits.instant(state.now());
    if let Some(signer) = signer {
        limits.wallet_reads.charge_at(signer, tokens, now);
    }
    if let Some(source) = source(headers) {
        limits.source_reads.charge_at(&source, tokens, now);
    }
}

#[cfg(test)]
mod tally_tests {
    use std::sync::Arc;

    use super::{Held, lock};

    /// A count that fails keeps what this process stored since the last
    /// one, so the node's figure still holds it.
    #[tokio::test]
    async fn a_failed_count_keeps_what_this_process_stored() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let state = Arc::new(
            crate::State::new(
                pool.clone(),
                crate::tests::support::temp_storage().await,
                Arc::default(),
                dcl_http::Outbound::for_tests(),
                dcl_http::Outbound::for_tests(),
                None,
                None,
                std::sync::Arc::default(),
                crate::Settings::default(),
            )
            .unwrap(),
        );
        let tallies = &state.limits.tallies;
        tallies.count(&state).await.unwrap();
        super::stored(&state, 10, 1);
        sqlx::query("ALTER TABLE builder.uploaded_contents RENAME TO uploaded_contents_away")
            .execute(&pool)
            .await
            .unwrap();
        assert!(tallies.count(&state).await.is_err());
        let kept = *lock(&tallies.added);
        sqlx::query("ALTER TABLE builder.uploaded_contents_away RENAME TO uploaded_contents")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            kept,
            Held {
                bytes: 10,
                files: 1
            }
        );
        tallies.count(&state).await.unwrap();
        assert_eq!(*lock(&tallies.added), Held::default());
    }
}
