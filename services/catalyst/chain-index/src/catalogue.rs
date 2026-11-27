//! The collections index's readiness rule over the catalogue it projects
//! (`shop.collections`, `shop.items`), shared by every reader of those
//! tables: the shop's gate (`market::catalogue`) and its readers outside it,
//! and Social's badge processor, which sits below the shop.
//!
//! A network's catalogue rows are written by one source at a time: the
//! collections and items mirrors, or the index's fold once the operator
//! hands it the network (`shop.collection_projection.owned_at`). A reader
//! selects [`STATE_COLUMNS`] in the statement or snapshot that reads its
//! rows and asks [`NetworkState::owned_verdict`]: `None` on a network the
//! mirrors serve (the reader keeps its own mirror check), else the index's
//! verdict for its [`Purpose`] ([`index_verdict`]).
//!
//! It also holds the publish marker ([`published_sql_on`]) that the
//! fail-closed readers of a chain walk bound their reads by.

use std::sync::LazyLock;

use chrono::{DateTime, Duration as ChronoDuration, NaiveDateTime, Utc};
use sqlx::postgres::PgRow;
use sqlx::{PgConnection, PgPool, Postgres, Row, Transaction};

use crate::shared::collections::item_awaited;

/// The collections index's cursor (`shop.chain_cursor.indexer`).
pub const INDEXER_COLLECTIONS: &str = "collections";

/// How old a publish may be before readers stop trusting it (a stopped job).
pub const READY_WINDOW: ChronoDuration = ChronoDuration::minutes(30);

/// How far ahead of the caller's clock the database's `updated_at` may be and
/// still count as advanced.
pub const MAX_CLOCK_SKEW: ChronoDuration = ChronoDuration::minutes(10);

/// The publish marker alone as SQL, for the fail-closed readers of an index
/// whose every pass advances `last_block` before it folds and publishes (the
/// shop's `trade_events` and the `nft` readers' `owned_items::Rule::Fresh`,
/// Social's badge sales): the whole history walked, a fold published
/// (`folded_block`, cleared by every rewind, divergence, unpublish,
/// quarantine and rebuild) within [`READY_WINDOW`] of the caller's clock,
/// bound to the expected chain and out of quarantine, without
/// `folded_block = last_block`, which a pass breaks for as long as it walks
/// and folds. `folded_at` is the walker's clock: one up to
/// [`MAX_CLOCK_SKEW`] ahead of the caller's still counts. A reader bounds
/// every row it reads by that `folded_block`, in the snapshot it read it in:
/// a walk may already have committed the next window.
pub fn published_sql_on(alias: &str, now_p: &str, ethereum_chain: i64, matic_chain: i64) -> String {
    format!(
        "({alias}.full_history AND {alias}.folded_block IS NOT NULL \
          AND {alias}.chain_id = CASE {alias}.network WHEN 'ethereum' THEN {ethereum_chain}::bigint WHEN 'matic' THEN {matic_chain}::bigint END \
          AND {alias}.quarantined_at IS NULL \
          AND {alias}.folded_at <= {now_p} + interval '{skew} seconds' \
          AND {alias}.folded_at >= {now_p} - interval '{window} seconds')",
        skew = MAX_CLOCK_SKEW.num_seconds(),
        window = READY_WINDOW.num_seconds(),
    )
}

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

/// The index's published fold an owned network is read at, and the
/// projection's generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Publication {
    pub folded_block: i64,
    pub folded_at: NaiveDateTime,
    pub generation: i64,
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

