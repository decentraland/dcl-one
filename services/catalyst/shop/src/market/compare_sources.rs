//! `catalyst shop compare-sources`: one index's logs over a block range,
//! read from the RPC and from a portal, compared window by window. Read-only:
//! a read-only database connection (the index's filters), no lease, no
//! write, no schema bootstrap.
//!
//! The requested range is accounted for whole:
//! - a prefix below the portal dataset's advertised first block, and a
//!   suffix above the RPC's safe head (Polygon's finalized block, Ethereum's
//!   head less its confirmations) or the portal's finalized head, are
//!   reported uncovered with their reason;
//! - each window's filters are built once, with one time anchor, and asked
//!   of both sources (its fingerprint is printed);
//! - the RPC side is bracketed as the walk brackets it, and each window must
//!   extend the previous one's end hash;
//! - when the portal covers less than the RPC reached, the RPC is read again
//!   up to where the portal stopped, so the comparison is anchored at a block
//!   both sides read; the next window starts after it;
//! - a window that cannot be compared stops the run, its suffix uncovered.
//!
//! The projection, the dataset, the database's cursor of the index (when
//! bound; `database_chain` reports it) and the RPC must name one chain before
//! any window is read; otherwise the run is refused. A read that fails (an
//! outage at admission, a bracket that moves mid-run) ends the run with the
//! windows compared so far and the rest uncovered with its reason.
//!
//! The outcome is `equal` only when every requested block was compared,
//! every compared field was available on both sides, and nothing differed.
//! Differences and missing coverage are both reported when both occur; the
//! exit code is 4 when refused, else 1 when anything differed, else 3 when
//! anything is uncovered or incomplete, else 0 (2 is left to the argument
//! parser's usage error). Every outcome is a report, an unreachable database
//! included (before any chain read). A failure is reported by its stage, its
//! blocks and a stable kind (`rpc_error`, `portal_error`, `database_error`),
//! never by an upstream's text, which may echo a credential. The portal's own
//! upstream RPC calls (chain-lake reads headers it does not hold from an
//! RPC) are not visible here.

use std::sync::atomic::{AtomicUsize, Ordering};

use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use super::chain_walk::{self, ChainIndex, ChainNet, LogGroup, LogSource, RpcSource};
use crate::chain_index::portal::compare::{self, AcceptedRead, Outcome, Verdict};
use crate::chain_index::portal::{AddressHistory, Coverage, PortalReader, SendCounts};
use crate::chain_index::rpc::Rpc;

/// Drain rounds a window may take to bring every portal batch to its end.
const ROUNDS: usize = 8;
/// Times a window may re-read one side to agree on a common end.
const AGREE: usize = 4;

/// The chain moved under a read: this module's own wording, safe to report.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct Moved(String);

/// A failed read as the report may show it: the stage and its blocks, and a
/// stable kind. An upstream's text (an RPC error can carry the response it
/// got, a gateway may echo a credential-bearing URL) never enters a report.
fn reason(stage: &str, e: &anyhow::Error) -> String {
    if let Some(moved) = e.downcast_ref::<Moved>() {
        return format!("{stage}: {moved}");
    }
    if let Some(p) = e.downcast_ref::<crate::chain_index::portal::PortalError>() {
        return format!("{stage}: portal_error: {p}");
    }
    let kind = if e
        .chain()
        .any(|c| c.is::<crate::chain_index::rpc::RpcError>())
    {
        "rpc_error"
    } else if e.chain().any(|c| c.is::<sqlx::Error>()) {
        "database_error"
    } else {
        "error"
    };
    format!("{stage}: {kind}")
}

/// A failure inside the windows: where it happened, and the error.
struct Failed {
    stage: String,
    error: anyhow::Error,
}

trait At<T> {
    fn at(self, stage: impl FnOnce() -> String) -> Result<T, Failed>;
}

impl<T, E: Into<anyhow::Error>> At<T> for Result<T, E> {
    fn at(self, stage: impl FnOnce() -> String) -> Result<T, Failed> {
        self.map_err(|e| Failed {
            stage: stage(),
            error: e.into(),
        })
    }
}

