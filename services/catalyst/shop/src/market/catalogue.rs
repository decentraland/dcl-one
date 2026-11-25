//! The gate every read of `shop.collections` / `shop.items` passes first.
//!
//! A network's catalogue rows are written by one source at a time: the
//! collections and items mirrors, or the collections index's fold once the
//! operator hands it the network (`shop.collection_projection.owned_at`,
//! [`super::collections_projection`]). [`begin`] opens one `REPEATABLE READ`
//! transaction and reads in it, per network, the ownership marker, the
//! index's cursor and projection, and the mirrors' passes. It then decides
//! readiness by the source that owns each network, and hands the route the
//! transaction, so its rows come from the same snapshot that decided it.
//!
//! A network the index owns follows the index's publication (the publish
//! marker, never `folded_block = last_block`, which every pass breaks while
//! it walks):
//! - every purpose: bound to the network's chain, out of quarantine, not
//!   diverged, its whole history walked, a fold published, the projection
//!   standing at that fold (`projected_through = folded_block`) and no walked
//!   collection waiting for its creation read (`pending = 0`). A rewind, a
//!   divergence or a hand-back clears one of them;
//! - [`Purpose::Priced`]: also the publication within
//!   [`READY_WINDOW`];
//! - [`Purpose::Authorize`]: [`Purpose::Priced`], and no collections event
//!   walked past the publication, so the fold is still every collection's
//!   state.
//!
//! A network the mirrors serve keeps the mirrors' rule, unchanged
//! ([`mirrors::require_ready`], [`mirrors::require_priced`]), over the
//! configured networks the index does not own.
//!
//! [`CatalogueRead::key`] names what was read (source, network and copy), so
//! a memo filed under one read never answers for another: a mirror pass, a
//! projection (every one raises the generation, a same-height repair and a
//! hand-back too) or an ownership change gives a new key.

use axum::response::Response;
use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::{Postgres, Row, Transaction};

use super::chain_walk::{ChainNet, INDEXER_COLLECTIONS, MAX_CLOCK_SKEW, READY_WINDOW};
use super::graph::Network;
use super::mirrors::{self, Mirror};
use super::{MarketplaceState, Source, unavailable};

/// What a read of the catalogue is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    /// What a page shows: the last publication, however old.
    Describe,
    /// A price or an availability a buyer acts on: the publication within
    /// [`READY_WINDOW`].
    Priced,
    /// What a write is allowed by: [`Purpose::Priced`], with nothing walked
    /// past the publication.
    Authorize,
}

/// Whether the read's transaction may write (a campaign whose approval
/// check and tag write share one snapshot).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    ReadOnly,
    ReadWrite,
}

/// The index's published fold an owned network is read at, and the
/// projection's generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Publication {
    pub folded_block: i64,
    pub folded_at: NaiveDateTime,
    pub generation: i64,
}

/// What the read takes from the mirrors: the pass of each mirror it needs on
/// each network that participates, and the oldest of them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct MirrorCopy {
    /// `(mirror, network, pass)`.
    passes: Vec<(Mirror, String, NaiveDateTime)>,
}

impl MirrorCopy {
    fn oldest(&self, mirror: Option<Mirror>) -> Option<NaiveDateTime> {
        self.passes
            .iter()
            .filter(|(m, _, _)| mirror.is_none_or(|w| w == *m))
            .map(|(_, _, at)| *at)
            .min()
    }
}

/// A read the gate allowed: its transaction, its clock, and the copies it
/// covers.
pub struct CatalogueRead {
    pub tx: Transaction<'static, Postgres>,
    /// The gate's clock: the trades check and the reads' trade cutoff use it
    /// too, so one instant decides them all.
    pub now: DateTime<Utc>,
    /// The mirrors' copy, when the read may return rows of a network they
    /// serve.
    mirror: Option<MirrorCopy>,
    /// The networks the index owns that the read covers, with their
    /// publication.
    owned: Vec<(Network, Publication)>,
}

impl CatalogueRead {
    /// Whether the index owns `network` in this read's snapshot.
    pub fn owns(&self, network: Network) -> bool {
        self.owned.iter().any(|(n, _)| *n == network)
    }

    /// The identity of what was read, for a memo key: every mirror pass on
    /// every network the read takes from the mirrors (not only the oldest,
    /// which a newer network's pass does not move), and each owned network's
    /// publication and generation.
    pub fn key(&self) -> String {
        let mut parts = Vec::new();
        if let Some(mirror) = &self.mirror {
            let passes: Vec<String> = mirror
                .passes
                .iter()
                .map(|(m, network, at)| {
                    format!("{}:{network}@{}", m.name(), at.and_utc().timestamp_micros())
                })
                .collect();
            parts.push(format!("mirror[{}]", passes.join(",")));
        }
        for (network, p) in &self.owned {
            parts.push(format!(
                "{}:index@{}#{}",
                network.db_name(),
                p.folded_block,
                p.generation
            ));
        }
        parts.join("|")
    }

    /// Which source answers each network the read covers (`mirror`,
    /// `index:<network>`), for the memo of a display aggregate that
    /// tolerates its own staleness (as the creator directory states it) but
    /// never answers across an ownership change.
    pub fn sources(&self) -> String {
        let mut parts = Vec::new();
        if self.mirror.is_some() {
            parts.push("mirror".to_string());
        }
        for (network, _) in &self.owned {
            parts.push(format!("index:{}", network.db_name()));
        }
        parts.join("|")
    }

    /// The oldest copy the read covers, for the answer's `as_of`.
    pub fn as_of(&self) -> Option<NaiveDateTime> {
        self.mirror
            .as_ref()
            .and_then(|m| m.oldest(None))
            .into_iter()
            .chain(self.owned.iter().map(|(_, p)| p.folded_at))
            .min()
    }

