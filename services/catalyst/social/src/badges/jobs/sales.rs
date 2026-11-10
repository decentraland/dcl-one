//! The Polygon shop logs → emotionista and fashionista. Upstream counted the
//! collections subgraph's L2 `sales` (primary mints, orders, bids) per
//! transaction hash; here the same sales come from the logs shop indexes on
//! Polygon:
//!
//! - `shop.sale_event` `mint` (a CollectionStore issue, the beneficiary
//!   buys) and `bid_accepted` (ERC721BidV2, the bidder buys);
//! - `shop.trade_event` `traded` (the off-chain marketplace: a listing's
//!   caller or a bid's signer buys);
//! - `shop.nft_event` `order_successful` (the legacy marketplace; the log has
//!   no transaction hash, so its chain position stands in for it).
//!
//! An item's category is its mirrored `shop.items.item_type`. Each log table
//! is read only up to the block its index has published (`shop.chain_cursor`
//! folded and fresh, shop's own readiness rule), from its own cursor
//! (`sales:<table>:matic`, block and log index), which starts at the
//! published block.
//!
//! A sale of an item the items mirror does not hold yet holds its cursor
//! (the logs are read in chain order, so the sales after it wait too) until
//! the item appears or the mirror completes a pass [`ITEM_WAIT_SECS`] after
//! the sale: a complete pass walks every item and at the defaults takes
//! minutes, so an item still missing then is not a collection item the
//! mirror will ever hold (a bid on a foreign ERC721), and the sale is passed
//! without effect. A mirror that stops completing passes holds the sales,
//! so the job runs only with `MARKETPLACE_MIRRORS_ENABLED`.

use std::collections::{HashMap, HashSet};

use serde_json::Value;
use sqlx::PgConnection;

use super::super::evaluators as ev;
use super::{Pass, award, completed, init_cursor, lock_cursor};
use crate::identity::Address;

/// 2^216: a v2 collection token id is `item << 216 | issued`.
const ITEM_SHIFT: &str = "105312291668557186697918027683670432318895095400549111254310977536";
const ASSET_TYPE_ERC721: i64 = 3;
const ASSET_TYPE_COLLECTION_ITEM: i64 = 4;
/// How long after a sale the items mirror must complete a pass before a
/// sale whose item it lacks is passed (a pass takes well under it: 50 pages
/// of 1,000 items every 300 s by default).
pub(crate) const ITEM_WAIT_SECS: i64 = 3_600;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Log {
    Sale,
    Trade,
    Order,
}

impl Log {
    pub const ALL: [Log; 3] = [Log::Sale, Log::Trade, Log::Order];