/// What to compare.
pub struct Request {
    pub net: ChainNet,
    pub indexer: String,
    pub from: u64,
    pub to: u64,
    pub window: u64,
    pub rpc_url: String,
    pub reader: PortalReader,
    pub history: AddressHistory,
}

/// A range of blocks and why it was not compared.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Uncovered {
    pub from: u64,
    pub to: u64,
    pub reason: String,
}

/// One compared window.
#[derive(Clone, Debug, Serialize)]
pub struct WindowReport {
    pub from: u64,
    pub to: u64,
    pub filter_fingerprint: String,
    pub rpc_reached: u64,
    #[serde(flatten)]
    pub outcome: Outcome,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Totals {
    /// The RPC's logs in the compared prefixes.
    pub rpc_logs: usize,
    /// The portal's logs in the compared prefixes that the exact filters keep.
    pub portal_logs: usize,
    pub missing: usize,
    pub extra: usize,
    pub mismatch: usize,
    pub timestamp: usize,
    pub block_hash: usize,
    pub drift_filtered: usize,
    pub arity_filtered: usize,
    pub branch_mismatches: usize,
    pub conflicts: usize,
    /// Fields of compared logs one side lacked, by name.
    pub unavailable: std::collections::BTreeMap<&'static str, usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Sends {
    /// HTTP requests to the RPC, batches counting one.
    pub rpc_requests: usize,
    pub portal: SendCounts,
    pub note: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub network: &'static str,
    pub indexer: String,
    pub dataset: String,
    pub coverage: Coverage,
    pub requested: (u64, u64),
    pub compared: Vec<(u64, u64)>,
    pub uncovered: Vec<Uncovered>,
    pub windows: Vec<WindowReport>,
    pub totals: Totals,
    /// `refused`, `different`, `incomplete` or `equal`, in that precedence.
    pub outcome: &'static str,
    /// Why nothing was compared: the projection, the dataset, the
    /// database's cursor and the RPC do not name one chain.
    pub refused: Option<String>,
    /// The chain the database's cursor of this index is bound to; `null`
    /// when it has no row or no binding, so the filters' chain is unproven.
    pub database_chain: Option<i64>,
    pub different: bool,
    pub incomplete: bool,
    pub sends: Sends,
}

impl Report {
    /// A report of nothing compared: the run was refused before any read.
    pub fn refused(
        net: &str,
        indexer: &str,
        dataset: &str,
        from: u64,
        to: u64,
        why: String,
    ) -> Self {
        Self {
            network: match ChainNet::parse(net) {
                Some(n) => n.db_name(),
                None => "unknown",
            },
            indexer: indexer.to_string(),
            dataset: dataset.to_string(),
            coverage: Coverage {
                advertised_start: None,
                address_history: AddressHistory::Unknown,
            },
            requested: (from, to),
            compared: Vec::new(),
            uncovered: vec![Uncovered {
                from: from.min(to),
                to: to.max(from),
                reason: "refused".into(),
            }],
            windows: Vec::new(),
            totals: Totals::default(),
            outcome: "refused",
            refused: Some(why),
            database_chain: None,
            different: false,
            incomplete: true,
            sends: Sends {
                rpc_requests: 0,
                portal: SendCounts::default(),
                note: "the portal's own upstream RPC calls are not counted",
            },
        }
    }