    /// The trades mirror's pass and the trade-events verdicts, read in this
    /// snapshot at the gate's clock: a read that joins the open trades
    /// decides on the trade state its rows come from.
    pub async fn trades_ready(&mut self, state: &MarketplaceState) -> Result<(), Response> {
        super::trades::ready_on(state, &mut self.tx, self.now)
            .await
            .map(|_| ())
    }

    /// [`Self::trades_ready`], typed ([`super::trades::ready_checked_on`]):
    /// the source that refused and why, `None` when it could not be read.
    pub async fn trades_checked(
        &mut self,
        state: &MarketplaceState,
    ) -> Result<(), Option<(Source, String)>> {
        super::trades::ready_checked_on(state, &mut self.tx, self.now)
            .await
            .map(|_| ())
    }
}

/// Why the gate refused a read.
#[derive(Debug)]
pub struct Refusal {
    /// The source that cannot answer and why; `None` when the snapshot
    /// could not be read (an internal error).
    pub refused: Option<(Source, String)>,
    /// The index refused: a network it owns cannot answer.
    pub by_index: bool,
    /// The networks the index owned in the refusing snapshot.
    pub owned: Vec<Network>,
}

impl Refusal {
    /// The unavailable envelope naming the source, or the internal error.
    pub fn into_response(self) -> Response {
        match self.refused {
            Some((source, message)) => unavailable(source, &message),
            None => super::internal_server_error(),
        }
    }
}

/// One network's state in the gate's snapshot.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetworkState {
    pub network: String,
    pub owned: bool,
    pub projected_through: Option<i64>,
    pub pending: i32,
    pub generation: i64,
    /// The `collections` cursor: `None` when the index never walked here.
    pub last_block: Option<i64>,
    pub chain_id: Option<i64>,
    pub quarantined: bool,
    pub diverged: bool,
    pub full_history: bool,
    pub folded_block: Option<i64>,
    pub folded_at: Option<NaiveDateTime>,
    /// A collections event walked past the published fold.
    pub walked_past: bool,
    /// The catalogue holds rows of this network (collections or items).
    pub has_rows: bool,
    pub collections_pass: Option<NaiveDateTime>,
    pub items_pass: Option<NaiveDateTime>,
    /// The chain each mirror's pass was proven on.
    pub collections_chain: Option<i64>,
    pub items_chain: Option<i64>,
}

impl NetworkState {
    /// `mirror`'s row for this network, as [`mirrors::copy_of`] reads it.
    fn pass_row(&self, mirror: Mirror) -> mirrors::PassRow {
        match mirror {
            Mirror::Collections => (
                self.network.clone(),
                self.collections_pass,
                self.collections_chain,
            ),
            Mirror::Items => (self.network.clone(), self.items_pass, self.items_chain),
        }
    }
}

/// The index's verdict on an owned network for `purpose` at `now`, the
/// network expected on `chain` (the chain this node serves for it; `None`
/// when it serves none the index walks): its publication, or why it cannot
/// answer.
pub fn index_verdict(
    s: &NetworkState,
    net: ChainNet,
    chain: Option<i64>,
    purpose: Purpose,
    now: DateTime<Utc>,
) -> Result<Publication, String> {
    let name = net.db_name();
    if s.last_block.is_none() {
        return Err(format!(
            "the collections index owns {name}'s catalogue and has no cursor there"
        ));
    }
    if !chain.is_some_and(|c| s.chain_id == Some(c)) {
        return Err(format!(
            "the collections index on {name} is not bound to the chain this node serves ({})",
            chain.map_or("none".to_string(), |c| c.to_string())
        ));
    }
    if s.quarantined {
        return Err(format!("the collections index on {name} is quarantined"));
    }
    if s.diverged {
        return Err(format!(
            "the collections index on {name} found its block on another branch and is recovering"
        ));
    }
    if !s.full_history {
        return Err(format!(
            "the collections index on {name} has not walked its whole history"
        ));
    }
    let (Some(folded_block), Some(folded_at)) = (s.folded_block, s.folded_at) else {
        return Err(format!(
            "the collections index on {name} has no published fold (catching up, or rewound after a reorg)"
        ));
    };
    if s.projected_through != Some(folded_block) {
        return Err(format!(
            "{name}'s catalogue does not stand at the index's published fold yet"
        ));
    }
    if s.pending > 0 {
        return Err(format!(
            "{} walked {name} collections wait for their creation read",
            s.pending
        ));
    }
    if purpose != Purpose::Describe {
        let age = now.naive_utc().signed_duration_since(folded_at);
        if age < -MAX_CLOCK_SKEW || age > READY_WINDOW {
            return Err(format!(
                "the collections index on {name} published {} s ago, past the {} s a price may be read from",
                age.num_seconds(),
                READY_WINDOW.num_seconds()
            ));
        }
    }
    if purpose == Purpose::Authorize && s.walked_past {
        return Err(format!(
            "the collections index on {name} has walked catalogue events it has not published yet"
        ));
    }
    Ok(Publication {
        folded_block,
        folded_at,
        generation: s.generation,
    })
}

/// The networks a mirror verdict covers.
struct MirrorScope<'a> {
    /// Every network the index does not own, with its state.
    unowned: &'a [&'a NetworkState],
    /// The networks the read may return mirror rows of: those in its scope
    /// the index does not own and the catalogue holds rows of.
    returnable: &'a [Network],
    /// The configured networks the index does not own.
    served: &'a [Network],
    /// Whether any subgraph is configured.
    configured: bool,
    /// The networks the index owns.
    owned: &'a [Network],
}

