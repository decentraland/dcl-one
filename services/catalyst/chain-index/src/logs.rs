//! The source a chain walk reads its logs from. A source answers five
//! reads, and each read reports the requests it took:
//!
//! - the chain id it serves;
//! - the newest block;
//! - the newest finalized block;
//! - the header of a block by height;
//! - a window of logs for a set of filter groups, with the block it proved
//!   complete;
//!
//! plus the block times those logs need.
//!
//! The walk's policy stays with its domain (ADR 2026-11-13): the target
//! block, the confirmation depth, the cursor, the request budget,
//! cancellation, reorg handling and publication. [`RpcSource`] reads over
//! Ethereum JSON-RPC.
//!
//! The requests a read reports are the JSON-RPC requests it sent, not a
//! provider's bill; a window also reports what the walk charges its log
//! budget for it ([`Window::charge`]), which over [`RpcSource`] is its
//! requests. For [`RpcSource`] they are HTTP requests:
//! - a JSON-RPC batch counts one;
//! - a refused request counts one;
//! - every request a window sends counts, as it goes out: those that narrow
//!   its range and an address-count refusal after them included.

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};

use crate::rpc::{BlockHeader, Rpc, RpcError, RpcLog, hex_to_i64, is_address_limit};

/// One `eth_getLogs` filter: these addresses (split in chunks) and this
/// `topics` array. The groups of one walk name disjoint addresses.
#[derive(Clone, Debug)]
pub struct LogGroup {
    pub addresses: Vec<String>,
    pub topics: Value,
}

/// What one window read returned.
#[derive(Debug, Default)]
pub struct Window {
    pub logs: Vec<RpcLog>,
    /// Every log of `[from, reached]` of every group is in `logs`.
    pub reached: u64,
    /// The JSON-RPC requests the window sent, counted as the module
    /// documentation says.
    pub requests: usize,
    /// What the walk charges its log budget for the window: `requests` for
    /// an RPC read; for a read from a portal
    /// ([`crate::portal::admit::AdmittedPortal`]), what the window's filters
    /// cost as RPC log requests, never less than the RPC log requests it
    /// sent.
    pub charge: usize,
    /// The portal requests the window sent, discarded attempts included.
    pub portal_sends: usize,
}

/// A value and the requests reading it took, as the module documentation
/// counts them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read<T> {
    pub value: T,
    pub requests: usize,
}

impl<T> Read<T> {
    fn one(value: T) -> Self {
        Self { value, requests: 1 }
    }
}

pub fn log_position(log: &RpcLog) -> (i64, i64) {
    (
        hex_to_i64(&log.block_number).unwrap_or(i64::MAX),
        hex_to_i64(&log.log_index).unwrap_or(i64::MAX),
    )
}

/// The requests one window will need at the current chunk size.
pub fn requests_for(groups: &[LogGroup], chunk: usize) -> usize {
    groups
        .iter()
        .map(|g| g.addresses.len().div_ceil(chunk.max(1)))
        .sum()
}

/// What a chain walk reads. See the module documentation.
#[async_trait::async_trait]
pub trait LogSource: Send + Sync {
    /// The chain id the source serves.
    async fn chain_id(&self) -> Result<Read<u64>, RpcError>;
    /// The newest block number.
    async fn head(&self) -> Result<Read<u64>, RpcError>;
    /// The newest finalized block's header.
    async fn finalized(&self) -> Result<Read<BlockHeader>, RpcError>;
    /// The header of block `number`.
    async fn header(&self, number: u64) -> Result<Read<BlockHeader>, RpcError>;
    /// Every log of `[from, reached]` of every group, sorted by position and
    /// without duplicates. The implementation guarantees
    /// `from <= reached <= to` and that every log lies in `[from, reached]`;
    /// the walk commits up to `reached` on that guarantee. An endpoint that
    /// refuses the address count halves `chunk`, which the caller keeps for
    /// the rest of its pass.
    async fn window(
        &self,
        from: u64,
        to: u64,
        groups: &[LogGroup],
        chunk: &mut usize,
    ) -> Result<Window, RpcError>;
    /// [`LogSource::window`], adding to `sent` every request it sends as it
    /// goes out, whatever the read's outcome: a read that fails, or is
    /// dropped, has still spent them. `sent` is the caller's own counter for
    /// this read: the window's requests are what it grew by. The default
    /// adds the window's requests once it is read.
    async fn window_counted(
        &self,
        from: u64,
        to: u64,
        groups: &[LogGroup],
        chunk: &mut usize,
        sent: &AtomicUsize,
    ) -> Result<Window, RpcError> {
        let w = self.window(from, to, groups, chunk).await?;
        sent.fetch_add(w.requests, Ordering::SeqCst);
        Ok(w)
    }
    /// A historical RPC window with a hard send ceiling. Sources without a
    /// bounded transport can leave the optional replay pending.
    async fn archive_window(
        &self,
        from: u64,
        _to: u64,
        _groups: &[LogGroup],
        _chunk: &mut usize,
        _budget: usize,
    ) -> Result<Window, RpcError> {
        Err(RpcError::Budget {
            upper: from,
            requests: 0,
        })
    }
    /// The block time of every log's block.
    async fn block_times(&self, logs: &[RpcLog]) -> Result<Read<HashMap<i64, i64>>, RpcError>;
    /// A window the walk accepted: its bracket and predecessor check
    /// passed, its commit may still fail. A comparing source hands it off
    /// ([`crate::portal::shadow`]); it never waits, reads nothing and changes
    /// no outcome. The default does nothing.
    fn accepted(&self, _read: &crate::portal::compare::AcceptedView<'_>) {}
    /// A backfill lane window read below the cursor: never compared, only
    /// counted by a comparing source. The default does nothing.
    fn lane_read(&self) {}
}

/// The source over one Ethereum JSON-RPC endpoint: one request per header,
/// head, finalized or chain id read.
#[derive(Clone, Copy)]
pub struct RpcSource<'a> {
    rpc: Rpc<'a>,
}

