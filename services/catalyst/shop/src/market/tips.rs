//! The events-notifier tip forward, `POST /events-notifier/forward-blockchain`
//! (`{type: "tip-received", transactionHash}`, ADR-44 signed by the tipper),
//! as the auth dApp sends it after a MANA meta-transaction. Where the old
//! service read the receipt over RPC, the claim is checked against this node's
//! own Polygon MANA transfer index (`shop.mana_transfer`, the `mana_transfers`
//! chain walk): the transaction must hold exactly one published Transfer whose
//! `from` is the signer, and its `to` is told `tip_received`. The forward
//! arrives before the index reaches the transaction, so it is kept as a
//! claim (`shop.tip_claim`) and resolved then, by the request itself or the
//! `tip-notifications` job; a claim the index never confirms expires after
//! an hour. The table is bounded without refusing fresh claims: past a
//! sender's [`MAX_PENDING_PER_SENDER`] its own oldest unmatched claim makes
//! room, and past `MARKETPLACE_TIP_MAX_PENDING` the oldest unmatched claim
//! older than [`EVICTABLE_AFTER_SECS`] does; only when none is that old is
//! the forward answered with a retryable 429 and `Retry-After` (a forward
//! the index already holds is settled unstored instead). Each signer may
//! forward [`FORWARDS_PER_MINUTE`] a minute; before the signature is checked
//! a client source (a trusted proxy's `X-Real-IP`, or a public TCP peer; IPv6
//! by /64) may send [`PREAUTH_PER_MINUTE`], an IPv6 /48
//! [`PREAUTH_PER_MINUTE_PER_48`]; a private or loopback peer (an unlisted
//! gateway) is not limited before it.
//! A transfer below the minimum amount, or mined longer than the maximum age
//! before its claim, is rejected (`TipSettings`). The notification id is a
//! uuid v5 of the transaction hash.

use std::collections::{BTreeMap, HashMap};
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use alloy_primitives::U256;
use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{ConnectInfo, Extension, OriginalUri, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use chrono::NaiveDateTime;
use parking_lot::Mutex;
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::SharedState;
use super::chain_walk::{self, ChainNet, INDEXER_MANA};
use crate::jobs::{Jobs, PeriodicCfg};
use crate::shared::identity::Address;
use crate::shared::signed_fetch::{self, PayloadPolicy, SignedPath};
use crate::social::notifications::store::{NewNotification, insert_notifications};

pub const KIND: &str = "tip_received";
const SUBTYPE: &str = "tip-received";
const NET: ChainNet = ChainNet::Matic;
/// How long a claim waits for its transfer (the old service's window).
pub const CLAIM_TTL_SECS: i64 = 3600;
/// The age before another sender's flood may evict an unmatched claim: a
/// mined tip is indexed well within it.
pub const EVICTABLE_AFTER_SECS: i64 = 300;
/// Forwards one signer may send a minute.
pub const FORWARDS_PER_MINUTE: u32 = 30;
/// Unauthenticated requests one client source (IPv6 by /64) may send a minute.
pub const PREAUTH_PER_MINUTE: u32 = 120;
/// Unauthenticated requests every /64 of one IPv6 /48 may send a minute together.
pub const PREAUTH_PER_MINUTE_PER_48: u32 = 480;
/// Limiter keys kept; past it the least recently used goes.
const LIMITER_KEYS: usize = 10_000;
pub const MAX_PENDING_PER_SENDER: i64 = 20;
/// Claims one resolve pass settles at most.
const RESOLVE_LIMIT: i64 = 500;
const POLL_SECS: u64 = 15;

pub const DEFAULT_MIN_AMOUNT_WEI: &str = "1000000000000000000";
pub const DEFAULT_MAX_AGE_SECS: u64 = 86_400;
pub const DEFAULT_MAX_PENDING: u64 = 2_000;

/// What a tip must be to notify, and how many claims may wait at once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TipSettings {
    /// `MARKETPLACE_TIP_MIN_AMOUNT_WEI`: the smallest transfer that notifies.
    pub min_amount_wei: U256,
    /// `MARKETPLACE_TIP_MAX_AGE_SECS`: the oldest transfer (block time before the claim) that notifies.
    pub max_age_secs: i64,
    /// `MARKETPLACE_TIP_MAX_PENDING`: pending claims across every sender.
    pub max_pending: i64,
    /// `HTTP_TRUSTED_PROXY_CIDRS`: peers whose `X-Real-IP` names the client.
    pub trusted_proxy_cidrs: Vec<ipnet::IpNet>,
}

impl Default for TipSettings {
    fn default() -> Self {
        Self::parse(
            DEFAULT_MIN_AMOUNT_WEI,
            DEFAULT_MAX_AGE_SECS,
            DEFAULT_MAX_PENDING,
        )
        .expect("the defaults parse")
    }
}

impl TipSettings {
    pub fn parse(
        min_amount_wei: &str,
        max_age_secs: u64,
        max_pending: u64,
    ) -> anyhow::Result<Self> {
        let min_amount_wei = U256::from_str_radix(min_amount_wei.trim(), 10).map_err(|e| {
            anyhow::anyhow!("MARKETPLACE_TIP_MIN_AMOUNT_WEI is not a wei amount: {e}")
        })?;
        Ok(Self {
            min_amount_wei,
            max_age_secs: i64::try_from(max_age_secs).unwrap_or(i64::MAX),
            max_pending: i64::try_from(max_pending).unwrap_or(i64::MAX),
            trusted_proxy_cidrs: Vec::new(),
        })
    }

    pub fn with_trusted_proxies(mut self, cidrs: Vec<ipnet::IpNet>) -> Self {
        self.trusted_proxy_cidrs = cidrs;
        self
    }
}

pub fn notification_id(transaction_hash: &str) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_OID,
        format!("tip:{transaction_hash}").as_bytes(),
    )
}

pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/forward-blockchain", post(forward))
        .with_state(state)
}

struct Bucket {
    tokens: f64,
    last: Instant,
    /// Its place in the use order.
    used: u64,
}

/// Token buckets, at most `max_keys`: a key is stored only when a request
/// spends from it, and past the cap the least recently used goes, so keys a
/// denied flood invents never displace the buckets it exhausted.
struct ForwardLimiter {
    max_keys: usize,
    inner: Mutex<Buckets>,
}

#[derive(Default)]
struct Buckets {
    clock: u64,
    map: HashMap<String, Bucket>,
    order: BTreeMap<u64, String>,
}

impl ForwardLimiter {
    fn new(max_keys: usize) -> Self {
        Self {
            max_keys: max_keys.max(2),
            inner: Mutex::new(Buckets::default()),
        }
    }