/// The mirrors' verdict for a read that may return rows of the networks the
/// index does not own:
/// - every mirror pass of those networks is bound to the chain this node
///   serves ([`mirrors::bound`], main's rule over every stored publication,
///   a network no subgraph is configured for included: its rows are read);
/// - each network the read may return rows of (`returnable`), and with
///   subgraphs configured each one the index does not own (`served`), has a
///   complete pass of each mirror the read needs, the items one within its
///   money budget for a priced read; each of those passes files the read
///   (its key). A returnable network without a pass and without a subgraph
///   names its subgraph: no pass can come;
/// - with no subgraph configured, a priced read names the subgraph of the
///   first network no index owns, and so does a descriptive read when no
///   network is owned and no pass is recorded.
fn mirror_verdict(
    state: &MarketplaceState,
    scope: &MirrorScope<'_>,
    needs: &[Mirror],
    purpose: Purpose,
    now: DateTime<Utc>,
) -> Result<MirrorCopy, (Source, String)> {
    let MirrorScope {
        unowned,
        returnable,
        served,
        configured,
        owned: owned_networks,
    } = *scope;
    let missing_subgraph = |mirror: Mirror| {
        let missing = [Network::Matic, Network::Ethereum]
            .into_iter()
            .find(|n| !owned_networks.contains(n))
            .unwrap_or(Network::Matic);
        (
            Source::CollectionsSubgraph(missing),
            format!(
                "no collections subgraph is configured, so the {} mirror has nothing to walk on {}",
                mirror.name(),
                missing.db_name()
            ),
        )
    };
    let mut copy = MirrorCopy::default();
    for mirror in needs {
        let rows: Vec<mirrors::PassRow> = unowned.iter().map(|s| s.pass_row(*mirror)).collect();
        if !mirrors::bound(state, &rows) {
            return Err((
                mirror.source(),
                format!(
                    "a {} mirror pass in this database was read from another chain than this node serves",
                    mirror.name()
                ),
            ));
        }
        if !configured && purpose != Purpose::Describe {
            return Err(missing_subgraph(*mirror));
        }
        let mut required: Vec<Network> = served.to_vec();
        required.extend(returnable.iter().copied().filter(|n| !served.contains(n)));
        // No subgraph, no index and no pass: nothing could ever serve a row.
        if required.is_empty()
            && owned_networks.is_empty()
            && rows.iter().all(|(_, at, _)| at.is_none())
        {
            return Err(missing_subgraph(*mirror));
        }
        let mut passes = Vec::new();
        for network in required {
            match rows
                .iter()
                .find(|(n, _, _)| n == network.db_name())
                .and_then(|(_, at, _)| *at)
            {
                Some(at) => passes.push((network.db_name().to_string(), at)),
                None if served.contains(&network) => {
                    return Err(mirrors::not_ready_reason(state, *mirror, true));
                }
                // No subgraph is configured for it: no pass can come.
                None => {
                    return Err((
                        Source::CollectionsSubgraph(network),
                        format!(
                            "the catalogue holds {net} rows, and no collections subgraph is configured for {net} to give the {} mirror a pass of them",
                            mirror.name(),
                            net = network.db_name(),
                        ),
                    ));
                }
            }
        }
        // With no subgraph at all, a descriptive read serves the copy the
        // tables record: every recorded pass files it and dates it.
        if !configured {
            for (network, at, _) in &rows {
                if let Some(at) = at
                    && !passes.iter().any(|(n, _)| n == network)
                {
                    passes.push((network.clone(), *at));
                }
            }
        }
        if purpose != Purpose::Describe
            && *mirror == Mirror::Items
            && let Some(oldest) = passes.iter().map(|(_, at)| *at).min()
        {
            mirrors::price_verdict(oldest, now, mirrors::price_max_age(&state.settings))
                .map_err(|why| (Mirror::Items.source(), why))?;
        }
        copy.passes
            .extend(passes.into_iter().map(|(n, at)| (*mirror, n, at)));
    }
    Ok(copy)
}

/// The networks' states in the snapshot `tx` holds.
async fn read_states(
    tx: &mut Transaction<'static, Postgres>,
) -> Result<Vec<NetworkState>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT n.network, p.owned_at IS NOT NULL AS owned, p.projected_through, \
                COALESCE(p.pending, 0) AS pending, COALESCE(p.generation, 0) AS generation, \
                c.last_block, c.chain_id, c.quarantined_at IS NOT NULL AS quarantined, \
                c.diverged_at IS NOT NULL AS diverged, COALESCE(c.full_history, false) AS full_history, \
                c.folded_block, c.folded_at, \
                EXISTS (SELECT 1 FROM shop.collection_event e \
                        WHERE e.network = n.network AND e.block_number > c.folded_block \
                          AND e.block_number <= c.last_block) AS walked_past, \
                (EXISTS (SELECT 1 FROM shop.collections r WHERE r.network = n.network) \
                 OR EXISTS (SELECT 1 FROM shop.items r WHERE r.network = n.network)) AS has_rows, \
                mc.last_complete_pass_at AS collections_pass, mc.chain_id AS collections_chain, \
                mi.last_complete_pass_at AS items_pass, mi.chain_id AS items_chain \
         FROM unnest(ARRAY['ethereum', 'matic']) AS n(network) \
         LEFT JOIN shop.collection_projection p ON p.network = n.network \
         LEFT JOIN shop.chain_cursor c ON c.indexer = $1 AND c.network = n.network \
         LEFT JOIN shop.mirror_state mc ON mc.mirror = 'collections' AND mc.network = n.network \
         LEFT JOIN shop.mirror_state mi ON mi.mirror = 'items' AND mi.network = n.network",
    )
    .bind(INDEXER_COLLECTIONS)
    .fetch_all(&mut **tx)
    .await?;
    rows.iter()
        .map(|r| {
            Ok(NetworkState {
                network: r.try_get("network")?,
                owned: r.try_get("owned")?,
                projected_through: r.try_get("projected_through")?,
                pending: r.try_get("pending")?,
                generation: r.try_get("generation")?,
                last_block: r.try_get("last_block")?,
                chain_id: r.try_get("chain_id")?,
                quarantined: r.try_get("quarantined")?,
                diverged: r.try_get("diverged")?,
                full_history: r.try_get("full_history")?,
                folded_block: r.try_get("folded_block")?,
                folded_at: r.try_get("folded_at")?,
                walked_past: r.try_get("walked_past")?,
                has_rows: r.try_get("has_rows")?,
                collections_pass: r.try_get("collections_pass")?,
                items_pass: r.try_get("items_pass")?,
                collections_chain: r.try_get("collections_chain")?,
                items_chain: r.try_get("items_chain")?,
            })
        })
        .collect()
}

