//! chain-lake as the source of a walk's logs (`CHAIN_LOGS_SOURCE=portal`),
//! for the windows and addresses its coverage answer admits; the RPC for
//! everything else.
//!
//! [`AdmittedPortal`] wraps the walk's RPC source. The chain id, the heads,
//! every header (so the walk's bracket, checkpoints and rewinds) and the
//! block times stay on the RPC; only [`LogSource::window`] may read from the
//! portal. A window is read in three phases:
//!
//! 1. Admission. The coverage answer (`GET /datasets/<ds>/contracts`, read
//!    once per source, again on a revision change, and again, once a window,
//!    when a window starts above it and it is older than [`COVERAGE_TTL`])
//!    must name the
//!    dataset and the network's chain. Its `finalized = (N, hash)` clamps
//!    the window to `N`; an address is admitted when the answer lists it
//!    from a block at or below the window's first (`from == a` included).
//!    An address that starts inside the window, or is not listed, is read
//!    from the RPC for that whole window.
//! 2. The portal phase. The RPC's header at `N` must carry the answer's
//!    hash: the lake's durable history up to `N` is the walk's chain, which
//!    no page's own last header proves (the lake may take that header from
//!    an RPC of its own). Then every batch of the admitted filters is read
//!    over `[a, min(b, N)]`: complete pages only, each naming the answer's
//!    revision (chain-lake's [`super::REVISION_HEADER`]), up to the lowest
//!    last block of any batch. Each log is filtered again through the
//!    original filters, arity included: a log no filter keeps is drift. A
//!    page naming another revision drops every page read, and the window is
//!    admitted again from the original filters under a fresh answer, once.
//! 3. The RPC phase, on the final partition: the addresses not admitted,
//!    over `[a, reached]`, through the wrapped source. The window reaches
//!    the lower of the two ends, and holds the logs of both up to it.
//!
//! Anything else in the portal phase (an unreadable or foreign answer, an
//! anchor on another hash, a second revision change, a cut or conflicting
//! page, drift, the deadline) reads the whole window from the RPC instead.
//! An RPC read that fails fails the window, as in `rpc` mode.
//!
//! A window is tried only when the walk read, through this source, a chain
//! id that is the dataset's: a projection whose RPC serves another chain
//! reads everything from the RPC and leaves the portal's state alone.
//!
//! One deadline ([`Admission`]'s) bounds the whole of admission and the
//! portal phase, the limiter's waits included. The phase is a future of the
//! walk: a walk dropped on a stop or a lost lease drops it, and nothing is
//! spawned. After [`FAILURES_TO_COOL`] failed portal phases in a row, or an
//! anchor on another hash, the network's portal cools for
//! [`Admission`]'s cooldown, during which windows go straight to the RPC;
//! a hold the origin's limiter keeps cools it the same way until it passes.
//! A hold set while a window is in flight (its own 429 or 503) is read the
//! same way: what that window saw is the hold's, not a failure of the
//! portal. An anchor the RPC fails (or the deadline spent waiting on it) is
//! the RPC's fault and changes nothing. Once either passes, one window runs
//! as the probe and the others go to the RPC until it ends; only a probe that reads its pages ends the
//! cooling, and a probe dropped before it ends lets the next window probe.
//! A window that read no page (uncovered, above the coverage, another
//! chain) changes nothing, and one that entered before the latest cooling
//! began settles nothing. Each window settles one outcome, once its logs
//! are known to merge: no position held twice, and every block both sides
//! hold logs of given the same hash and time by both (otherwise the whole
//! window is read from the RPC).
//!
//! The window's `charge` (what the walk's log budget pays) is computed from
//! its original filters, with a floor of the RPC log sends it actually
//! made, discarded ones included; an early portal page can leave the pass at
//! an earlier block than `rpc` mode would. `requests` are the RPC requests
//! actually sent (the anchor counted as it is sent) and `portal_sends` the
//! portal ones,
//! discarded attempts included. Outcomes are counted in [`WINDOWS`], logs by
//! source in [`LOGS`], RPC sends by kind in [`RPC_SENDS`] (each as it is
//! sent, failed and dropped reads included) and portal sends in
//! [`super::shadow::SENDS`] (when the window ends, however it ends).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::time::Instant;