    /// Spends one token of every `(key, budget a minute)`, or none.
    fn allow(&self, keys: &[(String, u32)]) -> bool {
        let now = Instant::now();
        let mut b = self.inner.lock();
        let Buckets { clock, map, order } = &mut *b;
        let refilled = |key: &str, per_minute: u32| {
            let cap = f64::from(per_minute);
            map.get(key).map_or(cap, |b| {
                let elapsed = now.saturating_duration_since(b.last).as_secs_f64();
                (b.tokens + elapsed * cap / 60.0).min(cap)
            })
        };
        let tokens: Vec<f64> = keys.iter().map(|(k, n)| refilled(k, *n)).collect();
        let ok = tokens.iter().all(|t| *t >= 1.0);
        for ((key, _), tokens) in keys.iter().zip(tokens) {
            if !ok && !map.contains_key(key) {
                continue;
            }
            *clock += 1;
            let bucket = map.entry(key.clone()).or_insert(Bucket {
                tokens,
                last: now,
                used: 0,
            });
            order.remove(&bucket.used);
            bucket.used = *clock;
            order.insert(*clock, key.clone());
            if ok {
                bucket.tokens = tokens - 1.0;
                bucket.last = now;
            }
        }
        while map.len() > self.max_keys {
            let Some((_, oldest)) = order.pop_first() else {
                break;
            };
            map.remove(&oldest);
        }
        ok
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.inner.lock().map.len()
    }
}

/// The pre-auth keys of a client address and their budgets a minute.
fn source_keys(ip: &str) -> Vec<(String, u32)> {
    let mut keys = vec![(crate::shared::client_ip::source_key(ip), PREAUTH_PER_MINUTE)];
    if let Ok(IpAddr::V6(v6)) = ip.parse::<IpAddr>()
        && v6.to_ipv4_mapped().is_none()
    {
        let net = Ipv6Addr::from(u128::from(v6) & (u128::MAX << 80));
        keys.push((format!("{net}/48"), PREAUTH_PER_MINUTE_PER_48));
    }
    keys
}

/// Loose, before the signature is checked: per client source and IPv6 /48.
static SOURCE_LIMITER: LazyLock<ForwardLimiter> =
    LazyLock::new(|| ForwardLimiter::new(LIMITER_KEYS));
/// The real budget, per signer.
static SIGNER_LIMITER: LazyLock<ForwardLimiter> =
    LazyLock::new(|| ForwardLimiter::new(LIMITER_KEYS));

fn retry_later(message: &str, secs: i64) -> Response {
    let secs = secs.max(1);
    (
        StatusCode::TOO_MANY_REQUESTS,
        [(header::RETRY_AFTER, secs.to_string())],
        Json(json!({ "error": message, "retryAfter": secs })),
    )
        .into_response()
}

fn error(status: StatusCode, message: impl Into<String>) -> Response {
    (status, Json(json!({ "error": message.into() }))).into_response()
}

/// The old route's schema: exactly `{type, transactionHash}`, the type `tip-received`.
fn parse_body(body: &[u8]) -> Result<String, String> {
    let Ok(Value::Object(obj)) = serde_json::from_slice::<Value>(body) else {
        return Err("Invalid JSON body".into());
    };
    if obj.keys().any(|k| k != "type" && k != "transactionHash") {
        return Err("must NOT have additional properties".into());
    }
    match obj.get("type").and_then(Value::as_str) {
        Some(SUBTYPE) => {}
        Some(other) => return Err(format!("Unsupported event type: {other}")),
        None => return Err("must have required property 'type'".into()),
    }
    match obj.get("transactionHash").and_then(Value::as_str) {
        Some(h)
            if h.len() == 66
                && h.starts_with("0x")
                && h[2..].bytes().all(|b| b.is_ascii_hexdigit()) =>
        {
            Ok(h.to_ascii_lowercase())
        }
        Some(_) => Err("transactionHash must match pattern \"^0x[a-fA-F0-9]{64}$\"".into()),
        None => Err("must have required property 'transactionHash'".into()),
    }
}

async fn forward(
    State(state): State<SharedState>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let peer = peer.map(|Extension(ConnectInfo(p))| p);
    // Only an address naming one client: a gateway peer (HTTP_TRUSTED_PROXY_CIDRS unset) would be one shared bucket.
    let ip = crate::shared::client_ip::distinct_client_ip(
        &headers,
        peer,
        &state.settings.market.tips.trusted_proxy_cidrs,
    );
    if let Some(ip) = ip
        && !SOURCE_LIMITER.allow(&source_keys(&ip))
    {
        return retry_later("rate limit exceeded", 1);
    }
    let signer = match signed_fetch::require_signer_at(
        &headers,
        "post",
        &SignedPath::at(&original, "/forward-blockchain"),
        PayloadPolicy::V6ThenLegacy,
        state.eip1654.as_deref(),
    )
    .await
    {
        Ok(s) => s.into_string().to_ascii_lowercase(),
        Err(e) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": e.to_string(),
                    "message": "This endpoint requires a signed fetch request. See ADR-44.",
                })),
            )
                .into_response();
        }
    };
    if !SIGNER_LIMITER.allow(&[(signer.clone(), FORWARDS_PER_MINUTE)]) {
        return retry_later("rate limit exceeded", 2);
    }
    let tx = match parse_body(&body) {
        Ok(tx) => tx,
        Err(message) => return error(StatusCode::BAD_REQUEST, message),
    };
    match claim(&state.pool, &state.settings.market.tips, &tx, &signer).await {
        Ok(Outcome::Notified) => StatusCode::NO_CONTENT.into_response(),
        Ok(Outcome::Pending) => (StatusCode::ACCEPTED, Json(json!({}))).into_response(),
        Ok(Outcome::Busy { retry_after_secs }) => {
            retry_later("Too many tips waiting for confirmation", retry_after_secs)
        }
        Ok(Outcome::Rejected(why)) => error(StatusCode::BAD_REQUEST, why.message(&tx)),
        Err(e) => {
            tracing::error!(error = %e, tx, "tip forward failed");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Tip could not be processed",
            )
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Notified,
    Pending,
    Rejected(Rejection),
    /// Every pending claim is too young to evict: retry after this long.
    Busy {
        retry_after_secs: i64,
    },
}

/// Why a claim was refused; each is the old route's 400.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    NotSingleTransfer,
    TooSmall,
    TooOld,
    Malformed,
}

impl Rejection {
    fn message(self, tx: &str) -> String {
        match self {
            Rejection::NotSingleTransfer => {
                format!("Transaction is not a single MANA transfer from the signer: {tx}")
            }
            Rejection::TooSmall => format!("Transaction {tx} transfers less than the minimum tip"),
            Rejection::TooOld => format!("Transaction {tx} is too old"),
            Rejection::Malformed => format!("Transaction {tx} could not be read"),
        }
    }
}

