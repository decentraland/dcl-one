//! Shadow sampling: windows the RPC walk accepted, compared with a portal in
//! the background (`CHAIN_LOGS_SOURCE=shadow`).
//!
//! The walk calls [`crate::logs::LogSource::accepted`] once its bracket and
//! predecessor check pass, before the commit (which may still fail). A
//! [`Shadowed`] source hands the read to its [`Shadow`]: a channel of one
//! slot, reserved before anything is copied, then a size bound before the
//! copy. A busy worker, a read past the bound, or another chain skips the
//! sample, counted; the walk never waits, reads nothing more from the RPC and
//! commits the same rows. Only the hand-off itself costs local CPU.
//!
//! The [`ShadowWorker`] samples one read at a time: the dataset's chain
//! (fixed by its name) and its advertised first block (metadata, cached for
//! ten minutes), then [`compare::fetch`] with up to two drain rounds, then
//! [`compare::compare`], all within one deadline (limiter waits included).
//! Every comparable sample is anchored on its own common hash; the cached
//! metadata never stands in for it.
//! Every outcome is counted ([`WINDOWS`], [`SKIPPED`], [`LOGS`], [`BLOCKS`],
//! [`SENDS`]); differences are logged with up to five positions, never data.
//! NFT backfill lanes are not compared; each lane window counts as skipped.

use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use super::compare::{self, AcceptedRead, AcceptedView, Outcome, Verdict};
use super::{AddressHistory, Coverage, PortalReader, SendCounts};
use crate::logs::{LogGroup, LogSource, Read, Window};
use crate::rpc::{BlockHeader, RpcError, RpcLog};

/// Everything one sample may take: metadata, pages, splits, drains,
/// anchor reads and the limiter's waits.
pub const SAMPLE_DEADLINE: Duration = Duration::from_secs(20);
/// Drain rounds after the first, for batches that ended early.
pub const EXTRA_ROUNDS: usize = 2;
/// How long the advertised first block (metadata) is cached; samples are
/// anchored each on its own.
const METADATA_TTL: Duration = Duration::from_secs(600);

/// `{net, result}`: one per offered window, by verdict (`equal`,
/// `incomplete`, `different`, `branch_mismatch`, `conflict`, `unanchored`,
/// `skipped`).
pub const WINDOWS: &str = "chain_logs_shadow_windows_total";
/// `{net, reason}`: why a window was not compared.
pub const SKIPPED: &str = "chain_logs_shadow_skipped_total";
/// `{net, kind}`: logs that differed (`missing`, `extra`, `mismatch`,
/// `timestamp`, `block_hash`, `drift`) or were dropped as expected (`arity`).
pub const LOGS: &str = "chain_logs_shadow_logs_total";
/// `{net, kind}`: blocks `compared` and `skipped`.
pub const BLOCKS: &str = "chain_logs_shadow_blocks_total";
/// `{net, field}`: fields of compared logs one side lacked, so not compared.
pub const UNAVAILABLE: &str = "chain_logs_shadow_unavailable_total";
/// `{net, kind}`: portal requests sent (`admission`, `pages`, `retries`, `headers`).
pub const SENDS: &str = "chain_logs_portal_sends_total";

fn skipped(net: &'static str, reason: &'static str) {
    dcl_metrics::counter!(WINDOWS, "net" => net, "result" => "skipped").increment(1);
    dcl_metrics::counter!(SKIPPED, "net" => net, "reason" => reason).increment(1);
}

/// The walk's side of one network's shadow.
#[derive(Debug)]
pub struct Shadow {
    tx: mpsc::Sender<AcceptedRead>,
    net: &'static str,
    chain: u64,
}

impl Shadow {
    /// Hands `view` to the worker when the worker is free, the read is on
    /// the portal's chain and within the hand-off bounds; otherwise counts
    /// why not. Never waits.
    pub fn offer(&self, view: &AcceptedView<'_>) {
        if view.chain_id != self.chain {
            return skipped(self.net, "chain_mismatch");
        }
        let permit = match self.tx.try_reserve() {
            Ok(permit) => permit,
            Err(mpsc::error::TrySendError::Full(())) => return skipped(self.net, "busy"),
            // The worker is gone: shown apart, never as a busy one.
            Err(mpsc::error::TrySendError::Closed(())) => return skipped(self.net, "closed"),
        };
        if view.bounded_size().is_none() {
            return skipped(self.net, "oversize");
        }
        permit.send(view.to_owned());
    }