    /// 4 when refused, else 1 when anything differed, else 3 when anything
    /// is uncovered or a compared field was unavailable, else 0. (2 is the
    /// argument parser's usage error, which prints no report.)
    pub fn exit_code(&self) -> i32 {
        if self.refused.is_some() {
            4
        } else if self.different {
            1
        } else if self.incomplete {
            3
        } else {
            0
        }
    }
}

/// The index `indexer` of `net`, as the supervisor builds it, reading
/// mainnet (the portal datasets are mainnet).
pub fn index_for(indexer: &str, net: ChainNet) -> anyhow::Result<Box<dyn ChainIndex>> {
    Ok(match (indexer, net) {
        (chain_walk::INDEXER_NFT, _) => Box::new(super::nft_index::NftIndex::new(net)),
        (chain_walk::INDEXER_SALES, ChainNet::Ethereum) => {
            Box::new(super::ethereum_sales::EthereumSalesIndex)
        }
        (chain_walk::INDEXER_SALES, ChainNet::Matic) => Box::new(super::sales_index::SalesIndex),
        (chain_walk::INDEXER_TRADE_EVENTS, _) => Box::new(super::trade_events::TradeEventsIndex {
            net,
            chain_id: net.chain_id(),
        }),
        (chain_walk::INDEXER_COMMITTEE, ChainNet::Matic) => {
            Box::new(super::committee_index::CommitteeIndex)
        }
        (chain_walk::INDEXER_COLLECTIONS, ChainNet::Matic) => {
            Box::new(super::collections_index::CollectionsIndex::new())
        }
        (chain_walk::INDEXER_THIRD_PARTY_REGISTRY, ChainNet::Matic) => {
            Box::new(super::third_party_registry::ThirdPartyRegistryIndex)
        }
        (chain_walk::INDEXER_MANA, _) => {
            Box::new(super::mana_transfers::ManaTransfersIndex { net })
        }
        (crate::rentals::index::INDEXER_RENTALS, ChainNet::Ethereum) => {
            Box::new(crate::rentals::index::RentalsIndex::mainnet())
        }
        (other, net) => anyhow::bail!("no {other} index on {}", net.db_name()),
    })
}

/// The sha256 of the groups as canonical JSON (addresses lowercased and sorted).
pub fn fingerprint(groups: &[LogGroup]) -> String {
    let canonical: Vec<serde_json::Value> = groups
        .iter()
        .map(|g| {
            let mut addresses: Vec<String> =
                g.addresses.iter().map(|a| a.to_ascii_lowercase()).collect();
            addresses.sort();
            addresses.dedup();
            serde_json::json!({"addresses": addresses, "topics": g.topics})
        })
        .collect();
    hex::encode(Sha256::digest(
        serde_json::Value::Array(canonical).to_string().as_bytes(),
    ))
}

/// One RPC read of `[from, to]` bracketed as the walk brackets it, which
/// must extend `parent` (the previous end's hash) when given.
async fn rpc_read(
    source: &dyn LogSource,
    chain_id: u64,
    from: u64,
    to: u64,
    groups: &[LogGroup],
    parent: Option<&str>,
    chunk: &mut usize,
) -> anyhow::Result<AcceptedRead> {
    let before = source.header(to).await?.value;
    let window = source.window(from, to, groups, chunk).await?;
    let times = source.block_times(&window.logs).await?.value;
    let reached = if window.reached == to {
        None
    } else {
        Some(source.header(window.reached).await?.value)
    };
    let predecessor = match parent {
        Some(_) if from > 0 => Some(source.header(from - 1).await?.value),
        _ => None,
    };
    let after = source.header(to).await?.value;
    if before != after {
        return Err(Moved(format!("the chain changed while reading [{from}, {to}]")).into());
    }
    if let (Some(p), Some(parent)) = (&predecessor, parent)
        && p.hash != parent
    {
        return Err(Moved(format!(
            "block {} is no longer the previous window's end",
            from - 1
        ))
        .into());
    }
    let header = reached.unwrap_or(after);
    Ok(AcceptedRead {
        chain_id,
        from,
        reached: window.reached,
        reached_hash: header.hash,
        groups: groups.to_vec(),
        logs: window.logs,
        times,
    })
}

/// Compares `req` over `pool`'s index state. See the module documentation.
/// A read that fails is reported, never raised: the windows compared before
/// it are kept and the rest of the range is uncovered with the reason.
pub async fn run(pool: &PgPool, http: &crate::http::Outbound, req: Request) -> Report {
    let refuse = |why: String| {
        Report::refused(
            req.net.db_name(),
            &req.indexer,
            req.reader.dataset(),
            req.from,
            req.to,
            why,
        )
    };
    if req.from > req.to {
        return refuse(format!("--from {} is after --to {}", req.from, req.to));
    }
    let ix = match index_for(&req.indexer, req.net) {
        Ok(ix) => ix,
        Err(e) => return refuse(e.to_string()),
    };
    let sent = AtomicUsize::new(0);
    let source = RpcSource::new(Rpc::at(http, &req.rpc_url).counting(&sent));
    let mut out = Report {
        network: req.net.db_name(),
        indexer: req.indexer.clone(),
        dataset: req.reader.dataset().to_string(),
        coverage: Coverage {
            advertised_start: None,
            address_history: req.history,
        },
        requested: (req.from, req.to),
        compared: Vec::new(),
        uncovered: Vec::new(),
        windows: Vec::new(),
        totals: Totals::default(),
        outcome: "incomplete",
        refused: None,
        database_chain: None,
        different: false,
        incomplete: false,
        sends: Sends {
            rpc_requests: 0,
            portal: SendCounts::default(),
            note: "the portal's own upstream RPC calls are not counted",
        },
    };
    measure(pool, &source, ix.as_ref(), &req, &mut out).await;
    out.uncovered.sort_by_key(|u| u.from);
    out.incomplete = out.incomplete || !out.uncovered.is_empty() || out.compared.is_empty();
    out.outcome = if out.refused.is_some() {
        "refused"
    } else if out.different {
        "different"
    } else if out.incomplete {
        "incomplete"
    } else {
        "equal"
    };
    out.sends.rpc_requests = sent.load(Ordering::SeqCst);
    out.sends.portal = req.reader.sends();
    out
}

/// Everything from the requested range on, into `out`.
async fn measure(
    pool: &PgPool,
    source: &dyn LogSource,
    ix: &dyn ChainIndex,
    req: &Request,
    out: &mut Report,
) {
    let all = |reason: String| Uncovered {
        from: req.from,
        to: req.to,
        reason,
    };
    // The projection, the dataset, the database's cursor and the RPC must
    // name one chain before any window is read; the dataset is known
    // before any read.
    let projection = req.net.chain_id() as u64;
    if req.reader.chain() != projection {
        out.refused = Some(format!(
            "the dataset {} serves chain {}, the {} {} projection reads chain {projection}",
            req.reader.dataset(),
            req.reader.chain(),
            req.net.db_name(),
            req.indexer
        ));
        out.uncovered.push(all("refused".into()));
        return;
    }
    // The index's filters read the database: an unreachable one ends the
    // run before any chain read. A cursor bound to another chain (a testnet
    // or staging database) is refused; an unbound one is reported.
    let cursor_chain = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT chain_id FROM shop.chain_cursor WHERE indexer = $1 AND network = $2",
    )
    .bind(&req.indexer)
    .bind(req.net.db_name())
    .fetch_optional(pool)
    .await;
    match cursor_chain {
        Err(e) => {
            let e = anyhow::Error::new(e);
            return out.uncovered.push(all(format!(
                "database_unavailable: {}",
                reason("probe", &e)
            )));
        }
        Ok(row) => out.database_chain = row.flatten(),
    }
    if let Some(c) = out.database_chain
        && c as u64 != projection
    {
        out.refused = Some(format!(
            "the database's {} {} cursor is bound to chain {c}, the projection reads chain {projection}",
            req.net.db_name(),
            req.indexer
        ));
        out.uncovered.push(all("refused".into()));
        return;
    }
    let rpc_chain = match source.chain_id().await {
        Ok(c) => c.value,
        Err(e) => {
            let e = anyhow::Error::new(e);
            return out.uncovered.push(all(format!(
                "rpc_unavailable: {}",
                reason("eth_chainId", &e)
            )));
        }
    };
    if rpc_chain != projection {
        out.refused = Some(format!(
            "the RPC serves chain {rpc_chain}, the {} {} projection reads chain {projection}",
            req.net.db_name(),
            req.indexer
        ));
        out.uncovered.push(all("refused".into()));
        return;
    }
    let advertised = match req.reader.advertised_start().await {
        Ok(a) => a,
        Err(e) => {
            let e = anyhow::Error::new(e);
            return out.uncovered.push(all(format!(
                "portal_unavailable: {}",
                reason("metadata", &e)
            )));
        }
    };
    out.coverage.advertised_start = advertised;
    let rpc_safe = match req.net {
        ChainNet::Matic => source.finalized().await.map(|f| f.value.number),
        ChainNet::Ethereum => source
            .head()
            .await
            .map(|h| h.value.saturating_sub(req.net.confirmations())),
    };
    let rpc_safe = match rpc_safe {
        Ok(n) => n,
        Err(e) => {
            let e = anyhow::Error::new(e);
            return out
                .uncovered
                .push(all(format!("rpc_unavailable: {}", reason("safe head", &e))));
        }
    };
    let portal_final = match req.reader.finalized().await {
        Ok(h) => h.number,
        Err(e) => {
            let e = anyhow::Error::new(e);
            return out.uncovered.push(all(format!(
                "portal_unavailable: {}",
                reason("finalized head", &e)
            )));
        }
    };
    let lo = req.from.max(advertised.unwrap_or(0));
    let hi = req.to.min(rpc_safe).min(portal_final);
    if lo > req.from {
        out.uncovered.push(Uncovered {
            from: req.from,
            to: (lo - 1).min(req.to),
            reason: "below_dataset_start".into(),
        });
    }
    if hi < req.to {
        out.uncovered.push(Uncovered {
            from: (hi + 1).max(req.from),
            to: req.to,
            reason: if rpc_safe < portal_final {
                "above_rpc_safe_head".into()
            } else {
                "above_portal_finalized".into()
            },
        });
    }
    if lo <= hi {
        let mut next = lo;
        if let Err(f) =
            compare_range(pool, source, ix, req, projection, lo, hi, &mut next, out).await
        {
            out.uncovered.push(Uncovered {
                from: next,
                to: hi,
                reason: format!("read_failed: {}", reason(&f.stage, &f.error)),
            });
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the run's sources, request, range and report"
)]
async fn compare_range(
    pool: &PgPool,
    source: &dyn LogSource,
    ix: &dyn ChainIndex,
    req: &Request,
    chain: u64,
    lo: u64,
    hi: u64,
    next: &mut u64,
    out: &mut Report,
) -> Result<(), Failed> {
    let start = source
        .header(lo.saturating_sub(1))
        .await
        .at(|| format!("rpc header {}", lo.saturating_sub(1)))?
        .value;
    let anchor = (start.number, start.timestamp);
    let mut parent = (lo > 0).then_some(start.hash);
    let mut chunk = 1_000usize;
    while *next <= hi {
        let from = *next;
        let to = from.saturating_add(req.window.max(1) - 1).min(hi);
        let groups = ix
            .groups(pool, from, to, anchor)
            .await
            .at(|| format!("filters [{from}, {to}]"))?;
        let fingerprint = fingerprint(&groups);
        let Ok(filters) = compare::filters(&groups) else {
            out.uncovered.push(Uncovered {
                from,
                to: hi,
                reason: "unsupported_filter".into(),
            });
            *next = hi + 1;
            return Ok(());
        };
        let mut read = rpc_read(
            source,
            chain,
            from,
            to,
            &groups,
            parent.as_deref(),
            &mut chunk,
        )
        .await
        .at(|| format!("rpc window [{from}, {to}]"))?;
        let rpc_reached = read.reached;
        let mut outcome: Option<Outcome> = None;
        for _ in 0..AGREE {
            let fetched = compare::fetch(&req.reader, from, read.reached, &filters, ROUNDS).await;
            let Some(upper) = fetched.common_upper else {
                let reason = fetched
                    .stopped
                    .as_ref()
                    .and_then(|s| serde_json::to_value(s).ok())
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_else(|| "no_cover".into());
                out.uncovered.push(Uncovered {
                    from,
                    to: hi,
                    reason,
                });
                *next = hi + 1;
                return Ok(());
            };
            if upper >= read.reached {
                outcome = Some(compare::compare(&read, &fetched, out.coverage));
                break;
            }
            // The portal stopped at `upper`: read the RPC again up to it.
            read = rpc_read(
                source,
                chain,
                from,
                upper,
                &groups,
                parent.as_deref(),
                &mut chunk,
            )
            .await
            .at(|| format!("rpc reread [{from}, {upper}]"))?;
            if read.reached == upper {
                outcome = Some(compare::compare(&read, &fetched, out.coverage));
                break;
            }
        }
        let Some(outcome) = outcome else {
            out.uncovered.push(Uncovered {
                from,
                to: hi,
                reason: "no_common_end".into(),
            });
            *next = hi + 1;
            return Ok(());
        };
        let t = &mut out.totals;
        t.rpc_logs += outcome.rpc_logs;
        t.portal_logs += outcome.portal_logs;
        t.missing += outcome.missing;
        t.extra += outcome.extra;
        t.mismatch += outcome.mismatch;
        t.timestamp += outcome.timestamp;
        t.block_hash += outcome.block_hash;
        t.drift_filtered += outcome.drift_filtered;
        t.arity_filtered += outcome.arity_filtered;
        for (field, n) in &outcome.unavailable {
            *t.unavailable.entry(field).or_default() += n;
        }
        if outcome.incomplete {
            out.incomplete = true;
        }
        let end = read.reached;
        let mut stop = false;
        match &outcome.verdict {
            Verdict::Equal | Verdict::Incomplete | Verdict::Different => {
                out.compared.push((from, end));
                if outcome.verdict == Verdict::Different {
                    out.different = true;
                }
            }
            Verdict::BranchMismatch { .. } | Verdict::Conflict => {
                if matches!(outcome.verdict, Verdict::Conflict) {
                    out.totals.conflicts += 1;
                } else {
                    out.totals.branch_mismatches += 1;
                }
                out.different = true;
                out.uncovered.push(Uncovered {
                    from,
                    to: end,
                    reason: "not_comparable".into(),
                });
            }
            Verdict::Unanchored | Verdict::Skipped { .. } => {
                out.uncovered.push(Uncovered {
                    from,
                    to: hi,
                    reason: "unanchored".into(),
                });
                stop = true;
            }
        }
        out.windows.push(WindowReport {
            from,
            to,
            filter_fingerprint: fingerprint,
            rpc_reached,
            outcome,
        });
        if stop {
            *next = hi + 1;
            return Ok(());
        }
        parent = Some(read.reached_hash.clone());
        *next = end + 1;
    }
    Ok(())
}

/// The read-only pool the command uses: every transaction read-only, one
/// connection, opened lazily (an unreachable database is reported by
/// [`run`], not raised here), no bootstrap.
pub fn read_only_pool(database_url: &str) -> Result<PgPool, String> {
    use std::str::FromStr;
    let opts = sqlx::postgres::PgConnectOptions::from_str(database_url)
        .map_err(|_| "DATABASE_URL is not a PostgreSQL connection URL".to_string())?
        .options([("default_transaction_read_only", "on")]);
    Ok(sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect_lazy_with(opts))
}

/// The command end to end, as the binary runs it: the lazy read-only pool,
/// then [`run`]. Every outcome is a report.
pub async fn command(database_url: &str, http: &crate::http::Outbound, req: Request) -> Report {
    let pool = match read_only_pool(database_url) {
        Ok(pool) => pool,
        Err(why) => {
            return Report::refused(
                req.net.db_name(),
                &req.indexer,
                req.reader.dataset(),
                req.from,
                req.to,
                why,
            );
        }
    };
    let report = run(&pool, http, req).await;
    pool.close().await;
    report
}