use super::compare::{self, Kept, Stop};
use super::{LakeCoverage, PortalError, PortalLog, PortalReader, SendCounts};
use crate::logs::{LogGroup, LogSource, Read, Window, log_position, requests_for};
use crate::rpc::{BlockHeader, RpcError, RpcLog, hex_to_i64};

/// Everything one window's admission and portal phase may take.
pub const DEFAULT_DEADLINE: Duration = Duration::from_secs(20);
/// Failed portal phases in a row before the network's portal cools.
pub const FAILURES_TO_COOL: u32 = 3;
/// How long a network's portal cools.
pub const COOLDOWN: Duration = Duration::from_secs(300);
/// How old a coverage answer may be before a window above its finalized
/// block reads it again.
pub const COVERAGE_TTL: Duration = Duration::from_secs(10);

/// `{net, result}`: one per window read with a portal configured, by
/// [`Outcome::label`].
pub const WINDOWS: &str = "chain_logs_portal_windows_total";
/// `{net, source}`: logs a window returned, by where they were read
/// (`portal`, `rpc`).
pub const LOGS: &str = "chain_logs_portal_logs_total";
/// `{net, kind}`: RPC requests a portal-mode window sent: `logs` for the
/// addresses not admitted, `anchor` for the durable anchor's header,
/// `fallback` for a window read whole from the RPC.
pub const RPC_SENDS: &str = "chain_logs_source_rpc_sends_total";

/// How a window was read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Every address from the portal.
    Admitted,
    /// Some addresses from the portal, the others from the RPC.
    Split,
    /// No address admitted: the RPC.
    Uncovered,
    /// The window starts above the answer's finalized block (or the answer
    /// has none): the RPC.
    AboveCoverage,
    /// A filter the portal cannot be asked: the RPC.
    UnsupportedFilter,
    /// The walk's RPC was not seen to serve the dataset's chain (no chain
    /// id read through this source, or another one): the RPC.
    OtherChain,
    /// The network's portal cools, or another window probes it: the RPC.
    Cooling,
    /// The coverage answer could not be read.
    CoverageUnavailable,
    /// The coverage answer is malformed, or names another dataset or chain.
    CoverageRefused,
    /// The RPC's header at `N` carries another hash than the answer's.
    AnchorMismatch,
    /// The RPC could not answer the header at `N` (or the deadline passed
    /// while it was asked): an RPC fault, not the portal's.
    AnchorUnavailable,
    /// Pages named another revision again after the window was admitted
    /// again.
    RevisionMoved,
    /// A page could not be read whole.
    PortalError,
    /// Two pages, or two lines, disagreed on a block (its hash or its time)
    /// or a log.
    Conflict,
    /// A log the admitted filters do not keep.
    Drift,
    /// The deadline passed.
    Deadline,
    /// The portal's and the RPC's logs share a position.
    Duplicate,
}

impl Outcome {
    pub fn label(self) -> &'static str {
        match self {
            Outcome::Admitted => "admitted",
            Outcome::Split => "split",
            Outcome::Uncovered => "rpc_uncovered",
            Outcome::AboveCoverage => "rpc_above_coverage",
            Outcome::UnsupportedFilter => "rpc_unsupported_filter",
            Outcome::OtherChain => "rpc_other_chain",
            Outcome::Cooling => "cooling",
            Outcome::CoverageUnavailable => "coverage_unavailable",
            Outcome::CoverageRefused => "coverage_refused",
            Outcome::AnchorMismatch => "rpc_anchor_mismatch",
            Outcome::AnchorUnavailable => "rpc_anchor_unavailable",
            Outcome::RevisionMoved => "rpc_revision_moved",
            Outcome::PortalError => "rpc_portal_error",
            Outcome::Conflict => "rpc_conflict",
            Outcome::Drift => "rpc_drift",
            Outcome::Deadline => "rpc_portal_deadline",
            Outcome::Duplicate => "rpc_duplicate",
        }
    }

    /// The portal answered the window as it should.
    pub fn succeeded(self) -> bool {
        matches!(self, Outcome::Admitted | Outcome::Split)
    }

    /// The window says nothing of the portal's health: it read no page, or
    /// the RPC failed it.
    pub fn neutral(self) -> bool {
        matches!(
            self,
            Outcome::Uncovered
                | Outcome::AboveCoverage
                | Outcome::UnsupportedFilter
                | Outcome::OtherChain
                | Outcome::Cooling
                | Outcome::AnchorUnavailable
        )
    }

    /// The portal failed: counted toward its cooling.
    pub fn failed(self) -> bool {
        !self.succeeded() && !self.neutral()
    }
}

