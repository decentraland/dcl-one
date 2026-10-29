//! The LAND authorization indexer as one `jobs::spawn_periodic` pass:
//! `eth_getLogs` over the two registries through the ONE client, appended to
//! `land_authz.authz_event` in `UNNEST` chunks with the cursor advanced in the
//! same transaction (a `kill -9` between passes re-fetches at most one range
//! and `ON CONFLICT DO NOTHING` drops the duplicates), a bounded number of
//! RPC requests per pass (the backpressure: a full replay is many passes,
//! each leaving a durable `last_block`), then the fold of the touched keys
//! into `token_right` / `estate_land` / `account_right` and, only when the
//! chain head is within [`HEAD_LAG_BLOCKS`] of what was indexed, the publish
//! marker `folded_block`. Readers trust the fold through [`index_ready`]:
//! `folded_block = last_block` and `folded_at` at most [`READY_WINDOW`] old,
//! the old server's rule.
//!
//! The fold is deterministic over the log (DATABASE.md rule 5): every leg is
//! decided by the single latest event that touches its key, so a rebuild is
//! the same three statements with `since = -1`; an incremental pass folds
//! only the keys an event past the previous `folded_block` touched, and
//! arrives at the same rows because each key's history is read whole.

use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, bail};
use chrono::{DateTime, Duration as ChronoDuration, NaiveDateTime, Utc};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};

use super::events::{ALL_TOPICS, AuthzEvent, DecodeError, decode_log};
use super::{KIND_ADD_LAND, KIND_REMOVE_LAND, LandAuthzState, Settings};
use crate::jobs::JobContext;

/// Publish a fold only when the head has not moved further than this past
/// the indexed block: a fold of an old snapshot is not "current rights".
pub const HEAD_LAG_BLOCKS: u64 = 25;
/// How old a published fold may be before readers fall back (a stopped indexer).
pub const READY_WINDOW: ChronoDuration = ChronoDuration::minutes(30);
/// Rows per `INSERT ... UNNEST` statement.
const INSERT_CHUNK: usize = 1_000;
const RPC_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
/// A filtered `eth_getLogs` answer for 10,000 logs is a few MB; this caps a misbehaving endpoint.
const RPC_BODY_CAP: usize = 64 * 1024 * 1024;
/// Headers per JSON-RPC batch request ([`Rpc::block_headers`]).
pub const HEADER_BATCH: usize = 100;
/// Chain/host clocks may differ slightly; authorization still uses strict chain time.
pub const HEADER_CLOCK_SKEW_SECONDS: i64 = 60;
/// Retry batch support after this many fallback fetches, without restarting.
pub const HEADER_BATCH_RETRY_FETCHES: usize = 4;
/// Repair cannot hold up the next live pass after publishing current rights.
pub const LEGACY_REPAIR_TIME_LIMIT: std::time::Duration = std::time::Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("RPC request budget exhausted at range ending {upper}")]
    Budget { upper: u64, requests: usize },
    #[error("rpc transport: {0}")]
    Transport(String),
    #[error("rpc returned an error that carries no usable block range: {0}")]
    Rpc(String),
    #[error("database: {0}")]
    Db(#[from] sqlx::Error),
    #[error("could not decode a log at block {block} index {index}: {source}")]
    Decode {
        block: i64,
        index: i32,
        #[source]
        source: DecodeError,
    },
}

#[derive(Debug, Deserialize)]
pub struct RpcLog {
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
    #[serde(rename = "blockNumber")]
    pub block_number: String,
    #[serde(rename = "logIndex")]
    pub log_index: String,
    #[serde(rename = "blockTimestamp")]
    pub block_timestamp: Option<String>,
    #[serde(rename = "blockHash", default)]
    pub block_hash: Option<String>,
    #[serde(rename = "transactionHash", default)]
    pub transaction_hash: Option<String>,
}

pub fn hex_to_i64(s: &str) -> Option<i64> {
    i64::from_str_radix(s.trim_start_matches("0x"), 16).ok()
}

/// The public RPC caps a filtered `eth_getLogs` at ten thousand rows but names
/// a range that would fit; following that hint is what keeps a full history
/// scan to tens of requests instead of thousands of fixed-width windows.
pub fn suggested_upper_bound(message: &str) -> Option<u64> {
    let open = message.rfind('[')?;
    let close = message[open..].find(']')? + open;
    let inner = &message[open + 1..close];
    let (_, hi) = inner.split_once(',')?;
    u64::from_str_radix(hi.trim().trim_start_matches("0x"), 16).ok()
}

/// The other refusal shape the public endpoints answer (observed on
/// rpc.decentraland.org's second backend, 2026-09-25): "range 50000 exceeds
/// limit of 10000" names the widest window it takes, not a block.
pub fn suggested_range_limit(message: &str) -> Option<u64> {
    let (_, tail) = message.split_once("exceeds limit of ")?;
    let digits: String = tail
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse::<u64>().ok().filter(|n| *n > 0)
}

/// A refusal no block range fixes: the filter names more addresses than
/// the endpoint takes ("exceed max addresses or topics per search
/// position"). The caller splits its address list instead of the range.
pub fn is_address_limit(message: &str) -> bool {
    let m = message.to_ascii_lowercase();
    m.contains("max addresses") || m.contains("too many addresses")
}

/// One block header's identity: the reorg check and the time bound of the
/// marketplace indexers read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockHeader {
    pub number: u64,
    pub hash: String,
    pub timestamp: i64,
}

/// The LAND registries' reader over the shared JSON-RPC client
/// (`crate::http::jsonrpc`): the endpoint is `ETH_RPC_URL` (https to a
/// public host, `Config::eth_rpc_url`), the same one the EIP-1654 validator
/// uses. The marketplace indexers reuse it over their own
/// endpoints and filters ([`Rpc::at`], [`Rpc::get_logs_matching`]).
pub struct Rpc<'a> {
    http: &'a crate::http::Outbound,
    url: &'a str,
    land_registry: &'a str,
    estate_registry: &'a str,
}

impl<'a> Rpc<'a> {
    pub fn new(http: &'a crate::http::Outbound, url: &'a str, settings: &'a Settings) -> Self {
        Self {
            http,
            url,
            land_registry: &settings.land_registry,
            estate_registry: &settings.estate_registry,
        }
    }

    /// The client over another endpoint, for callers that pass their own
    /// filter to [`Rpc::get_logs_matching`].
    pub fn at(http: &'a crate::http::Outbound, url: &'a str) -> Self {
        Self {
            http,
            url,
            land_registry: "",
            estate_registry: "",
        }
    }

