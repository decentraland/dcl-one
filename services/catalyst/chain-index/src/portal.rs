//! A read-only client for one dataset of a portal that speaks the SQD portal
//! protocol: the SQD portal, or chain-lake's. It reads finalized blocks only
//! (`/finalized-stream`), the dataset's heads and, where the portal publishes
//! it, the dataset's first block.
//!
//! It is not a [`crate::logs::LogSource`]: a chain walk cannot read from it.
//! What it returns is compared with what the RPC walk accepted. A portal's
//! answer proves which blocks it covered, not that it holds every log of
//! those blocks for the addresses asked: [`Coverage`] keeps the dataset's
//! advertised first block apart from any claim on address history.
//!
//! Protocol, as both portals serve it:
//! - `GET /datasets/<ds>/head` and `/finalized-head` answer `{number, hash}`;
//!   `GET /datasets/<ds>/metadata` answers `{start_block, ..}` on the SQD
//!   portal (chain-lake has none).
//! - `POST /datasets/<ds>/finalized-stream` answers JSON lines, one per
//!   block: the blocks with matching logs, and the last block it covered,
//!   which may be before `toBlock`. A range above the finalized head answers
//!   204. Only a complete, decoded body whose last line is a header counts
//!   as covering anything ([`Page::upper`]); an empty 200 does not.
//!
//! Every HTTP request is counted as it is sent, by kind ([`Sends`]), and
//! waits for a token of the process-wide limiter of its origin and
//! credential ([`OriginLimiter`]), which also holds a shared cooldown after
//! a 429, 529 or 503. The credential is sent only as `x-api-key` (the
//! scheme the SQD portal's OpenAPI documents) to the configured origin, by a
//! client that follows no redirect; it never enters a URL, a log line or an
//! error.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use tokio::time::Instant;

use serde::Serialize;
use serde_json::Value;

use crate::logs::LogGroup;

/// The portal's limits on one query (chain-lake's; the SQD portal publishes
/// none and accepts at least these).
pub const MAX_LOG_REQUESTS: usize = 1_000;
pub const MAX_LIST: usize = 10_000;
pub const MAX_VALUES: usize = 20_000;
/// Below chain-lake's 1 MiB body cap.
pub const MAX_REQUEST_BYTES: usize = 900 * 1024;
/// Bounds on what one answer may hold before it is refused.
pub const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_BODY_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_LOGS: usize = 200_000;
/// The most lines one answer may hold, however wide its range: each is
/// kept until the page decodes.
pub const MAX_PAGE_LINES: usize = 100_000;
/// How many times a rejected batch may be halved.
pub const MAX_SPLIT_DEPTH: u32 = 6;
const TIMEOUT: Duration = Duration::from_secs(60);
const MIN_COOLDOWN: Duration = Duration::from_secs(10);
/// The longest hold a portal's `Retry-After` can set.
pub const MAX_COOLDOWN: Duration = Duration::from_secs(300);
/// The slowest rate a limiter accepts: one request every hundred seconds.
pub const MIN_RATE: f64 = 0.01;

/// The chain a portal dataset serves, by its name; `None` for a name this
/// node does not know, which is refused rather than guessed.
pub fn dataset_chain(dataset: &str) -> Option<u64> {
    Some(match dataset {
        "ethereum-mainnet" => 1,
        "polygon-mainnet" => 137,
        "ethereum-sepolia" => 11_155_111,
        "polygon-amoy-testnet" => 80_002,
        _ => return None,
    })
}

/// A credential, printed as `<redacted>` by `Debug` and never by `Display`.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into().trim().to_string();
        (!value.is_empty()).then_some(Self(value))
    }

    /// The limiter's key for it: never printed, compared within the process only.
    fn fingerprint(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.0.hash(&mut h);
        h.finish()
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

/// What is known of a portal's coverage, kept apart from what a comparison
/// measured: the dataset's advertised first block bounds the dataset, never
/// an address's history.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Coverage {
    /// The first block the portal says the dataset holds (`/metadata`
    /// `start_block`), when it publishes one.
    pub advertised_start: Option<u64>,
    pub address_history: AddressHistory,
}