    pub fn source(self) -> &'static str {
        match self {
            Log::Sale => "sales:sale_event:matic",
            Log::Trade => "sales:trade_event:matic",
            Log::Order => "sales:nft_event:matic",
        }
    }

    fn indexer(self) -> &'static str {
        match self {
            Log::Sale => "sales",
            Log::Trade => "trade_events",
            Log::Order => "nft",
        }
    }

    fn rows_sql(self) -> String {
        match self {
            Log::Sale => format!(
                "SELECT e.block_number, e.log_index, e.block_time, e.buyer, e.tx_hash, \
                        NULL::jsonb AS assets, NULL::text AS caller, \
                        CASE WHEN i.item_type IS NULL THEN '{{}}'::text[] ELSE ARRAY[i.item_type] END AS item_types \
                 FROM shop.sale_event e \
                 LEFT JOIN shop.items i ON i.network = e.network AND i.contract_address = e.contract_address \
                   AND i.item_id = CASE WHEN e.kind = 'mint' THEN e.item_id ELSE div(e.token_id, {ITEM_SHIFT}) END \
                 WHERE e.network = 'matic' AND e.kind IN ('mint', 'bid_accepted') \
                   AND (e.block_number, e.log_index) > ($1, $2) AND e.block_number <= $3 \
                 ORDER BY e.block_number, e.log_index LIMIT $4"
            ),
            Log::Trade => {
                "SELECT e.block_number, e.log_index, e.block_time, e.signer AS buyer, e.tx_hash, \
                        e.assets, e.caller, '{}'::text[] AS item_types \
                 FROM shop.trade_event e \
                 WHERE e.network = 'matic' AND e.kind = 'traded' \
                   AND (e.block_number, e.log_index) > ($1, $2) AND e.block_number <= $3 \
                 ORDER BY e.block_number, e.log_index LIMIT $4"
                    .to_string()
            }
            Log::Order => format!(
                "SELECT e.block_number, e.log_index, e.block_time, e.to_address AS buyer, \
                        'nft_event:matic:' || e.block_number || ':' || e.log_index AS tx_hash, \
                        NULL::jsonb AS assets, NULL::text AS caller, \
                        CASE WHEN i.item_type IS NULL THEN '{{}}'::text[] ELSE ARRAY[i.item_type] END AS item_types \
                 FROM shop.nft_event e \
                 LEFT JOIN shop.items i ON i.network = e.network AND i.contract_address = e.contract_address \
                   AND i.item_id = div(e.token_id, {ITEM_SHIFT}) \
                 WHERE e.network = 'matic' AND e.kind = 'order_successful' \
                   AND (e.block_number, e.log_index) > ($1, $2) AND e.block_number <= $3 \
                 ORDER BY e.block_number, e.log_index LIMIT $4"
            ),
        }
    }
}

#[derive(sqlx::FromRow)]
struct Row {
    block_number: i64,
    log_index: i32,
    block_time: i64,
    buyer: Option<String>,
    tx_hash: String,
    assets: Option<Value>,
    caller: Option<String>,
    item_types: Vec<String>,
}

/// The badge a sold item's type counts for (`item-sold` `category`).
fn badge_of(item_type: &str) -> &'static str {
    if item_type.starts_with("emote") {
        ev::EMOTIONISTA
    } else {
        ev::FASHIONISTA
    }
}

/// (asset type, contract, value) of a Traded good.
type Good = (i64, String, String);

/// The goods (an ERC721 token or a collection item) of one side of a Traded log.
fn side(assets: &Value, key: &str) -> Vec<Good> {
    assets[key]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| {
            let kind = match &a["assetType"] {
                Value::Number(n) => n.as_i64()?,
                Value::String(s) => s.parse().ok()?,
                _ => return None,
            };
            Some((
                kind,
                a["contractAddress"].as_str()?.to_ascii_lowercase(),
                a["value"].as_str()?.to_string(),
            ))
        })
        .filter(|(kind, _, _)| *kind == ASSET_TYPE_ERC721 || *kind == ASSET_TYPE_COLLECTION_ITEM)
        .collect()
}

/// A Traded log's buyer and goods: the signer sent the goods (a listing,
/// the caller buys) or received them (a bid, the signer buys).
fn trade(row: &Row) -> Option<(String, Vec<Good>)> {
    let assets = row.assets.as_ref()?;
    let sent = side(assets, "sent");
    if !sent.is_empty() {
        return Some((row.caller.clone()?, sent));
    }
    let received = side(assets, "received");
    (!received.is_empty()).then(|| (row.buyer.clone().unwrap_or_default(), received))
}

/// The mirrored item types of Traded goods, by (contract, value).
async fn trade_item_types(
    conn: &mut PgConnection,
    goods: &[Good],
) -> Result<HashMap<(String, String), String>, sqlx::Error> {
    if goods.is_empty() {
        return Ok(HashMap::new());
    }
    let contracts: Vec<&str> = goods.iter().map(|g| g.1.as_str()).collect();
    let values: Vec<&str> = goods.iter().map(|g| g.2.as_str()).collect();
    let tokens: Vec<bool> = goods.iter().map(|g| g.0 == ASSET_TYPE_ERC721).collect();
    let rows: Vec<(String, String, String)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT u.c, u.v, i.item_type \
         FROM unnest($1::text[], $2::text[], $3::bool[]) AS u(c, v, t) \
         JOIN shop.items i ON i.network = 'matic' AND i.contract_address = u.c \
           AND i.item_id = CASE WHEN u.t THEN div(u.v::numeric, {ITEM_SHIFT}) ELSE u.v::numeric END \
         WHERE u.v ~ '^[0-9]{{1,78}}$'"
    )))
    .bind(&contracts)
    .bind(&values)
    .bind(&tokens)
    .fetch_all(conn)
    .await?;
    Ok(rows.into_iter().map(|(c, v, t)| ((c, v), t)).collect())
}

