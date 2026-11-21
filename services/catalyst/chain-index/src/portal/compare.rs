//! A portal's logs set against a read the RPC walk accepted.
//!
//! The RPC side is an [`AcceptedRead`]: a window whose bracket and
//! predecessor check passed (its commit may still fail). The portal side is
//! [`fetch`]ed in batches within the portal's limits; each batch advances on
//! its own, a page at a time, and only a complete page moves it. What every
//! batch covered is the common upper.
//!
//! What was fetched and what was compared are kept apart. A comparison is
//! anchored at the highest height up to the common upper where both sides
//! hold a hash: the RPC's (its header at `reached`, or a log's `blockHash`)
//! and the portal's (a line's header). Equal hashes there let the prefix
//! from the window's start to that height be compared log by log; different
//! hashes are a branch mismatch, and no lower height is searched for an
//! equal verdict. The predecessor proves no block inside the window. With no
//! common height the read is unanchored and nothing is compared.
//!
//! The RPC side's own conflicts are not seen: `fetch_window` already keeps
//! one log per position.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::Serialize;

use super::{
    Coverage, Filter, LogRequest, MAX_BODY_BYTES, MAX_LIST, MAX_LOGS, MAX_SPLIT_DEPTH, Page,
    PortalError, PortalLog, PortalReader, pack, split,
};
use crate::logs::{LogGroup, log_position};
use crate::rpc::RpcLog;

/// A window the RPC walk accepted, as the comparison takes it.
#[derive(Clone, Debug)]
pub struct AcceptedRead {
    pub chain_id: u64,
    pub from: u64,
    pub reached: u64,
    /// The RPC header hash at `reached`, from the bracket.
    pub reached_hash: String,
    pub groups: Vec<LogGroup>,
    pub logs: Vec<RpcLog>,
    pub times: HashMap<i64, i64>,
}

/// The same, borrowed from the walk: sized before anything is copied.
#[derive(Clone, Copy, Debug)]
pub struct AcceptedView<'a> {
    pub chain_id: u64,
    pub from: u64,
    pub reached: u64,
    pub reached_hash: &'a str,
    pub groups: &'a [LogGroup],
    pub logs: &'a [RpcLog],
    pub times: &'a HashMap<i64, i64>,
}

/// What one hand-off may copy at most.
pub const MAX_OFFER_LOGS: usize = 50_000;
pub const MAX_OFFER_ADDRESSES: usize = 50_000;
pub const MAX_OFFER_BYTES: usize = 16 * 1024 * 1024;

impl AcceptedView<'_> {
    /// An upper bound of the bytes [`AcceptedView::to_owned`] copies, from
    /// lengths alone; `None` past the hand-off bounds.
    pub fn bounded_size(&self) -> Option<usize> {
        let addresses: usize = self.groups.iter().map(|g| g.addresses.len()).sum();
        if self.logs.len() > MAX_OFFER_LOGS || addresses > MAX_OFFER_ADDRESSES {
            return None;
        }
        let logs: usize = self
            .logs
            .iter()
            .map(|l| {
                l.address.len()
                    + l.data.len()
                    + l.topics.iter().map(String::len).sum::<usize>()
                    + 200
            })
            .sum();
        let groups: usize = self
            .groups
            .iter()
            .map(|g| g.addresses.len() * 48 + g.topics.to_string().len())
            .sum();
        let total = logs + groups + self.times.len() * 16;
        (total <= MAX_OFFER_BYTES).then_some(total)
    }

    pub fn to_owned(&self) -> AcceptedRead {
        AcceptedRead {
            chain_id: self.chain_id,
            from: self.from,
            reached: self.reached,
            reached_hash: self.reached_hash.to_ascii_lowercase(),
            groups: self.groups.to_vec(),
            logs: self.logs.to_vec(),
            times: self.times.clone(),
        }
    }
}

/// Why fetching stopped before every batch reached the end.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stop {
    AbovePortalFinalized,
    NoUpper,
    Truncated,
    Malformed,
    TooLarge,
    Irreducible,
    Unavailable,
    Refused,
    NotFound,
    Transport,
    Rounds,
}