    fn jsonrpc(&self) -> crate::http::jsonrpc::Rpc<'a> {
        crate::http::jsonrpc::Rpc::new(self.http, self.url)
            .with_timeout(RPC_TIMEOUT)
            .with_body_cap(RPC_BODY_CAP)
    }

    /// The whole envelope: the log readers act on the `error` message (a
    /// range hint), so the shared client hands it over unread.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value, IndexError> {
        self.jsonrpc()
            .envelope(method, params)
            .await
            .map_err(|e| IndexError::Transport(format!("{method}: {e}")))
    }

    pub async fn head_block(&self) -> Result<u64, IndexError> {
        let v = self.call("eth_blockNumber", json!([])).await?;
        v.get("result")
            .and_then(Value::as_str)
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .ok_or_else(|| IndexError::Rpc(format!("eth_blockNumber returned {v}")))
    }

    pub async fn finalized_header(&self) -> Result<BlockHeader, IndexError> {
        let v = self
            .call("eth_getBlockByNumber", json!(["finalized", false]))
            .await?;
        let block = &v["result"];
        let number = block["number"]
            .as_str()
            .and_then(hex_to_i64)
            .filter(|n| *n >= 0)
            .ok_or_else(|| IndexError::Rpc("invalid finalized number".into()))?
            as u64;
        let hash = block["hash"]
            .as_str()
            .filter(|h| {
                h.len() == 66
                    && h.starts_with("0x")
                    && h[2..].bytes().all(|c| c.is_ascii_hexdigit())
            })
            .ok_or_else(|| IndexError::Rpc("invalid finalized hash".into()))?
            .to_ascii_lowercase();
        let timestamp = block["timestamp"]
            .as_str()
            .and_then(hex_to_i64)
            .filter(|t| *t >= 0)
            .ok_or_else(|| IndexError::Rpc("invalid finalized timestamp".into()))?;
        Ok(BlockHeader {
            number,
            hash,
            timestamp,
        })
    }

    /// `eth_getBlockByNumber` without transactions: the hash and time of one block.
    pub async fn block_header(&self, number: u64) -> Result<BlockHeader, IndexError> {
        let v = self
            .call(
                "eth_getBlockByNumber",
                json!([format!("0x{number:x}"), false]),
            )
            .await?;
        let block = v.get("result").filter(|r| !r.is_null()).ok_or_else(|| {
            IndexError::Rpc(format!("eth_getBlockByNumber {number} returned {v}"))
        })?;
        if block
            .get("number")
            .and_then(Value::as_str)
            .and_then(hex_to_i64)
            != i64::try_from(number).ok()
        {
            return Err(IndexError::Rpc("block header number mismatch".into()));
        }
        let hash = block
            .get("hash")
            .and_then(Value::as_str)
            .filter(|h| {
                h.len() == 66
                    && h.starts_with("0x")
                    && h[2..].bytes().all(|c| c.is_ascii_hexdigit())
            })
            .map(str::to_ascii_lowercase);
        let timestamp = block
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(hex_to_i64);
        match (hash, timestamp) {
            (Some(hash), Some(timestamp)) => Ok(BlockHeader {
                number,
                hash,
                timestamp,
            }),
            _ => Err(IndexError::Rpc(format!(
                "eth_getBlockByNumber {number}: no hash or timestamp"
            ))),
        }
    }

    /// `eth_getBlockByNumber` for many blocks, in JSON-RPC batches of at
    /// most [`HEADER_BATCH`] (one HTTP request each), keyed by number, and
    /// the requests it took. A batch the endpoint answers without an element
    /// leaves that block to one single call. The marketplace indexes need it:
    /// 4 in 10 of rpc.decentraland.org/polygon's backends omit a log's
    /// `blockTimestamp` (sampled 2026-09-25), and a window can hold a
    /// thousand distinct blocks.
    pub async fn block_headers(
        &self,
        numbers: &[u64],
    ) -> Result<(std::collections::HashMap<u64, BlockHeader>, usize), IndexError> {
        let mut out = std::collections::HashMap::new();
        let mut requests = 0usize;
        for chunk in numbers.chunks(HEADER_BATCH) {
            let body: Vec<Value> = chunk
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    crate::http::jsonrpc::request(
                        i as u64,
                        "eth_getBlockByNumber",
                        json!([format!("0x{n:x}"), false]),
                    )
                })
                .collect();
            let items =
                self.jsonrpc().batch(&body).await.map_err(|e| {
                    IndexError::Transport(format!("eth_getBlockByNumber batch: {e}"))
                })?;
            requests += 1;
            {
                for item in items {
                    let Some(block) = item.get("result").filter(|r| !r.is_null()) else {
                        continue;
                    };
                    let number = block
                        .get("number")
                        .and_then(Value::as_str)
                        .and_then(hex_to_i64);
                    let hash = block
                        .get("hash")
                        .and_then(Value::as_str)
                        .map(str::to_ascii_lowercase);
                    let timestamp = block
                        .get("timestamp")
                        .and_then(Value::as_str)
                        .and_then(hex_to_i64);
                    if let (Some(number), Some(hash), Some(timestamp)) = (number, hash, timestamp) {
                        out.insert(
                            number as u64,
                            BlockHeader {
                                number: number as u64,
                                hash,
                                timestamp,
                            },
                        );
                    }
                }
            }
            for n in chunk {
                if !out.contains_key(n) {
                    let header = self.block_header(*n).await?;
                    requests += 1;
                    out.insert(*n, header);
                }
            }
        }
        Ok((out, requests))
    }

    /// Resolve a bounded set within the remaining request budget. Some RPC
    /// endpoints reject batches or omit individual responses. Keep their valid
    /// answers, then use single requests for a bounded number of fetches before
    /// probing batches again. Two-request passes still make progress between
    /// probes, while transient failures cannot permanently disable batching.
    async fn timestamp_headers(
        &self,
        numbers: &[u64],
        singles: &AtomicUsize,
        budget: usize,
        requests: &mut usize,
    ) -> std::collections::HashMap<u64, BlockHeader> {
        let mut out = std::collections::HashMap::new();
        if numbers.is_empty() || budget == 0 {
            return out;
        }
        debug_assert!(numbers.len() <= HEADER_BATCH);
        let start = *requests;
        let retry_batch = singles
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                remaining.checked_sub(1)
            })
            .is_err();
        if retry_batch {
            let body: Vec<Value> = numbers
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    crate::http::jsonrpc::request(
                        i as u64,
                        "eth_getBlockByNumber",
                        json!([format!("0x{n:x}"), false]),
                    )
                })
                .collect();
            *requests += 1;
            match self.jsonrpc().batch(&body).await {
                Ok(items) => {
                    for item in items {
                        let Some(id) = item["id"]
                            .as_u64()
                            .and_then(|id| usize::try_from(id).ok())
                            .filter(|id| *id < numbers.len())
                        else {
                            continue;
                        };
                        if let Some(header) = timestamp_header(&item["result"], numbers[id]) {
                            out.insert(header.number, header);
                        }
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, "land-authz: timestamp batch unavailable; using budgeted single headers")
                }
            }
            if out.len() < numbers.len() {
                singles.store(HEADER_BATCH_RETRY_FETCHES, Ordering::SeqCst);
            }
        }
        for number in numbers {
            if out.contains_key(number) {
                continue;
            }
            if *requests - start >= budget {
                break;
            }
            *requests += 1;
            match self.block_header(*number).await {
                Ok(header) if valid_header_time(header.timestamp) => {
                    out.insert(*number, header);
                }
                result => {
                    tracing::warn!(
                        block = number,
                        ?result,
                        "land-authz: timestamp header unresolved; retaining verified prefix"
                    );
                    break;
                }
            }
        }
        out
    }

    /// The logs of `[from, upper]` for some `upper <= to` the endpoint
    /// accepted, plus the number of requests it took: a refusal that names
    /// a range narrows to it, one that does not halves the window.
    pub async fn get_logs(
        &self,
        from: u64,
        to: u64,
    ) -> Result<(Vec<RpcLog>, u64, usize), IndexError> {
        self.get_logs_matching(
            from,
            to,
            &json!([self.land_registry, self.estate_registry]),
            &json!([ALL_TOPICS]),
        )
        .await
    }

    /// [`Rpc::get_logs`] over any filter: `address` and `topics` are the
    /// `eth_getLogs` fields as given. A refusal naming a block
    /// (`[from, hi]`) or a window width ("exceeds limit of N") narrows to
    /// it, one naming neither halves the window, and an address-count
    /// refusal is returned at once for the caller to split its list.
    pub async fn get_logs_matching(
        &self,
        from: u64,
        to: u64,
        address: &Value,
        topics: &Value,
    ) -> Result<(Vec<RpcLog>, u64, usize), IndexError> {
        self.get_logs_bounded(from, to, address, topics, usize::MAX)
            .await
    }

    async fn get_logs_bounded(
        &self,
        from: u64,
        to: u64,
        address: &Value,
        topics: &Value,
        budget: usize,
    ) -> Result<(Vec<RpcLog>, u64, usize), IndexError> {
        let mut upper = to;
        let mut requests = 0usize;
        loop {
            if requests >= budget {
                return Err(IndexError::Budget { upper, requests });
            }
            requests += 1;
            let params = json!([{
                "fromBlock": format!("0x{from:x}"),
                "toBlock": format!("0x{upper:x}"),
                "address": address,
                "topics": topics,
            }]);
            let v = self.call("eth_getLogs", params).await?;
            if let Some(result) = v.get("result").filter(|r| !r.is_null()) {
                let logs: Vec<RpcLog> = serde_json::from_value(result.clone())
                    .map_err(|e| IndexError::Rpc(format!("cannot read logs: {e}")))?;
                return Ok((logs, upper, requests));
            }
            let message = v
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("no result and no error")
                .to_string();
            if is_address_limit(&message) {
                return Err(IndexError::Rpc(message));
            }
            let by_width = suggested_range_limit(&message).map(|n| from + n.saturating_sub(1));
            match suggested_upper_bound(&message).or(by_width) {
                Some(hi) if hi >= from && hi < upper => upper = hi,
                _ => {
                    let halved = from + (upper - from) / 2;
                    if halved <= from && upper == from {
                        return Err(IndexError::Rpc(message));
                    }
                    upper = halved.max(from);
                }
            }
        }
    }
}