/// The block `log`'s index has published, when it has (shop's readiness:
/// full history folded through the last indexed block within 30 minutes).
async fn published(conn: &mut PgConnection, log: Log) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT folded_block FROM shop.chain_cursor \
         WHERE indexer = $1 AND network = 'matic' AND full_history \
           AND folded_block = last_block \
           AND folded_at <= timezone('UTC', now()) \
           AND folded_at >= timezone('UTC', now()) - interval '30 minutes'",
    )
    .bind(log.indexer())
    .fetch_optional(conn)
    .await
    .map(Option::flatten)
}

pub(super) async fn run(pass: &Pass) -> anyhow::Result<u64> {
    let mut passed = 0;
    for log in Log::ALL {
        for _ in 0..super::MAX_BATCHES_PER_TICK {
            let (n, more) = batch(pass, log).await?;
            passed += n;
            if !more {
                break;
            }
        }
    }
    Ok(passed)
}

pub(crate) async fn batch(pass: &Pass, log: Log) -> Result<(u64, bool), sqlx::Error> {
    let mut tx = pass.pool.begin().await?;
    let Some(until) = published(&mut tx, log).await? else {
        tx.rollback().await?;
        return Ok((0, false));
    };
    let Some((block, log_index)) = lock_cursor(&mut tx, log.source()).await? else {
        init_cursor(&mut tx, log.source(), until, i32::MAX).await?;
        tx.commit().await?;
        return Ok((0, false));
    };
    super::report_lag(log.source(), until - block);
    let limit = pass.batch();
    let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(log.rows_sql()))
        .bind(block)
        .bind(log_index)
        .bind(until)
        .bind(limit)
        .fetch_all(&mut *tx)
        .await?;
    if rows.is_empty() {
        tx.rollback().await?;
        return Ok((0, false));
    }
    // The last complete items pass, unix seconds.
    let items_pass: Option<i64> = sqlx::query_scalar(
        "SELECT extract(epoch FROM last_complete_pass_at)::bigint FROM shop.mirror_state \
         WHERE mirror = 'items' AND network = 'matic'",
    )
    .fetch_optional(&mut *tx)
    .await?
    .flatten();
    // (buyer, tx key, sale time, badges)
    let mut sales: Vec<(String, String, i64, Vec<&'static str>)> = Vec::new();
    let now = crate::shared::now_utc();
    let goods: Vec<Good> = if log == Log::Trade {
        rows.iter().filter_map(trade).flat_map(|(_, g)| g).collect()
    } else {
        Vec::new()
    };
    let traded = trade_item_types(&mut tx, &goods).await?;
    let mut passed = 0;
    for row in &rows {
        let at = chrono::DateTime::from_timestamp(row.block_time, 0)
            .map(|t| t.naive_utc())
            .unwrap_or(now);
        let (buyer, types, unmirrored): (Option<String>, Vec<String>, bool) = if log == Log::Trade {
            match trade(row) {
                Some((buyer, goods)) => {
                    let wanted = goods.len();
                    let types: Vec<String> = goods
                        .into_iter()
                        .filter_map(|(_, c, v)| traded.get(&(c, v)).cloned())
                        .collect();
                    let missing = types.len() < wanted;
                    (Some(buyer), types, missing)
                }
                None => (None, Vec::new(), false),
            }
        } else {
            let missing = row.buyer.is_some() && row.item_types.is_empty();
            (row.buyer.clone(), row.item_types.clone(), missing)
        };
        if unmirrored && !pass.too_old(at, now) {
            let waited = items_pass
                .is_some_and(|done| done >= row.block_time.saturating_add(ITEM_WAIT_SECS));
            if !waited {
                tracing::debug!(
                    source = log.source(),
                    block = row.block_number,
                    log = row.log_index,
                    "sale of an item not mirrored yet: held"
                );
                break;
            }
            tracing::debug!(
                source = log.source(),
                block = row.block_number,
                log = row.log_index,
                "sale of an item the mirror never held: passed"
            );
        }
        passed += 1;
        if pass.too_old(at, now) {
            continue;
        }
        let mut badges: Vec<&'static str> = types.iter().map(|t| badge_of(t)).collect();
        badges.sort();
        badges.dedup();
        let Some(buyer) = buyer.filter(|_| !badges.is_empty()) else {
            tracing::debug!(
                source = log.source(),
                block = row.block_number,
                log = row.log_index,
                "sale without a buyer or a mirrored item: passed"
            );
            continue;
        };
        sales.push((
            buyer.to_ascii_lowercase(),
            row.tx_hash.to_ascii_lowercase(),
            row.block_time.saturating_mul(1000),
            badges,
        ));
    }
    let Some(last) = rows[..passed].last().map(|r| (r.block_number, r.log_index)) else {
        tx.rollback().await?;
        return Ok((0, false));
    };
    let addresses: Vec<String> = sales
        .iter()
        .map(|s| s.0.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let done = completed(&mut tx, &addresses).await?;
    let notify = Some(pass.asset_base.as_str());
    for (buyer, tx_hash, sale_at, badges) in &sales {
        let Ok(address) = Address::parse(buyer) else {
            continue;
        };
        for badge in badges {
            if done.contains(&(buyer.clone(), badge.to_string())) {
                continue;
            }
            award(&mut tx, &address, badge, notify, |p| {
                ev::buyer(p, badge, tx_hash, *sale_at)
            })
            .await?;
        }
    }
    super::advance(&mut tx, log.source(), last.0, last.1).await?;
    tx.commit().await?;
    super::report_lag(log.source(), until - last.0);
    Ok((
        passed as u64,
        passed == rows.len() && rows.len() as i64 == limit,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(signer: &str, caller: &str, assets: Value) -> Row {
        Row {
            block_number: 1,
            log_index: 0,
            block_time: 0,
            buyer: Some(signer.into()),
            tx_hash: "0x".into(),
            assets: Some(assets),
            caller: Some(caller.into()),
            item_types: Vec::new(),
        }
    }

    #[test]
    fn a_listing_is_bought_by_its_caller_a_bid_by_its_signer() {
        let listing = row(
            "0xseller",
            "0xbuyer",
            json!({"sent": [{"assetType": 4, "contractAddress": "0xC", "value": "2"}],
                   "received": [{"assetType": 1, "contractAddress": "0xm", "value": "10"}]}),
        );
        assert_eq!(
            trade(&listing),
            Some(("0xbuyer".into(), vec![(4, "0xc".into(), "2".into())]))
        );
        let bid = row(
            "0xbidder",
            "0xowner",
            json!({"sent": [{"assetType": 1, "contractAddress": "0xm", "value": "10"}],
                   "received": [{"assetType": "3", "contractAddress": "0xc", "value": "77"}]}),
        );
        assert_eq!(
            trade(&bid),
            Some(("0xbidder".into(), vec![(3, "0xc".into(), "77".into())]))
        );
        let currency_only = row(
            "0xa",
            "0xb",
            json!({"sent": [{"assetType": 1, "contractAddress": "0xm", "value": "1"}]}),
        );
        assert_eq!(trade(&currency_only), None);
    }

    #[test]
    fn item_types_pick_the_buyer_badge() {
        assert_eq!(badge_of("emote_v1"), ev::EMOTIONISTA);
        assert_eq!(badge_of("wearable_v2"), ev::FASHIONISTA);
        assert_eq!(badge_of("smart_wearable_v1"), ev::FASHIONISTA);
    }
}