#[derive(Debug, Default)]
struct Health {
    failures: u32,
    cooling_until: Option<Instant>,
    probing: bool,
    /// Moves each time the portal starts cooling: a window that entered
    /// under an older epoch settles nothing.
    epoch: u64,
    /// The origin limiter's holds already seen ([`PortalReader::holds`]).
    seen_holds: u64,
}

/// A network's portal as a source: its reader, its deadline, and whether
/// it cools. One per network, shared by every walk of it.
#[derive(Debug)]
pub struct Admission {
    reader: PortalReader,
    net: &'static str,
    deadline: Duration,
    cooldown: Duration,
    coverage_ttl: Duration,
    health: Mutex<Health>,
}

/// A window's leave to try the portal: the epoch it entered under, and
/// whether it is the probe of a cooled portal, released when it ends,
/// settled or not.
struct Ticket<'a> {
    admission: &'a Admission,
    epoch: u64,
    probe: bool,
}

impl Drop for Ticket<'_> {
    fn drop(&mut self) {
        if self.probe {
            self.admission.health().probing = false;
        }
    }
}

impl Admission {
    pub fn new(reader: PortalReader, net: &'static str, deadline: Duration) -> Self {
        Self::with_cooldown(reader, net, deadline, COOLDOWN)
    }

    /// With another cooldown (tests).
    pub fn with_cooldown(
        reader: PortalReader,
        net: &'static str,
        deadline: Duration,
        cooldown: Duration,
    ) -> Self {
        let seen_holds = reader.holds();
        Self {
            reader,
            net,
            deadline,
            cooldown,
            coverage_ttl: COVERAGE_TTL,
            health: Mutex::new(Health {
                seen_holds,
                ..Health::default()
            }),
        }
    }

    /// With another coverage age limit (tests).
    pub fn with_coverage_ttl(mut self, ttl: Duration) -> Self {
        self.coverage_ttl = ttl;
        self
    }

    pub fn reader(&self) -> &PortalReader {
        &self.reader
    }

    fn health(&self) -> std::sync::MutexGuard<'_, Health> {
        self.health.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Whether a window may try the portal: not while it cools (its own
    /// cooling, or a hold the origin's limiter keeps), nor while another
    /// window probes it; once a cooling or a hold passes, one window probes.
    fn enter(&self) -> Option<Ticket<'_>> {
        let mut h = self.health();
        let now = Instant::now();
        // A hold the limiter set (a 429, 503 or 529), seen now or passed
        // since: every window that entered before it settles nothing (a
        // probe in flight included), and the portal cools until it passes
        // (a longer cooling stays), then one window probes it.
        let holds = self.reader.holds();
        let held = self.reader.cooling();
        if holds != h.seen_holds || (held && h.cooling_until.is_none()) {
            h.seen_holds = holds;
            h.failures = 0;
            h.epoch += 1;
            if h.cooling_until.is_none_or(|until| until < now) {
                h.cooling_until = Some(now);
            }
        }
        if held {
            return None;
        }
        let epoch = h.epoch;
        match h.cooling_until {
            None => Some(Ticket {
                admission: self,
                epoch,
                probe: false,
            }),
            Some(until) if until > now => None,
            Some(_) if h.probing => None,
            Some(_) => {
                h.probing = true;
                Some(Ticket {
                    admission: self,
                    epoch,
                    probe: true,
                })
            }
        }
    }