/// Whether the logs of the addresses asked are known to be all there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AddressHistory {
    /// No claim: a page shows which blocks were covered, not which logs the
    /// portal holds. A custom portal can answer canonical headers and no
    /// logs for addresses it never indexed.
    Unknown,
    /// The operator asserts the provider holds every log of the dataset
    /// (`CHAIN_LOGS_PORTAL_ASSERT_FULL_HISTORY`): an assertion, not a
    /// measurement.
    AssertedByOperator,
}

/// The kinds of request counted apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendKind {
    Admission,
    Page,
    Retry,
    Header,
}

/// Every HTTP request sent, by kind, counted before it leaves.
#[derive(Debug, Default)]
pub struct Sends {
    pub admission: AtomicUsize,
    pub pages: AtomicUsize,
    pub retries: AtomicUsize,
    pub headers: AtomicUsize,
}

impl Sends {
    fn count(&self, kind: SendKind) {
        let c = match kind {
            SendKind::Admission => &self.admission,
            SendKind::Page => &self.pages,
            SendKind::Retry => &self.retries,
            SendKind::Header => &self.headers,
        };
        c.fetch_add(1, Ordering::SeqCst);
    }

    pub fn snapshot(&self) -> SendCounts {
        SendCounts {
            admission: self.admission.load(Ordering::SeqCst),
            pages: self.pages.load(Ordering::SeqCst),
            retries: self.retries.load(Ordering::SeqCst),
            headers: self.headers.load(Ordering::SeqCst),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SendCounts {
    pub admission: usize,
    pub pages: usize,
    pub retries: usize,
    pub headers: usize,
}

/// The limiters of the process, by origin and credential fingerprint.
type Limiters = HashMap<(String, u64), Arc<OriginLimiter>>;

/// The sends of one origin and credential in this process: a token bucket
/// refilled at `rate` per second holding one token (a burst of one), and a
/// cooldown every reader of the origin honours.
#[derive(Debug)]
pub struct OriginLimiter {
    rate: f64,
    state: Mutex<Bucket>,
}

#[derive(Debug)]
struct Bucket {
    tokens: f64,
    at: Instant,
    cooldown_until: Option<Instant>,
}

impl OriginLimiter {
    /// `rate` requests per second, at least [`MIN_RATE`]; a rate that is not
    /// a positive number is refused upstream (config and the command).
    fn new(rate: f64) -> Self {
        let rate = if rate.is_finite() && rate > 0.0 {
            rate.max(MIN_RATE)
        } else {
            1.0
        };
        Self {
            rate,
            state: Mutex::new(Bucket {
                tokens: 1.0,
                at: Instant::now(),
                cooldown_until: None,
            }),
        }
    }

    /// The limiter every reader of `origin` with this credential shares.
    /// The first reader's rate holds.
    pub fn shared(origin: &str, key: Option<&Secret>, rate: f64) -> Arc<Self> {
        static ALL: OnceLock<Mutex<Limiters>> = OnceLock::new();
        let all = ALL.get_or_init(Default::default);
        let k = (origin.to_string(), key.map_or(0, Secret::fingerprint));
        all.lock()
            .unwrap_or_else(|p| p.into_inner())
            .entry(k)
            .or_insert_with(|| Arc::new(Self::new(rate)))
            .clone()
    }

    /// Waits for the cooldown to pass and a token, then takes it.
    pub async fn acquire(&self) {
        loop {
            let wait = {
                let mut b = self.state.lock().unwrap_or_else(|p| p.into_inner());
                let now = Instant::now();
                match b.cooldown_until {
                    Some(until) if until > now => until - now,
                    _ => {
                        b.cooldown_until = None;
                        let elapsed = now.duration_since(b.at).as_secs_f64();
                        b.tokens = (b.tokens + elapsed * self.rate).min(1.0);
                        b.at = now;
                        if b.tokens >= 1.0 {
                            b.tokens -= 1.0;
                            return;
                        }
                        Duration::try_from_secs_f64((1.0 - b.tokens) / self.rate)
                            .unwrap_or(MAX_COOLDOWN)
                            .min(MAX_COOLDOWN)
                    }
                }
            };
            tokio::time::sleep(wait).await;
        }
    }

    /// Holds every reader of the origin for `retry_after`, between
    /// [`MIN_COOLDOWN`] and [`MAX_COOLDOWN`] (a portal's header bounds no
    /// hold of ours), never shortening a longer hold already set.
    pub fn cool(&self, retry_after: Option<Duration>) {
        let hold = retry_after
            .unwrap_or(MIN_COOLDOWN)
            .clamp(MIN_COOLDOWN, MAX_COOLDOWN);
        let now = Instant::now();
        let until = now.checked_add(hold).unwrap_or(now);
        let mut b = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if b.cooldown_until.is_none_or(|u| u < until) {
            b.cooldown_until = Some(until);
        }
    }
}

/// A dataset head.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Head {
    pub number: u64,
    pub hash: String,
}

/// A block header as a portal line carries it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Header {
    pub number: u64,
    pub hash: String,
    pub parent_hash: String,
    /// Seconds since the epoch.
    pub timestamp: u64,
}

/// One log as a portal line carries it, normalized to lowercase hex.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PortalLog {
    pub block: u64,
    pub log_index: u32,
    pub transaction_index: u32,
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
    pub transaction_hash: String,
}