pub fn decode_logs(logs: &[RpcLog]) -> Result<Vec<AuthzEvent>, IndexError> {
    let mut events = Vec::with_capacity(logs.len());
    for log in logs {
        let block = hex_to_i64(&log.block_number)
            .ok_or_else(|| IndexError::Rpc(format!("bad blockNumber {}", log.block_number)))?;
        let index = hex_to_i64(&log.log_index)
            .ok_or_else(|| IndexError::Rpc(format!("bad logIndex {}", log.log_index)))?
            as i32;
        let time = log
            .block_timestamp
            .as_deref()
            .and_then(hex_to_i64)
            .filter(|time| valid_header_time(*time))
            .ok_or_else(|| {
                IndexError::Rpc(format!("missing or invalid block timestamp at {block}"))
            })?;
        match decode_log(&log.address, &log.topics, &log.data, block, index, time) {
            Ok(ev) => events.push(ev),
            Err(DecodeError::UnhandledTopic(t)) => {
                tracing::warn!(topic = %t, block, index, "land-authz: ignoring a log this indexer does not model")
            }
            Err(source) => {
                return Err(IndexError::Decode {
                    block,
                    index,
                    source,
                });
            }
        }
    }
    Ok(events)
}

fn valid_header_time(time: i64) -> bool {
    time > 0 && time <= Utc::now().timestamp() + HEADER_CLOCK_SKEW_SECONDS
}

fn timestamp_header(block: &Value, expected: u64) -> Option<BlockHeader> {
    let number = block["number"]
        .as_str()
        .and_then(hex_to_i64)
        .filter(|n| *n >= 0)? as u64;
    if number != expected {
        return None;
    }
    let hash = block["hash"]
        .as_str()
        .filter(|h| {
            h.len() == 66 && h.starts_with("0x") && h[2..].bytes().all(|c| c.is_ascii_hexdigit())
        })?
        .to_ascii_lowercase();
    let timestamp = block["timestamp"]
        .as_str()
        .and_then(hex_to_i64)
        .filter(|t| valid_header_time(*t))?;
    Some(BlockHeader {
        number,
        hash,
        timestamp,
    })
}