/// The index's verdict on an owned network for `purpose` at `now`, the
/// network expected on `chain` (the chain this node serves for it; `None`
/// when it serves none the index walks): its publication, or why it cannot
/// answer.
pub fn index_verdict(
    s: &NetworkState,
    chain: Option<i64>,
    purpose: Purpose,
    now: DateTime<Utc>,
) -> Result<Publication, String> {
    let name = s.network.as_str();
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

/// The columns of one network's state, over a source that names the network
/// `n.network` and the joins in [`STATE_JOINS`], which alias their tables
/// `st_*` so a reader's own aliases never meet them. A reader outside the gate
/// selects them in the statement that reads its rows, so one snapshot decides
/// both, and reads them back with [`NetworkState::from_row`].
pub const STATE_COLUMNS: &str = "n.network, st_p.owned_at IS NOT NULL AS owned, st_p.projected_through, \
     COALESCE(st_p.pending, 0) AS pending, COALESCE(st_p.generation, 0) AS generation, \
     st_c.last_block, st_c.chain_id, st_c.quarantined_at IS NOT NULL AS quarantined, \
     st_c.diverged_at IS NOT NULL AS diverged, COALESCE(st_c.full_history, false) AS full_history, \
     st_c.folded_block, st_c.folded_at, \
     EXISTS (SELECT 1 FROM shop.collection_event e \
             WHERE e.network = n.network AND e.block_number > st_c.folded_block \
               AND e.block_number <= st_c.last_block) AS walked_past, \
     (EXISTS (SELECT 1 FROM shop.collections r WHERE r.network = n.network) \
      OR EXISTS (SELECT 1 FROM shop.items r WHERE r.network = n.network)) AS has_rows, \
     st_mc.last_complete_pass_at AS collections_pass, st_mc.chain_id AS collections_chain, \
     st_mi.last_complete_pass_at AS items_pass, st_mi.chain_id AS items_chain";

/// The joins [`STATE_COLUMNS`] read, by primary key.
pub static STATE_JOINS: LazyLock<String> = LazyLock::new(|| {
    format!(
        "LEFT JOIN shop.collection_projection st_p ON st_p.network = n.network \
         LEFT JOIN shop.chain_cursor st_c ON st_c.indexer = '{INDEXER_COLLECTIONS}' AND st_c.network = n.network \
         LEFT JOIN shop.mirror_state st_mc ON st_mc.mirror = 'collections' AND st_mc.network = n.network \
         LEFT JOIN shop.mirror_state st_mi ON st_mi.mirror = 'items' AND st_mi.network = n.network"
    )
});

impl NetworkState {
    /// The state [`STATE_COLUMNS`] selected in `row`.
    pub fn from_row(r: &PgRow) -> Result<Self, sqlx::Error> {
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
    }

    /// The index's verdict for `purpose` at `now` when it owns this network,
    /// on `chain` (the chain the reader serves the network on); `None` when
    /// the mirrors serve it, and the reader keeps its own mirror check.
    pub fn owned_verdict(
        &self,
        chain: i64,
        purpose: Purpose,
        now: DateTime<Utc>,
    ) -> Option<Result<Publication, String>> {
        if !self.owned {
            return None;
        }
        Some(index_verdict(self, Some(chain), purpose, now))
    }
}

/// The items mirror's last complete pass in unix seconds, as the sale
/// producers have always read it, selected beside [`STATE_COLUMNS`] for
/// [`ItemWait::from_row`].
pub const ITEM_WAIT_COLUMNS: &str =
    "extract(epoch FROM st_mi.last_complete_pass_at)::bigint AS items_pass_epoch";

/// What a sale whose item the catalogue lacks waits for on its network, by
/// the source that serves the network, read in the snapshot that read the
/// sale's description.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemWait {
    /// The mirrors: the items mirror's last complete pass, unix seconds.
    Mirror(Option<i64>),
    /// The collections index owns the network: the block its descriptive
    /// publication covers, `None` while it refuses one.
    Index(Option<i64>),
}

impl ItemWait {
    /// The wait of the network whose state (with [`ITEM_WAIT_COLUMNS`]) `row`
    /// holds, on `chain`.
    pub fn from_row(row: &PgRow, chain: i64, now: DateTime<Utc>) -> sqlx::Result<Self> {
        let state = NetworkState::from_row(row)?;
        Ok(match state.owned_verdict(chain, Purpose::Describe, now) {
            Some(verdict) => ItemWait::Index(verdict.ok().map(|p| p.folded_block)),
            None => ItemWait::Mirror(row.try_get("items_pass_epoch")?),
        })
    }

    /// The wait on `network` (served on `chain`) in `conn`'s snapshot.
    pub async fn read(
        conn: &mut PgConnection,
        network: &str,
        chain: i64,
        now: DateTime<Utc>,
    ) -> sqlx::Result<Self> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {STATE_COLUMNS}, {ITEM_WAIT_COLUMNS} FROM (SELECT $1::text AS network) AS n {}",
            *STATE_JOINS
        )))
        .bind(network)
        .fetch_one(conn)
        .await?;
        Self::from_row(&row, chain, now)
    }

    /// What becomes of a sale at `block_number` and `block_time` at `now`
    /// (unix seconds). `described`: the catalogue holds a row of its item now.
    /// `mirror_waits`: under the mirrors, the producer waits for this sale's
    /// item (the items mirror runs, and the sale is of a kind it waits for).
    /// - The mirrors: a sale whose item has no row waits until a complete
    ///   items pass ran an hour after it ([`item_awaited`]), and never past
    ///   [`ITEM_HOLD_MAX_SECS`](crate::shared::collections::ITEM_HOLD_MAX_SECS),
    ///   so a stalled mirror never freezes a cursor; then it is described
    ///   from whatever row there is.
    /// - The index: a sale its publication covers is described; every item
    ///   added by then is in the catalogue, so a missing one never comes.
    ///   Until then every sale waits, row or not, however long: a row read
    ///   while the index refuses (a rewind leaves the rows in place) may be
    ///   a discarded branch's, so it never picks a recipient, and a sale is
    ///   never given up because its producer fell behind. An index that
    ///   refuses for good holds the producers until it is fixed or the
    ///   network is handed back to the mirrors
    ///   (`MARKETPLACE_COLLECTIONS_INDEX_OWNS`); they then drain under that
    ///   network's rule.
    pub fn decide(
        self,
        mirror_waits: bool,
        described: bool,
        block_number: i64,
        block_time: i64,
        now: i64,
    ) -> Waited {
        match self {
            ItemWait::Mirror(pass) => {
                if !described && item_awaited(mirror_waits, pass, block_time, now) {
                    Waited::Hold
                } else {
                    Waited::Describe
                }
            }
            ItemWait::Index(Some(covered)) if covered >= block_number => Waited::Describe,
            ItemWait::Index(_) => Waited::Hold,
        }
    }
}