/// Records the claim under the sender's lock and resolves it at once if
/// the index already has it. Past a cap, unmatched claims are evicted
/// oldest first (the sender's own at any age, others' only past
/// [`EVICTABLE_AFTER_SECS`]); when nothing may go, an indexed forward is
/// settled unstored and any other is `Busy`.
pub async fn claim(
    pool: &PgPool,
    settings: &TipSettings,
    tx: &str,
    sender: &str,
) -> anyhow::Result<Outcome> {
    let mut txn = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('tip_claim:' || $1))")
        .bind(sender)
        .execute(&mut *txn)
        .await?;
    let (exists, mine, all): (bool, i64, i64) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM shop.tip_claim WHERE transaction_hash = $1 AND sender = $2), \
                (SELECT count(*) FROM shop.tip_claim WHERE sender = $2), \
                (SELECT count(*) FROM shop.tip_claim)",
    )
    .bind(tx)
    .bind(sender)
    .fetch_one(&mut *txn)
    .await?;
    if !exists {
        let mut room = true;
        let mut all = all;
        if mine >= MAX_PENDING_PER_SENDER {
            let need = mine - MAX_PENDING_PER_SENDER + 1;
            let freed = evict(&mut txn, Some(sender), need, 0).await?;
            all -= freed;
            room = freed >= need;
        }
        // The global count is advisory: concurrent senders may pass it together.
        if room && all >= settings.max_pending {
            let need = all - settings.max_pending + 1;
            room = evict(&mut txn, None, need, EVICTABLE_AFTER_SECS).await? >= need;
        }
        if !room {
            let retry_after_secs =
                retry_after(&mut txn, sender, mine >= MAX_PENDING_PER_SENDER).await?;
            txn.rollback().await?;
            return Ok(resolve_unstored(pool, settings, tx, sender)
                .await?
                .unwrap_or(Outcome::Busy { retry_after_secs }));
        }
        sqlx::query(
            "INSERT INTO shop.tip_claim (transaction_hash, sender) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(tx)
        .bind(sender)
        .execute(&mut *txn)
        .await?;
    }
    txn.commit().await?;
    let resolved = resolve(pool, settings, Some((tx, sender))).await?;
    Ok(resolved
        .into_iter()
        .next()
        .map(|(_, o)| o)
        .unwrap_or(Outcome::Pending))
}

/// SQL: claim `c` has no indexed transfer yet (published or not).
fn unmatched() -> String {
    format!(
        "NOT EXISTS (SELECT 1 FROM shop.mana_transfer m WHERE m.network = '{}' \
         AND m.sender = c.sender AND m.transaction_hash = c.transaction_hash)",
        NET.db_name()
    )
}

/// Deletes up to `limit` unmatched claims (of `sender`, or anyone's) at
/// least `min_age_secs` old, oldest first; returns how many. Anyone's claim
/// must also be `min_age_secs` older than the MANA index's newest block, so
/// a stalled or catching-up index evicts no accepted tip it has not reached.
async fn evict(
    conn: &mut PgConnection,
    sender: Option<&str>,
    limit: i64,
    min_age_secs: i64,
) -> sqlx::Result<i64> {
    let n = sqlx::query(sqlx::AssertSqlSafe(format!(
        "DELETE FROM shop.tip_claim d USING ( \
             SELECT c.transaction_hash, c.sender FROM shop.tip_claim c \
             WHERE ($1::text IS NULL OR c.sender = $1) \
               AND c.created_at <= timezone('UTC', now()) - make_interval(secs => $2) AND {} \
               AND ($1::text IS NOT NULL OR c.created_at <= ( \
                   SELECT to_timestamp(k.last_block_time) AT TIME ZONE 'UTC' - make_interval(secs => $2) \
                   FROM shop.chain_cursor k WHERE k.indexer = '{}' AND k.network = '{}')) \
             ORDER BY c.created_at LIMIT $3 FOR UPDATE SKIP LOCKED) e \
         WHERE d.transaction_hash = e.transaction_hash AND d.sender = e.sender",
        unmatched(),
        INDEXER_MANA,
        NET.db_name()
    )))
    .bind(sender)
    .bind(min_age_secs as f64)
    .bind(limit)
    .execute(conn)
    .await?
    .rows_affected();
    Ok(i64::try_from(n).unwrap_or(i64::MAX))
}

/// Seconds until the oldest unmatched claim may be evicted (the sender's
/// own when `own`), or a resolve pass when every claim is matched.
async fn retry_after(conn: &mut PgConnection, sender: &str, own: bool) -> sqlx::Result<i64> {
    let wait: Option<f64> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT EXTRACT(EPOCH FROM min(c.created_at) + make_interval(secs => $2) - timezone('UTC', now()))::float8 \
         FROM shop.tip_claim c WHERE ($1::text IS NULL OR c.sender = $1) AND {}",
        unmatched()
    )))
    .bind(own.then_some(sender))
    .bind(if own { 0.0 } else { EVICTABLE_AFTER_SECS as f64 })
    .fetch_one(conn)
    .await?;
    Ok(wait
        .map_or(POLL_SECS as i64, |w| w.ceil() as i64)
        .clamp(1, EVICTABLE_AFTER_SECS))
}

#[derive(sqlx::FromRow)]
struct ClaimRow {
    transaction_hash: String,
    sender: String,
    created_at: NaiveDateTime,
    recipient: Option<String>,
    amount: Option<String>,
    timestamp: Option<i64>,
}

/// `ethers.formatUnits(wei, 18)`: at least one fraction digit, no trailing zeros.
pub fn format_mana(wei: &str) -> Option<String> {
    let v = U256::from_str_radix(wei, 10).ok()?;
    let unit = U256::from(10u64).pow(U256::from(18u64));
    let frac = format!("{:0>18}", (v % unit).to_string());
    let frac = frac.trim_end_matches('0');
    Some(format!(
        "{}.{}",
        v / unit,
        if frac.is_empty() { "0" } else { frac }
    ))
}

/// JavaScript `String(n)` for a non-negative finite number.
fn js_number(n: f64) -> String {
    if n < 1e21 {
        format!("{n}")
    } else {
        format!("{n:e}").replace('e', "e+")
    }
}

/// The old processor's metadata: `amount` in MANA, `manaAmount` its wei as a JS number.
pub fn metadata(sender: &str, recipient: &str, amount: &str) -> Value {
    let mana_amount = js_number(amount.parse::<f64>().unwrap_or(0.0) * 1e18);
    json!({
        "amount": amount,
        "manaAmount": mana_amount,
        "senderAddress": sender,
        "receiverAddress": recipient,
    })
}