/// Resolve missing times and return the first unresolved block. Only logs
/// strictly before it may advance the durable cursor. Matching the header hash
/// prevents combining logs from one fork with a same-height header from another.
async fn fill_missing_timestamps(
    state: &LandAuthzState,
    rpc: &Rpc<'_>,
    logs: &mut [RpcLog],
    budget: usize,
    requests: &mut usize,
) -> Result<Option<u64>, IndexError> {
    let blocks: std::collections::BTreeSet<u64> = logs
        .iter()
        .filter(|log| {
            log.block_timestamp
                .as_deref()
                .and_then(hex_to_i64)
                .is_none_or(|time| time <= 0)
        })
        .map(|log| {
            hex_to_i64(&log.block_number)
                .filter(|n| *n >= 0)
                .map(|n| n as u64)
                .ok_or_else(|| IndexError::Rpc("invalid block number for timestamp lookup".into()))
        })
        .collect::<Result<_, _>>()?;
    let numbers: Vec<u64> = blocks.into_iter().collect();
    let headers = rpc
        .timestamp_headers(&numbers, &state.single_timestamp_headers, budget, requests)
        .await;
    let mut barrier = numbers
        .iter()
        .copied()
        .find(|number| !headers.contains_key(number));
    for log in logs {
        if let Some(header) = hex_to_i64(&log.block_number).and_then(|n| headers.get(&(n as u64))) {
            if !log
                .block_hash
                .as_deref()
                .is_some_and(|hash| hash.eq_ignore_ascii_case(&header.hash))
            {
                tracing::warn!(
                    block = header.number,
                    "land-authz: log/header hash mismatch; retaining verified prefix"
                );
                barrier = Some(barrier.map_or(header.number, |first| first.min(header.number)));
                continue;
            }
            log.block_timestamp = Some(format!("0x{:x}", header.timestamp));
        }
    }
    Ok(barrier)
}

/// Repair only this network's configured registries after live ingest. Apply
/// every resolved block durably, then take another bounded batch while budget
/// remains. A missing/failing header leaves the gate false without starving
/// current logs; an empty final due-set marks completion in the same pass.
async fn repair_timestamps(
    state: &LandAuthzState,
    rpc: &Rpc<'_>,
    budget: usize,
    requests: &mut usize,
) -> Result<(), IndexError> {
    let pool = &state.pool;
    let settings = &state.settings;
    let ready: bool = sqlx::query_scalar(
        "SELECT history_timestamps_ready FROM land_authz.index_cursor WHERE network = $1",
    )
    .bind(&settings.network)
    .fetch_one(pool)
    .await?;
    if ready {
        return Ok(());
    }
    let start = *requests;
    loop {
        let blocks: Vec<i64> = sqlx::query_scalar("SELECT DISTINCT block_number FROM land_authz.authz_event WHERE block_time <= 0 AND token_address IN ($1, $2) ORDER BY block_number LIMIT $3")
            .bind(&settings.land_registry).bind(&settings.estate_registry).bind(HEADER_BATCH as i64).fetch_all(pool).await?;
        if blocks.is_empty() {
            sqlx::query("UPDATE land_authz.index_cursor SET history_timestamps_ready = true WHERE network = $1 AND land_registry = $2 AND estate_registry = $3")
                .bind(&settings.network).bind(&settings.land_registry).bind(&settings.estate_registry).execute(pool).await?;
            return Ok(());
        }
        if *requests - start >= budget {
            return Ok(());
        }
        let numbers = blocks
            .iter()
            .map(|n| {
                u64::try_from(*n).map_err(|_| IndexError::Rpc("negative historical block".into()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let headers = rpc
            .timestamp_headers(
                &numbers,
                &state.single_timestamp_headers,
                budget - (*requests - start),
                requests,
            )
            .await;
        if headers.is_empty() {
            return Ok(());
        }
        let repaired: Vec<i64> = numbers
            .iter()
            .filter(|n| headers.contains_key(n))
            .map(|n| *n as i64)
            .collect();
        let times: Vec<i64> = repaired
            .iter()
            .map(|n| headers[&(*n as u64)].timestamp)
            .collect();
        sqlx::query("UPDATE land_authz.authz_event e SET block_time = b.time FROM unnest($1::bigint[], $2::bigint[]) b(number, time) WHERE e.block_number = b.number AND e.block_time <= 0 AND e.token_address IN ($3, $4)")
            .bind(&repaired).bind(&times).bind(&settings.land_registry).bind(&settings.estate_registry).execute(pool).await?;
    }
}

/// One `UNNEST` chunk, column by column.
#[derive(Default)]
struct Columns {
    blocks: Vec<i64>,
    indexes: Vec<i32>,
    times: Vec<i64>,
    addrs: Vec<String>,
    kinds: Vec<String>,
    token_ids: Vec<Option<String>>,
    estate_ids: Vec<Option<String>>,
    accounts: Vec<Option<String>>,
    operators: Vec<Option<String>>,
    approvals: Vec<Option<bool>>,
}

/// Appends `events` (`ON CONFLICT (block_number, log_index) DO NOTHING`);
/// returns the rows actually written.
pub async fn insert_events(
    conn: &mut PgConnection,
    events: &[AuthzEvent],
) -> Result<usize, sqlx::Error> {
    let mut written = 0usize;
    for chunk in events.chunks(INSERT_CHUNK) {
        let mut c = Columns::default();
        for e in chunk {
            c.blocks.push(e.block_number);
            c.indexes.push(e.log_index);
            c.times.push(e.block_time);
            c.addrs.push(e.token_address.clone());
            c.kinds.push(e.kind.to_string());
            c.token_ids.push(e.token_id.clone());
            c.estate_ids.push(e.estate_id.clone());
            c.accounts.push(e.account.clone());
            c.operators.push(e.operator.clone());
            c.approvals.push(e.approved);
        }
        let result = sqlx::query(
            "INSERT INTO land_authz.authz_event \
                 (block_number, log_index, block_time, token_address, kind, token_id, estate_id, account, operator, approved) \
             SELECT b, i, t, a, k, tid::numeric, eid::numeric, acct, op, appr \
             FROM UNNEST($1::bigint[], $2::int[], $3::bigint[], $4::text[], $5::text[], \
                         $6::text[], $7::text[], $8::text[], $9::text[], $10::boolean[]) \
                  AS u(b, i, t, a, k, tid, eid, acct, op, appr) \
             ON CONFLICT (block_number, log_index) DO NOTHING",
        )
        .bind(&c.blocks)
        .bind(&c.indexes)
        .bind(&c.times)
        .bind(&c.addrs)
        .bind(&c.kinds)
        .bind(&c.token_ids)
        .bind(&c.estate_ids)
        .bind(&c.accounts)
        .bind(&c.operators)
        .bind(&c.approvals)
        .execute(&mut *conn)
        .await?;
        written += result.rows_affected() as usize;
    }
    Ok(written)
}

/// `land_authz.index_cursor`, one row per network.
#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct Cursor {
    pub network: String,
    pub land_registry: String,
    pub estate_registry: String,
    pub last_block: i64,
    pub folded_block: Option<i64>,
    pub folded_at: Option<NaiveDateTime>,
    pub full_replay_started: bool,
    pub updated_at: NaiveDateTime,
}

const CURSOR_COLUMNS: &str = "network, land_registry, estate_registry, last_block, folded_block, folded_at, full_replay_started, updated_at";

pub async fn cursor(pool: &PgPool, network: &str) -> Result<Option<Cursor>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {CURSOR_COLUMNS} FROM land_authz.index_cursor WHERE network = $1"
    )))
    .bind(network)
    .fetch_optional(pool)
    .await
}