    /// Records how a portal phase ended: a neutral window changes nothing, a
    /// window that entered before the latest cooling settles nothing, a
    /// success ends the failures (and the cooling, when it probed), and a
    /// failure counts toward the next cooling (at once when it probed or
    /// the anchor is on another hash).
    fn settle(&self, outcome: Outcome, ticket: &Ticket<'_>) {
        if outcome.neutral() {
            return;
        }
        let mut h = self.health();
        let holds = self.reader.holds();
        if holds != h.seen_holds {
            // A hold set while the window was in flight (its own 429 or 503,
            // or another reader's): what it saw is the hold's, not the
            // portal's. The portal waits for the hold, then one window probes.
            let now = Instant::now();
            h.seen_holds = holds;
            h.failures = 0;
            h.epoch += 1;
            if h.cooling_until.is_none_or(|until| until < now) {
                h.cooling_until = Some(now);
            }
            return;
        }
        if h.epoch != ticket.epoch {
            return;
        }
        if outcome.succeeded() {
            h.failures = 0;
            if ticket.probe {
                h.cooling_until = None;
            }
            return;
        }
        h.failures += 1;
        if ticket.probe || outcome == Outcome::AnchorMismatch || h.failures >= FAILURES_TO_COOL {
            h.failures = 0;
            h.epoch += 1;
            let now = Instant::now();
            h.cooling_until = Some(now.checked_add(self.cooldown).unwrap_or(now));
            tracing::warn!(
                network = self.net,
                reason = outcome.label(),
                "chain logs portal: reading from the RPC while the portal cools"
            );
        }
    }
}

/// What the portal phase settled: the portal's logs up to `reached`, and
/// the groups the RPC reads.
struct Plan {
    logs: Vec<RpcLog>,
    reached: u64,
    rpc_groups: Vec<LogGroup>,
    outcome: Outcome,
}

/// What a window sent before it read the whole of it from the RPC: the
/// anchor, the RPC logs of a split it discarded, and the portal requests.
#[derive(Clone, Copy, Debug, Default)]
struct Sent {
    anchor: usize,
    logs: usize,
    portal: usize,
}

/// Records a window's portal sends when it is dropped, however it ends.
struct Flush<'r> {
    net: &'static str,
    reader: &'r PortalReader,
}

impl Drop for Flush<'_> {
    fn drop(&mut self) {
        record_sends(self.net, self.reader.sends());
    }
}

/// Records a window's RPC sends of one kind when it is dropped, however the
/// read ends: counted as each went out.
struct RpcFlush<'c> {
    net: &'static str,
    kind: &'static str,
    sent: &'c AtomicUsize,
}

impl Drop for RpcFlush<'_> {
    fn drop(&mut self) {
        let n = self.sent.load(Ordering::SeqCst);
        if n > 0 {
            dcl_metrics::counter!(RPC_SENDS, "net" => self.net, "kind" => self.kind)
                .increment(n as u64);
        }
    }
}

/// The walk's source in portal mode: `inner` (the RPC) for everything but
/// the windows `admission` admits. Without an admission it is `inner`.
pub struct AdmittedPortal<'a, S> {
    inner: S,
    admission: Option<&'a Admission>,
    coverage: Mutex<Option<(Instant, Arc<LakeCoverage>)>>,
    /// The chain id the walk read through this source: a window is admitted
    /// only when it is the dataset's.
    verified: Mutex<Option<u64>>,
}

impl<'a, S> AdmittedPortal<'a, S> {
    pub fn new(inner: S, admission: Option<&'a Admission>) -> Self {
        Self {
            inner,
            admission,
            coverage: Mutex::new(None),
            verified: Mutex::new(None),
        }
    }
}

fn total(s: SendCounts) -> usize {
    s.admission + s.pages + s.retries + s.headers
}

fn record_sends(net: &'static str, s: SendCounts) {
    for (kind, n) in [
        ("admission", s.admission),
        ("pages", s.pages),
        ("retries", s.retries),
        ("headers", s.headers),
    ] {
        if n > 0 {
            dcl_metrics::counter!(super::shadow::SENDS, "net" => net, "kind" => kind)
                .increment(n as u64);
        }
    }
}

fn record(net: &'static str, outcome: Outcome) {
    dcl_metrics::counter!(WINDOWS, "net" => net, "result" => outcome.label()).increment(1);
}

fn rpc_log(log: &PortalLog, hash: &str, time: u64) -> RpcLog {
    RpcLog {
        address: log.address.clone(),
        topics: log.topics.clone(),
        data: log.data.clone(),
        block_number: format!("0x{:x}", log.block),
        log_index: format!("0x{:x}", log.log_index),
        block_timestamp: Some(format!("0x{time:x}")),
        block_hash: Some(hash.to_string()),
        transaction_hash: Some(log.transaction_hash.clone()),
    }
}