/// Settles a claim that was not stored from the published index; `None` when it cannot yet.
async fn resolve_unstored(
    pool: &PgPool,
    settings: &TipSettings,
    tx: &str,
    sender: &str,
) -> anyhow::Result<Option<Outcome>> {
    let Some(cursor) = chain_walk::ready_cursor(pool, INDEXER_MANA, NET, chrono::Utc::now()).await
    else {
        return Ok(None);
    };
    let rows: Vec<ClaimRow> = sqlx::query_as(
        "SELECT transaction_hash, sender, timezone('UTC', now())::timestamp(3) AS created_at, \
                recipient, amount::text AS amount, timestamp \
         FROM shop.mana_transfer WHERE network = $1 AND sender = $2 AND transaction_hash = $3 AND block_number <= $4 \
         ORDER BY log_index",
    )
    .bind(NET.db_name())
    .bind(sender)
    .bind(tx)
    .bind(cursor.last_block)
    .fetch_all(pool)
    .await?;
    let Some(first) = rows.first() else {
        return Ok(None);
    };
    let transfers: Vec<(&str, &str, i64)> = rows
        .iter()
        .filter_map(|r| Some((r.recipient.as_deref()?, r.amount.as_deref()?, r.timestamp?)))
        .collect();
    Ok(Some(match judge(settings, first, &transfers) {
        Ok(n) => {
            insert_notifications(pool, &[n]).await?;
            Outcome::Notified
        }
        Err(why) => Outcome::Rejected(why),
    }))
}

/// The verdict on a claim's published transfers.
fn judge(
    settings: &TipSettings,
    claim: &ClaimRow,
    transfers: &[(&str, &str, i64)],
) -> Result<NewNotification, Rejection> {
    let [(recipient, amount, timestamp)] = transfers else {
        return Err(Rejection::NotSingleTransfer);
    };
    if *recipient == claim.sender {
        return Err(Rejection::NotSingleTransfer);
    }
    let (Ok(address), Ok(wei), Some(mana)) = (
        Address::parse(recipient),
        U256::from_str_radix(amount, 10),
        format_mana(amount),
    ) else {
        return Err(Rejection::Malformed);
    };
    if wei < settings.min_amount_wei {
        return Err(Rejection::TooSmall);
    }
    if claim.created_at.and_utc().timestamp() - timestamp > settings.max_age_secs {
        return Err(Rejection::TooOld);
    }
    Ok(NewNotification {
        id: notification_id(&claim.transaction_hash),
        address,
        kind: KIND.into(),
        metadata: metadata(&claim.sender, recipient, &mana),
        occurred_at: claim.created_at,
    })
}

/// Settles the claims (one, or up to [`RESOLVE_LIMIT`] with `None`) the
/// published index holds a transfer for: a single transfer from the signer
/// notifies its recipient, any other shape is rejected; a claim without one
/// stays pending and is not read.
pub async fn resolve(
    pool: &PgPool,
    settings: &TipSettings,
    only: Option<(&str, &str)>,
) -> anyhow::Result<Vec<(String, Outcome)>> {
    let now = chrono::Utc::now();
    let Some(cursor) = chain_walk::ready_cursor(pool, INDEXER_MANA, NET, now).await else {
        return Ok(Vec::new());
    };
    let mut txn = pool.begin().await?;
    let rows: Vec<ClaimRow> = sqlx::query_as(
        "SELECT c.transaction_hash, c.sender, c.created_at, t.recipient, t.amount::text AS amount, t.timestamp \
         FROM (SELECT * FROM shop.tip_claim tc \
               WHERE ($3::text IS NULL OR (tc.transaction_hash = $3 AND tc.sender = $4)) \
                 AND EXISTS (SELECT 1 FROM shop.mana_transfer m WHERE m.network = $1 AND m.sender = tc.sender \
                             AND m.transaction_hash = tc.transaction_hash AND m.block_number <= $2) \
               ORDER BY tc.created_at, tc.transaction_hash, tc.sender LIMIT $5 FOR UPDATE SKIP LOCKED) c \
         JOIN shop.mana_transfer t ON t.network = $1 AND t.sender = c.sender \
              AND t.transaction_hash = c.transaction_hash AND t.block_number <= $2 \
         ORDER BY c.transaction_hash, c.sender, t.log_index",
    )
    .bind(NET.db_name())
    .bind(cursor.last_block)
    .bind(only.map(|(tx, _)| tx))
    .bind(only.map(|(_, s)| s))
    .bind(RESOLVE_LIMIT)
    .fetch_all(&mut *txn)
    .await?;
    let mut outcomes = Vec::new();
    let mut notifications = Vec::new();
    let mut settled: Vec<(String, String)> = Vec::new();
    for group in
        rows.chunk_by(|a, b| (&a.transaction_hash, &a.sender) == (&b.transaction_hash, &b.sender))
    {
        let first = &group[0];
        let transfers: Vec<(&str, &str, i64)> = group
            .iter()
            .filter_map(|r| Some((r.recipient.as_deref()?, r.amount.as_deref()?, r.timestamp?)))
            .collect();
        let outcome = match judge(settings, first, &transfers) {
            Ok(n) => {
                notifications.push(n);
                Outcome::Notified
            }
            Err(why) => {
                if why == Rejection::Malformed {
                    tracing::warn!(
                        tx = first.transaction_hash,
                        "tip claim: malformed indexed transfer; rejected"
                    );
                }
                Outcome::Rejected(why)
            }
        };
        settled.push((first.transaction_hash.clone(), first.sender.clone()));
        outcomes.push((first.transaction_hash.clone(), outcome));
    }
    insert_notifications(&mut *txn, &notifications).await?;
    let (txs, senders): (Vec<String>, Vec<String>) = settled.into_iter().unzip();
    sqlx::query(
        "DELETE FROM shop.tip_claim c USING UNNEST($1::text[], $2::text[]) AS s(tx, sender) \
         WHERE c.transaction_hash = s.tx AND c.sender = s.sender",
    )
    .bind(&txs)
    .bind(&senders)
    .execute(&mut *txn)
    .await?;
    txn.commit().await?;
    Ok(outcomes)
}

/// Drops claims past their hour; returns how many.
async fn expire(pool: &PgPool) -> anyhow::Result<u64> {
    Ok(sqlx::query(
        "DELETE FROM shop.tip_claim WHERE created_at < timezone('UTC', now()) - make_interval(secs => $1)",
    )
    .bind(CLAIM_TTL_SECS as f64)
    .execute(pool)
    .await?
    .rows_affected())
}