/// Readiness observations share the database clock that writes folded_at.
/// Keeping both values in one query also makes the status age consistent with
/// its ready flag, without depending on an HTTP worker's local clock.
#[derive(Debug, sqlx::FromRow)]
pub struct CursorObservation {
    #[sqlx(flatten)]
    pub cursor: Cursor,
    pub observed_at: NaiveDateTime,
}

pub async fn cursor_observed(
    pool: &PgPool,
    network: &str,
) -> Result<Option<CursorObservation>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {CURSOR_COLUMNS}, timezone('UTC', now()) AS observed_at FROM land_authz.index_cursor WHERE network = $1"
    ))).bind(network).fetch_optional(pool).await
}

/// The row a fresh database starts from: `last_block = start - 1` so the
/// first pass begins at the LAND registry's deployment block, and this IS
/// the full replay (`full_replay_started`). An existing row is left alone.
pub async fn ensure_cursor(pool: &PgPool, settings: &Settings) -> Result<Cursor, anyhow::Error> {
    sqlx::query(
        "INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block, full_replay_started) \
         VALUES ($1, $2, $3, $4, true) ON CONFLICT (network) DO NOTHING",
    )
    .bind(&settings.network)
    .bind(&settings.land_registry)
    .bind(&settings.estate_registry)
    .bind(settings.start_block as i64 - 1)
    .execute(pool)
    .await?;
    let row = cursor(pool, &settings.network)
        .await?
        .context("the cursor row vanished after its insert")?;
    if row.land_registry != settings.land_registry
        || row.estate_registry != settings.estate_registry
    {
        bail!(
            "land_authz.index_cursor for {} was indexed from registries {}/{} but this node is configured with {}/{}: \
             the log would mix two contracts; set LAND_AUTHZ_REPLAY_FROM_BLOCK={} to replay it from the start",
            settings.network,
            row.land_registry,
            row.estate_registry,
            settings.land_registry,
            settings.estate_registry,
            settings.start_block
        );
    }
    Ok(row)
}

pub async fn set_cursor(
    conn: &mut PgConnection,
    network: &str,
    block: i64,
) -> Result<(), sqlx::Error> {
    let updated = sqlx::query(
        "UPDATE land_authz.index_cursor SET last_block = $2, updated_at = timezone('UTC', now()) WHERE network = $1",
    )
    .bind(network)
    .bind(block)
    .execute(conn)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(sqlx::Error::RowNotFound);
    }
    Ok(())
}

/// Rewind durably before a repair scan: `last_block` never moves forward
/// here, the publish marker is cleared at once (readers fall back to the
/// subgraph until the repaired history is folded at chain head), and a
/// rewind to the start block records that a full replay ran, which
/// [`mark_folded`] requires. A later rewind cannot skip an earlier gap.
pub async fn begin_replay(
    pool: &PgPool,
    settings: &Settings,
    from: u64,
) -> Result<(), sqlx::Error> {
    let before = from as i64 - 1;
    let full = from <= settings.start_block;
    sqlx::query(
        "INSERT INTO land_authz.index_cursor \
             (network, land_registry, estate_registry, last_block, folded_block, folded_at, full_replay_started) \
         VALUES ($1, $2, $3, $4, NULL, NULL, $5) \
         ON CONFLICT (network) DO UPDATE SET \
             last_block = LEAST(land_authz.index_cursor.last_block, EXCLUDED.last_block), \
             folded_block = NULL, folded_at = NULL, \
             full_replay_started = land_authz.index_cursor.full_replay_started OR EXCLUDED.full_replay_started, \
             land_registry = CASE WHEN EXCLUDED.full_replay_started THEN EXCLUDED.land_registry ELSE land_authz.index_cursor.land_registry END, \
             estate_registry = CASE WHEN EXCLUDED.full_replay_started THEN EXCLUDED.estate_registry ELSE land_authz.index_cursor.estate_registry END, \
             updated_at = timezone('UTC', now())",
    )
    .bind(&settings.network)
    .bind(&settings.land_registry)
    .bind(&settings.estate_registry)
    .bind(before)
    .bind(full)
    .execute(pool)
    .await?;
    Ok(())
}

/// Publishes a completed fold: only on the row whose `last_block` is the
/// block the fold covered and whose history was replayed in full. A failed
/// or interrupted fold leaves the previous marker (or none) behind.
pub async fn mark_folded(pool: &PgPool, network: &str, head: u64) -> Result<bool, sqlx::Error> {
    let updated = sqlx::query(
        "UPDATE land_authz.index_cursor SET folded_block = last_block, folded_at = timezone('UTC', now()) \
         WHERE network = $1 AND last_block = $2 AND full_replay_started",
    )
    .bind(network)
    .bind(head as i64)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() == 1)
}

/// The readiness rule: the latest event cursor has been folded and the fold
/// is recent. Production passes the database observation time; unit tests
/// can inject a clock without affecting production readiness authority.
pub fn index_ready(cursor: &Cursor, now: DateTime<Utc>) -> bool {
    let Some(folded_at) = cursor.folded_at else {
        return false;
    };
    let age = now.naive_utc().signed_duration_since(folded_at);
    cursor.folded_block == Some(cursor.last_block)
        && age >= ChronoDuration::zero()
        && age <= READY_WINDOW
}