/// The groups split by address: those `admitted` keeps, and the rest; each
/// with its group's own topics, empty parts left out.
fn partition(
    groups: &[LogGroup],
    admitted: impl Fn(&str) -> bool,
) -> (Vec<LogGroup>, Vec<LogGroup>) {
    let (mut portal, mut rpc) = (Vec::new(), Vec::new());
    for g in groups {
        let (yes, no): (Vec<String>, Vec<String>) =
            g.addresses.iter().cloned().partition(|a| admitted(a));
        for (part, out) in [(yes, &mut portal), (no, &mut rpc)] {
            if !part.is_empty() {
                out.push(LogGroup {
                    addresses: part,
                    topics: g.topics.clone(),
                });
            }
        }
    }
    (portal, rpc)
}

impl<S: LogSource> AdmittedPortal<'_, S> {
    /// The coverage answer: the one this source read, unless `fresh`; and
    /// how long ago it was read.
    async fn coverage(
        &self,
        reader: &PortalReader,
        fresh: bool,
    ) -> Result<(Arc<LakeCoverage>, Duration), Outcome> {
        if !fresh
            && let Some((at, c)) = self
                .coverage
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clone()
        {
            return Ok((c, at.elapsed()));
        }
        let answer = reader.contracts().await.map_err(|e| match e {
            PortalError::Malformed(_) => Outcome::CoverageRefused,
            _ => Outcome::CoverageUnavailable,
        })?;
        if answer.chain_id != reader.chain() || answer.dataset != reader.dataset() {
            return Err(Outcome::CoverageRefused);
        }
        let answer = Arc::new(answer);
        *self.coverage.lock().unwrap_or_else(|p| p.into_inner()) =
            Some((Instant::now(), answer.clone()));
        Ok((answer, Duration::ZERO))
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the window and what the phase reports while it runs"
    )]
    async fn portal_phase(
        &self,
        net: &'static str,
        reader: &PortalReader,
        coverage_ttl: Duration,
        from: u64,
        to: u64,
        groups: &[LogGroup],
        anchor_sends: &AtomicUsize,
        in_anchor: &AtomicBool,
    ) -> Result<Plan, Outcome> {
        let (mut fresh, mut readmitted, mut reread) = (false, false, false);
        loop {
            let (coverage, age) = self.coverage(reader, std::mem::take(&mut fresh)).await?;
            let above = coverage.finalized.as_ref().is_none_or(|n| from > n.number);
            if above && !reread && age > coverage_ttl {
                // The lake may have finalized past the answer since it was
                // read (a long pass): read it again, once a window.
                reread = true;
                fresh = true;
                continue;
            }
            let Some(n) = coverage.finalized.clone().filter(|_| !above) else {
                return Err(Outcome::AboveCoverage);
            };
            let upto = to.min(n.number);
            let (portal_groups, rpc_groups) = partition(groups, |a| {
                coverage
                    .from
                    .get(&a.to_ascii_lowercase())
                    .is_some_and(|f| *f <= from)
            });
            if portal_groups.is_empty() {
                return Err(Outcome::Uncovered);
            }
            let filters =
                compare::filters(&portal_groups).map_err(|_| Outcome::UnsupportedFilter)?;
            // The durable anchor: the lake's history up to N is the RPC's.
            // Counted as it is sent, whatever comes back.
            anchor_sends.fetch_add(1, Ordering::SeqCst);
            dcl_metrics::counter!(RPC_SENDS, "net" => net, "kind" => "anchor").increment(1);
            in_anchor.store(true, Ordering::SeqCst);
            let anchor = self.inner.header(n.number).await;
            in_anchor.store(false, Ordering::SeqCst);
            let anchor = anchor.map_err(|_| Outcome::AnchorUnavailable)?;
            if anchor.requests > 1 {
                let more = anchor.requests - 1;
                anchor_sends.fetch_add(more, Ordering::SeqCst);
                dcl_metrics::counter!(RPC_SENDS, "net" => net, "kind" => "anchor")
                    .increment(more as u64);
            }
            if anchor.value.hash.to_ascii_lowercase() != n.hash {
                return Err(Outcome::AnchorMismatch);
            }
            let fetched = compare::fetch(reader, from, upto, &filters, 0).await;
            if fetched
                .revisions
                .iter()
                .any(|r| r.as_deref() != Some(coverage.revision.as_str()))
            {
                if readmitted {
                    return Err(Outcome::RevisionMoved);
                }
                // Every page read is dropped: the window is admitted again
                // from the original filters under a fresh answer.
                readmitted = true;
                fresh = true;
                continue;
            }
            if !matches!(fetched.stopped, None | Some(Stop::Rounds)) {
                return Err(Outcome::PortalError);
            }
            let Some(reached) = fetched.common_upper else {
                return Err(Outcome::PortalError);
            };
            if !fetched.hash_conflicts.is_empty() || !fetched.log_conflicts.is_empty() {
                return Err(Outcome::Conflict);
            }
            let mut logs = Vec::with_capacity(fetched.logs.len());
            for log in fetched.logs.values() {
                if log.block > reached {
                    continue;
                }
                match compare::kept(&filters, log) {
                    Kept::Match => {}
                    Kept::ArityOnly => continue,
                    Kept::No => return Err(Outcome::Drift),
                }
                let Some((hash, time)) = fetched.headers.get(&log.block) else {
                    return Err(Outcome::PortalError);
                };
                logs.push(rpc_log(log, hash, *time));
            }
            let outcome = if rpc_groups.is_empty() {
                Outcome::Admitted
            } else {
                Outcome::Split
            };
            return Ok(Plan {
                logs,
                reached,
                rpc_groups,
                outcome,
            });
        }
    }

    /// The whole window from the RPC, after what the window already sent:
    /// its charge covers the RPC logs it discarded too.
    #[allow(
        clippy::too_many_arguments,
        reason = "the window, its charge and what it already sent"
    )]
    async fn whole_from_rpc(
        &self,
        net: &'static str,
        from: u64,
        to: u64,
        groups: &[LogGroup],
        chunk: &mut usize,
        logical: usize,
        sent: Sent,
    ) -> Result<Window, RpcError> {
        let whole = AtomicUsize::new(0);
        let w = {
            let _flush = RpcFlush {
                net,
                kind: "fallback",
                sent: &whole,
            };
            self.inner
                .window_counted(from, to, groups, chunk, &whole)
                .await?
        };
        let whole = whole.load(Ordering::SeqCst);
        dcl_metrics::counter!(LOGS, "net" => net, "source" => "rpc").increment(w.logs.len() as u64);
        Ok(Window {
            charge: logical.max(sent.logs + whole),
            requests: sent.anchor + sent.logs + whole,
            portal_sends: sent.portal,
            logs: w.logs,
            reached: w.reached,
        })
    }
}