    pub fn lane_read(&self) {
        skipped(self.net, "lane");
    }
}

/// The sampling side: one read at a time.
pub struct ShadowWorker {
    rx: mpsc::Receiver<AcceptedRead>,
    reader: PortalReader,
    net: &'static str,
    history: AddressHistory,
    metadata: Option<(Instant, Option<u64>)>,
}

/// A shadow of `net` over `reader`: the walk's handle and the worker to run.
pub fn channel(
    reader: PortalReader,
    net: &'static str,
    history: AddressHistory,
) -> (Arc<Shadow>, ShadowWorker) {
    let (tx, rx) = mpsc::channel(1);
    let shadow = Arc::new(Shadow {
        tx,
        net,
        chain: reader.chain(),
    });
    let worker = ShadowWorker {
        rx,
        reader,
        net,
        history,
        metadata: None,
    };
    (shadow, worker)
}

fn verdict_label(v: &Verdict) -> &'static str {
    match v {
        Verdict::Equal => "equal",
        Verdict::Incomplete => "incomplete",
        Verdict::Different => "different",
        Verdict::BranchMismatch { .. } => "branch_mismatch",
        Verdict::Conflict => "conflict",
        Verdict::Unanchored => "unanchored",
        Verdict::Skipped { .. } => "skipped",
    }
}

fn skip_reason(reason: &str) -> &'static str {
    match reason {
        "deadline" => "deadline",
        "metadata" => "metadata",
        "below_dataset_start" => "below_dataset_start",
        "unsupported_filter" => "unsupported_filter",
        "above_portal_finalized" => "above_portal_finalized",
        "no_upper" => "no_upper",
        "truncated" => "truncated",
        "malformed" => "malformed",
        "too_large" => "too_large",
        "irreducible" => "irreducible",
        "unavailable" => "unavailable",
        "refused" => "refused",
        "not_found" => "not_found",
        "transport" => "transport",
        "rounds" => "rounds",
        _ => "other",
    }
}

impl ShadowWorker {
    fn coverage(&self) -> Coverage {
        Coverage {
            advertised_start: self.metadata.and_then(|(_, s)| s),
            address_history: self.history,
        }
    }

    /// Samples each offered read until `stop` resolves; a sample in flight
    /// is dropped then, which aborts its requests.
    pub async fn run(mut self, stop: impl Future<Output = ()>) {
        tokio::pin!(stop);
        loop {
            let read = tokio::select! {
                biased;
                _ = &mut stop => return,
                read = self.rx.recv() => match read {
                    Some(read) => read,
                    None => return,
                },
            };
            let before = self.reader.sends();
            let outcome = tokio::select! {
                biased;
                _ = &mut stop => return,
                out = tokio::time::timeout(SAMPLE_DEADLINE, self.sample(&read)) => out,
            };
            let outcome = outcome.unwrap_or_else(|_| {
                Outcome::skipped_before_fetch(&read, self.coverage(), "deadline")
            });
            self.record(&read, &outcome, before, self.reader.sends());
        }
    }

    async fn advertised_start(&mut self) -> Result<Option<u64>, super::PortalError> {
        if let Some((at, start)) = self.metadata
            && at.elapsed() < METADATA_TTL
        {
            return Ok(start);
        }
        let start = self.reader.advertised_start().await?;
        self.metadata = Some((Instant::now(), start));
        Ok(start)
    }

    async fn sample(&mut self, read: &AcceptedRead) -> Outcome {
        let Ok(filters) = compare::filters(&read.groups) else {
            return Outcome::skipped_before_fetch(read, self.coverage(), "unsupported_filter");
        };
        let start = match self.advertised_start().await {
            Ok(start) => start,
            Err(_) => return Outcome::skipped_before_fetch(read, self.coverage(), "metadata"),
        };
        if start.is_some_and(|s| s > read.from) {
            return Outcome::skipped_before_fetch(read, self.coverage(), "below_dataset_start");
        }
        let fetched = compare::fetch(
            &self.reader,
            read.from,
            read.reached,
            &filters,
            EXTRA_ROUNDS,
        )
        .await;
        compare::compare(read, &fetched, self.coverage())
    }

