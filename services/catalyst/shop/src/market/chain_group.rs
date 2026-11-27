//! The shared walk: projections of one network, read from one endpoint and
//! standing at the same checkpoint, walk their windows together. Each
//! window is read once for all of them (an address two of them read is
//! asked once, with the union of their first topics) and every log goes to
//! each projection whose own filter takes it; each projection then commits
//! its share with its own cursor advance, fenced by its own owner, in a fixed
//! order (nft before sales, which derives a sale's seller from it).
//!
//! What stays a projection's own: its gate, its claim, the opening of its
//! pass (binding, reorg check), its filters, its commit and its close (the
//! fold and the publish, which the supervisor runs in a task of its own).
//! Each of those runs under a deadline and catches its own panic, side by
//! side with the other members' (commits alone keep their fixed order): a
//! member that overruns it or panics leaves the group, its state suspect (the
//! supervisor gives it a fresh index and reads its cursor again), and the
//! others go on.
//!
//! What a member spends is counted twice: the requests the shared walk
//! attempted, once, in the caller's count (the measure; a cut short walk
//! keeps it too), and each member's logical debit, what the window would
//! have cost it alone, estimated ([`requests_for`] of its own filters at the
//! window's starting chunk), against its own [`Budget`]. The estimate leaves
//! out what a read spends narrowing a range the endpoint caps or halving a
//! chunk it refuses, so a shared window may cost more than its members were
//! debited; the count shows it. A shared read
//! that fails ends the group: each member goes on alone with what is left of
//! its budget, from its last committed checkpoint.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use anyhow::Context;
use futures::FutureExt;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::casts;
use super::chain_jobs::Gate;
use super::chain_walk::{
    self, Budget, ChainIndex, Cursor, Fenced, Holdings, IndexSettings, LogGroup, LogSource,
    Opening, Owner, PassOutcome, RpcSource, WrongChain, requests_for,
};
use crate::chain_index::portal::admit::{Admission, AdmittedPortal};
use crate::chain_index::portal::compare::AcceptedView;
use crate::chain_index::portal::shadow::{Shadow, Shadowed};
use crate::chain_index::rpc::{BlockHeader, Rpc, RpcLog};
use crate::jobs::LeaseControl;

/// Where a projection stands: its last block, that block's hash, and whether
/// its history is whole. Projections walk together only from the same one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    pub block: i64,
    pub hash: String,
    pub full_history: bool,
}

impl Checkpoint {
    /// `cursor`'s checkpoint, when its last block has a hash.
    pub fn of(cursor: &Cursor) -> Option<Self> {
        Some(Self {
            block: cursor.last_block,
            hash: cursor.last_block_hash.clone()?,
            full_history: cursor.full_history,
        })
    }
}

/// One projection in a shared walk.
pub struct Member {
    /// Its slot in the supervisor, which is also its commit order.
    pub slot: usize,
    pub index: Arc<dyn ChainIndex>,
    pub holdings: Arc<Holdings>,
    pub lease: Option<LeaseControl>,
    pub gate: Option<Gate>,
}

/// How a member's part of a shared walk ended.
pub enum MemberEnd {
    /// It walked to `out.indexed_to`, standing at `checkpoint`. `close` is
    /// `Some(Close)` when it reached the target and its close comes
    /// next; `None` when its budget paused it.
    Walked {
        out: PassOutcome,
        checkpoint: Option<Checkpoint>,
        close: Option<chain_walk::Close>,
    },
    /// Its gate said wait.
    Waiting(String),
    /// Its pass ended in its opening: a rebuild, a quarantine or a rewind.
    Ended(PassOutcome),
    /// It failed. `tainted`: a step overran its deadline or panicked, so its
    /// in-memory state and its last commit are uncertain.
    Failed { error: anyhow::Error, tainted: bool },
    /// It does not walk with this group: from `checkpoint` (re-read), alone
    /// next time when `solo` (backfill lanes owed, a rebuild asked, filters
    /// a shared read does not take).
    Regroup {
        checkpoint: Option<Checkpoint>,
        solo: bool,
    },
    /// A shared read failed: it goes on alone with `budget`.
    Fallback { budget: Budget },
}