/// `Some(cursor)` when the local index may answer on the database clock.
/// a database error reads as not ready (the fallback, never a grant).
pub async fn ready_cursor(pool: &PgPool, network: &str) -> Option<Cursor> {
    match cursor_observed(pool, network).await {
        Ok(Some(observation))
            if index_ready(&observation.cursor, observation.observed_at.and_utc()) =>
        {
            Some(observation.cursor)
        }
        Ok(_) => None,
        Err(e) => {
            tracing::warn!(error = %e, "land-authz: cannot read index_cursor; using the fallback reader");
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FoldCounts {
    pub tokens: u64,
    pub accounts: u64,
    pub estate_lands: u64,
}

const ZERO_ADDRESS: &str = "0x0000000000000000000000000000000000000000";

const FOLD_TOKEN_SQL: &str = "
WITH touched AS (
    SELECT DISTINCT token_address, token_id FROM land_authz.authz_event
    WHERE token_id IS NOT NULL AND kind IN ('approval', 'transfer', 'update_operator') AND block_number > $2
),
latest_touch AS (
    SELECT DISTINCT ON (e.token_address, e.token_id) e.token_address, e.token_id, e.block_number, e.log_index
    FROM land_authz.authz_event e JOIN touched t ON (t.token_address, t.token_id) = (e.token_address, e.token_id)
    WHERE e.token_id IS NOT NULL AND e.kind IN ('approval', 'transfer', 'update_operator')
    ORDER BY e.token_address, e.token_id, e.block_number DESC, e.log_index DESC
),
latest_operator AS (
    SELECT DISTINCT ON (e.token_address, e.token_id) e.token_address, e.token_id,
           CASE WHEN e.kind = 'approval' THEN e.operator END AS operator
    FROM land_authz.authz_event e JOIN touched t ON (t.token_address, t.token_id) = (e.token_address, e.token_id)
    WHERE e.token_id IS NOT NULL AND e.kind IN ('approval', 'transfer')
    ORDER BY e.token_address, e.token_id, e.block_number DESC, e.log_index DESC
),
latest_update_operator AS (
    SELECT DISTINCT ON (e.token_address, e.token_id) e.token_address, e.token_id,
           CASE WHEN e.kind = 'update_operator' THEN e.operator END AS update_operator
    FROM land_authz.authz_event e JOIN touched t ON (t.token_address, t.token_id) = (e.token_address, e.token_id)
    WHERE e.token_id IS NOT NULL AND e.kind IN ('update_operator', 'transfer')
    ORDER BY e.token_address, e.token_id, e.block_number DESC, e.log_index DESC
),
latest_owner AS (
    SELECT DISTINCT ON (e.token_address, e.token_id) e.token_address, e.token_id, e.account AS owner
    FROM land_authz.authz_event e JOIN touched t ON (t.token_address, t.token_id) = (e.token_address, e.token_id)
    WHERE e.token_id IS NOT NULL AND e.kind = 'transfer'
    ORDER BY e.token_address, e.token_id, e.block_number DESC, e.log_index DESC
),
merged AS (
    SELECT token_address, token_id,
           NULLIF(w.owner, $3) AS owner,
           NULLIF(o.operator, $3) AS operator,
           NULLIF(u.update_operator, $3) AS update_operator,
           lt.block_number AS updated_block, lt.log_index AS updated_log
    FROM latest_touch lt
    LEFT JOIN latest_operator o USING (token_address, token_id)
    LEFT JOIN latest_update_operator u USING (token_address, token_id)
    LEFT JOIN latest_owner w USING (token_address, token_id)
)
INSERT INTO land_authz.token_right
    (token_address, token_id, x, y, owner, operator, update_operator, updated_block, updated_log)
SELECT token_address, token_id,
       CASE WHEN token_address = $1 THEN land_authz.token_x(token_id) END,
       CASE WHEN token_address = $1 THEN land_authz.token_y(token_id) END,
       owner, operator, update_operator, updated_block, updated_log
FROM merged
ON CONFLICT (token_address, token_id) DO UPDATE SET
    owner           = EXCLUDED.owner,
    operator        = EXCLUDED.operator,
    update_operator = EXCLUDED.update_operator,
    x               = EXCLUDED.x,
    y               = EXCLUDED.y,
    updated_block   = EXCLUDED.updated_block,
    updated_log     = EXCLUDED.updated_log";

const FOLD_ACCOUNT_SQL: &str = "
WITH touched AS (
    SELECT DISTINCT token_address, account, operator, kind FROM land_authz.authz_event
    WHERE kind IN ('update_manager', 'approved_for_all') AND account IS NOT NULL AND operator IS NOT NULL
      AND block_number > $1
),
latest AS (
    SELECT DISTINCT ON (e.token_address, e.account, e.operator, e.kind)
           e.token_address, e.account, e.operator, e.kind, e.approved, e.block_number, e.log_index
    FROM land_authz.authz_event e
    JOIN touched t ON (t.token_address, t.account, t.operator, t.kind) = (e.token_address, e.account, e.operator, e.kind)
    WHERE e.account IS NOT NULL AND e.operator IS NOT NULL
    ORDER BY e.token_address, e.account, e.operator, e.kind, e.block_number DESC, e.log_index DESC
)
INSERT INTO land_authz.account_right
    (token_address, account, operator, kind, is_approved, updated_block, updated_log)
SELECT token_address, account, operator, kind, COALESCE(approved, false), block_number, log_index
FROM latest
ON CONFLICT (token_address, account, operator, kind) DO UPDATE SET
    is_approved   = EXCLUDED.is_approved,
    updated_block = EXCLUDED.updated_block,
    updated_log   = EXCLUDED.updated_log";

const FOLD_ESTATE_SQL: &str = "
WITH touched AS (
    SELECT DISTINCT token_id FROM land_authz.authz_event
    WHERE token_address = $2 AND kind IN ('add_land', 'remove_land') AND block_number > $1
),
latest AS (
    SELECT DISTINCT ON (e.token_id) e.token_id, e.estate_id, e.kind, e.block_number, e.log_index
    FROM land_authz.authz_event e JOIN touched t ON t.token_id = e.token_id
    WHERE e.token_address = $2 AND e.kind IN ('add_land', 'remove_land')
    ORDER BY e.token_id, e.block_number DESC, e.log_index DESC
),
removed AS (
    DELETE FROM land_authz.estate_land el USING latest l
    WHERE el.land_token_id = l.token_id AND l.kind = 'remove_land'
    RETURNING el.land_token_id
)
INSERT INTO land_authz.estate_land (land_token_id, estate_token_id, updated_block, updated_log)
SELECT token_id, estate_id, block_number, log_index FROM latest WHERE kind = 'add_land'
ON CONFLICT (land_token_id) DO UPDATE SET
    estate_token_id = EXCLUDED.estate_token_id,
    updated_block   = EXCLUDED.updated_block,
    updated_log     = EXCLUDED.updated_log";

/// Folds the log into current rights, in one transaction: the keys an event
/// with `block_number > since` touched (`since = None`: every key, the
/// rebuild). Each per-token leg is decided by the single latest event that
/// touches it, so "granted then revoked" and "granted then transferred
/// away" both end denied; the zero address is stored as no grant at all;
/// `owner` is the latest `Transfer`'s `to`; a LAND's estate is its latest
/// `AddLand` unless a later `RemoveLand` deleted the row.
pub async fn fold(
    pool: &PgPool,
    settings: &Settings,
    since: Option<i64>,
) -> Result<FoldCounts, sqlx::Error> {
    let since = since.unwrap_or(-1);
    let mut tx = pool.begin().await?;
    // A job, not a request: the pool's 60 s statement_timeout is for the
    // routes. The rebuild fold (since = -1) over the whole registry history
    // is one statement per leg and may outlive it (the first live fold of
    // 34,034 events, 0.5 s warm, was cancelled at 60 s while the killed
    // node's orphaned fold still held the rows, 2026-09-25); a failed fold
    // retries with backoff, so the bound stays generous but finite.
    sqlx::query("SET LOCAL statement_timeout = '1800s'")
        .execute(&mut *tx)
        .await?;
    // The fold follows the bulk insert of the same pass, before autoanalyze
    // has seen the rows: without column statistics the planner sizes the
    // `touched` CTE at 68 rows and nests loops over it (34,397 events: the
    // live fold ran 169 s until the connection was lost; with statistics
    // the same statement hash-joins in 0.5 s, 2026-09-25). One ANALYZE per
    // fold is the price of a plan that matches the data.
    sqlx::query("ANALYZE land_authz.authz_event")
        .execute(&mut *tx)
        .await?;
    let tokens = sqlx::query(FOLD_TOKEN_SQL)
        .bind(&settings.land_registry)
        .bind(since)
        .bind(ZERO_ADDRESS)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    let accounts = sqlx::query(FOLD_ACCOUNT_SQL)
        .bind(since)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    let estate_lands = sqlx::query(FOLD_ESTATE_SQL)
        .bind(since)
        .bind(&settings.estate_registry)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    tx.commit().await?;
    Ok(FoldCounts {
        tokens,
        accounts,
        estate_lands,
    })
}

/// What one pass did; the job trace and the tests read it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PassOutcome {
    pub head: u64,
    /// The block `last_block` reached when the pass returned.
    pub indexed_to: i64,
    pub requests: usize,
    pub events_written: usize,
    /// The pass returned before reaching the target: the request budget or a shutdown.
    pub paused: bool,
    pub folded: Option<FoldCounts>,
    pub published: bool,
}

/// Indexes `[from, to]` with at most `max(request_budget, 2)` RPC requests, each
/// answered range committed with its cursor advance; returns where it
/// stopped and whether it ran out of budget (or was cancelled) first.
pub async fn sync_range(
    state: &LandAuthzState,
    rpc: &Rpc<'_>,
    from: u64,
    to: u64,
    request_budget: usize,
    ctx: Option<&JobContext>,
    outcome: &mut PassOutcome,
) -> Result<(), IndexError> {
    let network = &state.settings.network;
    // Two calls are the smallest useful range: getLogs plus omitted timestamps.
    let request_budget = request_budget.max(2);
    let mut next = from;
    while next <= to {
        if ctx.is_some_and(|c| c.is_cancelled()) || outcome.requests >= request_budget {
            outcome.paused = true;
            return Ok(());
        }
        let retry_upper = state.retry_upper.load(Ordering::SeqCst);
        let upper = if retry_upper >= next {
            to.min(retry_upper)
        } else {
            to
        };
        let (mut logs, mut reached, used) = match rpc
            .get_logs_bounded(
                next,
                upper,
                &json!([state.settings.land_registry, state.settings.estate_registry]),
                &json!([ALL_TOPICS]),
                request_budget - outcome.requests,
            )
            .await
        {
            Ok(result) => result,
            Err(IndexError::Budget { upper, requests }) => {
                state.retry_upper.store(upper, Ordering::SeqCst);
                outcome.requests += requests;
                outcome.paused = true;
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        // Cap hydration at one batch and commit only that verified prefix.
        // If log retries used the budget, keep their narrowed hint for the
        // next pass; no cursor advance can outrun its mandatory timestamps.
        let missing: std::collections::BTreeSet<i64> = logs
            .iter()
            .filter(|log| {
                log.block_timestamp
                    .as_deref()
                    .and_then(hex_to_i64)
                    .is_none_or(|time| time <= 0)
            })
            .filter_map(|log| hex_to_i64(&log.block_number))
            .collect();
        let needs_headers = !missing.is_empty();
        if let Some(first_uncovered) = missing.into_iter().nth(HEADER_BATCH) {
            reached = (first_uncovered - 1) as u64;
            logs.retain(|log| hex_to_i64(&log.block_number).is_some_and(|n| n < first_uncovered));
        }
        outcome.requests += used;
        if needs_headers && outcome.requests >= request_budget {
            state.retry_upper.store(reached, Ordering::SeqCst);
            outcome.paused = true;
            return Ok(());
        }
        if let Some(unresolved) = fill_missing_timestamps(
            state,
            rpc,
            &mut logs,
            request_budget - outcome.requests,
            &mut outcome.requests,
        )
        .await?
        {
            reached = unresolved.saturating_sub(1);
            state.retry_upper.store(reached.max(next), Ordering::SeqCst);
            if reached < next {
                outcome.paused = true;
                return Ok(());
            }
            logs.retain(|log| hex_to_i64(&log.block_number).is_some_and(|n| n < unresolved as i64));
        }
        let events = decode_logs(&logs)?;
        let mut tx = state.pool.begin().await?;
        let written = insert_events(&mut tx, &events).await?;
        set_cursor(&mut tx, network, reached as i64).await?;
        tx.commit().await?;
        outcome.events_written += written;
        outcome.indexed_to = reached as i64;
        tracing::info!(network = %network, from = next, to = reached, logs = logs.len(), written, "land-authz: indexed range");
        if let Some(ctx) = ctx {
            ctx.progress(format!(
                "indexed to block {reached} of {to} ({} requests this pass)",
                outcome.requests
            ));
        }
        state.retry_upper.store(u64::MAX, Ordering::SeqCst);
        next = reached + 1;
    }
    Ok(())
}

/// One periodic pass: replay if requested, catch up under the request
/// budget, and when caught up fold and (within [`HEAD_LAG_BLOCKS`] of the
/// head, with no `stop_block` holding the index short) publish.
pub async fn run_pass(state: &LandAuthzState, ctx: &JobContext) -> anyhow::Result<PassOutcome> {
    let settings = &state.settings;
    let Some(rpc_url) = settings.rpc_url.as_deref() else {
        bail!("ETH_RPC_URL is unset: the land-authz indexer has no log source");
    };
    let pool = &state.pool;
    let mut row = ensure_cursor(pool, settings).await?;
    if let Some(from) = settings.replay_from_block
        && !state.replay_requested.swap(true, Ordering::SeqCst)
    {
        begin_replay(pool, settings, from)
            .await
            .context("beginning the replay")?;
        row = cursor(pool, &settings.network)
            .await?
            .context("cursor row after the replay rewind")?;
        tracing::warn!(network = %settings.network, from, "land-authz: replay started; readers use the subgraph until it is folded at head");
    }
    let rpc = Rpc::new(&state.http, rpc_url, settings);
    let head = rpc.head_block().await.context("eth_blockNumber")?;
    let target = settings.stop_block.map_or(head, |s| s.min(head));
    let mut outcome = PassOutcome {
        head,
        indexed_to: row.last_block,
        ..Default::default()
    };
    let from = (row.last_block + 1) as u64;
    if from <= target {
        sync_range(
            state,
            &rpc,
            from,
            target,
            settings.requests_per_pass,
            Some(ctx),
            &mut outcome,
        )
        .await
        .context("eth_getLogs")?;
        if outcome.paused {
            return Ok(outcome);
        }
    }
    let since = row.folded_block;
    let counts = fold(pool, settings, since)
        .await
        .context("folding the log")?;
    outcome.folded = Some(counts);
    let latest = rpc
        .head_block()
        .await
        .context("eth_blockNumber after the fold")?;
    let held_short = settings.stop_block.is_some_and(|s| s < latest);
    if held_short {
        ctx.progress(format!("indexed to {target}, head {latest}: LAND_AUTHZ_STOP_BLOCK holds the index short, not published"));
        return Ok(outcome);
    }
    if latest.saturating_sub(target) > HEAD_LAG_BLOCKS {
        ctx.progress(format!(
            "folded at {target} but the head moved to {latest}; publishing after the next catch-up"
        ));
        return Ok(outcome);
    }
    outcome.published = mark_folded(pool, &settings.network, target)
        .await
        .context("publishing the fold")?;
    if !outcome.published {
        tracing::warn!(network = %settings.network, target, "land-authz: fold not published (cursor moved or no full replay recorded)");
    }
    // Publish ordinary ownership before potentially slow historical repair.
    // A cancelled/timed-out repair retains committed chunks and leaves its
    // collaborator gate false until a subsequent pass observes an empty set.
    let remaining = settings
        .requests_per_pass
        .max(2)
        .saturating_sub(outcome.requests);
    tokio::select! {
        biased;
        _ = ctx.cancel.cancelled() => { outcome.paused = true; }
        _ = tokio::time::sleep(LEGACY_REPAIR_TIME_LIMIT) => {
            tracing::warn!("land-authz: historical timestamp repair time budget exhausted");
        }
        result = repair_timestamps(state, &rpc, remaining, &mut outcome.requests) => {
            if let Err(error) = result {
                tracing::warn!(%error, "land-authz: legacy timestamp repair deferred");
            }
        }
    }
    ctx.progress(format!(
        "folded at block {target} (head {latest}): {} tokens, {} account rights, {} estate lands; published={}",
        counts.tokens, counts.accounts, counts.estate_lands, outcome.published
    ));
    Ok(outcome)
}

/// The estate-membership kinds, named here so the fold SQL and the schema
/// CHECK are read next to each other.
pub const ESTATE_KINDS: [&str; 2] = [KIND_ADD_LAND, KIND_REMOVE_LAND];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_upper_bound_the_rpc_suggests() {
        let msg =
            "Log response size exceeded. ... this block range should work: [0x4b7fc2, 0xe95489]";
        assert_eq!(suggested_upper_bound(msg), Some(0xe95489));
        let infura = "query returned more than 10000 results. Try with this block range [0x4B7FC2, 0x4B8000].";
        assert_eq!(suggested_upper_bound(infura), Some(0x4b8000));
    }

    #[test]
    fn a_message_without_a_range_yields_nothing() {
        assert_eq!(suggested_upper_bound("query timeout"), None);
    }

    #[test]
    fn a_window_limit_and_an_address_limit_are_read_from_the_message() {
        assert_eq!(
            suggested_range_limit("range 50000 exceeds limit of 10000"),
            Some(10_000)
        );
        assert_eq!(suggested_range_limit("query timeout"), None);
        assert!(is_address_limit(
            "exceed max addresses or topics per search position"
        ));
        assert!(!is_address_limit("range 50000 exceeds limit of 10000"));
    }

    #[test]
    fn hex_block_numbers_parse() {
        assert_eq!(hex_to_i64("0x589fce"), Some(5_808_078));
        assert_eq!(hex_to_i64("0x0"), Some(0));
    }

    #[test]
    fn index_readiness_requires_a_complete_recent_fold() {
        let now = Utc::now();
        let row = |last: i64, folded: Option<i64>, at: Option<DateTime<Utc>>| Cursor {
            network: "ethereum-mainnet".into(),
            land_registry: String::new(),
            estate_registry: String::new(),
            last_block: last,
            folded_block: folded,
            folded_at: at.map(|t| t.naive_utc()),
            full_replay_started: true,
            updated_at: now.naive_utc(),
        };
        assert!(!index_ready(&row(100, None, None), now));
        assert!(!index_ready(&row(101, Some(100), Some(now)), now));
        assert!(!index_ready(
            &row(100, Some(100), Some(now - ChronoDuration::minutes(31))),
            now
        ));
        assert!(!index_ready(
            &row(100, Some(100), Some(now + ChronoDuration::seconds(1))),
            now
        ));
        assert!(index_ready(
            &row(100, Some(100), Some(now - ChronoDuration::minutes(2))),
            now
        ));
    }
}