/// What a notice producer does with one sale ([`ItemWait::decide`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waited {
    /// It holds the cursor before the sale.
    Hold,
    /// It describes the sale from the catalogue's rows (a missing row gives
    /// no notice).
    Describe,
}

/// A `REPEATABLE READ, READ ONLY` transaction: the snapshot a job reads a
/// catalogue verdict and the rows it decides in.
pub async fn read_snapshot(pool: &PgPool) -> sqlx::Result<Transaction<'static, Postgres>> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

/// Fixtures that hand a network to the collections index, for the tests of
/// the readers that follow it.
#[cfg(any(test, feature = "test-support"))]
#[allow(clippy::unwrap_used, reason = "test support")]
pub mod owning {
    use sqlx::PgPool;

    /// Hands `network` (walked on `chain`) to the collections index,
    /// published at `block` `age_secs` ago. The mirrors' passes stay as a
    /// takeover leaves them: recorded, and no longer written.
    pub async fn own(pool: &PgPool, network: &str, chain: i64, block: i64, age_secs: i64) {
        sqlx::query(
            "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, last_block_hash, \
                    full_history, chain_id, folded_block, folded_at) \
             VALUES ('collections', $1, 1, $3, '0xabababababababababababababababababababababababababababababababab', true, $2, $3, \
                     timezone('UTC', now()) - make_interval(secs => $4)) \
             ON CONFLICT (indexer, network) DO UPDATE SET last_block = EXCLUDED.last_block, chain_id = EXCLUDED.chain_id, \
                 full_history = true, folded_block = EXCLUDED.folded_block, folded_at = EXCLUDED.folded_at, \
                 quarantined_at = NULL, diverged_at = NULL",
        )
        .bind(network)
        .bind(chain)
        .bind(block)
        .bind(age_secs as f64)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shop.collection_projection (network, owned_at, projected_through, pending, generation) \
             VALUES ($1, timezone('UTC', now()), $2, 0, 1) \
             ON CONFLICT (network) DO UPDATE SET owned_at = EXCLUDED.owned_at, \
                 projected_through = EXCLUDED.projected_through, pending = 0, \
                 generation = shop.collection_projection.generation + 1",
        )
        .bind(network)
        .bind(block)
        .execute(pool)
        .await
        .unwrap();
    }

    /// A rewind withdraws `network`'s publication and clears its projection.
    pub async fn rewind(pool: &PgPool, network: &str) {
        sqlx::query(
            "UPDATE shop.chain_cursor SET folded_block = NULL, folded_at = NULL \
             WHERE indexer = 'collections' AND network = $1",
        )
        .bind(network)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE shop.collection_projection SET projected_through = NULL WHERE network = $1",
        )
        .bind(network)
        .execute(pool)
        .await
        .unwrap();
    }

    /// The walk reads a collections event at `block`, past the publication,
    /// before it folds it.
    pub async fn walk_past(pool: &PgPool, network: &str, block: i64, contract: &str) {
        sqlx::query(
            "UPDATE shop.chain_cursor SET last_block = $2 WHERE indexer = 'collections' AND network = $1",
        )
        .bind(network)
        .bind(block)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shop.collection_event (network, block_number, log_index, block_time, kind, contract_address) \
             VALUES ($1, $2, 0, 0, 'set_approved', $3)",
        )
        .bind(network)
        .bind(block)
        .bind(contract)
        .execute(pool)
        .await
        .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::collections::{ITEM_HOLD_MAX_SECS, ITEM_WAIT_SECS};

    /// Under the mirrors nothing waits past [`ITEM_HOLD_MAX_SECS`]; on an
    /// owned network a sale waits for the publication over its block
    /// however long the index refuses, and is described once it is covered.
    #[test]
    fn only_the_mirrors_bound_the_wait() {
        let (block, t) = (100, 1_700_000_000);
        let past = t + 3 * ITEM_HOLD_MAX_SECS;
        let mirror = ItemWait::Mirror(None);
        assert_eq!(mirror.decide(true, false, block, t, t + 60), Waited::Hold);
        assert_eq!(
            mirror.decide(true, false, block, t, past),
            Waited::Describe,
            "a stalled mirror"
        );
        assert_eq!(
            ItemWait::Mirror(Some(t + ITEM_WAIT_SECS)).decide(
                true,
                false,
                block,
                t,
                t + ITEM_WAIT_SECS
            ),
            Waited::Describe,
            "a complete pass an hour after the sale"
        );
        for wait in [ItemWait::Index(None), ItemWait::Index(Some(block - 1))] {
            for described in [false, true] {
                assert_eq!(
                    wait.decide(false, described, block, t, t + 60),
                    Waited::Hold
                );
                assert_eq!(
                    wait.decide(true, described, block, t, past),
                    Waited::Hold,
                    "however long the index refuses"
                );
            }
        }
        assert_eq!(
            ItemWait::Index(Some(block)).decide(false, false, block, t, past),
            Waited::Describe,
            "covered, however late"
        );
    }
}