/// A member's step that did not finish.
enum StepError {
    Failed(anyhow::Error),
    TimedOut,
    Panicked(String),
}

impl StepError {
    fn end(self, what: &str) -> MemberEnd {
        match self {
            StepError::Failed(error) => MemberEnd::Failed {
                error,
                tainted: false,
            },
            StepError::TimedOut => MemberEnd::Failed {
                error: anyhow::anyhow!("{what} overran the member deadline"),
                tainted: true,
            },
            StepError::Panicked(msg) => MemberEnd::Failed {
                error: anyhow::anyhow!("{what} panicked: {msg}"),
                tainted: true,
            },
        }
    }
}

/// `fut` under `deadline`, its panic caught.
async fn timed<T>(
    deadline: Duration,
    fut: impl std::future::Future<Output = anyhow::Result<T>>,
) -> Result<T, StepError> {
    match tokio::time::timeout(deadline, AssertUnwindSafe(fut).catch_unwind()).await {
        Err(_) => Err(StepError::TimedOut),
        Ok(Err(panic)) => Err(StepError::Panicked(super::chain_jobs::panic_message(
            panic.as_ref(),
        ))),
        Ok(Ok(Err(e))) => Err(StepError::Failed(e)),
        Ok(Ok(Ok(v))) => Ok(v),
    }
}

/// A read every member needs (the chain id, the heads, the checkpoint's
/// header), tried again once after half a second.
async fn common<T, F, Fut>(read: F) -> anyhow::Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    match read().await {
        Ok(v) => Ok(v),
        Err(_) => {
            tokio::time::sleep(Duration::from_millis(500)).await;
            read().await
        }
    }
}

/// Whether `ix` must walk alone next ([`ChainIndex::walks_alone`]), a
/// planning hint read under `deadline` with its panic caught: a read that
/// fails, overruns or panics is logged and says yes, so the next turn walks
/// alone and its opening asks again.
pub(crate) async fn walks_alone(pool: &PgPool, ix: &dyn ChainIndex, deadline: Duration) -> bool {
    match timed(deadline, ix.walks_alone(pool)).await {
        Ok(alone) => alone,
        Err(e) => {
            let why = match e {
                StepError::Failed(e) => format!("{e:#}"),
                StepError::TimedOut => "it overran the member deadline".to_string(),
                StepError::Panicked(msg) => format!("it panicked: {msg}"),
            };
            tracing::warn!(indexer = ix.indexer(), network = ix.net().db_name(), error = %why, "marketplace chain index: could not read whether a projection walks alone; its next turn walks alone");
            true
        }
    }
}

/// The first topics a filter takes: `None` when it takes any.
type TopicSet = Option<BTreeSet<String>>;

/// The first topics of `group`, when its `topics` constrain the first
/// position only, with a string or a non-empty list of strings; `None` when
/// it has no positions at all (any log, one without topics too). `Err` for
/// anything else, an open first position (`null`, an empty list) included:
/// it requires a first topic of any value, which a shared read does not tell
/// apart, so such a member walks alone.
fn first_topics(group: &LogGroup) -> Result<TopicSet, ()> {
    let topics = match &group.topics {
        Value::Null => return Ok(None),
        Value::Array(topics) => topics,
        _ => return Err(()),
    };
    match topics.as_slice() {
        [] => Ok(None),
        [Value::String(t)] => Ok(Some(BTreeSet::from([t.to_ascii_lowercase()]))),
        [Value::Array(list)] if list.is_empty() => Err(()),
        [Value::Array(list)] => list
            .iter()
            .map(|t| t.as_str().map(str::to_ascii_lowercase).ok_or(()))
            .collect::<Result<BTreeSet<_>, _>>()
            .map(Some),
        _ => Err(()),
    }
}