/// One complete, decoded answer: the header of every line, the logs, and
/// the last block it covered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    pub headers: Vec<Header>,
    pub logs: Vec<PortalLog>,
    /// Positions that carried two different logs in this answer; they are
    /// kept out of `logs`, never deduplicated into one.
    pub conflicts: Vec<(u64, u32)>,
    pub upper: Header,
    /// The decoded lines' bytes, for a caller's aggregate bound.
    pub bytes: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum PortalError {
    #[error("the portal answered {status}; retry later")]
    Unavailable {
        status: u16,
        retry_after: Option<Duration>,
    },
    #[error("the range is above the portal's finalized head ({finalized:?})")]
    AboveHead { finalized: Option<u64> },
    #[error("the portal answered 200 with no block: no coverage proven")]
    NoUpper,
    #[error("the portal's answer broke off before its end")]
    Truncated,
    #[error("the portal's answer is malformed: {0}")]
    Malformed(String),
    #[error("the portal's answer exceeds the {0} bound")]
    TooLarge(&'static str),
    /// A 400 or 413. The portal's own text is never kept: an upstream may
    /// echo what it was sent. `size` says whether it named a size limit.
    #[error("the portal rejected the query ({status}{})", if *.size { ", a size limit" } else { "" })]
    Rejected { status: u16, size: bool },
    #[error("the portal refused the credential ({status})")]
    Refused { status: u16 },
    #[error("the portal has no such dataset or route ({status})")]
    NotFound { status: u16 },
    #[error("the portal's parent block check failed (409)")]
    ParentMismatch,
    #[error("the request cannot be split further and was rejected ({status})")]
    Irreducible { status: u16 },
    #[error("unsupported filter: {0}")]
    Unsupported(String),
    #[error("portal transport: {0}")]
    Transport(String),
}

impl PortalError {
    /// A rejection a smaller request may avoid (a limit named in a 400, or a 413).
    pub fn is_size_rejection(&self) -> bool {
        matches!(self, PortalError::Rejected { size: true, .. })
    }
}

/// One log request of a portal query.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LogRequest {
    pub address: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic0: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic1: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic2: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic3: Option<Vec<String>>,
}

impl LogRequest {
    fn values(&self) -> usize {
        self.address.len()
            + [&self.topic0, &self.topic1, &self.topic2, &self.topic3]
                .iter()
                .map(|t| t.as_ref().map_or(0, Vec::len))
                .sum::<usize>()
    }

    fn longest_list(&self) -> usize {
        [&self.topic0, &self.topic1, &self.topic2, &self.topic3]
            .iter()
            .map(|t| t.as_ref().map_or(0, Vec::len))
            .chain(std::iter::once(self.address.len()))
            .max()
            .unwrap_or(0)
    }
}

