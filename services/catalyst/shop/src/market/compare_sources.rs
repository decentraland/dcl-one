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
//! The outcome is `equal` only when every requested block was compared and
//! nothing differed. Differences and missing coverage are both reported
//! when both occur; the exit code is 1 when anything differed, else 2 when
//! anything is uncovered, else 0. The portal's own upstream RPC calls (chain-
//! lake reads headers it does not hold from an RPC) are not visible here.

use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Context;
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
    pub missing: usize,
    pub extra: usize,
    pub mismatch: usize,
    pub timestamp: usize,
    pub block_hash: usize,
    pub drift_filtered: usize,
    pub arity_filtered: usize,
    pub branch_mismatches: usize,
    pub conflicts: usize,
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
    /// `equal`, `different` or `incomplete` (different wins when both hold).
    pub outcome: &'static str,
    pub different: bool,
    pub incomplete: bool,
    pub sends: Sends,
}

impl Report {
    /// 1 when anything differed, else 2 when anything is uncovered, else 0.
    pub fn exit_code(&self) -> i32 {
        if self.different {
            1
        } else if self.incomplete {
            2
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
        (chain_walk::INDEXER_SALES, ChainNet::Matic) => Box::new(super::sales_index::SalesIndex),
        (chain_walk::INDEXER_TRADE_EVENTS, _) => Box::new(super::trade_events::TradeEventsIndex {
            net,
            chain_id: chain_walk::mainnet_chain(net),
        }),
        (chain_walk::INDEXER_COMMITTEE, ChainNet::Matic) => {
            Box::new(super::committee_index::CommitteeIndex)
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
    let window = source
        .window(from, to, groups, chunk)
        .await
        .with_context(|| format!("eth_getLogs [{from}, {to}]"))?;
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
    anyhow::ensure!(
        before == after,
        "the chain changed while reading [{from}, {to}]"
    );
    if let (Some(p), Some(parent)) = (&predecessor, parent) {
        anyhow::ensure!(
            p.hash == parent,
            "block {} is no longer the previous window's end",
            from - 1
        );
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
pub async fn run(
    pool: &PgPool,
    http: &crate::http::Outbound,
    req: Request,
) -> anyhow::Result<Report> {
    let ix = index_for(&req.indexer, req.net)?;
    let sent = AtomicUsize::new(0);
    let source = RpcSource::new(Rpc::at(http, &req.rpc_url).counting(&sent));
    let chain = source.chain_id().await.context("eth_chainId")?.value;
    anyhow::ensure!(
        chain == req.reader.chain(),
        "the RPC serves chain {chain}, the portal dataset {} chain {}",
        req.reader.dataset(),
        req.reader.chain()
    );
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
        different: false,
        incomplete: false,
        sends: Sends {
            rpc_requests: 0,
            portal: SendCounts::default(),
            note: "the portal's own upstream RPC calls are not counted",
        },
    };
    anyhow::ensure!(
        req.from <= req.to,
        "--from {} is after --to {}",
        req.from,
        req.to
    );
    let advertised = req.reader.advertised_start().await?;
    out.coverage.advertised_start = advertised;
    let rpc_safe = match req.net {
        ChainNet::Matic => source.finalized().await?.value.number,
        ChainNet::Ethereum => source
            .head()
            .await?
            .value
            .saturating_sub(req.net.confirmations()),
    };
    let portal_final = req.reader.finalized().await?.number;
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
        compare_range(pool, &source, ix.as_ref(), &req, chain, lo, hi, &mut out).await?;
    }
    out.uncovered.sort_by_key(|u| u.from);
    out.incomplete = !out.uncovered.is_empty() || out.compared.is_empty();
    out.outcome = if out.different {
        "different"
    } else if out.incomplete {
        "incomplete"
    } else {
        "equal"
    };
    out.sends.rpc_requests = sent.load(Ordering::SeqCst);
    out.sends.portal = req.reader.sends();
    Ok(out)
}

#[allow(
    clippy::too_many_arguments,
    reason = "the run's sources, request and report"
)]
async fn compare_range(
    pool: &PgPool,
    source: &dyn LogSource,
    ix: &dyn ChainIndex,
    req: &Request,
    chain: u64,
    lo: u64,
    hi: u64,
    out: &mut Report,
) -> anyhow::Result<()> {
    let start = source.header(lo.saturating_sub(1)).await?.value;
    let anchor = (start.number, start.timestamp);
    let mut parent = (lo > 0).then_some(start.hash);
    let mut chunk = 1_000usize;
    let mut next = lo;
    while next <= hi {
        let to = (next + req.window.max(1) - 1).min(hi);
        let groups = ix.groups(pool, next, to, anchor).await?;
        let fingerprint = fingerprint(&groups);
        let Ok(filters) = compare::filters(&groups) else {
            out.uncovered.push(Uncovered {
                from: next,
                to: hi,
                reason: "unsupported_filter".into(),
            });
            return Ok(());
        };
        let mut read = rpc_read(
            source,
            chain,
            next,
            to,
            &groups,
            parent.as_deref(),
            &mut chunk,
        )
        .await?;
        let rpc_reached = read.reached;
        let mut outcome: Option<Outcome> = None;
        for _ in 0..AGREE {
            let fetched = compare::fetch(&req.reader, next, read.reached, &filters, ROUNDS).await;
            let Some(upper) = fetched.common_upper else {
                let reason = fetched
                    .stopped
                    .as_ref()
                    .and_then(|s| serde_json::to_value(s).ok())
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_else(|| "no_cover".into());
                out.uncovered.push(Uncovered {
                    from: next,
                    to: hi,
                    reason,
                });
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
                next,
                upper,
                &groups,
                parent.as_deref(),
                &mut chunk,
            )
            .await?;
            if read.reached == upper {
                outcome = Some(compare::compare(&read, &fetched, out.coverage));
                break;
            }
        }
        let Some(outcome) = outcome else {
            out.uncovered.push(Uncovered {
                from: next,
                to: hi,
                reason: "no_common_end".into(),
            });
            return Ok(());
        };
        let t = &mut out.totals;
        t.missing += outcome.missing;
        t.extra += outcome.extra;
        t.mismatch += outcome.mismatch;
        t.timestamp += outcome.timestamp;
        t.block_hash += outcome.block_hash;
        t.drift_filtered += outcome.drift_filtered;
        t.arity_filtered += outcome.arity_filtered;
        let end = read.reached;
        match &outcome.verdict {
            Verdict::Equal | Verdict::Different => {
                out.compared.push((next, end));
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
                    from: next,
                    to: end,
                    reason: "not_comparable".into(),
                });
            }
            Verdict::Unanchored | Verdict::Skipped { .. } => {
                out.uncovered.push(Uncovered {
                    from: next,
                    to: hi,
                    reason: "unanchored".into(),
                });
                out.windows.push(WindowReport {
                    from: next,
                    to,
                    filter_fingerprint: fingerprint,
                    rpc_reached,
                    outcome,
                });
                return Ok(());
            }
        }
        out.windows.push(WindowReport {
            from: next,
            to,
            filter_fingerprint: fingerprint,
            rpc_reached,
            outcome,
        });
        parent = Some(read.reached_hash.clone());
        next = end + 1;
    }
    Ok(())
}

/// The read-only connection the command uses: every transaction read-only,
/// one connection, no bootstrap.
pub async fn read_only_pool(database_url: &str) -> anyhow::Result<PgPool> {
    use std::str::FromStr;
    let opts = sqlx::postgres::PgConnectOptions::from_str(database_url)
        .context("DATABASE_URL")?
        .options([("default_transaction_read_only", "on")]);
    Ok(sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .context("connecting to DATABASE_URL")?)
}
