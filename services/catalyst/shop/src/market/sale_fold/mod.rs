//! The common sales prefix. Check each walk's history identity before
//! publishing. A normal advance preserves its identity and covered prefix.
mod creators;
mod settlements;

use std::collections::BTreeSet;

use anyhow::{Context, ensure};
use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::{FromRow, PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::chain_walk::{self, ChainNet};
use super::{ethereum_sales, listings, nft_index, sale_history, sales_index, trade_events};
use crate::chain_index::rpc::Rpc;

#[derive(FromRow)]
struct Input {
    indexer: String,
    last_block: i64,
    hash: String,
    folded_block: i64,
    folded_at: NaiveDateTime,
    epoch: Uuid,
    revision: i64,
    dirty_from: Option<i64>,
}

#[derive(FromRow)]
struct FactoryProof {
    last_block: i64,
    hash: String,
    epoch: Uuid,
    revision: i64,
    required_from: i64,
    required_to: i64,
}

struct Inputs {
    factory: Option<FactoryProof>,
    rows: Vec<Input>,
    late_collections: i64,
    first_backfill: Option<i64>,
    collection_changes: Vec<String>,
}

impl Inputs {
    fn common(&self) -> i64 {
        self.rows.iter().map(|r| r.folded_block).min().unwrap()
    }

    fn epochs(&self) -> Vec<Uuid> {
        self.rows.iter().map(|r| r.epoch).collect()
    }

    fn revisions(&self) -> Vec<i64> {
        self.rows.iter().map(|r| r.revision).collect()
    }

    fn reusable_prefix(&self, marker: &Marker) -> Option<i64> {
        if marker.input_version != sale_history::VERSION
            || marker.input_epochs.get(..self.rows.len()) != self.epochs().get(..self.rows.len())
            || marker.input_revisions.len() < self.rows.len()
        {
            return None;
        }
        let mut prefix = marker.block_number.min(self.common());
        if let Some(from) = self.first_backfill {
            prefix = prefix.min(from.checked_sub(1)?);
        }
        for (row, previous) in self.rows.iter().zip(&marker.input_revisions) {
            if row.revision != *previous {
                prefix = prefix.min(row.dirty_from?.checked_sub(1)?);
            }
        }
        Some(prefix)
    }

    fn covers(&self, marker: &Marker) -> bool {
        marker.input_version == sale_history::VERSION
            && marker.input_epochs.starts_with(&self.epochs())
            && marker.input_revisions.starts_with(&self.revisions())
            && marker.late_collections == self.late_collections
            && self.collection_changes.is_empty()
            && marker.block_number <= self.common()
    }
}

#[derive(FromRow, PartialEq, Eq)]
struct Marker {
    block_number: i64,
    block_hash: String,
    input_version: i32,
    generation: Uuid,
    input_epochs: Vec<Uuid>,
    input_revisions: Vec<i64>,
    late_collections: i64,
    published_at: Option<NaiveDateTime>,
}

async fn marker(c: &mut PgConnection, net: ChainNet) -> Result<Option<Marker>, sqlx::Error> {
    sqlx::query_as("SELECT * FROM shop.sale_publication WHERE network=$1")
        .bind(net.db_name())
        .fetch_optional(c)
        .await
}

async fn inputs(
    c: &mut PgConnection,
    net: ChainNet,
    now: DateTime<Utc>,
    through: Option<i64>,
) -> anyhow::Result<Option<Inputs>> {
    let rows: Vec<Input> = sqlx::query_as(include_str!("inputs.sql"))
        .bind(net.db_name())
        .bind(net.chain_id())
        .bind(sale_history::VERSION)
        .bind(now.naive_utc())
        .bind(nft_index::start_block(net) as i64)
        .bind(
            trade_events::marketplaces(net)
                .iter()
                .map(|d| d.1)
                .min()
                .context("missing trade deployments")? as i64,
        )
        .fetch_all(&mut *c)
        .await?;
    if !rows
        .iter()
        .map(|r| r.indexer.as_str())
        .eq(["nft", "sales", "trade_events"])
    {
        return Ok(None);
    }
    let common = rows.iter().map(|r| r.folded_block).min().unwrap();
    let (late_collections, first_backfill, collection_changes, owed): (i64, Option<i64>, Vec<String>, bool) = sqlx::query_as(
        "WITH current AS (SELECT contract_address,backfill_from_block,refold_owed OR
                   coalesce(backfilled_to_block,backfill_from_block-1)<walked_from_block-1 AS owed
             FROM shop.nft_contract WHERE network=$1 AND backfill_from_block IS NOT NULL),
         saved AS (SELECT contract_address,backfill_from_block FROM shop.sale_covered_collection WHERE network=$1),
         compared AS (SELECT contract_address,c.backfill_from_block AS new,s.backfill_from_block AS old,c.owed
             FROM current c FULL JOIN saved s USING(contract_address))
         SELECT count(new),min(least(new,old)) FILTER (WHERE new IS DISTINCT FROM old),
             coalesce(array_agg(contract_address||'/'||coalesce(new::text,'-')||'/'||coalesce(old::text,'-') ORDER BY contract_address)
                      FILTER (WHERE new IS DISTINCT FROM old),'{}'::text[]),coalesce(bool_or(owed),false)
         FROM compared")
        .bind(net.db_name()).fetch_one(&mut *c).await?;
    let undiscovered:bool=sqlx::query_scalar(
        "SELECT EXISTS(SELECT FROM shop.collections c JOIN shop.chain_cursor n ON n.network=c.network AND n.indexer='nft'
         WHERE c.network=$1 AND c.created_at<=n.last_block_time AND NOT EXISTS
           (SELECT FROM shop.nft_contract k WHERE k.network=c.network AND k.contract_address=c.contract_address))")
        .bind(net.db_name()).fetch_one(&mut *c).await?;
    // A Bought line outside the current catalogue may name a late genuine
    // collection. Only a complete factory walk through that purchase can
    // distinguish it from an unrelated address.
    if owed || undiscovered {
        return Ok(None);
    }
    let factory = if net == ChainNet::Matic {
        let (needs_issue, first, last): (bool, Option<i64>, Option<i64>) =
            sqlx::query_as(include_str!("store_coverage.sql"))
                .bind(through.unwrap_or(common).min(common))
                .bind(sale_history::VERSION)
                .fetch_one(&mut *c)
                .await?;
        if needs_issue {
            return Ok(None);
        }
        if let (Some(first), Some(last)) = (first, last) {
            let proof:Option<FactoryProof>=sqlx::query_as("SELECT last_block,last_block_hash AS hash,
                sale_input_epoch AS epoch,reorg_rewinds::bigint AS revision,$4::bigint AS required_from,$2::bigint AS required_to
                FROM shop.chain_cursor WHERE indexer='collections' AND network='matic'
                  AND chain_id=137 AND full_history AND start_block<=$1 AND folded_block>=$2
                  AND folded_at BETWEEN $3::timestamp - interval '30 minutes' AND $3 AND last_block_hash IS NOT NULL
                  AND quarantined_at IS NULL AND diverged_at IS NULL AND rebuild_requested_at IS NULL")
                .bind(super::collections_index::START_BLOCK as i64).bind(last).bind(now.naive_utc()).bind(first)
                .fetch_optional(&mut *c).await?;
            let Some(proof) = proof else {
                return Ok(None);
            };
            if rows
                .iter()
                .any(|r| r.last_block == proof.last_block && r.hash != proof.hash)
            {
                return Ok(None);
            }
            Some(proof)
        } else {
            None
        }
    } else {
        None
    };
    Ok(Some(Inputs {
        factory,
        rows,
        late_collections,
        first_backfill,
        collection_changes,
    }))
}

pub struct SalesSnapshot {
    pub transaction: Transaction<'static, Postgres>,
    pub expires_at: DateTime<Utc>,
    pub generations: Vec<(ChainNet, Uuid)>,
}

/// Readiness and subsequent queries share this read-only transaction. The
/// earliest deadline applies to all requested networks.
pub async fn snapshot(
    pool: &PgPool,
    nets: &[ChainNet],
    now: DateTime<Utc>,
) -> anyhow::Result<Option<SalesSnapshot>> {
    ensure!(!nets.is_empty(), "a sales snapshot needs a network");
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let mut deadline = now + chain_walk::READY_WINDOW;
    let mut generations = Vec::with_capacity(nets.len());
    for &net in nets {
        let Some(marker) = marker(&mut tx, net).await? else {
            return Ok(None);
        };
        let Some(inputs) = inputs(&mut tx, net, now, Some(marker.block_number)).await? else {
            return Ok(None);
        };
        let Some(at) = marker.published_at else {
            return Ok(None);
        };
        if inputs.factory.is_some() || !inputs.covers(&marker) || at > now.naive_utc() {
            return Ok(None);
        }
        generations.push((net, marker.generation));
        let earliest = inputs
            .rows
            .iter()
            .map(|r| r.folded_at)
            .fold(at, std::cmp::min);
        deadline = deadline.min(earliest.and_utc() + chain_walk::READY_WINDOW);
    }
    Ok((now <= deadline).then_some(SalesSnapshot {
        transaction: tx,
        expires_at: deadline,
        generations,
    }))
}

async fn write_rows(c: &mut PgConnection, net: ChainNet, from: i64, to: i64) -> anyhow::Result<()> {
    let orders: Vec<_> = nft_index::legacy_marketplaces(net)
        .iter()
        .map(|d| d.0)
        .collect();
    let trades: Vec<_> = trade_events::marketplaces(net)
        .iter()
        .map(|d| d.0)
        .collect();
    let bid = match net {
        ChainNet::Ethereum => ethereum_sales::ERC721_BID,
        ChainNet::Matic => sales_index::BID_V2_MATIC,
    };
    sqlx::query(include_str!("executions.sql"))
        .bind(net.db_name())
        .bind(from)
        .bind(to)
        .bind(orders)
        .bind(bid)
        .bind(sales_index::STORE_MATIC)
        .bind(&ethereum_sales::NAME_CONTROLLERS[..])
        .bind(trades)
        .execute(&mut *c)
        .await?;
    sqlx::query(include_str!("legs.sql"))
        .bind(net.db_name())
        .bind(from)
        .bind(to)
        .bind(listings::mana_token(net.chain_id()).context("missing MANA deployment")?)
        .execute(&mut *c)
        .await?;
    sqlx::query(
        "UPDATE shop.sale_asset a SET item_id=n.item_id FROM shop.nft n
        WHERE a.network=$1 AND a.block_number>$2 AND a.block_number<=$3 AND a.item_id IS NULL
          AND (a.network,a.contract_address,a.token_id)=(n.network,n.contract_address,n.token_id)
          AND n.item_id IS NOT NULL AND n.minted_block<=a.block_number",
    )
    .bind(net.db_name())
    .bind(from)
    .bind(to)
    .execute(&mut *c)
    .await?;
    Ok(())
}

async fn rollups(c: &mut PgConnection, net: ChainNet, days: &[i64]) -> anyhow::Result<()> {
    if days.is_empty() {
        return Ok(());
    }
    const ACCOUNTS: &str =
        "SELECT DISTINCT address FROM shop.sale_account_day WHERE network=$1 AND day=ANY($2)";
    let mut touched: Vec<String> = sqlx::query_scalar(ACCOUNTS)
        .bind(net.db_name())
        .bind(days)
        .fetch_all(&mut *c)
        .await?;
    for clear in [
        "DELETE FROM shop.sale_analytics_day WHERE network=$1 AND day=ANY($2)",
        "DELETE FROM shop.sale_item_day WHERE network=$1 AND day=ANY($2)",
        "DELETE FROM shop.sale_account_day WHERE network=$1 AND day=ANY($2)",
    ] {
        sqlx::query(clear)
            .bind(net.db_name())
            .bind(days)
            .execute(&mut *c)
            .await?;
    }
    sqlx::query(include_str!("rollups.sql"))
        .bind(net.db_name())
        .bind(days)
        .bind(ethereum_sales::BURNING_NAME_CONTROLLER)
        .execute(&mut *c)
        .await?;
    touched.extend(
        sqlx::query_scalar::<_, String>(ACCOUNTS)
            .bind(net.db_name())
            .bind(days)
            .fetch_all(&mut *c)
            .await?,
    );
    touched.sort_unstable();
    touched.dedup();
    sqlx::query("DELETE FROM shop.sale_account_total WHERE network=$1 AND address=ANY($2)")
        .bind(net.db_name())
        .bind(&touched)
        .execute(&mut *c)
        .await?;
    sqlx::query("INSERT INTO shop.sale_account_total(network,currency,address,sales,purchases,spent,gross_sold,net_sold,unknown_proceeds,received,royalties,fees_received)
       SELECT network,currency,address,sum(sales),sum(purchases),sum(spent),sum(gross_sold),sum(net_sold),sum(unknown_proceeds),sum(received),sum(royalties),sum(fees_received)
       FROM shop.sale_account_day WHERE network=$1 AND address=ANY($2) GROUP BY network,currency,address")
      .bind(net.db_name()).bind(touched).execute(&mut *c).await?;
    Ok(())
}

pub(super) async fn pass(pool: &PgPool, rpc: &Rpc<'_>, net: ChainNet) -> anyhow::Result<()> {
    let mut c = pool.acquire().await?;
    let Some(start) = inputs(&mut c, net, Utc::now(), None).await? else {
        return Ok(());
    };
    let previous = marker(&mut c, net).await?;
    drop(c);
    let same_history = previous.as_ref().is_some_and(|m| start.covers(m));
    let initial = nft_index::start_block(net) as i64 - 1;
    let from = previous
        .as_ref()
        .and_then(|m| start.reusable_prefix(m))
        .unwrap_or(initial)
        .max(initial);
    let preserved_marker = previous.as_ref().filter(|m| m.block_number == from);
    let limit = start.common().min(from + 250_000);
    // Limit RPC receipt work even in a dense block range. Never split a block.
    let to: i64 = sqlx::query_scalar(include_str!("window.sql"))
        .bind(net.db_name())
        .bind(from)
        .bind(limit)
        .fetch_one(pool)
        .await?;
    ensure!(to >= from, "common sales prefix precedes its start");
    use crate::chain_index::logs::{LogSource, RpcSource};
    ensure!(
        RpcSource::new(*rpc).chain_id().await?.value == net.chain_id() as u64,
        "wrong sales accounting RPC chain"
    );
    let headers: BTreeSet<_> = start
        .rows
        .iter()
        .map(|r| r.last_block as u64)
        .chain(start.factory.iter().map(|f| f.last_block as u64))
        .chain([to as u64])
        .chain(preserved_marker.iter().map(|m| m.block_number as u64))
        .collect();
    let (blocks, _) = rpc
        .block_headers(&headers.into_iter().collect::<Vec<_>>())
        .await?;
    for row in &start.rows {
        ensure!(
            blocks[&(row.last_block as u64)].hash == row.hash,
            "sales input is on a different branch"
        );
    }
    if let Some(f) = &start.factory {
        ensure!(
            blocks[&(f.last_block as u64)].hash == f.hash,
            "store factory input is on a different branch"
        );
    }
    if let Some(m) = preserved_marker {
        ensure!(
            blocks[&(m.block_number as u64)].hash == m.block_hash,
            "sales prefix changed branch"
        );
    }
    let hash = &blocks[&(to as u64)].hash;
    let mut tx = pool.begin().await?;
    let mut days: Vec<i64> = Vec::new();
    if !same_history {
        // Capture the old UTC days before the cascading delete. A replacement
        // can move the only purchase to another day, seller or item.
        days = sqlx::query_scalar("WITH removed AS (
            DELETE FROM shop.sale_execution WHERE network=$1 AND block_number>$2 RETURNING block_time)
            SELECT DISTINCT block_time/86400*86400 FROM removed")
            .bind(net.db_name()).bind(from).fetch_all(&mut *tx).await?;
    }
    write_rows(&mut tx, net, from, to).await?;
    days.extend(creators::write(&mut tx, rpc, net, from, to).await?);
    days.extend(settlements::write(&mut tx, rpc, net, from, to).await?);
    days.sort_unstable();
    days.dedup();
    // These checkout providers take custody before delivery. Preserve the
    // immediate beneficiary and the payment sender alongside the public buyer.
    sqlx::query("WITH delivered AS (
        SELECT a.block_number,a.log_index,a.ordinal,
          (SELECT n.to_address FROM shop.nft_event n WHERE n.network=a.network AND n.contract_address=a.contract_address
            AND n.token_id=a.token_id AND n.block_number=a.block_number AND n.kind='transfer'
            AND n.from_address=a.beneficiary AND n.log_index>a.log_index ORDER BY n.log_index LIMIT 1) AS recipient
        FROM shop.sale_asset a JOIN shop.sale_execution e USING(network,block_number,log_index)
        WHERE a.network=$1 AND e.block_time/86400*86400=ANY($2) AND a.beneficiary=ANY($3))
        UPDATE shop.sale_asset a SET buyer=d.recipient FROM delivered d WHERE a.network=$1
          AND (a.block_number,a.log_index,a.ordinal)=(d.block_number,d.log_index,d.ordinal)
          AND d.recipient IS NOT NULL AND a.buyer IS DISTINCT FROM d.recipient")
        .bind(net.db_name()).bind(&days).bind(&["0xed038688ecf1193f8d9717eb3930f0bf0d745cb4","0xea749fd6ba492dbc14c24fe8a3d08769229b896c"][..])
        .execute(&mut *tx).await?;
    if !days.is_empty() {
        // A later receipt can replace the checkout provider with the recipient.
        // The old buyer may still have a later purchase of the same item.
        sqlx::query("MERGE INTO shop.sale_first_purchase f USING (
            SELECT k.ctid AS id, (
                SELECT min(e.block_time)/86400*86400 FROM shop.sale_asset a
                JOIN shop.sale_execution e USING(network,block_number,log_index)
                WHERE (a.network,a.buyer,a.seller,a.contract_address,a.currency)=
                      (k.network,k.buyer,k.seller,k.contract_address,k.currency)
                  AND a.item_id IS NOT DISTINCT FROM k.item_id AND a.price>0
            ) AS first_day
            FROM shop.sale_first_purchase k WHERE network=$1 AND first_day=ANY($2)
        ) remaining ON f.ctid=remaining.id
        WHEN MATCHED AND remaining.first_day IS NULL THEN DELETE
        WHEN MATCHED AND remaining.first_day<>f.first_day THEN UPDATE SET first_day=remaining.first_day")
            .bind(net.db_name()).bind(&days).execute(&mut *tx).await?;
    }
    rollups(&mut tx, net, &days).await?;
    let point = rpc.block_header(to as u64).await?;
    ensure!(point.hash == *hash, "sales prefix changed during fold");
    let head = rpc.head_block().await?;
    let previously_published = previous
        .as_ref()
        .filter(|_| same_history)
        .and_then(|m| m.published_at);
    let published_at = if (to == start.common() || previously_published.is_some())
        && head >= to as u64
        && chain_walk::within_publish_lag(net, head, to as u64)
    {
        Some(Utc::now().naive_utc())
    } else if to < start.common() && head >= to as u64 {
        // This prefix extends the same verified history. Retain its original
        // freshness deadline until a near-head measurement earns a new one.
        previously_published
    } else {
        None
    };
    // Take the same cursor locks used by the owners before visibility.
    // A rewind or a repair committed during the fold changes this comparison.
    sqlx::query("SELECT indexer FROM shop.chain_cursor WHERE network=$1 AND indexer IN ('nft','sales','trade_events','collections') ORDER BY indexer FOR UPDATE")
        .bind(net.db_name()).fetch_all(&mut *tx).await?;
    let end = inputs(&mut tx, net, Utc::now(), Some(start.common()))
        .await?
        .context("sales inputs withdrawn during fold")?;
    ensure!(
        start.epochs() == end.epochs()
            && start.revisions() == end.revisions()
            && start.late_collections == end.late_collections
            && start.collection_changes == end.collection_changes
            && to <= end.common(),
        "sales history changed during fold"
    );
    ensure!(
        marker(&mut tx, net).await? == previous,
        "another sales fold committed"
    );
    // The factory attests only to the absence of a creation through the
    // purchased block. Its cursor clips this prefix on rewind and clears it
    // on rebuild. A newly discovered issuer always reopens Issue repair.
    match (&start.factory, &end.factory) {
        (Some(a), Some(b))
            if (a.epoch, a.revision) == (b.epoch, b.revision)
                && a.required_from == b.required_from
                && a.required_to == b.required_to =>
        {
            sqlx::query(
                "UPDATE shop.chain_cursor SET sale_input_version=$1,
                sale_verified_through=CASE WHEN sale_input_version=$1 THEN greatest(sale_verified_through,$2) ELSE $2 END
                WHERE indexer='collections' AND network='matic'",
            )
            .bind(sale_history::VERSION)
            .bind(a.required_to)
            .execute(&mut *tx)
            .await?;
        }
        (None, None) => {}
        _ => anyhow::bail!("store factory history changed during fold"),
    }
    // Acknowledging the earliest affected block is atomic with replacing the
    // suffix and recording the revisions. Owners use the same locked cursors.
    sqlx::query(
        "UPDATE shop.chain_cursor SET sale_dirty_from=NULL WHERE network=$1
        AND indexer IN ('nft','sales','trade_events') AND sale_dirty_from IS NOT NULL",
    )
    .bind(net.db_name())
    .execute(&mut *tx)
    .await?;
    if !start.collection_changes.is_empty() {
        sqlx::query("DELETE FROM shop.sale_covered_collection s WHERE network=$1 AND NOT EXISTS
             (SELECT FROM shop.nft_contract c WHERE c.network=s.network AND c.contract_address=s.contract_address
               AND c.backfill_from_block IS NOT NULL)").bind(net.db_name()).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO shop.sale_covered_collection AS s(network,contract_address,backfill_from_block)
             SELECT network,contract_address,backfill_from_block FROM shop.nft_contract WHERE network=$1 AND backfill_from_block IS NOT NULL
             ON CONFLICT(network,contract_address) DO UPDATE SET backfill_from_block=excluded.backfill_from_block
             WHERE s.backfill_from_block<>excluded.backfill_from_block").bind(net.db_name()).execute(&mut *tx).await?;
    }
    sqlx::query("INSERT INTO shop.sale_publication (network,block_number,block_hash,input_version,input_epochs,input_revisions,late_collections,published_at)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT (network) DO UPDATE SET block_number=EXCLUDED.block_number,
        generation=CASE WHEN $9 THEN EXCLUDED.generation ELSE sale_publication.generation END,
        block_hash=EXCLUDED.block_hash,input_version=EXCLUDED.input_version,input_epochs=EXCLUDED.input_epochs,
        input_revisions=EXCLUDED.input_revisions,late_collections=EXCLUDED.late_collections,published_at=EXCLUDED.published_at")
        .bind(net.db_name()).bind(to).bind(hash).bind(sale_history::VERSION).bind(start.epochs()).bind(start.revisions())
        .bind(start.late_collections).bind(published_at).bind(!same_history || from!=to || !days.is_empty()).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