/// An `eth_getLogs` filter (one [`LogGroup`]) as the portal is asked it and
/// as it is reapplied, exactly, to every log a portal returns: the address
/// set, each topic position, and the arity (`topics` of length n, trailing
/// nulls included, requires n topics, as `eth_getLogs` does).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    pub addresses: BTreeSet<String>,
    pub positions: Vec<Option<BTreeSet<String>>>,
}

fn hex_of(s: &str, len: usize) -> Option<String> {
    let s = s.to_ascii_lowercase();
    (s.len() == len && s.starts_with("0x") && s[2..].bytes().all(|c| c.is_ascii_hexdigit()))
        .then_some(s)
}

impl Filter {
    /// The filter of `group`, or why it is not supported: a topics value
    /// that is not a list, more than four positions, an empty alternative
    /// list, or a malformed address or topic.
    pub fn from_group(group: &LogGroup) -> Result<Self, PortalError> {
        let addresses = group
            .addresses
            .iter()
            .map(|a| {
                hex_of(a, 42).ok_or_else(|| PortalError::Unsupported(format!("address {a:?}")))
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        let topics = match &group.topics {
            Value::Null => Vec::new(),
            Value::Array(a) => a.clone(),
            other => return Err(PortalError::Unsupported(format!("topics {other}"))),
        };
        if topics.len() > 4 {
            return Err(PortalError::Unsupported(
                "more than four topic positions".into(),
            ));
        }
        let mut positions = Vec::with_capacity(topics.len());
        for (i, t) in topics.iter().enumerate() {
            let topic = |v: &Value| {
                v.as_str()
                    .and_then(|s| hex_of(s, 66))
                    .ok_or_else(|| PortalError::Unsupported(format!("topic {i}: {v}")))
            };
            positions.push(match t {
                Value::Null => None,
                Value::String(_) => Some(BTreeSet::from([topic(t)?])),
                Value::Array(alts) if alts.is_empty() => {
                    return Err(PortalError::Unsupported(format!(
                        "topic {i}: an empty alternative list"
                    )));
                }
                Value::Array(alts) => Some(alts.iter().map(topic).collect::<Result<_, _>>()?),
                other => {
                    return Err(PortalError::Unsupported(format!("topic {i}: {other}")));
                }
            });
        }
        Ok(Self {
            addresses,
            positions,
        })
    }

    /// Whether `eth_getLogs` with this filter returns a log of `address`
    /// with `topics`.
    pub fn matches(&self, address: &str, topics: &[String]) -> bool {
        self.addresses.contains(&address.to_ascii_lowercase())
            && topics.len() >= self.positions.len()
            && self.positions.iter().zip(topics).all(|(p, t)| {
                p.as_ref()
                    .is_none_or(|alts| alts.contains(&t.to_ascii_lowercase()))
            })
    }

    /// The portal log requests: the addresses in pieces of at most `chunk`,
    /// each with the topic lists. No address, no request: a request without
    /// an address list would match every address.
    pub fn requests(&self, chunk: usize) -> Vec<LogRequest> {
        let topic = |i: usize| -> Option<Vec<String>> {
            self.positions
                .get(i)
                .and_then(|p| p.as_ref())
                .map(|s| s.iter().cloned().collect())
        };
        let addresses: Vec<String> = self.addresses.iter().cloned().collect();
        addresses
            .chunks(chunk.clamp(1, MAX_LIST))
            .map(|piece| LogRequest {
                address: piece.to_vec(),
                topic0: topic(0),
                topic1: topic(1),
                topic2: topic(2),
                topic3: topic(3),
            })
            .collect()
    }
}

/// The JSON body of a finalized-stream query over `[from, to]`.
pub fn query_body(from: u64, to: u64, requests: &[LogRequest]) -> Value {
    serde_json::json!({
        "type": "evm",
        "fromBlock": from,
        "toBlock": to,
        "fields": {
            "block": {"number": true, "hash": true, "parentHash": true, "timestamp": true},
            "log": {
                "address": true, "topics": true, "data": true,
                "transactionHash": true, "logIndex": true, "transactionIndex": true
            }
        },
        "logs": requests,
    })
}

/// Packs `requests` into batches each within the portal's limits: log
/// requests, values per list and in all, and serialized bytes. A request
/// alone over a limit is its own batch, which the portal then rejects.
pub fn pack(from: u64, to: u64, requests: Vec<LogRequest>) -> Vec<Vec<LogRequest>> {
    let base = query_body(from, to, &[]).to_string().len();
    let mut out: Vec<Vec<LogRequest>> = Vec::new();
    let mut batch: Vec<LogRequest> = Vec::new();
    let (mut values, mut bytes) = (0usize, base);
    for r in requests {
        let (v, b) = (
            r.values(),
            serde_json::to_string(&r).map_or(0, |s| s.len() + 1),
        );
        if !batch.is_empty()
            && (batch.len() + 1 > MAX_LOG_REQUESTS
                || values + v > MAX_VALUES
                || bytes + b > MAX_REQUEST_BYTES)
        {
            out.push(std::mem::take(&mut batch));
            values = 0;
            bytes = base;
        }
        values += v;
        bytes += b;
        batch.push(r);
    }
    if !batch.is_empty() {
        out.push(batch);
    }
    out
}

/// Halves a rejected batch: by log requests, or the one request by its
/// address list; `None` when it holds one request with one address.
pub fn split(batch: &[LogRequest]) -> Option<(Vec<LogRequest>, Vec<LogRequest>)> {
    match batch {
        [] => None,
        [one] if one.address.len() <= 1 && one.longest_list() <= 1 => None,
        [one] if one.address.len() > 1 => {
            let mid = one.address.len() / 2;
            let half = |addresses: &[String]| LogRequest {
                address: addresses.to_vec(),
                ..one.clone()
            };
            Some((
                vec![half(&one.address[..mid])],
                vec![half(&one.address[mid..])],
            ))
        }
        [_] => None,
        many => {
            let mid = many.len() / 2;
            Some((many[..mid].to_vec(), many[mid..].to_vec()))
        }
    }
}

/// The reader over one dataset of one portal.
#[derive(Clone)]
pub struct PortalReader {
    client: reqwest::Client,
    origin: Arc<str>,
    dataset: Arc<str>,
    chain: u64,
    key: Option<Secret>,
    limiter: Arc<OriginLimiter>,
    sends: Arc<Sends>,
    max_page_lines: usize,
}

/// The lines an answer over `[from, to]` may hold: one a block (every line
/// is a block's header), and never more than `absolute`.
pub fn line_cap(from: u64, to: u64, absolute: usize) -> usize {
    let span = to.saturating_sub(from).saturating_add(1);
    usize::try_from(span).unwrap_or(usize::MAX).min(absolute)
}

impl std::fmt::Debug for PortalReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortalReader")
            .field("origin", &self.origin)
            .field("dataset", &self.dataset)
            .field("chain", &self.chain)
            .field("key", &self.key)
            .finish()
    }
}