impl<'a> RpcSource<'a> {
    pub fn new(rpc: Rpc<'a>) -> Self {
        Self { rpc }
    }
}

#[async_trait::async_trait]
impl LogSource for RpcSource<'_> {
    async fn chain_id(&self) -> Result<Read<u64>, RpcError> {
        let v = self.rpc.call("eth_chainId", json!([])).await?;
        v.get("result")
            .and_then(Value::as_str)
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .map(Read::one)
            .ok_or_else(|| RpcError::Malformed(format!("eth_chainId returned {v}")))
    }

    async fn head(&self) -> Result<Read<u64>, RpcError> {
        self.rpc.head_block().await.map(Read::one)
    }

    async fn finalized(&self) -> Result<Read<BlockHeader>, RpcError> {
        self.rpc.finalized_header().await.map(Read::one)
    }

    async fn header(&self, number: u64) -> Result<Read<BlockHeader>, RpcError> {
        self.rpc.block_header(number).await.map(Read::one)
    }

    async fn window(
        &self,
        from: u64,
        to: u64,
        groups: &[LogGroup],
        chunk: &mut usize,
    ) -> Result<Window, RpcError> {
        self.window_counted(from, to, groups, chunk, &AtomicUsize::new(0))
            .await
    }

    async fn window_counted(
        &self,
        from: u64,
        to: u64,
        groups: &[LogGroup],
        chunk: &mut usize,
        sent: &AtomicUsize,
    ) -> Result<Window, RpcError> {
        fetch_window(&self.rpc, from, to, groups, chunk, sent, usize::MAX).await
    }

    async fn archive_window(
        &self,
        from: u64,
        to: u64,
        groups: &[LogGroup],
        chunk: &mut usize,
        budget: usize,
    ) -> Result<Window, RpcError> {
        let sent = AtomicUsize::new(0);
        fetch_window(&self.rpc, from, to, groups, chunk, &sent, budget)
            .await
            .map_err(|error| match error {
                RpcError::Budget { .. } => error,
                source => RpcError::Counted {
                    requests: sent.load(Ordering::SeqCst),
                    source: Box::new(source),
                },
            })
    }

    async fn block_times(&self, logs: &[RpcLog]) -> Result<Read<HashMap<i64, i64>>, RpcError> {
        let sent = AtomicUsize::new(0);
        block_times(&self.rpc.also_counting(&sent), logs)
            .await
            .map(|(value, requests)| Read { value, requests })
            .map_err(|source| RpcError::Counted {
                requests: sent.load(Ordering::SeqCst),
                source: Box::new(source),
            })
    }
}