#[async_trait::async_trait]
impl<S: LogSource> LogSource for AdmittedPortal<'_, S> {
    async fn chain_id(&self) -> Result<Read<u64>, RpcError> {
        let read = self.inner.chain_id().await?;
        *self.verified.lock().unwrap_or_else(|p| p.into_inner()) = Some(read.value);
        Ok(read)
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
        let Some(admission) = self.admission else {
            return self.inner.window(from, to, groups, chunk).await;
        };
        let net = admission.net;
        let logical = requests_for(groups, *chunk);
        let verified = *self.verified.lock().unwrap_or_else(|p| p.into_inner());
        if verified != Some(admission.reader.chain()) {
            record(net, Outcome::OtherChain);
            return self
                .whole_from_rpc(net, from, to, groups, chunk, logical, Sent::default())
                .await;
        }
        let Some(ticket) = admission.enter() else {
            record(net, Outcome::Cooling);
            return self
                .whole_from_rpc(net, from, to, groups, chunk, logical, Sent::default())
                .await;
        };
        let reader = admission.reader.with_own_sends();
        let flush = Flush {
            net,
            reader: &reader,
        };
        let anchor = AtomicUsize::new(0);
        let in_anchor = AtomicBool::new(false);
        let phase = tokio::time::timeout(
            admission.deadline,
            self.portal_phase(
                net,
                &reader,
                admission.coverage_ttl,
                from,
                to,
                groups,
                &anchor,
                &in_anchor,
            ),
        )
        .await
        .unwrap_or_else(|_| {
            // A deadline spent waiting on the RPC's anchor is the RPC's.
            Err(if in_anchor.load(Ordering::SeqCst) {
                Outcome::AnchorUnavailable
            } else {
                Outcome::Deadline
            })
        });
        let mut sent = Sent {
            anchor: anchor.load(Ordering::SeqCst),
            logs: 0,
            portal: total(reader.sends()),
        };
        drop(flush);
        let plan = match phase {
            Ok(plan) => plan,
            Err(outcome) => {
                admission.settle(outcome, &ticket);
                drop(ticket);
                record(net, outcome);
                return self
                    .whole_from_rpc(net, from, to, groups, chunk, logical, sent)
                    .await;
            }
        };
        let (mut logs, mut reached) = (plan.logs, plan.reached);
        let mut from_rpc = Vec::new();
        if !plan.rpc_groups.is_empty() {
            // An RPC read that fails fails the window, as in `rpc` mode; it
            // says nothing of the portal.
            let split = AtomicUsize::new(0);
            let r = {
                let _flush = RpcFlush {
                    net,
                    kind: "logs",
                    sent: &split,
                };
                self.inner
                    .window_counted(from, reached, &plan.rpc_groups, chunk, &split)
                    .await?
            };
            sent.logs = split.load(Ordering::SeqCst);
            // Both sides hold every log of their addresses up to their own
            // end: the window reaches the lower one.
            reached = reached.min(r.reached);
            // Saturates: every i64 block is at or below a larger `reached`.
            let cut = i64::try_from(reached).unwrap_or(i64::MAX);
            logs.retain(|l| log_position(l).0 <= cut);
            from_rpc = r
                .logs
                .into_iter()
                .filter(|l| log_position(l).0 <= cut)
                .collect();
        }
        // A block both sources hold logs of must be one block to both: the
        // same hash and the same time where each carries them.
        let blocks: HashMap<i64, (&str, Option<i64>)> = logs
            .iter()
            .filter_map(|l| {
                let time = l.block_timestamp.as_deref().and_then(hex_to_i64);
                Some((log_position(l).0, (l.block_hash.as_deref()?, time)))
            })
            .collect();
        let disagree = from_rpc.iter().any(|l| {
            blocks.get(&log_position(l).0).is_some_and(|(hash, time)| {
                l.block_hash
                    .as_deref()
                    .is_some_and(|h| !h.eq_ignore_ascii_case(hash))
                    || matches!(
                        (l.block_timestamp.as_deref().and_then(hex_to_i64), time),
                        (Some(a), Some(b)) if a != *b
                    )
            })
        });
        let (portal_count, rpc_count) = (logs.len(), from_rpc.len());
        logs.extend(from_rpc);
        logs.sort_by_key(log_position);
        let failed = if disagree {
            Some(Outcome::Conflict)
        } else if logs
            .windows(2)
            .any(|w| log_position(&w[0]) == log_position(&w[1]))
        {
            Some(Outcome::Duplicate)
        } else {
            None
        };
        if let Some(outcome) = failed {
            admission.settle(outcome, &ticket);
            drop(ticket);
            record(net, outcome);
            return self
                .whole_from_rpc(net, from, to, groups, chunk, logical, sent)
                .await;
        }
        admission.settle(plan.outcome, &ticket);
        drop(ticket);
        record(net, plan.outcome);
        dcl_metrics::counter!(LOGS, "net" => net, "source" => "portal")
            .increment(portal_count as u64);
        dcl_metrics::counter!(LOGS, "net" => net, "source" => "rpc").increment(rpc_count as u64);
        Ok(Window {
            logs,
            reached,
            requests: sent.anchor + sent.logs,
            charge: logical.max(sent.logs),
            portal_sends: sent.portal,
        })
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

    async fn block_times(&self, logs: &[RpcLog]) -> Result<Read<HashMap<i64, i64>>, RpcError> {
        // Logs read from the portal carry their block's time: only the
        // others need headers.
        self.inner.block_times(logs).await
    }

    fn accepted(&self, read: &compare::AcceptedView<'_>) {
        self.inner.accepted(read);
    }

    fn lane_read(&self) {
        self.inner.lane_read();
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    reason = "test support"
)]
#[cfg(test)]
mod tests;