/// `origin` must be an http(s) URL with no path, query, credentials or
/// fragment: requests go to `<origin>/datasets/<dataset>/...` only.
fn parse_origin(origin: &str) -> Result<String, String> {
    let url = url::Url::parse(origin.trim()).map_err(|e| format!("portal origin: {e}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("portal origin: http or https only".into());
    }
    if url.scheme() == "http"
        && !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))
    {
        return Err("portal origin: https required except for loopback".into());
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err("portal origin: no path, query, credentials or fragment".into());
    }
    Ok(url.origin().ascii_serialization())
}

impl PortalReader {
    /// A reader of `dataset` at `origin`, with its own client (no redirects)
    /// and the shared limiter of the origin and credential.
    pub fn new(
        origin: &str,
        dataset: &str,
        key: Option<Secret>,
        max_rps: f64,
    ) -> Result<Self, String> {
        let origin = parse_origin(origin)?;
        let chain = dataset_chain(dataset).ok_or_else(|| format!("unknown dataset {dataset:?}"))?;
        let client = crate::http::try_http_client(&crate::http::HttpClientCfg {
            total_timeout: Some(TIMEOUT),
            max_redirects: None,
            ..Default::default()
        })
        .map_err(|e| format!("portal client: {}", e.without_url()))?;
        let limiter = OriginLimiter::shared(&origin, key.as_ref(), max_rps);
        Ok(Self {
            client,
            origin: origin.into(),
            dataset: dataset.into(),
            chain,
            key,
            limiter,
            sends: Arc::new(Sends::default()),
            max_page_lines: MAX_PAGE_LINES,
        })
    }