    fn record(&self, read: &AcceptedRead, out: &Outcome, before: SendCounts, after: SendCounts) {
        let net = self.net;
        match &out.verdict {
            Verdict::Skipped { reason } => skipped(net, skip_reason(reason)),
            v => {
                dcl_metrics::counter!(WINDOWS, "net" => net, "result" => verdict_label(v))
                    .increment(1);
            }
        }
        for (kind, n) in [
            ("missing", out.missing),
            ("extra", out.extra),
            ("mismatch", out.mismatch),
            ("timestamp", out.timestamp),
            ("block_hash", out.block_hash),
            ("drift", out.drift_filtered),
            ("arity", out.arity_filtered),
        ] {
            if n > 0 {
                dcl_metrics::counter!(LOGS, "net" => net, "kind" => kind).increment(n as u64);
            }
        }
        for (field, n) in &out.unavailable {
            dcl_metrics::counter!(UNAVAILABLE, "net" => net, "field" => *field)
                .increment(*n as u64);
        }
        dcl_metrics::counter!(BLOCKS, "net" => net, "kind" => "compared")
            .increment(out.compared_blocks);
        dcl_metrics::counter!(BLOCKS, "net" => net, "kind" => "skipped")
            .increment(out.skipped_blocks);
        for (kind, n) in [
            ("admission", after.admission - before.admission),
            ("pages", after.pages - before.pages),
            ("retries", after.retries - before.retries),
            ("headers", after.headers - before.headers),
        ] {
            if n > 0 {
                dcl_metrics::counter!(SENDS, "net" => net, "kind" => kind).increment(n as u64);
            }
        }
        match &out.verdict {
            Verdict::Different | Verdict::BranchMismatch { .. } | Verdict::Conflict => {
                tracing::warn!(
                    network = net,
                    from = read.from,
                    reached = read.reached,
                    verdict = verdict_label(&out.verdict),
                    compared = ?out.compared,
                    missing = out.missing,
                    extra = out.extra,
                    mismatch = out.mismatch,
                    timestamp = out.timestamp,
                    block_hash = out.block_hash,
                    drift = out.drift_filtered,
                    examples = ?out.examples,
                    address_history = ?out.coverage.address_history,
                    "chain logs shadow: the portal disagrees with an accepted RPC read"
                );
            }
            v => tracing::debug!(
                network = net,
                from = read.from,
                reached = read.reached,
                verdict = verdict_label(v),
                compared = ?out.compared,
                "chain logs shadow"
            ),
        }
    }
}

/// A source whose accepted windows go to a shadow. Every read is the inner
/// source's, unchanged.
pub struct Shadowed<'s, S> {
    inner: S,
    shadow: Option<&'s Shadow>,
}

impl<'s, S> Shadowed<'s, S> {
    pub fn new(inner: S, shadow: Option<&'s Shadow>) -> Self {
        Self { inner, shadow }
    }
}

#[async_trait::async_trait]
impl<S: LogSource> LogSource for Shadowed<'_, S> {
    async fn chain_id(&self) -> Result<Read<u64>, RpcError> {
        self.inner.chain_id().await
    }

    async fn head(&self) -> Result<Read<u64>, RpcError> {
        self.inner.head().await
    }

    async fn finalized(&self) -> Result<Read<BlockHeader>, RpcError> {
        self.inner.finalized().await
    }

    async fn header(&self, number: u64) -> Result<Read<BlockHeader>, RpcError> {
        self.inner.header(number).await
    }

    async fn window(
        &self,
        from: u64,
        to: u64,
        groups: &[LogGroup],
        chunk: &mut usize,
    ) -> Result<Window, RpcError> {
        self.inner.window(from, to, groups, chunk).await
    }

    async fn archive_window(
        &self,
        from: u64,
        to: u64,
        groups: &[LogGroup],
        chunk: &mut usize,
        budget: usize,
    ) -> Result<Window, RpcError> {
        self.inner
            .archive_window(from, to, groups, chunk, budget)
            .await
    }

    async fn block_times(
        &self,
        logs: &[RpcLog],
    ) -> Result<Read<std::collections::HashMap<i64, i64>>, RpcError> {
        self.inner.block_times(logs).await
    }

    fn accepted(&self, read: &AcceptedView<'_>) {
        if let Some(shadow) = self.shadow {
            shadow.offer(read);
        }
    }

    fn lane_read(&self) {
        if let Some(shadow) = self.shadow {
            shadow.lane_read();
        }
    }
}

#[cfg(test)]
mod tests;