/// Every log of `[from, reached]` (some `reached <= to` every request
/// accepted) of every group: a request that narrows the range narrows the
/// window for all of them (the logs past it are dropped and re-read next
/// window), and an address-count refusal halves `chunk` and repeats. Its
/// requests are counted by the client it is given ([`Rpc::also_counting`]).
pub(crate) async fn fetch_window(
    rpc: &Rpc<'_>,
    from: u64,
    to: u64,
    groups: &[LogGroup],
    chunk: &mut usize,
    sent: &AtomicUsize,
    budget: usize,
) -> Result<Window, RpcError> {
    let (Ok(from_i), Ok(to_i)) = (i64::try_from(from), i64::try_from(to)) else {
        return Err(RpcError::Rpc(format!(
            "log window {from}..={to} is past i64"
        )));
    };
    let before = sent.load(Ordering::SeqCst);
    let rpc = rpc.also_counting(sent);
    // `out.reached` as i64: it starts at `to` and only drops to a checked `upper`.
    let mut reached_i = to_i;
    let mut out = Window {
        logs: Vec::new(),
        reached: to,
        ..Window::default()
    };
    for group in groups {
        let mut i = 0usize;
        while i < group.addresses.len() {
            let end = (i + (*chunk).max(1)).min(group.addresses.len());
            let slice = &group.addresses[i..end];
            let address = if slice.len() == 1 {
                json!(slice[0])
            } else {
                json!(slice)
            };
            let spent = sent.load(Ordering::SeqCst) - before;
            if spent >= budget {
                return Err(RpcError::Budget {
                    upper: out.reached,
                    requests: spent,
                });
            }
            match rpc
                .get_logs_bounded(from, out.reached, &address, &group.topics, budget - spent)
                .await
            {
                Ok((logs, upper, _)) => {
                    // In the window, `upper <= to` fits i64.
                    let in_window = upper >= from && upper <= out.reached;
                    let upper_i = i64::try_from(upper).ok().filter(|_| in_window);
                    let Some(upper_i) = upper_i else {
                        return Err(RpcError::Rpc(
                            "invalid log window bounds or position".into(),
                        ));
                    };
                    if logs.iter().any(|l| {
                        let (block, index) = log_position(l);
                        block < from_i
                            || block > upper_i
                            || index < 0
                            || index > i64::from(i32::MAX)
                    }) {
                        return Err(RpcError::Rpc(
                            "invalid log window bounds or position".into(),
                        ));
                    }
                    if upper < out.reached {
                        out.reached = upper;
                        reached_i = upper_i;
                        out.logs.retain(|l| log_position(l).0 <= upper_i);
                    }
                    let cut = reached_i;
                    out.logs
                        .extend(logs.into_iter().filter(|l| log_position(l).0 <= cut));
                    i = end;
                }
                Err(RpcError::Budget { upper, .. }) => {
                    return Err(RpcError::Budget {
                        upper,
                        requests: sent.load(Ordering::SeqCst) - before,
                    });
                }
                Err(RpcError::Rpc(message)) if is_address_limit(&message) && *chunk > 1 => {
                    *chunk = (*chunk / 2).max(1);
                    tracing::info!(
                        chunk = *chunk,
                        "chain logs: the endpoint refused the address count; halving the chunk"
                    );
                }
                Err(e) => return Err(e),
            }
        }
    }
    out.requests = sent.load(Ordering::SeqCst) - before;
    out.charge = out.requests;
    out.logs.sort_by_key(log_position);
    out.logs.dedup_by(|a, b| log_position(a) == log_position(b));
    Ok(out)
}

/// The block time of every log: the endpoint's `blockTimestamp` when it sent
/// one, else the headers of the distinct blocks without one, fetched in
/// JSON-RPC batches (`Rpc::block_headers`; the requests are counted).
pub(crate) async fn block_times(
    rpc: &Rpc<'_>,
    logs: &[RpcLog],
) -> Result<(HashMap<i64, i64>, usize), RpcError> {
    let mut times: HashMap<i64, i64> = HashMap::new();
    let mut missing: BTreeSet<i64> = BTreeSet::new();
    for log in logs {
        let block = log_position(log).0;
        match log.block_timestamp.as_deref().and_then(hex_to_i64) {
            Some(t) => {
                times.insert(block, t);
            }
            None => {
                missing.insert(block);
            }
        }
    }
    let wanted: Vec<u64> = missing
        .into_iter()
        .filter(|b| !times.contains_key(b))
        .map(|b| u64::try_from(b).map_err(|_| RpcError::Rpc(format!("negative log block {b}"))))
        .collect::<Result<_, _>>()?;
    if wanted.is_empty() {
        return Ok((times, 0));
    }
    let (headers, requests) = rpc.block_headers(&wanted).await?;
    for (number, header) in headers {
        let number = i64::try_from(number)
            .map_err(|_| RpcError::Rpc(format!("block header {number} is past i64")))?;
        times.insert(number, header.timestamp);
    }
    Ok((times, requests))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn block_times_refuses_a_negative_log_block() {
        let http = crate::http::Outbound::for_tests();
        let rpc = Rpc::at(&http, "http://127.0.0.1:1");
        let log = RpcLog {
            address: String::new(),
            topics: vec![],
            data: String::new(),
            block_number: "0x-5".into(),
            log_index: "0x0".into(),
            block_timestamp: None,
            block_hash: None,
            transaction_hash: None,
        };
        let err = block_times(&rpc, &[log]).await.unwrap_err();
        assert!(err.to_string().contains("negative log block -5"), "{err}");
    }

    #[test]
    fn a_window_needs_one_request_per_address_chunk_per_group() {
        let g = |n: usize| LogGroup {
            addresses: (0..n).map(|i| format!("0x{i:040x}")).collect(),
            topics: json!([]),
        };
        assert_eq!(requests_for(&[g(2_500), g(2)], 1_000), 4);
        assert_eq!(requests_for(&[g(0)], 1_000), 0);
    }
}