    /// The same reader with another absolute line cap (tests).
    #[cfg(test)]
    pub(crate) fn with_line_cap(mut self, cap: usize) -> Self {
        self.max_page_lines = cap;
        self
    }

    pub fn chain(&self) -> u64 {
        self.chain
    }

    pub fn dataset(&self) -> &str {
        &self.dataset
    }

    pub fn sends(&self) -> SendCounts {
        self.sends.snapshot()
    }

    fn url(&self, route: &str) -> String {
        format!("{}/datasets/{}/{route}", self.origin, self.dataset)
    }

    async fn send(
        &self,
        kind: SendKind,
        req: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, PortalError> {
        self.limiter.acquire().await;
        self.sends.count(kind);
        let req = match &self.key {
            Some(k) => req.header("x-api-key", &k.0),
            None => req,
        };
        let resp = req
            .send()
            .await
            .map_err(|e| PortalError::Transport(e.without_url().to_string()))?;
        let status = resp.status().as_u16();
        match status {
            200 | 204 => Ok(resp),
            409 => Err(PortalError::ParentMismatch),
            401 | 403 => Err(PortalError::Refused { status }),
            404 => Err(PortalError::NotFound { status }),
            413 => Err(PortalError::Rejected { status, size: true }),
            400 => Err(PortalError::Rejected {
                status,
                size: names_a_size_limit(resp).await,
            }),
            429 | 503 | 529 => {
                let retry_after = resp
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(crate::http::parse_retry_after);
                self.limiter.cool(retry_after);
                Err(PortalError::Unavailable {
                    status,
                    retry_after,
                })
            }
            _ => Err(PortalError::Unavailable {
                status,
                retry_after: None,
            }),
        }
    }

    async fn head_at(&self, route: &str) -> Result<Head, PortalError> {
        let resp = self
            .send(SendKind::Admission, self.client.get(self.url(route)))
            .await?;
        let v = json_body(resp).await?;
        let number = v["number"]
            .as_u64()
            .ok_or_else(|| PortalError::Malformed(format!("{route}: number")))?;
        let hash = v["hash"]
            .as_str()
            .and_then(|h| hex_of(h, 66))
            .ok_or_else(|| PortalError::Malformed(format!("{route}: hash")))?;
        Ok(Head { number, hash })
    }

    /// The newest block the portal serves.
    pub async fn latest(&self) -> Result<Head, PortalError> {
        self.head_at("head").await
    }

    /// The newest finalized block the portal serves.
    pub async fn finalized(&self) -> Result<Head, PortalError> {
        self.head_at("finalized-head").await
    }

    /// The dataset's advertised first block (`/metadata` `start_block`);
    /// `None` where the portal publishes no metadata or no such field.
    pub async fn advertised_start(&self) -> Result<Option<u64>, PortalError> {
        let resp = match self
            .send(SendKind::Admission, self.client.get(self.url("metadata")))
            .await
        {
            Err(PortalError::NotFound { .. }) => return Ok(None),
            other => other?,
        };
        Ok(json_body(resp).await?["start_block"].as_u64())
    }

    /// The header of finalized block `n`.
    pub async fn header(&self, n: u64) -> Result<Header, PortalError> {
        let page = self.page_as(SendKind::Header, n, n, &[]).await?;
        if page.upper.number != n || page.headers.len() != 1 {
            return Err(PortalError::Malformed(format!(
                "asked block {n}, got {}",
                page.upper.number
            )));
        }
        Ok(page.upper)
    }

    /// One finalized-stream request over `[from, to]`.
    pub async fn page(
        &self,
        from: u64,
        to: u64,
        requests: &[LogRequest],
        retry: bool,
    ) -> Result<Page, PortalError> {
        let kind = if retry {
            SendKind::Retry
        } else {
            SendKind::Page
        };
        self.page_as(kind, from, to, requests).await
    }

    async fn page_as(
        &self,
        kind: SendKind,
        from: u64,
        to: u64,
        requests: &[LogRequest],
    ) -> Result<Page, PortalError> {
        if from > to {
            return Err(PortalError::Malformed(format!(
                "empty range [{from}, {to}]"
            )));
        }
        let body = query_body(from, to, requests);
        let resp = self
            .send(
                kind,
                self.client
                    .post(self.url("finalized-stream"))
                    .header(reqwest::header::CONTENT_TYPE, "application/json")
                    .body(body.to_string()),
            )
            .await?;
        if resp.status().as_u16() == 204 {
            let finalized = resp
                .headers()
                .get("x-sqd-finalized-head-number")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok());
            return Err(PortalError::AboveHead { finalized });
        }
        let lines = read_lines(resp, line_cap(from, to, self.max_page_lines)).await?;
        decode_page(&lines, from, to)
    }
}

