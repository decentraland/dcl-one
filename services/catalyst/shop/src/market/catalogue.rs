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

/// Whether the read's transaction may write (an import that records what
/// it checked in the same snapshot).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    ReadOnly,
    ReadWrite,
}

/// The copy a network's rows were read from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Copy {
    /// The mirrors' oldest last complete pass over the networks they serve.
    Mirror { pass_at: NaiveDateTime },
    /// The index's published fold, and the projection's generation.
    Index {
        folded_block: i64,
        folded_at: NaiveDateTime,
        generation: i64,
    },
}

/// A read the gate allowed: its transaction, and the copies it covers.
pub struct CatalogueRead {
    pub tx: Transaction<'static, Postgres>,
    /// The mirrors' copy, when the read covers a network they serve.
    mirror: Option<Copy>,
    /// The networks the index owns that the read covers, with their copy.
    owned: Vec<(Network, Copy)>,
}

impl CatalogueRead {
    /// Whether the index owns `network` in this read's snapshot.
    pub fn owns(&self, network: Network) -> bool {
        self.owned.iter().any(|(n, _)| *n == network)
    }

    /// The identity of what was read, for a memo key: the mirrors' pass and
    /// each owned network's publication and generation.
    pub fn key(&self) -> String {
        let mut parts = Vec::new();
        if let Some(Copy::Mirror { pass_at }) = self.mirror {
            parts.push(format!("mirror@{}", pass_at.and_utc().timestamp_micros()));
        }
        for (network, copy) in &self.owned {
            if let Copy::Index {
                folded_block,
                generation,
                ..
            } = copy
            {
                parts.push(format!(
                    "{}:index@{folded_block}#{generation}",
                    network.db_name()
                ));
            }
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
            .iter()
            .chain(self.owned.iter().map(|(_, c)| c))
            .map(|c| match c {
                Copy::Mirror { pass_at } => *pass_at,
                Copy::Index { folded_at, .. } => *folded_at,
            })
            .min()
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
    pub collections_pass: Option<NaiveDateTime>,
    pub items_pass: Option<NaiveDateTime>,
}

impl NetworkState {
    fn pass(&self, mirror: Mirror) -> Option<NaiveDateTime> {
        match mirror {
            Mirror::Collections => self.collections_pass,
            Mirror::Items => self.items_pass,
        }
    }
}

/// The index's verdict on an owned network for `purpose` at `now`: its copy,
/// or why it cannot answer.
pub fn index_verdict(
    s: &NetworkState,
    net: ChainNet,
    purpose: Purpose,
    now: DateTime<Utc>,
) -> Result<Copy, String> {
    let name = net.db_name();
    if s.last_block.is_none() {
        return Err(format!(
            "the collections index owns {name}'s catalogue and has no cursor there"
        ));
    }
    if s.chain_id != Some(net.chain_id()) {
        return Err(format!(
            "the collections index on {name} is not bound to chain {}",
            net.chain_id()
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
    Ok(Copy::Index {
        folded_block,
        folded_at,
        generation: s.generation,
    })
}

/// The mirrors' verdict over the networks they serve (`served`, the
/// configured networks the index does not own), as [`mirrors::require_ready`]
/// and [`mirrors::require_priced`] decide it: the oldest last complete pass
/// of each mirror the read needs; with no subgraph configured, a descriptive
/// read serves the passes the tables record.
fn mirror_verdict(
    state: &MarketplaceState,
    states: &[NetworkState],
    served: &[Network],
    configured: bool,
    needs: &[Mirror],
    purpose: Purpose,
    now: DateTime<Utc>,
) -> Result<Copy, axum::response::Response> {
    let mut oldest: Option<NaiveDateTime> = None;
    for mirror in needs {
        let passes: Vec<Option<NaiveDateTime>> = if configured {
            served
                .iter()
                .map(|n| {
                    states
                        .iter()
                        .find(|s| s.network == n.db_name())
                        .and_then(|s| s.pass(*mirror))
                })
                .collect()
        } else if purpose == Purpose::Describe {
            states
                .iter()
                .filter(|s| !s.owned)
                .filter_map(|s| s.pass(*mirror))
                .map(Some)
                .collect()
        } else {
            vec![None]
        };
        let at = if passes.is_empty() || passes.iter().any(Option::is_none) {
            None
        } else {
            passes.into_iter().flatten().min()
        };
        let Some(at) = at else {
            return Err(if configured {
                mirrors::not_ready(state, *mirror, true)
            } else {
                // Nothing configured: name the subgraph of the first network
                // no index owns, the one without a source.
                let missing = [Network::Matic, Network::Ethereum]
                    .into_iter()
                    .find(|n| !states.iter().any(|s| s.owned && s.network == n.db_name()))
                    .unwrap_or(Network::Matic);
                unavailable(
                    Source::CollectionsSubgraph(missing),
                    &format!(
                        "no collections subgraph is configured, so the {} mirror has nothing to walk on {}",
                        mirror.name(),
                        missing.db_name()
                    ),
                )
            });
        };
        if purpose != Purpose::Describe && *mirror == Mirror::Items {
            mirrors::price_verdict(at, now, mirrors::price_max_age(&state.settings))
                .map_err(|why| unavailable(Mirror::Items.source(), &why))?;
        }
        oldest = Some(oldest.map_or(at, |o| o.min(at)));
    }
    let pass_at = oldest.expect("a read needs at least one mirror");
    Ok(Copy::Mirror { pass_at })
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
                (SELECT m.last_complete_pass_at FROM shop.mirror_state m \
                  WHERE m.mirror = 'collections' AND m.network = n.network) AS collections_pass, \
                (SELECT m.last_complete_pass_at FROM shop.mirror_state m \
                  WHERE m.mirror = 'items' AND m.network = n.network) AS items_pass \
         FROM unnest(ARRAY['ethereum', 'matic']) AS n(network) \
         LEFT JOIN shop.collection_projection p ON p.network = n.network \
         LEFT JOIN shop.chain_cursor c ON c.indexer = $1 AND c.network = n.network",
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
                collections_pass: r.try_get("collections_pass")?,
                items_pass: r.try_get("items_pass")?,
            })
        })
        .collect()
}

/// Opens the read of the catalogue a route answers from: the snapshot, the
/// verdict of each source it covers for `purpose`, and the transaction to
/// read the rows in. `needs` names the mirrors whose tables the route
/// reads; `scope` the network it asks for (`None`: every network, the
/// configured ones and the ones the index owns).
pub async fn begin(
    state: &MarketplaceState,
    purpose: Purpose,
    needs: &[Mirror],
    scope: Option<Network>,
    access: Access,
) -> Result<CatalogueRead, axum::response::Response> {
    let failed = |e: sqlx::Error| {
        tracing::error!(error = %e, "catalogue snapshot read failed");
        super::internal_server_error()
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
        let copy = index_verdict(s, net, purpose, now)
            .map_err(|why| unavailable(Source::CollectionsIndex, &why))?;
        owned.push((*network, copy));
    }
    // The mirrors serve the configured networks the index does not own; a
    // read scoped to an owned network reads no mirror row.
    let served: Vec<Network> = configured
        .iter()
        .copied()
        .filter(|n| !owned_networks.contains(n))
        .collect();
    let reads_mirror = match scope {
        Some(n) => !owned_networks.contains(&n),
        None => !served.is_empty() || configured.is_empty(),
    };
    let mirror = if reads_mirror {
        Some(mirror_verdict(
            state,
            &states,
            &served,
            !configured.is_empty(),
            needs,
            purpose,
            now,
        )?)
    } else {
        None
    };
    let read = CatalogueRead { tx, mirror, owned };
    for mirror_needed in needs {
        if let Some(Copy::Mirror { pass_at }) = read.mirror {
            mirrors::note_copy_at(state, *mirror_needed, pass_at);
        }
    }
    for (_, copy) in &read.owned {
        if let Copy::Index { folded_at, .. } = copy {
            mirrors::note_index_copy(*folded_at);
        }
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
        mirror: Some(Copy::Mirror {
            pass_at: DateTime::<Utc>::UNIX_EPOCH.naive_utc(),
        }),
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
            collections_pass: None,
            items_pass: None,
        }
    }