/// [`begin_checked`], answering a refusal with its envelope.
pub async fn begin(
    state: &MarketplaceState,
    purpose: Purpose,
    needs: &[Mirror],
    scope: Option<Network>,
    access: Access,
) -> Result<CatalogueRead, Response> {
    begin_checked(state, purpose, needs, scope, access)
        .await
        .map_err(Refusal::into_response)
}

/// Opens the read of the catalogue a route answers from: the snapshot, the
/// verdict of each source it covers for `purpose`, and the transaction to
/// read the rows in. `needs` names the mirrors whose tables the route
/// reads; `scope` the network it asks for (`None`: every network). A read
/// covers each owned network in scope by the index's rule, and every other
/// network it may return rows of by the mirrors' rule.
///
/// The transaction holds a pooled connection until it is dropped: a route
/// releases it before any pool work that does not read the catalogue, and
/// reuses it for what does, so a request never waits on a second
/// connection while it holds the first.
pub async fn begin_checked(
    state: &MarketplaceState,
    purpose: Purpose,
    needs: &[Mirror],
    scope: Option<Network>,
    access: Access,
) -> Result<CatalogueRead, Refusal> {
    let failed = |e: sqlx::Error| {
        tracing::error!(error = %e, "catalogue snapshot read failed");
        Refusal {
            refused: None,
            by_index: false,
            owned: Vec::new(),
        }
    };
    let mut tx = state.pool.begin().await.map_err(failed)?;
    sqlx::query(match access {
        Access::ReadOnly => "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY",
        Access::ReadWrite => "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ",
    })
    .execute(&mut *tx)
    .await
    .map_err(failed)?;
    let states = read_states(&mut tx).await.map_err(failed)?;
    let now = Utc::now();
    let configured = state.settings.collection_networks();
    let owned_networks: Vec<Network> = states
        .iter()
        .filter(|s| s.owned)
        .filter_map(|s| Network::parse_db(&s.network))
        .collect();
    let refused = |source: Source, message: String, by_index: bool| Refusal {
        refused: Some((source, message)),
        by_index,
        owned: owned_networks.clone(),
    };
    // The chain this node serves for each network (a testnet reader serves
    // Sepolia and Amoy), as the mirrors' rule binds their passes.
    let pair = crate::land_authz::land_network(&state.settings.eth_network)
        .map(super::nft_index::Catalogue::from_land);
    let mut owned = Vec::new();
    for network in owned_networks
        .iter()
        .filter(|n| scope.is_none_or(|s| s == **n))
    {
        let net =
            ChainNet::parse(network.db_name()).expect("a catalogue network is a chain network");
        let s = states
            .iter()
            .find(|s| s.network == network.db_name())
            .expect("every network is read");
        let publication = index_verdict(s, net, pair.map(|p| p.chain_id(net)), purpose, now)
            .map_err(|why| refused(Source::CollectionsIndex, why, true))?;
        owned.push((*network, publication));
    }
    // Every network the index does not own that the read may return rows
    // of: the one asked for, or both. The mirrors serve the configured ones.
    let unowned: Vec<&NetworkState> = states
        .iter()
        .filter(|s| !s.owned)
        .filter(|s| scope.is_none_or(|n| n.db_name() == s.network))
        .collect();
    let mirror = if unowned.is_empty() {
        None
    } else {
        let all_unowned: Vec<&NetworkState> = states.iter().filter(|s| !s.owned).collect();
        // Polygon first, as a missing subgraph is named.
        let mut returnable: Vec<Network> = unowned
            .iter()
            .filter(|s| s.has_rows)
            .filter_map(|s| Network::parse_db(&s.network))
            .collect();
        returnable.sort_by_key(|n| *n != Network::Matic);
        let served: Vec<Network> = configured
            .iter()
            .copied()
            .filter(|n| !owned_networks.contains(n))
            .collect();
        Some(
            mirror_verdict(
                state,
                &MirrorScope {
                    unowned: &all_unowned,
                    returnable: &returnable,
                    served: &served,
                    configured: !configured.is_empty(),
                    owned: &owned_networks,
                },
                needs,
                purpose,
                now,
            )
            .map_err(|(source, why)| refused(source, why, false))?,
        )
    };
    let read = CatalogueRead {
        tx,
        now,
        mirror,
        owned,
    };
    if let Some(mirror) = &read.mirror {
        for needed in needs {
            if let Some(at) = mirror.oldest(Some(*needed)) {
                mirrors::note_copy_at(state, *needed, at);
            }
        }
    }
    for (_, p) in &read.owned {
        mirrors::note_index_copy(p.folded_at);
    }
    Ok(read)
}