impl Stop {
    fn of(e: &PortalError) -> Self {
        match e {
            PortalError::AboveHead { .. } => Stop::AbovePortalFinalized,
            PortalError::NoUpper => Stop::NoUpper,
            PortalError::Truncated => Stop::Truncated,
            PortalError::Malformed(_)
            | PortalError::ParentMismatch
            | PortalError::Unsupported(_) => Stop::Malformed,
            PortalError::TooLarge(_) => Stop::TooLarge,
            PortalError::Irreducible { .. } | PortalError::Rejected { .. } => Stop::Irreducible,
            PortalError::Unavailable { .. } => Stop::Unavailable,
            PortalError::Refused { .. } => Stop::Refused,
            PortalError::NotFound { .. } => Stop::NotFound,
            PortalError::Transport(_) => Stop::Transport,
        }
    }
}

/// What the portal returned for `[from, to]`, every complete page of every
/// batch kept: hashes and logs past the common upper too, for later anchors
/// and for conflicts.
#[derive(Clone, Debug, Default)]
pub struct Fetched {
    pub from: u64,
    pub to: u64,
    /// The highest block every batch covered.
    pub common_upper: Option<u64>,
    /// Each block a line carried: its hash and time.
    pub headers: BTreeMap<u64, (String, u64)>,
    /// Heights two lines gave different hashes.
    pub hash_conflicts: BTreeSet<u64>,
    pub logs: BTreeMap<(u64, u32), PortalLog>,
    pub log_conflicts: BTreeSet<(u64, u32)>,
    pub bytes: usize,
    pub stopped: Option<Stop>,
}

impl Fetched {
    fn absorb(&mut self, page: Page) -> Result<(), PortalError> {
        self.bytes += page.bytes;
        if self.bytes > MAX_BODY_BYTES {
            return Err(PortalError::TooLarge("sample bytes"));
        }
        for h in page.headers {
            match self.headers.get(&h.number) {
                Some((hash, _)) if *hash != h.hash => {
                    self.hash_conflicts.insert(h.number);
                }
                Some(_) => {}
                None => {
                    self.headers.insert(h.number, (h.hash, h.timestamp));
                }
            }
        }
        for at in page.conflicts {
            self.log_conflicts.insert(at);
        }
        for log in page.logs {
            let at = (log.block, log.log_index);
            match self.logs.get(&at) {
                Some(seen) if *seen != log => {
                    self.log_conflicts.insert(at);
                }
                Some(_) => {}
                None => {
                    if self.logs.len() >= MAX_LOGS {
                        return Err(PortalError::TooLarge("sample logs"));
                    }
                    self.logs.insert(at, log);
                }
            }
        }
        Ok(())
    }
}

struct Batch {
    requests: Vec<LogRequest>,
    next: u64,
    upper: Option<u64>,
    depth: u32,
    resend: bool,
}

/// The portal's answer to `filters` over `[from, to]`: one round of pages,
/// then up to `extra_rounds` more for the batches that ended early. A
/// rejected batch is halved and resent; an irreducible one stops the fetch.
/// Whatever stops it, the complete pages already absorbed are kept.
pub async fn fetch(
    reader: &PortalReader,
    from: u64,
    to: u64,
    filters: &[Filter],
    extra_rounds: usize,
) -> Fetched {
    let mut out = Fetched {
        from,
        to,
        ..Default::default()
    };
    let requests: Vec<LogRequest> = filters.iter().flat_map(|f| f.requests(MAX_LIST)).collect();
    let mut batches: Vec<Batch> = pack(from, to, requests)
        .into_iter()
        .map(|requests| Batch {
            requests,
            next: from,
            upper: None,
            depth: 0,
            resend: false,
        })
        .collect();
    if batches.is_empty() {
        // No address to ask: one page with no log request still proves the
        // blocks covered and their hashes.
        batches.push(Batch {
            requests: Vec::new(),
            next: from,
            upper: None,
            depth: 0,
            resend: false,
        });
    }
    'rounds: for _ in 0..=extra_rounds {
        let mut i = 0;
        while i < batches.len() {
            if batches[i].upper.is_some_and(|u| u >= to) {
                i += 1;
                continue;
            }
            let b = &batches[i];
            match reader.page(b.next, to, &b.requests, b.resend).await {
                Ok(page) => {
                    let upper = page.upper.number;
                    if let Err(e) = out.absorb(page) {
                        out.stopped = Some(Stop::of(&e));
                        break 'rounds;
                    }
                    let b = &mut batches[i];
                    b.upper = Some(upper);
                    b.next = upper + 1;
                    b.resend = false;
                    i += 1;
                }
                Err(e) if e.is_size_rejection() => {
                    let halves = (b.depth < MAX_SPLIT_DEPTH)
                        .then(|| split(&b.requests))
                        .flatten();
                    let Some((left, right)) = halves else {
                        out.stopped = Some(Stop::Irreducible);
                        break 'rounds;
                    };
                    let (next, upper, depth) = (b.next, b.upper, b.depth + 1);
                    let half = |requests| Batch {
                        requests,
                        next,
                        upper,
                        depth,
                        resend: true,
                    };
                    batches.splice(i..=i, [half(left), half(right)]);
                }
                Err(e) => {
                    out.stopped = Some(Stop::of(&e));
                    break 'rounds;
                }
            }
        }
        if batches.iter().all(|b| b.upper.is_some_and(|u| u >= to)) {
            break;
        }
    }
    if out.stopped.is_none() && batches.iter().any(|b| b.upper.is_none_or(|u| u < to)) {
        out.stopped = Some(Stop::Rounds);
    }
    out.common_upper = batches
        .iter()
        .map(|b| b.upper)
        .collect::<Option<Vec<u64>>>()
        .and_then(|u| u.into_iter().min());
    out
}