    const PURPOSES: [Purpose; 3] = [Purpose::Describe, Purpose::Priced, Purpose::Authorize];

    #[test]
    fn a_published_projection_answers_every_purpose() {
        let now = Utc::now();
        let s = ready_state(now);
        for purpose in PURPOSES {
            assert_eq!(
                index_verdict(&s, ChainNet::Matic, purpose, now),
                Ok(Copy::Index {
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
            assert!(index_verdict(&walking, ChainNet::Matic, purpose, now).is_ok());
        }
        let walked_events = NetworkState {
            walked_past: true,
            ..walking
        };
        assert!(index_verdict(&walked_events, ChainNet::Matic, Purpose::Priced, now).is_ok());
        assert!(
            index_verdict(&walked_events, ChainNet::Matic, Purpose::Authorize, now)
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
        assert!(index_verdict(&old, ChainNet::Matic, Purpose::Describe, now).is_ok());
        for purpose in [Purpose::Priced, Purpose::Authorize] {
            assert!(
                index_verdict(&old, ChainNet::Matic, purpose, now)
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
                    index_verdict(s, ChainNet::Matic, purpose, now).is_err(),
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

    #[test]
    fn a_publication_ahead_of_the_clock_past_the_skew_is_refused() {
        let now = Utc::now();
        let ahead = NetworkState {
            folded_at: Some(now.naive_utc() + MAX_CLOCK_SKEW + Duration::minutes(1)),
            ..ready_state(now)
        };
        assert!(index_verdict(&ahead, ChainNet::Matic, Purpose::Priced, now).is_err());
        let within = NetworkState {
            folded_at: Some(now.naive_utc() + Duration::minutes(1)),
            ..ready_state(now)
        };
        assert!(index_verdict(&within, ChainNet::Matic, Purpose::Priced, now).is_ok());
    }
}