/// One member's filters, by lowercased address: the first topics each takes.
type Filter = HashMap<String, TopicSet>;

/// `groups` as a filter, or `Err` when one takes more than first topics.
pub(crate) fn filter_of(groups: &[LogGroup]) -> Result<Filter, ()> {
    let mut filter: Filter = HashMap::new();
    for group in groups {
        let topics = first_topics(group)?;
        for address in &group.addresses {
            let entry = filter
                .entry(address.to_ascii_lowercase())
                .or_insert_with(|| Some(BTreeSet::new()));
            *entry = match (entry.take(), &topics) {
                (None, _) | (_, None) => None,
                (Some(mut have), Some(more)) => {
                    have.extend(more.iter().cloned());
                    Some(have)
                }
            };
        }
    }
    Ok(filter)
}

/// Whether `filter` takes `log`.
pub(crate) fn takes(filter: &Filter, log: &RpcLog) -> bool {
    match filter.get(&log.address.to_ascii_lowercase()) {
        None => false,
        Some(None) => true,
        Some(Some(topics)) => log
            .topics
            .first()
            .is_some_and(|t| topics.contains(&t.to_ascii_lowercase())),
    }
}

/// One read for all of `filters`: every address once, with the union of the
/// first topics the members take on it, addresses grouped by that union.
pub(crate) fn union(filters: &[&Filter]) -> Vec<LogGroup> {
    let mut by_address: BTreeMap<String, TopicSet> = BTreeMap::new();
    for filter in filters {
        for (address, topics) in filter.iter() {
            let entry = by_address
                .entry(address.clone())
                .or_insert_with(|| Some(BTreeSet::new()));
            *entry = match (entry.take(), topics) {
                (None, _) | (_, None) => None,
                (Some(mut have), Some(more)) => {
                    have.extend(more.iter().cloned());
                    Some(have)
                }
            };
        }
    }
    let mut by_topics: BTreeMap<Option<Vec<String>>, Vec<String>> = BTreeMap::new();
    for (address, topics) in by_address {
        by_topics
            .entry(topics.map(|t| t.into_iter().collect()))
            .or_default()
            .push(address);
    }
    by_topics
        .into_iter()
        .map(|(topics, addresses)| LogGroup {
            addresses,
            topics: match topics {
                None => json!([]),
                Some(topics) => json!([topics]),
            },
        })
        .collect()
}

/// A member still walking.
struct Walking {
    member: Member,
    owner: Owner,
    out: PassOutcome,
    budget: Budget,
    expected: (i64, String),
    fold_since: Option<i64>,
    filter: Option<Filter>,
    groups: Vec<LogGroup>,
}

impl Walking {
    fn checkpoint(&self, full_history: bool) -> Checkpoint {
        Checkpoint {
            block: self.expected.0,
            hash: self.expected.1.clone(),
            full_history,
        }
    }
}

/// Ends `member` with a failure; a fenced one forgets its generation.
fn failed(member: &Member, end: MemberEnd) -> MemberEnd {
    if let MemberEnd::Failed { error, .. } = &end
        && error.chain().any(|c| c.downcast_ref::<Fenced>().is_some())
    {
        member
            .holdings
            .forget(member.index.indexer(), member.index.net());
    }
    end
}