/// Whether a 400's message names a size limit. The text is read here and
/// dropped: it never reaches an error, a log line or a report.
async fn names_a_size_limit(resp: reqwest::Response) -> bool {
    let bytes = crate::http::read_body_capped(resp, 16 * 1024)
        .await
        .unwrap_or_default();
    let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    let m = match &v["error"] {
        Value::String(s) => s.to_ascii_lowercase(),
        Value::Object(o) => o
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase(),
        _ => String::new(),
    };
    m.contains("at most") || m.contains("larger than") || m.contains("too large")
}

/// A JSON error by its kind and place, never by the text it read: serde
/// quotes the offending value, which a portal controls.
fn json_error(e: &serde_json::Error) -> PortalError {
    PortalError::Malformed(format!(
        "{:?} JSON error at line {} column {}",
        e.classify(),
        e.line(),
        e.column()
    ))
}

async fn json_body(resp: reqwest::Response) -> Result<Value, PortalError> {
    let bytes = crate::http::read_body_capped(resp, 1024 * 1024)
        .await
        .map_err(|_| PortalError::Truncated)?;
    serde_json::from_slice(&bytes).map_err(|e| json_error(&e))
}

/// The body's lines, each within [`MAX_LINE_BYTES`], all within
/// [`MAX_BODY_BYTES`] and at most `max_lines` of them (one a block); a body
/// that breaks off is [`PortalError::Truncated`].
async fn read_lines(
    mut resp: reqwest::Response,
    max_lines: usize,
) -> Result<Vec<Vec<u8>>, PortalError> {
    let mut lines = Vec::new();
    let mut current: Vec<u8> = Vec::new();
    let mut total = 0usize;
    loop {
        let chunk = match resp.chunk().await {
            Ok(Some(c)) => c,
            Ok(None) => break,
            Err(_) => return Err(PortalError::Truncated),
        };
        total += chunk.len();
        if total > MAX_BODY_BYTES {
            return Err(PortalError::TooLarge("body"));
        }
        for &b in chunk.iter() {
            if b == b'\n' {
                if !current.iter().all(u8::is_ascii_whitespace) {
                    if lines.len() >= max_lines {
                        return Err(PortalError::TooLarge("lines"));
                    }
                    lines.push(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
            } else {
                current.push(b);
                if current.len() > MAX_LINE_BYTES {
                    return Err(PortalError::TooLarge("line"));
                }
            }
        }
    }
    if !current.iter().all(u8::is_ascii_whitespace) {
        if lines.len() >= max_lines {
            return Err(PortalError::TooLarge("lines"));
        }
        lines.push(current);
    }
    Ok(lines)
}

fn header_of(v: &Value) -> Result<Header, PortalError> {
    let bad = |what: &str| PortalError::Malformed(format!("header {what}"));
    Ok(Header {
        number: v["number"].as_u64().ok_or_else(|| bad("number"))?,
        hash: v["hash"]
            .as_str()
            .and_then(|h| hex_of(h, 66))
            .ok_or_else(|| bad("hash"))?,
        parent_hash: v["parentHash"]
            .as_str()
            .and_then(|h| hex_of(h, 66))
            .ok_or_else(|| bad("parentHash"))?,
        timestamp: v["timestamp"].as_u64().ok_or_else(|| bad("timestamp"))?,
    })
}

fn log_of(block: u64, v: &Value) -> Result<PortalLog, PortalError> {
    let bad = |what: &str| PortalError::Malformed(format!("log at block {block}: {what}"));
    let topics = v["topics"]
        .as_array()
        .ok_or_else(|| bad("topics"))?
        .iter()
        .map(|t| t.as_str().and_then(|s| hex_of(s, 66)))
        .collect::<Option<Vec<_>>>()
        .filter(|t| t.len() <= 4)
        .ok_or_else(|| bad("topics"))?;
    let data = v["data"]
        .as_str()
        .map(str::to_ascii_lowercase)
        .filter(|d| {
            d.starts_with("0x") && d.len() % 2 == 0 && d[2..].bytes().all(|c| c.is_ascii_hexdigit())
        })
        .ok_or_else(|| bad("data"))?;
    let index = |key: &str| {
        v[key]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n <= i32::MAX as u32)
            .ok_or_else(|| bad(key))
    };
    Ok(PortalLog {
        block,
        log_index: index("logIndex")?,
        transaction_index: index("transactionIndex")?,
        address: v["address"]
            .as_str()
            .and_then(|a| hex_of(a, 42))
            .ok_or_else(|| bad("address"))?,
        topics,
        data,
        transaction_hash: v["transactionHash"]
            .as_str()
            .and_then(|h| hex_of(h, 66))
            .ok_or_else(|| bad("transactionHash"))?,
    })
}

/// A complete answer over `[from, to]`: at least one line; every line a
/// header with strictly increasing numbers inside the range; every log
/// well-formed; the last line the block covered. Two different logs at one
/// position are a conflict, never deduplicated.
pub fn decode_page(lines: &[Vec<u8>], from: u64, to: u64) -> Result<Page, PortalError> {
    if lines.is_empty() {
        return Err(PortalError::NoUpper);
    }
    let mut headers: Vec<Header> = Vec::with_capacity(lines.len());
    let mut by_position: BTreeMap<(u64, u32), PortalLog> = BTreeMap::new();
    let mut conflicts: BTreeSet<(u64, u32)> = BTreeSet::new();
    let mut count = 0usize;
    for line in lines {
        let v: Value = serde_json::from_slice(line).map_err(|e| json_error(&e))?;
        let header = header_of(&v["header"])?;
        if header.number < from || header.number > to {
            return Err(PortalError::Malformed(format!(
                "block {} outside [{from}, {to}]",
                header.number
            )));
        }
        if headers.last().is_some_and(|h| h.number >= header.number) {
            return Err(PortalError::Malformed(format!(
                "block {} out of order",
                header.number
            )));
        }
        match &v["logs"] {
            Value::Null => {}
            Value::Array(logs) => {
                for l in logs {
                    count += 1;
                    if count > MAX_LOGS {
                        return Err(PortalError::TooLarge("logs"));
                    }
                    let log = log_of(header.number, l)?;
                    let at = (log.block, log.log_index);
                    match by_position.get(&at) {
                        Some(seen) if *seen != log => {
                            conflicts.insert(at);
                        }
                        Some(_) => {}
                        None => {
                            by_position.insert(at, log);
                        }
                    }
                }
            }
            _ => return Err(PortalError::Malformed("logs is not a list".into())),
        }
        headers.push(header);
    }
    for at in &conflicts {
        by_position.remove(at);
    }
    let upper = headers.last().cloned().ok_or(PortalError::NoUpper)?;
    Ok(Page {
        headers,
        logs: by_position.into_values().collect(),
        conflicts: conflicts.into_iter().collect(),
        upper,
        bytes: lines.iter().map(Vec::len).sum(),
    })
}

pub mod compare;
pub mod shadow;

#[cfg(test)]
mod tests;