/// How a compared read ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Verdict {
    Equal,
    Different,
    BranchMismatch { height: u64 },
    Conflict,
    Unanchored,
    Skipped { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Outcome {
    pub verdict: Verdict,
    pub coverage: Coverage,
    /// `[from, common upper]` as fetched, when every batch covered something.
    pub fetched: Option<(u64, u64)>,
    /// The anchored prefix compared log by log.
    pub compared: Option<(u64, u64)>,
    pub compared_blocks: u64,
    /// The window's blocks not compared: fetched past the anchor, or never fetched.
    pub skipped_blocks: u64,
    pub missing: usize,
    pub extra: usize,
    pub mismatch: usize,
    pub timestamp: usize,
    pub block_hash: usize,
    /// Fields one side lacked, by name: never counted as equal.
    pub unavailable: BTreeMap<&'static str, usize>,
    /// Portal logs the exact filter drops only for their topic count, which
    /// the portal query cannot express (a trailing or lone `null`): expected.
    pub arity_filtered: usize,
    /// Portal logs no filter asked for: a sign the portal's filter semantics drift.
    pub drift_filtered: usize,
    pub stopped: Option<Stop>,
}

impl Outcome {
    fn skipped(read: &AcceptedRead, coverage: Coverage, reason: &str) -> Self {
        Self {
            verdict: Verdict::Skipped {
                reason: reason.to_string(),
            },
            coverage,
            fetched: None,
            compared: None,
            compared_blocks: 0,
            skipped_blocks: read.reached.saturating_sub(read.from) + 1,
            missing: 0,
            extra: 0,
            mismatch: 0,
            timestamp: 0,
            block_hash: 0,
            unavailable: BTreeMap::new(),
            arity_filtered: 0,
            drift_filtered: 0,
            stopped: None,
        }
    }

    /// Skipped before anything was fetched.
    pub fn skipped_before_fetch(read: &AcceptedRead, coverage: Coverage, reason: &str) -> Self {
        Self::skipped(read, coverage, reason)
    }
}

/// The filters of `read`, or why the portal cannot be asked them.
pub fn filters(groups: &[LogGroup]) -> Result<Vec<Filter>, PortalError> {
    groups.iter().map(Filter::from_group).collect()
}

enum Kept {
    Match,
    ArityOnly,
    No,
}

fn kept(filters: &[Filter], log: &PortalLog) -> Kept {
    if filters.iter().any(|f| f.matches(&log.address, &log.topics)) {
        return Kept::Match;
    }
    let constrained_ok = filters.iter().any(|f| {
        f.addresses.contains(&log.address)
            && f.positions
                .iter()
                .zip(&log.topics)
                .all(|(p, t)| p.as_ref().is_none_or(|a| a.contains(t)))
            && log.topics.len() < f.positions.len()
    });
    if constrained_ok {
        Kept::ArityOnly
    } else {
        Kept::No
    }
}

fn lower(s: &str) -> String {
    s.to_ascii_lowercase()
}

/// `read` against `fetched`. See the module documentation for the anchor.
pub fn compare(read: &AcceptedRead, fetched: &Fetched, coverage: Coverage) -> Outcome {
    let filters = match filters(&read.groups) {
        Ok(f) => f,
        Err(_) => return Outcome::skipped(read, coverage, "unsupported_filter"),
    };
    let mut out = Outcome::skipped(read, coverage, "");
    out.stopped = fetched.stopped.clone();
    let Some(common) = fetched.common_upper else {
        let reason = match &fetched.stopped {
            Some(s) => serde_json::to_value(s)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_else(|| "no_cover".into()),
            None => "no_cover".into(),
        };
        out.verdict = Verdict::Skipped { reason };
        return out;
    };
    out.fetched = Some((fetched.from, common));
    let upto = common.min(read.reached);

    // The hashes the RPC side observed, up to the common upper.
    let mut rpc_hashes: BTreeMap<u64, String> = BTreeMap::new();
    let mut rpc_conflict = false;
    let mut note =
        |block: u64, hash: String, rpc_hashes: &mut BTreeMap<u64, String>| match rpc_hashes
            .get(&block)
        {
            Some(seen) if *seen != hash => rpc_conflict = true,
            Some(_) => {}
            None => {
                rpc_hashes.insert(block, hash);
            }
        };
    for log in &read.logs {
        let block = log_position(log).0 as u64;
        if let Some(h) = log.block_hash.as_deref() {
            note(block, lower(h), &mut rpc_hashes);
        }
    }
    note(read.reached, lower(&read.reached_hash), &mut rpc_hashes);
    if rpc_conflict || fetched.hash_conflicts.iter().any(|h| *h <= upto) {
        out.verdict = Verdict::Conflict;
        return out;
    }
    let Some(anchor) = rpc_hashes
        .range(..=upto)
        .rev()
        .map(|(h, _)| *h)
        .find(|h| fetched.headers.contains_key(h))
    else {
        out.verdict = Verdict::Unanchored;
        return out;
    };
    if fetched.headers[&anchor].0 != rpc_hashes[&anchor] {
        out.verdict = Verdict::BranchMismatch { height: anchor };
        return out;
    }
    if fetched
        .log_conflicts
        .iter()
        .any(|(b, _)| *b >= read.from && *b <= anchor)
    {
        out.verdict = Verdict::Conflict;
        return out;
    }
    out.compared = Some((read.from, anchor));
    out.compared_blocks = anchor - read.from + 1;
    out.skipped_blocks = read.reached - anchor;

    let rpc: BTreeMap<(u64, u32), &RpcLog> = read
        .logs
        .iter()
        .filter_map(|l| {
            let (b, i) = log_position(l);
            (b >= read.from as i64 && b <= anchor as i64).then_some(((b as u64, i as u32), l))
        })
        .collect();
    let mut portal: BTreeMap<(u64, u32), &PortalLog> = BTreeMap::new();
    for (at, log) in fetched.logs.range((read.from, 0)..=(anchor, u32::MAX)) {
        match kept(&filters, log) {
            Kept::Match => {
                portal.insert(*at, log);
            }
            Kept::ArityOnly => out.arity_filtered += 1,
            Kept::No => out.drift_filtered += 1,
        }
    }
    let unavailable = |field: &'static str, out: &mut Outcome| {
        *out.unavailable.entry(field).or_default() += 1;
    };
    for (at, r) in &rpc {
        match portal.get(at) {
            None => out.missing += 1,
            Some(p) => {
                let same = lower(&r.address) == p.address
                    && r.topics
                        .iter()
                        .map(|t| lower(t))
                        .eq(p.topics.iter().cloned())
                    && lower(&r.data) == p.data;
                let tx = match r.transaction_hash.as_deref() {
                    Some(h) => lower(h) == p.transaction_hash,
                    None => {
                        unavailable("transaction_hash", &mut out);
                        true
                    }
                };
                if !(same && tx) {
                    out.mismatch += 1;
                }
            }
        }
    }
    out.extra = portal.keys().filter(|at| !rpc.contains_key(at)).count();
    let blocks: BTreeSet<u64> = rpc.keys().map(|(b, _)| *b).collect();
    for block in blocks {
        let Some((hash, time)) = fetched.headers.get(&block) else {
            continue;
        };
        match read.times.get(&(block as i64)) {
            Some(t) if *t as u64 == *time => {}
            Some(_) => out.timestamp += 1,
            None => unavailable("timestamp", &mut out),
        }
        let rpc_hash = read
            .logs
            .iter()
            .find(|l| log_position(l).0 as u64 == block)
            .and_then(|l| l.block_hash.as_deref());
        match rpc_hash {
            Some(h) if lower(h) == *hash => {}
            Some(_) => out.block_hash += 1,
            None => unavailable("block_hash", &mut out),
        }
    }
    out.verdict = if out.missing
        + out.extra
        + out.mismatch
        + out.timestamp
        + out.block_hash
        + out.drift_filtered
        == 0
    {
        Verdict::Equal
    } else {
        Verdict::Different
    };
    out
}

#[cfg(test)]
mod tests;