/// The shared walk of `members` from `at`, read from `url`, counting in
/// `sent` every request it attempts (as it is sent, whatever its outcome),
/// so a caller reads the walk's cost even when it is cut short. Returns how
/// each member's part ended.
#[allow(
    clippy::too_many_arguments,
    reason = "the members, where they stand, their source, their deadline and the count"
)]
pub async fn walk_together(
    pool: &PgPool,
    http: &crate::http::Outbound,
    url: &str,
    settings: &IndexSettings,
    members: Vec<Member>,
    at: &Checkpoint,
    deadline: Duration,
    sent: &AtomicUsize,
    shadow: Option<&Shadow>,
    portal: Option<&Admission>,
) -> Vec<(usize, MemberEnd)> {
    let source = Shadowed::new(
        AdmittedPortal::new(RpcSource::new(Rpc::at(http, url).counting(sent)), portal),
        shadow,
    );
    let mut ends = Vec::new();
    let started = std::time::Instant::now();

    // The gates, each its own, side by side; the gate is called inside its
    // deadline and catch, so even one that panics as it is called fails alone.
    let gated = futures::future::join_all(members.into_iter().map(|member| async move {
        let verdict = match member.gate.clone() {
            None => Ok(None),
            Some(gate) => timed(deadline, async move { gate().await }).await,
        };
        (member, verdict)
    }))
    .await;
    let mut open = Vec::new();
    for (member, verdict) in gated {
        match verdict {
            Ok(None) => open.push(member),
            Ok(Some(why)) => ends.push((member.slot, MemberEnd::Waiting(why))),
            Err(e) => ends.push((member.slot, e.end("its gate"))),
        }
    }
    if open.is_empty() {
        return ends;
    }

    // The endpoint's chain and the checkpoint's header, read once.
    let common_reads = async {
        let served = common(|| async {
            Ok::<_, anyhow::Error>(source.chain_id().await.context("eth_chainId")?.value)
        })
        .await?;
        let header = common(|| async {
            Ok::<_, anyhow::Error>(
                source
                    .header(casts::to_u64(at.block).unwrap_or(0))
                    .await
                    .context("the shared checkpoint's header")?
                    .value,
            )
        })
        .await?;
        Ok::<_, anyhow::Error>((casts::to_i64(served)?, served, header))
    }
    .await;
    let (served, served_id, at_header) = match common_reads {
        Ok(r) => r,
        Err(e) => {
            for member in open {
                let error = anyhow::anyhow!("{e:#}");
                ends.push((
                    member.slot,
                    MemberEnd::Failed {
                        error,
                        tainted: false,
                    },
                ));
            }
            return ends;
        }
    };

    // Each member's claim, binding and opening, side by side.
    let at_header = &at_header;
    let source_ref = &source;
    let opened = futures::future::join_all(open.into_iter().map(|member| async move {
        let ix = member.index.clone();
        if served != ix.chain_id() {
            let error = anyhow::Error::new(WrongChain {
                served,
                expected: ix.chain_id(),
            });
            return (
                member,
                Err(MemberEnd::Failed {
                    error,
                    tainted: false,
                }),
            );
        }
        let opening = timed(deadline, async {
            let owner = member
                .holdings
                .owner(pool, member.lease.as_ref(), ix.as_ref(), settings)
                .await?;
            if ix.walks_alone(pool).await?
                || (settings.sales_enabled
                    && super::sale_history::pending(pool, ix.as_ref())
                        .await?
                        .is_some())
            {
                return Ok(Err(MemberEnd::Regroup {
                    checkpoint: None,
                    solo: true,
                }));
            }
            let cur = chain_walk::cursor(pool, ix.indexer(), ix.net())
                .await?
                .context("the chain cursor row is missing (claiming the projection creates it)")?;
            if cur.rebuild_requested_at.is_some() {
                return Ok(Err(MemberEnd::Regroup {
                    checkpoint: None,
                    solo: true,
                }));
            }
            if Checkpoint::of(&cur).as_ref() != Some(at) {
                return Ok(Err(MemberEnd::Regroup {
                    checkpoint: Checkpoint::of(&cur),
                    solo: false,
                }));
            }
            let opened = chain_walk::open(
                pool,
                source_ref,
                ix.as_ref(),
                settings,
                &owner,
                started,
                Some(at_header),
                false,
            )
            .await?;
            Ok(Ok((owner, opened)))
        })
        .await;
        let result = match opening {
            Err(e) => Err(failed(&member, e.end("its opening"))),
            Ok(Err(end)) => Err(end),
            Ok(Ok((_, Opening::Ended(out)))) => Err(MemberEnd::Ended(out)),
            // Its publication withdrawn; it recovers in a pass alone.
            Ok(Ok((_, Opening::Diverged(_)))) => Err(MemberEnd::Regroup {
                checkpoint: None,
                solo: true,
            }),
            Ok(Ok((owner, Opening::Walk(cur, out)))) => {
                // The opening read the cursor again: it walks from `at` only
                // if it still stands there.
                match cur.last_block_hash.clone() {
                    None => Err(MemberEnd::Regroup {
                        checkpoint: None,
                        solo: true,
                    }),
                    Some(_) if Checkpoint::of(&cur).as_ref() != Some(at) => {
                        Err(MemberEnd::Regroup {
                            checkpoint: Checkpoint::of(&cur),
                            solo: false,
                        })
                    }
                    Some(hash) => Ok((owner, cur, out, hash)),
                }
            }
        };
        (member, result)
    }))
    .await;
    let mut walking: Vec<Walking> = Vec::new();
    for (member, result) in opened {
        match result {
            Err(end) => ends.push((member.slot, end)),
            Ok((owner, cur, out, hash)) => walking.push(Walking {
                expected: (cur.last_block, hash),
                fold_since: cur.fold_through,
                member,
                owner,
                out,
                budget: Budget::fresh(settings),
                filter: None,
                groups: Vec::new(),
            }),
        }
    }
    if walking.is_empty() {
        return ends;
    }

    // The heads, once.
    let net = walking[0].member.index.net();
    let heads = common(|| {
        let mut probe = PassOutcome::default();
        async move { chain_walk::safe_head(source_ref, net, &mut probe).await }
    })
    .await;
    let (head, safe, finalized) = match heads {
        Ok(h) => h,
        Err(e) => {
            for w in walking {
                let error = anyhow::anyhow!("{e:#}");
                ends.push((
                    w.member.slot,
                    MemberEnd::Failed {
                        error,
                        tainted: false,
                    },
                ));
            }
            return ends;
        }
    };
    let stop = settings.stop_block(net);
    let target = chain_walk::target_block(safe, stop);
    // Each member's first walk under its generation marks its cursor's age, as
    // a walk alone does once its head reads answered: the touches run side by
    // side, each under the member deadline, so a row lock on one member holds
    // back no other. A member whose touch fails, overruns or is fenced leaves
    // the walk, and is touched again on its next pass.
    let touched = futures::future::join_all(walking.into_iter().map(|w| async move {
        let at = (w.expected.0, Some(w.expected.1.as_str()));
        let done = timed(deadline, chain_walk::touch_once(pool, &w.owner, at, stop)).await;
        (w, done)
    }))
    .await;
    let mut walking = Vec::new();
    for (w, done) in touched {
        match done {
            Ok(()) => walking.push(w),
            Err(e) => {
                let end = e.end("its start's touch");
                ends.push((w.member.slot, failed(&w.member, end)));
            }
        }
    }
    if walking.is_empty() {
        return ends;
    }
    let anchor = (at_header.number, at_header.timestamp);
    for w in &mut walking {
        w.out.head = head;
    }

    // The windows.
    let mut chunk = settings.address_chunk.max(1);
    let mut next = casts::to_u64(at.block + 1).unwrap_or(0);
    let mut first = true;
    while next <= target && !walking.is_empty() {
        let to = (next + settings.window_blocks.max(1) - 1).min(target);
        // Each member's filters for this window, its own work, side by side.
        let begin = first;
        let filtered = futures::future::join_all(walking.into_iter().map(|w| async move {
            let ix = w.member.index.clone();
            let groups = timed(deadline, async {
                if begin {
                    ix.begin_pass();
                }
                ix.groups(pool, next, to, anchor).await
            })
            .await;
            (w, groups)
        }))
        .await;
        first = false;
        walking = Vec::new();
        for (mut w, groups) in filtered {
            match groups {
                Err(e) => {
                    let end = e.end("its filters");
                    ends.push((w.member.slot, failed(&w.member, end)));
                }
                Ok(groups) => match filter_of(&groups) {
                    Err(()) => {
                        let checkpoint = Some(w.checkpoint(at.full_history));
                        ends.push((
                            w.member.slot,
                            MemberEnd::Regroup {
                                checkpoint,
                                solo: true,
                            },
                        ));
                    }
                    Ok(filter) => {
                        w.filter = Some(filter);
                        w.groups = groups;
                        walking.push(w);
                    }
                },
            }
        }
        // Each member pays what the window would have cost it alone, at the
        // chunk the window starts with.
        let window_chunk = chunk;
        let mut paying = Vec::new();
        for mut w in walking {
            let need = requests_for(&w.groups, window_chunk).max(1);
            if need > w.budget.remaining && w.budget.first_window_used {
                w.out.paused = true;
                // The walk's requests are the group's, reported once.
                w.out.requests = 0;
                w.out.elapsed_ms = casts::millis(started.elapsed());
                let checkpoint = Some(w.checkpoint(at.full_history));
                ends.push((
                    w.member.slot,
                    MemberEnd::Walked {
                        out: w.out,
                        checkpoint,
                        close: None,
                    },
                ));
                continue;
            }
            w.budget.remaining = w.budget.remaining.saturating_sub(need);
            w.budget.first_window_used = true;
            paying.push(w);
        }
        walking = paying;
        if walking.is_empty() {
            break;
        }
        let filters: Vec<&Filter> = walking
            .iter()
            .map(|w| w.filter.as_ref().expect("set above"))
            .collect();
        let groups = union(&filters);
        // The shared read, bracketed as a walk alone brackets it: the header
        // at `to` before and after the logs, their times, the header at the
        // window's actual end and the one at the checkpoint every member
        // extends (they walk from the same one each window).
        let extends = walking[0].expected.clone();
        let read = async {
            let before = match finalized.as_ref().filter(|f| f.number == to) {
                Some(f) => f.clone(),
                None => source.header(to).await?.value,
            };
            let window = source
                .window(next, to, &groups, &mut chunk)
                .await
                .with_context(|| format!("eth_getLogs [{next}, {to}]"))?;
            let times = source
                .block_times(&window.logs)
                .await
                .context("block times")?
                .value;
            let reached = if window.reached == to {
                None
            } else {
                Some(source.header(window.reached).await?.value)
            };
            let predecessor = source.header(casts::to_u64(extends.0)?).await?.value;
            let after = source.header(to).await?.value;
            anyhow::ensure!(before == after, "chain changed while fetching window");
            let header = reached.unwrap_or(after);
            let reached_at = casts::to_i64(window.reached)?;
            Ok::<_, anyhow::Error>((window, times, header, predecessor, reached_at))
        }
        .await;
        let (window, times, header, predecessor, reached_at): (
            _,
            HashMap<i64, i64>,
            BlockHeader,
            BlockHeader,
            i64,
        ) = match read {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %format!("{e:#}"), "marketplace chain index: a shared read failed; its members go on alone");
                for w in walking {
                    ends.push((w.member.slot, MemberEnd::Fallback { budget: w.budget }));
                }
                return ends;
            }
        };
        if predecessor.hash != extends.1 {
            // A stable bracket that finds the checkpoint on another branch:
            // evidence about every member's cursor. Nothing is committed;
            // each withdraws its publication and recovers in a pass alone.
            tracing::warn!(block = extends.0, stored = %extends.1, chain = %predecessor.hash, "marketplace chain index: the checkpoint a shared walk extends is on another branch; its members recover alone");
            let withdrawn = futures::future::join_all(walking.into_iter().map(|w| async move {
                let at = (w.expected.0, Some(w.expected.1.as_str()));
                let done = timed(deadline, chain_walk::withdraw(pool, &w.owner, at)).await;
                (w, done)
            }))
            .await;
            for (w, done) in withdrawn {
                let end = match done {
                    Ok(()) => MemberEnd::Regroup {
                        checkpoint: None,
                        solo: true,
                    },
                    Err(e) => failed(&w.member, e.end("withdrawing its publication")),
                };
                ends.push((w.member.slot, end));
            }
            return ends;
        }
        // The shared read is accepted (its commits may still fail): offered
        // once to the shadow, whatever each member's share.
        source.accepted(&AcceptedView {
            chain_id: served_id,
            from: next,
            reached: window.reached,
            reached_hash: &header.hash,
            groups: &groups,
            logs: &window.logs,
            times: &times,
        });
        // Each member's share, committed in the fixed order.
        walking.sort_by_key(|w| w.member.slot);
        let mut committed = Vec::new();
        for mut w in walking {
            let filter = w.filter.as_ref().expect("set above");
            let owned: Vec<RpcLog> = window
                .logs
                .iter()
                .filter(|l| takes(filter, l))
                .cloned()
                .collect();
            let ix = w.member.index.clone();
            let expected = (w.expected.0, Some(w.expected.1.as_str()));
            let written = timed(deadline, async {
                let written = chain_walk::commit_window(
                    pool,
                    ix.as_ref(),
                    &w.owner,
                    expected,
                    &header,
                    &owned,
                    &times,
                )
                .await?;
                #[cfg(test)]
                ix.window_committed().await;
                Ok(written)
            })
            .await;
            match written {
                Err(e) => {
                    let end = e.end("its commit");
                    ends.push((w.member.slot, failed(&w.member, end)));
                }
                Ok(written) => {
                    w.out.logs += owned.len();
                    w.out.rows_written += written;
                    w.out.indexed_to = reached_at;
                    w.expected = (reached_at, header.hash.clone());
                    committed.push(w);
                }
            }
        }
        walking = committed;
        next = window.reached + 1;
    }
    for mut w in walking {
        // The walk's requests are the group's, reported once by its caller;
        // a member's outcome counts only what its close spends.
        w.out.requests = 0;
        w.out.elapsed_ms = casts::millis(started.elapsed());
        let reached = casts::to_u64(w.out.indexed_to).unwrap_or(0) >= target;
        let checkpoint = Some(w.checkpoint(at.full_history));
        ends.push((
            w.member.slot,
            MemberEnd::Walked {
                close: reached.then_some(chain_walk::Close {
                    fold_since: w.fold_since,
                    remaining: w.budget.remaining,
                }),
                out: w.out,
                checkpoint,
            },
        ));
    }
    ends
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log(address: &str, topic0: &str, n: u64) -> RpcLog {
        RpcLog {
            address: address.to_string(),
            topics: vec![topic0.to_string()],
            data: "0x".into(),
            block_number: format!("0x{n:x}"),
            log_index: "0x0".into(),
            block_timestamp: None,
            block_hash: None,
            transaction_hash: None,
        }
    }

    fn group(addresses: &[&str], topics: Value) -> LogGroup {
        LogGroup {
            addresses: addresses.iter().map(|a| a.to_string()).collect(),
            topics,
        }
    }

    /// The reference: what `eth_getLogs` with `groups` returns, read off the
    /// filter's definition (an address of a group, compared without case;
    /// its first topic one the group lists, any when it lists none).
    fn reference(groups: &[LogGroup], log: &RpcLog) -> bool {
        groups.iter().any(|g| {
            let address = g
                .addresses
                .iter()
                .any(|a| a.eq_ignore_ascii_case(&log.address));
            // As Geth reads it: no positions take any log, one without
            // topics too; a first position requires a first topic.
            let positions = g.topics.as_array().unwrap();
            let topic = positions.is_empty()
                || log.topics.first().is_some_and(|t0| match &positions[0] {
                    Value::String(t) => t0.eq_ignore_ascii_case(t),
                    Value::Array(list) => list
                        .iter()
                        .any(|t| t0.eq_ignore_ascii_case(t.as_str().unwrap())),
                    _ => unreachable!("filters a shared read takes"),
                });
            address && topic
        })
    }

    #[test]
    fn the_shared_read_and_its_split_give_each_projection_what_its_own_filter_takes() {
        let (a, b, c) = (
            "0x00000000000000000000000000000000000000aa",
            "0x00000000000000000000000000000000000000bb",
            "0x00000000000000000000000000000000000000cc",
        );
        let (t1, t2, t3) = ("0xt1", "0xt2", "0xt3");
        // One projection reads a and b for t1 or t2 (a list) and c for t3
        // (a string); the other reads b (in other case) for t2 and c for
        // any topic.
        let first = vec![group(&[a, b], json!([[t1, t2]])), group(&[c], json!([t3]))];
        let second = vec![
            group(
                &[&b.to_ascii_uppercase().replace("0X", "0x")],
                json!([[t2]]),
            ),
            group(&[c], json!([])),
        ];
        let logs = [
            log(a, t1, 1),
            log(a, t3, 2), // the right address with a topic no one asks there
            log(b, t2, 3), // a log both projections take
            log(&b.to_ascii_uppercase().replace("0X", "0x"), t1, 4),
            log(c, t3, 5),
            log(c, "0xt9", 6), // the second takes any topic on c
            RpcLog {
                topics: vec![],
                ..log(c, "-", 8) // no topics: only the one with no positions takes it
            },
            log("0x00000000000000000000000000000000000000dd", t1, 7),
        ];
        let (f1, f2) = (filter_of(&first).unwrap(), filter_of(&second).unwrap());
        let shared = union(&[&f1, &f2]);
        // Every address once across the shared read's groups.
        let mut seen: Vec<&String> = shared.iter().flat_map(|g| &g.addresses).collect();
        let total = seen.len();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), total, "{shared:?}");
        // What the shared read returns, as the endpoint answers it.
        let read: Vec<&RpcLog> = logs.iter().filter(|l| reference(&shared, l)).collect();
        for (groups, filter) in [(&first, &f1), (&second, &f2)] {
            let split: Vec<u64> = read
                .iter()
                .filter(|l| takes(filter, l))
                .map(|l| u64::from_str_radix(l.block_number.trim_start_matches("0x"), 16).unwrap())
                .collect();
            let alone: Vec<u64> = logs
                .iter()
                .filter(|l| reference(groups, l))
                .map(|l| u64::from_str_radix(l.block_number.trim_start_matches("0x"), 16).unwrap())
                .collect();
            assert_eq!(split, alone, "{groups:?}");
        }
        assert_eq!(
            read.iter()
                .filter(|l| takes(&f1, l) && takes(&f2, l))
                .count(),
            2,
            "the logs both take (b for t2, c for t3) go to both"
        );
    }

    #[test]
    fn filters_beyond_the_first_topic_walk_alone() {
        assert!(filter_of(&[group(&["0xaa"], json!(["0xt1", "0xt2"]))]).is_err());
        assert!(filter_of(&[group(&["0xaa"], json!([["0xt1"], null, ["0xt2"]]))]).is_err());
        assert!(filter_of(&[group(&["0xaa"], json!([[1]]))]).is_err());
        // An open first position requires a first topic of any value, which
        // a shared read does not tell apart from no positions: alone.
        assert!(filter_of(&[group(&["0xaa"], json!([null]))]).is_err());
        assert!(filter_of(&[group(&["0xaa"], json!([[]]))]).is_err());
        // No positions take any log, one without topics too.
        let any = filter_of(&[group(&["0xaa"], json!([]))]).unwrap();
        let bare = RpcLog {
            topics: vec![],
            ..log("0xaa", "-", 1)
        };
        assert!(takes(&any, &bare));
        let first = filter_of(&[group(&["0xaa"], json!([["0xt1"]]))]).unwrap();
        assert!(!takes(&first, &bare), "a first topic is required");
    }
}