pub fn spawn_job(state: &SharedState, jobs: &Jobs) {
    let settings = state.settings.market.tips.clone();
    if !state.settings.market.indexes.mana_transfers_enabled {
        tracing::warn!(
            "MARKETPLACE_MANA_TRANSFERS_INDEX_ENABLED is off here: tip forwards wait for an index another process must walk, and expire unconfirmed without one"
        );
    }
    let pool = state.pool.clone();
    jobs.spawn_periodic_leased(
        "tip-notifications",
        PeriodicCfg::every(Duration::from_secs(POLL_SECS))
            .with_jitter(Duration::from_secs(5))
            .with_error_backoff(Duration::from_secs(300)),
        move |ctx| {
            let (pool, settings) = (pool.clone(), settings.clone());
            async move {
                if chain_walk::ready_cursor(&pool, INDEXER_MANA, NET, chrono::Utc::now())
                    .await
                    .is_none()
                {
                    let expired = expire(&pool).await?;
                    let waiting: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.tip_claim")
                        .fetch_one(&pool)
                        .await?;
                    ctx.progress(format!(
                        "MANA transfer index ({}) not ready: {waiting} tip(s) waiting, {expired} expired",
                        NET.db_name()
                    ));
                    return Ok(());
                }
                let outcomes = resolve(&pool, &settings, None).await?;
                let expired = expire(&pool).await?;
                let notified = outcomes
                    .iter()
                    .filter(|(_, o)| *o == Outcome::Notified)
                    .count();
                let rejected = outcomes.len() - notified;
                if notified > 0 || rejected > 0 || expired > 0 {
                    tracing::info!(notified, rejected, expired, "tip notifications");
                }
                let waiting: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.tip_claim")
                    .fetch_one(&pool)
                    .await?;
                ctx.progress(format!(
                    "{notified} notified, {rejected} rejected, {waiting} waiting, {expired} expired"
                ));
                Ok(())
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts_format_as_the_old_processor() {
        assert_eq!(format_mana("10000000000000000000").as_deref(), Some("10.0"));
        assert_eq!(format_mana("1500000000000000000").as_deref(), Some("1.5"));
        assert_eq!(format_mana("1").as_deref(), Some("0.000000000000000001"));
        assert_eq!(format_mana("0").as_deref(), Some("0.0"));
        let m = metadata("0xa", "0xb", "10.0");
        assert_eq!(m["manaAmount"], "10000000000000000000");
        assert_eq!(metadata("0xa", "0xb", "1500.0")["manaAmount"], "1.5e+21");
        assert_eq!(
            metadata("0xa", "0xb", "0.1")["manaAmount"],
            "100000000000000000"
        );
    }

    const TIPPER: &str = "0x00000000000000000000000000000000000000a1";
    const RECEIVER: &str = "0x00000000000000000000000000000000000000b2";

    fn hash(n: u8) -> String {
        format!("0x{:064x}", n)
    }

    fn now_secs() -> i64 {
        chrono::Utc::now().timestamp()
    }

    #[allow(clippy::too_many_arguments)]
    async fn transfer(
        pool: &PgPool,
        block: i64,
        log: i32,
        tx: &str,
        from: &str,
        to: &str,
        amount: &str,
        timestamp: i64,
    ) {
        sqlx::query(
            "INSERT INTO shop.mana_transfer (network, block_number, log_index, transaction_hash, sender, recipient, amount, timestamp) \
             VALUES ('matic', $1, $2, $3, $4, $5, $6::numeric, $7)",
        )
        .bind(block)
        .bind(log)
        .bind(tx)
        .bind(from)
        .bind(to)
        .bind(amount)
        .bind(timestamp)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn publish(pool: &PgPool, block: i64) {
        publish_at(pool, block, now_secs()).await;
    }

    async fn publish_at(pool: &PgPool, block: i64, time: i64) {
        sqlx::query(
            "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, last_block_time, folded_block, folded_at, full_history) \
             VALUES ('mana_transfers', 'matic', 0, $1, $2, $1, timezone('UTC', now()), true) \
             ON CONFLICT (indexer, network) DO UPDATE SET last_block = $1, last_block_time = $2, folded_block = $1",
        )
        .bind(block)
        .bind(time)
        .execute(pool)
        .await
        .unwrap();
    }

    /// An unmatched claim waits its hour even after the published index
    /// passes it (a tip slow to mine), then expires.
    #[tokio::test]
    async fn unmatched_claims_wait_an_hour() {
        let Some(db) = crate::market::tests::support::OwnDb::create("tips_expiry").await else {
            return;
        };
        let pool = &db.pool;
        let settings = TipSettings::default();
        for n in 1..=3u8 {
            claim(pool, &settings, &hash(n), TIPPER).await.unwrap();
        }
        let age = |n: u8, minutes: i32| async move {
            sqlx::query(
                "UPDATE shop.tip_claim SET created_at = created_at - make_interval(mins => $2) WHERE transaction_hash = $1",
            )
            .bind(hash(n))
            .bind(minutes)
            .execute(pool)
            .await
            .unwrap();
        };
        age(1, 50).await;
        age(2, 61).await;
        publish_at(pool, 100, now_secs()).await;
        assert_eq!(expire(pool).await.unwrap(), 1);
        let left: Vec<String> = sqlx::query_scalar(
            "SELECT transaction_hash FROM shop.tip_claim ORDER BY transaction_hash",
        )
        .fetch_all(pool)
        .await
        .unwrap();
        assert_eq!(left, vec![hash(1), hash(3)], "50 minutes old still waits");
        db.drop().await;
    }

    fn forward_app(trusted: &[&str]) -> Router {
        let mut settings = crate::market::Settings::for_tests();
        settings.market.tips = TipSettings::default()
            .with_trusted_proxies(trusted.iter().map(|c| c.parse().unwrap()).collect());
        Router::new().nest(
            "/events-notifier",
            router(crate::market::tests::support::state_without_db_settings(
                settings,
            )),
        )
    }

    const PATH: &str = "/events-notifier/forward-blockchain";

    async fn send_signed(
        app: &Router,
        peer: &str,
        real_ip: Option<&str>,
        signed: Option<&crate::outfits::testkit::Wallet>,
    ) -> StatusCode {
        use tower::ServiceExt;
        let mut req = axum::http::Request::post(PATH)
            .extension(ConnectInfo(SocketAddr::new(peer.parse().unwrap(), 4242)));
        if let Some(ip) = real_ip {
            req = req.header("x-real-ip", ip);
        }
        if let Some(w) = signed {
            for (k, v) in &crate::outfits::testkit::signed_headers(w, "post", PATH).await {
                req = req.header(k, v);
            }
        }
        app.clone()
            .oneshot(req.body(axum::body::Body::from("{}")).unwrap())
            .await
            .unwrap()
            .status()
    }

    async fn send_from(app: &Router, peer: &str, real_ip: Option<&str>) -> StatusCode {
        send_signed(app, peer, real_ip, None).await
    }

    /// Before the signature a public peer, or a trusted proxy's `X-Real-IP`,
    /// is the key; a rotated header from anyone else changes nothing.
    #[tokio::test]
    async fn unsigned_forwards_are_loosely_limited_per_client() {
        let app = forward_app(&["192.0.2.0/24"]);
        for n in 0..PREAUTH_PER_MINUTE {
            let spoof = format!("198.51.100.{}", n % 250);
            assert_eq!(
                send_from(&app, "203.0.113.77", Some(&spoof)).await,
                StatusCode::UNAUTHORIZED
            );
        }
        assert_eq!(
            send_from(&app, "203.0.113.77", Some("198.51.100.250")).await,
            StatusCode::TOO_MANY_REQUESTS,
            "an untrusted peer's header is ignored"
        );
        for _ in 0..PREAUTH_PER_MINUTE {
            assert_eq!(
                send_from(&app, "192.0.2.9", Some("2001:db8:7:7::1")).await,
                StatusCode::UNAUTHORIZED
            );
        }
        assert_eq!(
            send_from(&app, "192.0.2.10", Some("2001:db8:7:7::2")).await,
            StatusCode::TOO_MANY_REQUESTS,
            "the proxied client's /64"
        );
        assert_eq!(
            send_from(&app, "192.0.2.9", Some("203.0.113.78")).await,
            StatusCode::UNAUTHORIZED
        );
    }

    /// With no trusted proxy the gateway is the peer of every client: junk
    /// through it builds no shared bucket, and a signed tip still passes.
    #[tokio::test]
    async fn an_unlisted_gateway_peer_shares_no_bucket() {
        let app = forward_app(&[]);
        for _ in 0..3 * PREAUTH_PER_MINUTE {
            assert_eq!(
                send_from(&app, "10.9.0.2", Some("198.51.100.1")).await,
                StatusCode::UNAUTHORIZED
            );
        }
        let w = crate::outfits::testkit::scratch_wallet();
        assert_eq!(
            send_signed(&app, "10.9.0.2", None, Some(&w)).await,
            StatusCode::BAD_REQUEST,
            "past the limiters to the body check"
        );
    }

    /// Unsigned junk from one client never spends a signer's budget; each
    /// signer has its own.
    #[tokio::test]
    async fn signed_forwards_are_limited_per_signer() {
        let app = forward_app(&["192.0.2.0/24"]);
        for _ in 0..PREAUTH_PER_MINUTE + 10 {
            send_from(&app, "192.0.2.1", Some("198.51.100.66")).await;
        }
        let w = crate::outfits::testkit::scratch_wallet();
        for n in 0..FORWARDS_PER_MINUTE {
            let client = format!("203.0.114.{n}");
            assert_eq!(
                send_signed(&app, "192.0.2.1", Some(&client), Some(&w)).await,
                StatusCode::BAD_REQUEST
            );
        }
        assert_eq!(
            send_signed(&app, "192.0.2.1", Some("203.0.114.200"), Some(&w)).await,
            StatusCode::TOO_MANY_REQUESTS,
            "the signer's budget, whatever its address"
        );
        let other = crate::outfits::testkit::scratch_wallet();
        assert_eq!(
            send_signed(&app, "192.0.2.1", Some("203.0.114.200"), Some(&other)).await,
            StatusCode::BAD_REQUEST
        );
    }

    fn ip_keys(ip: &str) -> Vec<(String, u32)> {
        source_keys(ip)
    }

    /// Cycling /64s of one /48 hits the /48 budget; denied keys are never
    /// stored, so a flood of fresh ones cannot reset an exhausted bucket.
    #[test]
    fn a_flood_of_fresh_keys_cannot_reset_a_bucket() {
        let limiter = ForwardLimiter::new(8);
        let allowed = (0..1_000u32)
            .filter(|n| limiter.allow(&ip_keys(&format!("2001:db8:1:{n:x}::1"))))
            .count();
        assert_eq!(allowed, PREAUTH_PER_MINUTE_PER_48 as usize);
        assert!(
            limiter.len() <= 8,
            "the least recently used goes past the cap"
        );
        for _ in 0..PREAUTH_PER_MINUTE {
            assert!(limiter.allow(&ip_keys("203.0.113.5")));
        }
        assert!(!limiter.allow(&ip_keys("203.0.113.5")));
        let before = limiter.len();
        for n in 1_000..3_000u32 {
            assert!(!limiter.allow(&ip_keys(&format!("2001:db8:1:{n:x}::1"))));
        }
        assert_eq!(limiter.len(), before, "denied keys are not stored");
        assert!(
            !limiter.allow(&ip_keys("203.0.113.5")),
            "still exhausted after a flood of fresh keys"
        );
    }

    async fn tip_count(pool: &PgPool) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM social.notifications WHERE type = 'tip_received'")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    /// Pending until the published index holds the transfer, then the
    /// recipient is told once; unpublished blocks, a stranger's claim and a
    /// multi-transfer transaction never notify, and the sender's cap evicts.
    #[tokio::test]
    async fn a_tip_notifies_its_recipient_once_the_index_publishes_it() {
        let Some(db) = crate::market::tests::support::OwnDb::create("tips").await else {
            return;
        };
        let pool = &db.pool;
        let settings = TipSettings::default();
        let tip = "2500000000000000000";
        let t = now_secs();
        transfer(pool, 100, 0, &hash(1), TIPPER, RECEIVER, tip, t).await;
        assert_eq!(
            claim(pool, &settings, &hash(1), TIPPER).await.unwrap(),
            Outcome::Pending,
            "an index that is not ready answers nothing"
        );
        publish(pool, 150).await;
        let outcomes = resolve(pool, &settings, None).await.unwrap();
        assert_eq!(outcomes, vec![(hash(1), Outcome::Notified)]);
        assert_eq!(
            claim(pool, &settings, &hash(1), TIPPER).await.unwrap(),
            Outcome::Notified,
            "a repeated forward is idempotent"
        );
        assert_eq!(tip_count(pool).await, 1);
        let (address, metadata, id): (String, Value, Uuid) = sqlx::query_as(
            "SELECT address, metadata, id FROM social.notifications WHERE type = 'tip_received'",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(
            (address.as_str(), id),
            (RECEIVER, notification_id(&hash(1)))
        );
        assert_eq!(
            metadata,
            json!({"amount": "2.5", "manaAmount": "2500000000000000000", "senderAddress": TIPPER, "receiverAddress": RECEIVER})
        );

        transfer(pool, 200, 0, &hash(2), TIPPER, RECEIVER, tip, t).await;
        assert_eq!(
            claim(pool, &settings, &hash(2), TIPPER).await.unwrap(),
            Outcome::Pending,
            "past the published block"
        );
        transfer(pool, 120, 0, &hash(3), TIPPER, RECEIVER, tip, t).await;
        assert_eq!(
            claim(
                pool,
                &settings,
                &hash(3),
                "0x00000000000000000000000000000000000000c3"
            )
            .await
            .unwrap(),
            Outcome::Pending,
            "someone else's transfer never matches"
        );
        transfer(pool, 130, 0, &hash(4), TIPPER, RECEIVER, tip, t).await;
        transfer(pool, 130, 1, &hash(4), TIPPER, RECEIVER, tip, t).await;
        assert_eq!(
            claim(pool, &settings, &hash(4), TIPPER).await.unwrap(),
            Outcome::Rejected(Rejection::NotSingleTransfer)
        );
        assert_eq!(tip_count(pool).await, 1);

        sqlx::query("UPDATE shop.tip_claim SET created_at = created_at - interval '2 hours'")
            .execute(pool)
            .await
            .unwrap();
        assert_eq!(expire(pool).await.unwrap(), 2);
        for n in 10..10 + MAX_PENDING_PER_SENDER as u8 {
            assert_eq!(
                claim(pool, &settings, &hash(n), TIPPER).await.unwrap(),
                Outcome::Pending
            );
        }
        sqlx::query("UPDATE shop.tip_claim SET created_at = created_at - interval '1 minute' WHERE transaction_hash = $1")
            .bind(hash(10))
            .execute(pool)
            .await
            .unwrap();
        assert_eq!(
            claim(pool, &settings, &hash(99), TIPPER).await.unwrap(),
            Outcome::Pending,
            "past the sender's cap its own oldest claim makes room"
        );
        let mine: Vec<String> =
            sqlx::query_scalar("SELECT transaction_hash FROM shop.tip_claim WHERE sender = $1")
                .bind(TIPPER)
                .fetch_all(pool)
                .await
                .unwrap();
        assert_eq!(mine.len(), MAX_PENDING_PER_SENDER as usize);
        assert!(!mine.contains(&hash(10)) && mine.contains(&hash(99)));
        assert_eq!(
            claim(pool, &settings, &hash(11), TIPPER).await.unwrap(),
            Outcome::Pending,
            "a repeated claim is not a new one"
        );
        db.drop().await;
    }

    /// Below the minimum or mined more than the maximum age before the
    /// claim is refused with the old route's 400; the boundaries notify.
    #[tokio::test]
    async fn small_or_old_tips_are_rejected() {
        let Some(db) = crate::market::tests::support::OwnDb::create("tips_thresholds").await else {
            return;
        };
        let pool = &db.pool;
        let settings = TipSettings::default();
        let t = now_secs();
        let one = "1000000000000000000";
        transfer(pool, 10, 0, &hash(1), TIPPER, RECEIVER, "1", t).await;
        transfer(
            pool,
            11,
            0,
            &hash(2),
            TIPPER,
            RECEIVER,
            "999999999999999999",
            t,
        )
        .await;
        transfer(pool, 12, 0, &hash(3), TIPPER, RECEIVER, one, t).await;
        transfer(
            pool,
            13,
            0,
            &hash(4),
            TIPPER,
            RECEIVER,
            one,
            t - 86_400 - 60,
        )
        .await;
        transfer(
            pool,
            14,
            0,
            &hash(5),
            TIPPER,
            RECEIVER,
            one,
            t - 86_400 + 60,
        )
        .await;
        transfer(pool, 15, 0, &hash(6), TIPPER, TIPPER, one, t).await;
        publish(pool, 100).await;
        let expect = [
            (1, Outcome::Rejected(Rejection::TooSmall)),
            (2, Outcome::Rejected(Rejection::TooSmall)),
            (3, Outcome::Notified),
            (4, Outcome::Rejected(Rejection::TooOld)),
            (5, Outcome::Notified),
            (6, Outcome::Rejected(Rejection::NotSingleTransfer)),
        ];
        for (n, outcome) in expect {
            assert_eq!(
                claim(pool, &settings, &hash(n), TIPPER).await.unwrap(),
                outcome,
                "tx {n}"
            );
        }
        assert_eq!(tip_count(pool).await, 2);
        let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.tip_claim")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(pending, 0, "every verdict settles its claim");
        let lenient = TipSettings::parse("1", 10 * 86_400, 10).unwrap();
        transfer(pool, 16, 0, &hash(7), TIPPER, RECEIVER, "1", t - 2 * 86_400).await;
        assert_eq!(
            claim(pool, &lenient, &hash(7), TIPPER).await.unwrap(),
            Outcome::Notified,
            "the thresholds are settings"
        );
        db.drop().await;
    }

    async fn stored(pool: &PgPool) -> Vec<String> {
        sqlx::query_scalar("SELECT transaction_hash FROM shop.tip_claim ORDER BY transaction_hash")
            .fetch_all(pool)
            .await
            .unwrap()
    }

    async fn age_claim(pool: &PgPool, n: u8, minutes: i32) {
        sqlx::query(
            "UPDATE shop.tip_claim SET created_at = created_at - make_interval(mins => $2) WHERE transaction_hash = $1",
        )
        .bind(hash(n))
        .bind(minutes)
        .execute(pool)
        .await
        .unwrap();
    }

    fn other(n: u8) -> String {
        format!("0x{:040x}", 0xf00 + u32::from(n))
    }

    /// Past the global cap the oldest unmatched claim old enough to evict
    /// makes room; a matched one never goes; with none old enough a forward
    /// is retryably busy, unless the index already holds it.
    #[tokio::test]
    async fn the_pending_claims_have_a_global_cap() {
        let Some(db) = crate::market::tests::support::OwnDb::create("tips_cap").await else {
            return;
        };
        let pool = &db.pool;
        let settings = TipSettings::parse(DEFAULT_MIN_AMOUNT_WEI, DEFAULT_MAX_AGE_SECS, 3).unwrap();
        for n in 1..=3u8 {
            assert_eq!(
                claim(pool, &settings, &hash(n), &other(n)).await.unwrap(),
                Outcome::Pending
            );
        }
        let Outcome::Busy { retry_after_secs } =
            claim(pool, &settings, &hash(9), &other(9)).await.unwrap()
        else {
            panic!("fresh claims are not evicted");
        };
        assert!(
            (EVICTABLE_AFTER_SECS - 5..=EVICTABLE_AFTER_SECS).contains(&retry_after_secs),
            "{retry_after_secs}"
        );
        let t = now_secs();
        let one = "1000000000000000000";
        transfer(pool, 10, 0, &hash(20), TIPPER, RECEIVER, one, t).await;
        transfer(pool, 11, 0, &hash(21), TIPPER, RECEIVER, "1", t).await;
        assert!(
            matches!(
                claim(pool, &settings, &hash(20), TIPPER).await.unwrap(),
                Outcome::Busy { .. }
            ),
            "an index that is not ready answers nothing"
        );
        publish(pool, 100).await;
        assert_eq!(
            claim(pool, &settings, &hash(20), TIPPER).await.unwrap(),
            Outcome::Notified,
            "an indexed tip is notified past the cap"
        );
        assert_eq!(
            claim(pool, &settings, &hash(21), TIPPER).await.unwrap(),
            Outcome::Rejected(Rejection::TooSmall)
        );
        assert_eq!(stored(pool).await, vec![hash(1), hash(2), hash(3)]);

        // hash(1) is the oldest but its transfer is indexed (not yet published).
        age_claim(pool, 1, 30).await;
        age_claim(pool, 2, 10).await;
        transfer(pool, 500, 0, &hash(1), &other(1), RECEIVER, one, t).await;
        assert_eq!(
            claim(pool, &settings, &hash(9), &other(9)).await.unwrap(),
            Outcome::Pending
        );
        assert_eq!(stored(pool).await, vec![hash(1), hash(3), hash(9)]);
        assert_eq!(tip_count(pool).await, 1);
        db.drop().await;
    }

    /// The cap is full of a flood's claims when a real tip is forwarded; its
    /// transfer is indexed only afterwards, and it is still notified while
    /// the flood goes on.
    #[tokio::test]
    async fn a_tip_forwarded_into_a_full_cap_is_notified_once_indexed() {
        let Some(db) = crate::market::tests::support::OwnDb::create("tips_cap_late").await else {
            return;
        };
        let pool = &db.pool;
        let settings = TipSettings::parse(DEFAULT_MIN_AMOUNT_WEI, DEFAULT_MAX_AGE_SECS, 3).unwrap();
        publish(pool, 100).await;
        for n in 1..=3u8 {
            claim(pool, &settings, &hash(n), &other(n)).await.unwrap();
            age_claim(pool, n, 10).await;
        }
        assert_eq!(
            claim(pool, &settings, &hash(50), TIPPER).await.unwrap(),
            Outcome::Pending,
            "the real tip is stored"
        );
        for n in 4..=8u8 {
            claim(pool, &settings, &hash(n), &other(n)).await.unwrap();
        }
        assert!(
            stored(pool).await.contains(&hash(50)),
            "the flood cannot evict it yet"
        );
        transfer(
            pool,
            150,
            0,
            &hash(50),
            TIPPER,
            RECEIVER,
            "1000000000000000000",
            now_secs(),
        )
        .await;
        publish(pool, 200).await;
        let outcomes = resolve(pool, &settings, None).await.unwrap();
        assert!(
            outcomes.contains(&(hash(50), Outcome::Notified)),
            "{outcomes:?}"
        );
        assert_eq!(tip_count(pool).await, 1);
        db.drop().await;
    }

    /// While the MANA index has not reached blocks mined well after a claim
    /// (stalled, or catching up), no other sender's forward evicts it.
    #[tokio::test]
    async fn an_accepted_claim_outlives_a_stalled_index() {
        let Some(db) = crate::market::tests::support::OwnDb::create("tips_stall").await else {
            return;
        };
        let pool = &db.pool;
        let settings = TipSettings::parse(DEFAULT_MIN_AMOUNT_WEI, DEFAULT_MAX_AGE_SECS, 3).unwrap();
        publish_at(pool, 100, now_secs() - 3_600).await;
        claim(pool, &settings, &hash(50), TIPPER).await.unwrap();
        for n in 1..=2u8 {
            claim(pool, &settings, &hash(n), &other(n)).await.unwrap();
        }
        for n in [50, 1, 2] {
            age_claim(pool, n, 30).await;
        }
        for n in 3..=8u8 {
            assert!(matches!(
                claim(pool, &settings, &hash(n), &other(n)).await.unwrap(),
                Outcome::Busy { .. }
            ));
        }
        assert_eq!(stored(pool).await, vec![hash(1), hash(2), hash(50)]);
        publish_at(pool, 200, now_secs() - 1_700).await;
        assert!(
            matches!(
                claim(pool, &settings, &hash(9), &other(9)).await.unwrap(),
                Outcome::Busy { .. }
            ),
            "the index is not yet 5 minutes past the claims"
        );
        publish_at(pool, 300, now_secs()).await;
        assert_eq!(
            claim(pool, &settings, &hash(9), &other(9)).await.unwrap(),
            Outcome::Pending,
            "an index past them may evict"
        );
        assert_eq!(stored(pool).await.len(), 3);
        db.drop().await;
    }

    /// A transfer the index holds in a shape the producer cannot read rejects
    /// its own claim; the rest of the batch is still settled.
    #[tokio::test]
    async fn a_malformed_transfer_rejects_only_its_claim() {
        let Some(db) = crate::market::tests::support::OwnDb::create("tips_malformed").await else {
            return;
        };
        let pool = &db.pool;
        let settings = TipSettings::default();
        let t = now_secs();
        let one = "1000000000000000000";
        claim(pool, &settings, &hash(1), TIPPER).await.unwrap();
        claim(pool, &settings, &hash(2), TIPPER).await.unwrap();
        sqlx::query("ALTER TABLE shop.mana_transfer DROP CONSTRAINT mana_transfer_recipient_check")
            .execute(pool)
            .await
            .unwrap();
        transfer(pool, 10, 0, &hash(1), TIPPER, "0xnot-an-address", one, t).await;
        transfer(pool, 11, 0, &hash(2), TIPPER, RECEIVER, one, t).await;
        publish(pool, 100).await;
        let mut outcomes = resolve(pool, &settings, None).await.unwrap();
        outcomes.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            outcomes,
            vec![
                (hash(1), Outcome::Rejected(Rejection::Malformed)),
                (hash(2), Outcome::Notified)
            ]
        );
        assert_eq!(tip_count(pool).await, 1);
        db.drop().await;
    }

    #[test]
    fn the_tip_settings_parse() {
        let d = TipSettings::default();
        assert_eq!(d.min_amount_wei, U256::from(10u64).pow(U256::from(18u64)));
        assert_eq!((d.max_age_secs, d.max_pending), (86_400, 2_000));
        assert!(TipSettings::parse("1e18", 1, 1).is_err());
    }

    #[tokio::test]
    async fn an_unsigned_forward_is_refused() {
        use tower::ServiceExt;
        let app = Router::new().nest(
            "/events-notifier",
            router(crate::market::tests::support::state_without_db()),
        );
        let res = app
            .oneshot(
                axum::http::Request::post("/events-notifier/forward-blockchain")
                    .body(axum::body::Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn the_body_keeps_the_old_schema() {
        let tx = format!("0x{}", "Ab".repeat(32));
        assert_eq!(
            parse_body(format!(r#"{{"type":"tip-received","transactionHash":"{tx}"}}"#).as_bytes()),
            Ok(tx.to_ascii_lowercase())
        );
        assert!(parse_body(br#"{"type":"tip-received","transactionHash":"0x12"}"#).is_err());
        assert!(
            parse_body(format!(r#"{{"type":"other","transactionHash":"{tx}"}}"#).as_bytes())
                .is_err()
        );
        assert!(
            parse_body(
                format!(r#"{{"type":"tip-received","transactionHash":"{tx}","x":1}}"#).as_bytes()
            )
            .is_err()
        );
    }
}