/// A read of the mirrors' copy that skips readiness, for a test of what a
/// route reads rather than whether it may.
#[cfg(test)]
pub(crate) async fn begin_unchecked(state: &MarketplaceState) -> CatalogueRead {
    let mut tx = state.pool.begin().await.unwrap();
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await
        .unwrap();
    CatalogueRead {
        tx,
        now: Utc::now(),
        mirror: Some(MirrorCopy::default()),
        owned: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn ready_state(now: DateTime<Utc>) -> NetworkState {
        NetworkState {
            network: "matic".into(),
            owned: true,
            projected_through: Some(100),
            pending: 0,
            generation: 7,
            last_block: Some(100),
            chain_id: Some(137),
            quarantined: false,
            diverged: false,
            full_history: true,
            folded_block: Some(100),
            folded_at: Some(now.naive_utc() - Duration::minutes(1)),
            walked_past: false,
            has_rows: true,
            collections_pass: None,
            items_pass: None,
            collections_chain: None,
            items_chain: None,
        }
    }

    const PURPOSES: [Purpose; 3] = [Purpose::Describe, Purpose::Priced, Purpose::Authorize];

    #[test]
    fn a_published_projection_answers_every_purpose() {
        let now = Utc::now();
        let s = ready_state(now);
        for purpose in PURPOSES {
            assert_eq!(
                index_verdict(&s, ChainNet::Matic, Some(137), purpose, now),
                Ok(Publication {
                    folded_block: 100,
                    folded_at: s.folded_at.unwrap(),
                    generation: 7
                }),
                "{purpose:?}"
            );
        }
    }

    /// Every pass commits its walk before it folds and publishes: while it
    /// walks, the publication still answers prices; an authorization only
    /// while nothing was walked past it.
    #[test]
    fn a_pass_in_the_middle_of_its_walk_still_answers() {
        let now = Utc::now();
        let walking = NetworkState {
            last_block: Some(140),
            ..ready_state(now)
        };
        for purpose in PURPOSES {
            assert!(index_verdict(&walking, ChainNet::Matic, Some(137), purpose, now).is_ok());
        }
        let walked_events = NetworkState {
            walked_past: true,
            ..walking
        };
        assert!(
            index_verdict(
                &walked_events,
                ChainNet::Matic,
                Some(137),
                Purpose::Priced,
                now
            )
            .is_ok()
        );
        assert!(
            index_verdict(
                &walked_events,
                ChainNet::Matic,
                Some(137),
                Purpose::Authorize,
                now
            )
            .unwrap_err()
            .contains("not published")
        );
    }

    #[test]
    fn a_description_serves_the_last_publication_however_old() {
        let now = Utc::now();
        let old = NetworkState {
            folded_at: Some(now.naive_utc() - Duration::days(2)),
            ..ready_state(now)
        };
        assert!(index_verdict(&old, ChainNet::Matic, Some(137), Purpose::Describe, now).is_ok());
        for purpose in [Purpose::Priced, Purpose::Authorize] {
            assert!(
                index_verdict(&old, ChainNet::Matic, Some(137), purpose, now)
                    .unwrap_err()
                    .contains("past the")
            );
        }
    }

    /// A rewind, a divergence, a quarantine, a rebuild, a projection not
    /// standing at the publication or a collection waiting for its creation
    /// read refuse every purpose.
    #[test]
    fn what_breaks_the_publication_refuses_every_purpose() {
        let now = Utc::now();
        let ready = ready_state(now);
        let broken = [
            NetworkState {
                folded_block: None,
                folded_at: None,
                ..ready.clone()
            },
            NetworkState {
                projected_through: None,
                ..ready.clone()
            },
            NetworkState {
                projected_through: Some(120),
                ..ready.clone()
            },
            NetworkState {
                pending: 1,
                ..ready.clone()
            },
            NetworkState {
                diverged: true,
                ..ready.clone()
            },
            NetworkState {
                quarantined: true,
                ..ready.clone()
            },
            NetworkState {
                full_history: false,
                ..ready.clone()
            },
            NetworkState {
                chain_id: Some(80002),
                ..ready.clone()
            },
            NetworkState {
                last_block: None,
                ..ready.clone()
            },
        ];
        for s in &broken {
            for purpose in PURPOSES {
                assert!(
                    index_verdict(s, ChainNet::Matic, Some(137), purpose, now).is_err(),
                    "{s:?} {purpose:?}"
                );
            }
        }
    }

    use crate::market::collections_projection;
    use crate::market::mirrors;
    use sqlx::PgPool;

    const ETHEREUM_CONTRACT: &str = "0x00000000000000000000000000000000000000e1";
    const POLYGON_CONTRACT: &str = "0x00000000000000000000000000000000000000a1";

    /// A catalogue whose Ethereum rows the mirrors serve and whose Polygon
    /// rows the index owns, published at block 100 (generation 3), every
    /// mirror pass complete now.
    async fn owned_polygon() -> Option<(PgPool, crate::market::SharedState)> {
        let pool = crate::db::scratch::isolated_pool(4).await?;
        let state =
            crate::market::tests::support::state_with_empty_readers(Some(pool.clone())).await;
        for (network, chain, contract) in [
            (Network::Ethereum, 1, ETHEREUM_CONTRACT),
            (Network::Matic, 137, POLYGON_CONTRACT),
        ] {
            mirrors::collections::upsert(
                &pool,
                network,
                chain,
                false,
                &[mirrors::collections::fixture(contract, "C", contract, true)],
            )
            .await
            .unwrap();
        }
        sqlx::query(
            "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at) \
             SELECT m, n, 'test', timezone('UTC', now()) \
             FROM unnest(ARRAY['collections', 'items']) m CROSS JOIN unnest(ARRAY['ethereum', 'matic']) n",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, last_block_hash, \
                    full_history, chain_id, folded_block, folded_at) \
             VALUES ('collections', 'matic', 1, 100, '0xabababababababababababababababababababababababababababababababab', true, 137, 100, timezone('UTC', now()))",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shop.collection_projection (network, owned_at, projected_through, pending, generation) \
             VALUES ('matic', timezone('UTC', now()), 100, 0, 3)",
        )
        .execute(&pool)
        .await
        .unwrap();
        Some((pool, state))
    }

    async fn open(
        state: &MarketplaceState,
        purpose: Purpose,
    ) -> Result<CatalogueRead, axum::response::Response> {
        begin(state, purpose, &[Mirror::Items], None, Access::ReadOnly).await
    }

    async fn source_of(r: axum::response::Response) -> String {
        assert_eq!(r.status(), axum::http::StatusCode::SERVICE_UNAVAILABLE);
        let body = axum::body::to_bytes(r.into_body(), 1 << 16).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        v["source"].as_str().unwrap().to_string()
    }

    async fn networks_read(read: &mut CatalogueRead) -> Vec<String> {
        sqlx::query_scalar("SELECT network FROM shop.collections ORDER BY network")
            .fetch_all(&mut *read.tx)
            .await
            .unwrap()
    }

    /// One snapshot reads owned Polygon by the index's rule and Ethereum by
    /// the mirrors': a priced read follows the Ethereum mirror's money budget
    /// and ignores the mirror's Polygon pass, which the index replaced.
    #[tokio::test]
    async fn an_owned_network_is_read_beside_the_mirrors_networks() {
        let Some((pool, state)) = owned_polygon().await else {
            return;
        };
        let mut read = open(&state, Purpose::Describe).await.unwrap();
        assert!(read.owns(Network::Matic) && !read.owns(Network::Ethereum));
        assert_eq!(read.sources(), "mirror|index:matic");
        assert!(read.key().contains("matic:index@100#3"), "{}", read.key());
        assert_eq!(networks_read(&mut read).await, ["ethereum", "matic"]);

        let age = |network: &'static str, hours: i32| {
            let pool = pool.clone();
            async move {
                sqlx::query(
                    "UPDATE shop.mirror_state SET last_complete_pass_at = timezone('UTC', now()) - make_interval(hours => $2) \
                     WHERE network = $1",
                )
                .bind(network)
                .bind(hours)
                .execute(&pool)
                .await
                .unwrap();
            }
        };
        age("ethereum", 6).await;
        let refused = open(&state, Purpose::Priced).await.err().unwrap();
        assert_eq!(source_of(refused).await, "items-mirror");
        age("ethereum", 0).await;
        age("matic", 72).await;
        assert!(open(&state, Purpose::Priced).await.is_ok());
    }

    /// Every pass commits its walk before it folds and publishes: a read in
    /// the middle of a pass answers from the publication. An authorization
    /// waits only while the walk holds a catalogue event past it.
    #[tokio::test]
    async fn a_read_in_the_middle_of_a_pass_answers() {
        let Some((pool, state)) = owned_polygon().await else {
            return;
        };
        sqlx::query("UPDATE shop.chain_cursor SET last_block = 140 WHERE indexer = 'collections'")
            .execute(&pool)
            .await
            .unwrap();
        for purpose in [Purpose::Describe, Purpose::Priced, Purpose::Authorize] {
            assert!(open(&state, purpose).await.is_ok(), "{purpose:?}");
        }
        sqlx::query(
            "INSERT INTO shop.collection_event (network, block_number, log_index, block_time, kind, contract_address) \
             VALUES ('matic', 120, 0, 1, 'set_approved', $1)",
        )
        .bind(POLYGON_CONTRACT)
        .execute(&pool)
        .await
        .unwrap();
        assert!(open(&state, Purpose::Describe).await.is_ok());
        assert!(open(&state, Purpose::Priced).await.is_ok());
        let refused = open(&state, Purpose::Authorize).await.err().unwrap();
        assert_eq!(source_of(refused).await, "collections-index");
    }

    /// A rewind (no publication, the projection cleared) refuses every
    /// purpose. A read holds its snapshot across a hand-back that commits
    /// meanwhile; the next read follows the mirrors, under another key.
    #[tokio::test]
    async fn a_rewind_refuses_and_a_hand_back_never_splits_a_read() {
        let Some((pool, state)) = owned_polygon().await else {
            return;
        };
        let mut held = open(&state, Purpose::Describe).await.unwrap();
        let held_key = held.key();
        assert!(
            collections_projection::hand_back(&pool, ChainNet::Matic)
                .await
                .unwrap()
        );
        assert!(held.owns(Network::Matic));
        assert_eq!(networks_read(&mut held).await, ["ethereum", "matic"]);
        drop(held);

        // The hand-back cleared the mirror's Polygon pass with the rows:
        // until its next pass, Polygon has no copy to read.
        let refused = open(&state, Purpose::Describe).await.err().unwrap();
        assert_eq!(source_of(refused).await, "items-mirror");
        sqlx::query(
            "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at) \
             SELECT m, 'matic', 'test', timezone('UTC', now()) FROM unnest(ARRAY['collections', 'items']) m",
        )
        .execute(&pool)
        .await
        .unwrap();
        let mut read = open(&state, Purpose::Describe).await.unwrap();
        assert!(!read.owns(Network::Matic));
        assert_eq!(read.sources(), "mirror");
        assert_ne!(read.key(), held_key);
        assert_eq!(networks_read(&mut read).await, ["ethereum"]);

        // Owned again, rewound: no publication, the projection cleared.
        sqlx::query(
            "UPDATE shop.collection_projection SET owned_at = timezone('UTC', now()), projected_through = NULL \
             WHERE network = 'matic'",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE shop.chain_cursor SET folded_block = NULL, folded_at = NULL WHERE indexer = 'collections'",
        )
        .execute(&pool)
        .await
        .unwrap();
        for purpose in [Purpose::Describe, Purpose::Priced, Purpose::Authorize] {
            let refused = open(&state, purpose).await.err().unwrap();
            assert_eq!(source_of(refused).await, "collections-index", "{purpose:?}");
        }
    }

    /// With no subgraph configured, a priced read answers only for the
    /// networks an index owns: unscoped, the network without a source names
    /// its subgraph; scoped to the owned network, it answers.
    #[tokio::test]
    async fn without_a_subgraph_a_priced_read_needs_a_source_per_network() {
        let Some((pool, _)) = owned_polygon().await else {
            return;
        };
        let state = std::sync::Arc::new(crate::market::MarketplaceState::new(
            pool.clone(),
            crate::http::Outbound::for_tests(),
            crate::market::Settings::for_tests(),
        ));
        assert!(state.settings.collection_networks().is_empty());
        let refused = open(&state, Purpose::Priced).await.err().unwrap();
        assert_eq!(source_of(refused).await, "collections-subgraph:ethereum");
        let read = begin(
            &state,
            Purpose::Priced,
            &[Mirror::Items],
            Some(Network::Matic),
            Access::ReadOnly,
        )
        .await
        .unwrap();
        assert_eq!(read.sources(), "index:matic");
        sqlx::query("UPDATE shop.collection_projection SET owned_at = NULL")
            .execute(&pool)
            .await
            .unwrap();
        let refused = open(&state, Purpose::Priced).await.err().unwrap();
        assert_eq!(source_of(refused).await, "collections-subgraph:matic");
    }

    /// Only Polygon's subgraph configured, and Polygon owned: the Ethereum
    /// rows left in the catalogue are still returned, so their mirror pass
    /// is required and files the read.
    async fn only_polygon_configured() -> Option<(PgPool, crate::market::SharedState)> {
        let (pool, base) = owned_polygon().await?;
        let state = std::sync::Arc::new(crate::market::MarketplaceState::new(
            pool.clone(),
            crate::http::Outbound::for_tests(),
            crate::market::Settings {
                eth_collections_subgraph_url: None,
                matic_collections_subgraph_url: Some("http://127.0.0.1:9".into()),
                ..base.settings.clone()
            },
        ));
        assert_eq!(state.settings.collection_networks(), [Network::Matic]);
        Some((pool, state))
    }

    async fn ethereum_read(
        state: &MarketplaceState,
        purpose: Purpose,
        scope: Option<Network>,
    ) -> Result<CatalogueRead, axum::response::Response> {
        begin(state, purpose, &[Mirror::Items], scope, Access::ReadOnly).await
    }

    /// A priced read needs a current items pass of every network it can
    /// return rows of, configured or not: a stale or a missing Ethereum pass
    /// refuses it (scoped to Ethereum or unscoped); a missing one refuses a
    /// descriptive read too, naming Ethereum's subgraph (no pass can come
    /// without it); with no Ethereum row left nothing is asked of that
    /// network.
    #[tokio::test]
    async fn a_read_needs_a_pass_of_every_network_it_can_return() {
        let Some((pool, state)) = only_polygon_configured().await else {
            return;
        };
        for scope in [Some(Network::Ethereum), None] {
            assert!(ethereum_read(&state, Purpose::Priced, scope).await.is_ok());
        }
        sqlx::query(
            "UPDATE shop.mirror_state SET last_complete_pass_at = timezone('UTC', now()) - interval '10 days', \
                    chain_id = 1 WHERE network = 'ethereum'",
        )
        .execute(&pool)
        .await
        .unwrap();
        for scope in [Some(Network::Ethereum), None] {
            let refused = ethereum_read(&state, Purpose::Priced, scope)
                .await
                .err()
                .unwrap();
            assert_eq!(source_of(refused).await, "items-mirror", "{scope:?}");
            assert!(
                ethereum_read(&state, Purpose::Describe, scope)
                    .await
                    .is_ok()
            );
        }
        sqlx::query("DELETE FROM shop.mirror_state WHERE network = 'ethereum'")
            .execute(&pool)
            .await
            .unwrap();
        for purpose in PURPOSES {
            for scope in [Some(Network::Ethereum), None] {
                let refused = ethereum_read(&state, purpose, scope).await.err().unwrap();
                assert_eq!(
                    source_of(refused).await,
                    "collections-subgraph:ethereum",
                    "{purpose:?} {scope:?}"
                );
            }
        }
        sqlx::query("DELETE FROM shop.collections WHERE network = 'ethereum'")
            .execute(&pool)
            .await
            .unwrap();
        for purpose in PURPOSES {
            let mut read = ethereum_read(&state, purpose, None).await.unwrap();
            assert_eq!(networks_read(&mut read).await, ["matic"]);
            assert!(!read.key().contains("ethereum"), "{}", read.key());
        }
    }

    /// Each pass of a network the read can return files it: a newer
    /// Ethereum pass gives an unscoped read a new key though no subgraph is
    /// configured for Ethereum.
    #[tokio::test]
    async fn a_newer_pass_of_an_unconfigured_returnable_network_gives_a_new_key() {
        let Some((pool, state)) = only_polygon_configured().await else {
            return;
        };
        let first = open(&state, Purpose::Describe).await.unwrap().key();
        assert!(first.contains("items:ethereum@"), "{first}");
        sqlx::query(
            "UPDATE shop.mirror_state SET last_complete_pass_at = last_complete_pass_at + interval '1 second' \
             WHERE network = 'ethereum'",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert_ne!(open(&state, Purpose::Describe).await.unwrap().key(), first);
    }

    /// The mirrors' rule binds every pass they serve to this node's chain
    /// ([`mirrors::copy_of`]): one proven on another chain refuses a read of
    /// a network they serve, and is ignored on a network the index owns.
    #[tokio::test]
    async fn a_mirror_pass_of_another_chain_refuses_only_where_the_mirror_serves() {
        let Some((pool, state)) = owned_polygon().await else {
            return;
        };
        let bind = |network: &'static str, chain: i64| {
            let pool = pool.clone();
            async move {
                // A testnet items pass marks its item rows (`mirror_state_verified_item_rows`).
                sqlx::query(
                    "UPDATE shop.mirror_state SET chain_id = $2, \
                            marks_item_rows = (mirror = 'items' AND $2 IN (11155111, 80002)) \
                     WHERE network = $1",
                )
                .bind(network)
                .bind(chain)
                .execute(&pool)
                .await
                .unwrap();
            }
        };
        bind("matic", 80002).await;
        bind("ethereum", 1).await;
        assert!(open(&state, Purpose::Describe).await.is_ok());
        bind("ethereum", 11155111).await;
        let refused = open(&state, Purpose::Describe).await.err().unwrap();
        assert_eq!(source_of(refused).await, "items-mirror");
    }

    /// The index is read only on the chain this node serves: a testnet
    /// reader (Amoy expected) refuses a projection bound to Polygon mainnet,
    /// and a node that serves no chain the index walks refuses it too.
    #[test]
    fn the_index_answers_only_on_the_chain_this_node_serves() {
        let now = Utc::now();
        let s = ready_state(now);
        for purpose in PURPOSES {
            assert!(index_verdict(&s, ChainNet::Matic, Some(80002), purpose, now).is_err());
            assert!(index_verdict(&s, ChainNet::Matic, None, purpose, now).is_err());
        }
    }

    /// A priced read joins the open trades: their pass marker is read in the
    /// catalogue's snapshot, so a trades pass committed after the snapshot
    /// opened does not stand for the trade state its rows come from.
    #[tokio::test]
    async fn the_trades_check_reads_the_catalogues_snapshot() {
        let Some((pool, base)) = owned_polygon().await else {
            return;
        };
        crate::market::trade_events::tests::publish_cursors(&pool).await;
        let state = std::sync::Arc::new(crate::market::MarketplaceState::new(
            pool.clone(),
            crate::http::Outbound::for_tests(),
            crate::market::Settings {
                trades_sync_upstream_url: Some("https://trades.example".into()),
                ..base.settings.clone()
            },
        ));
        sqlx::query(
            "INSERT INTO shop.trades_mirror (id, upstream_url) VALUES (1, 'https://trades.example')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let mut held = open(&state, Purpose::Priced).await.unwrap();
        sqlx::query("UPDATE shop.trades_mirror SET completed_at = timezone('UTC', now())")
            .execute(&pool)
            .await
            .unwrap();
        let refused = held.trades_ready(&state).await.err().unwrap();
        assert_eq!(source_of(refused).await, "trades-mirror");
        drop(held);
        let mut read = open(&state, Purpose::Priced).await.unwrap();
        assert!(read.trades_ready(&state).await.is_ok());
    }

    /// A memo key names every mirror pass a read takes, not only the oldest:
    /// a newer pass on one network gives a new key while `as_of` stays.
    #[tokio::test]
    async fn a_newer_pass_on_either_network_gives_a_new_key() {
        let Some((pool, state)) = owned_polygon().await else {
            return;
        };
        sqlx::query("UPDATE shop.collection_projection SET owned_at = NULL")
            .execute(&pool)
            .await
            .unwrap();
        let pass = |network: &'static str, minutes_ago: i32| {
            let pool = pool.clone();
            async move {
                sqlx::query(
                    "UPDATE shop.mirror_state SET last_complete_pass_at = \
                       date_trunc('second', timezone('UTC', now())) - make_interval(mins => $2) \
                     WHERE network = $1",
                )
                .bind(network)
                .bind(minutes_ago)
                .execute(&pool)
                .await
                .unwrap();
            }
        };
        pass("ethereum", 3).await;
        pass("matic", 2).await;
        let first = open(&state, Purpose::Describe).await.unwrap();
        pass("matic", 1).await;
        let second = open(&state, Purpose::Describe).await.unwrap();
        assert_ne!(first.key(), second.key());
        assert_eq!(
            first.as_of(),
            second.as_of(),
            "the oldest pass is Ethereum's"
        );
    }

    /// A testnet reader serves Amoy on Polygon: an owned projection bound to
    /// Polygon mainnet does not answer for it.
    #[tokio::test]
    async fn a_testnet_reader_refuses_a_mainnet_projection() {
        let Some((pool, base)) = owned_polygon().await else {
            return;
        };
        let state = std::sync::Arc::new(crate::market::MarketplaceState::new(
            pool.clone(),
            crate::http::Outbound::for_tests(),
            crate::market::Settings {
                eth_network: "sepolia".into(),
                ..base.settings.clone()
            },
        ));
        let refused = begin(
            &state,
            Purpose::Describe,
            &[Mirror::Collections],
            Some(Network::Matic),
            Access::ReadOnly,
        )
        .await
        .err()
        .unwrap();
        assert_eq!(source_of(refused).await, "collections-index");
    }

    #[test]
    fn a_publication_ahead_of_the_clock_past_the_skew_is_refused() {
        let now = Utc::now();
        let ahead = NetworkState {
            folded_at: Some(now.naive_utc() + MAX_CLOCK_SKEW + Duration::minutes(1)),
            ..ready_state(now)
        };
        assert!(index_verdict(&ahead, ChainNet::Matic, Some(137), Purpose::Priced, now).is_err());
        let within = NetworkState {
            folded_at: Some(now.naive_utc() + Duration::minutes(1)),
            ..ready_state(now)
        };
        assert!(index_verdict(&within, ChainNet::Matic, Some(137), Purpose::Priced, now).is_ok());
    }
}
