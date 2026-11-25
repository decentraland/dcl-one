//! Marketplace sales served from a common published prefix of the requested chains.
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::Value;
use sqlx::types::Json;
use std::collections::BTreeMap;

use super::chain_walk::ChainNet;
use super::sales::{Sale, SaleFilters, SaleSortBy};
use super::wire::{ApiError, Network};
use super::{MarketplaceState, Source, listings, nft_index, readers, sale_fold, unavailable};
use crate::land_authz::{ESTATE_REGISTRY_MAINNET, LAND_REGISTRY_MAINNET};

macro_rules! sales_query {
    ($nets:expr, wallet $param:literal; $($path:literal),+) => {
        sales_query!(@query $nets, concat!(", requested_wallet AS (SELECT ", $param, "::text AS address)",
            include_str!("wallet_assets.sql")); $($path),+)
    };
    ($nets:expr, $($path:literal),+) => {
        sales_query!(@query $nets, ", scoped_assets AS NOT MATERIALIZED (SELECT * FROM shop.sale_asset)"; $($path),+)
    };
    (@query $nets:expr, $scope:expr; $($path:literal),+) => {
        sqlx::query_as(concat!(include_str!("seller_evidence.sql"), $scope, include_str!("public_sales.sql"),$(include_str!($path)),+))
            .bind($nets.iter().map(|n|n.db_name()).collect::<Vec<_>>())
            .bind(listings::mana_token(1)).bind(listings::mana_token(137))
            .bind(LAND_REGISTRY_MAINNET).bind(ESTATE_REGISTRY_MAINNET).bind(nft_index::DCL_REGISTRAR)
    };
}

mod proceeds;
pub(crate) use proceeds::{proceeds_page, proceeds_summary};

fn db_error(error: impl Into<anyhow::Error>) -> Response {
    let error = error.into();
    if matches!(
        error.downcast_ref::<sqlx::Error>(),
        Some(sqlx::Error::PoolTimedOut)
    ) {
        return analytics_busy();
    }
    if error
        .downcast_ref::<sqlx::Error>()
        .and_then(sqlx::Error::as_database_error)
        .is_some_and(|e| e.code().as_deref() == Some("57014"))
    {
        tracing::warn!("published marketplace sales query exceeded its statement timeout");
        return super::nft_reads::timed_out();
    }
    tracing::error!(%error, "reading published marketplace sales");
    ApiError::internal("database error").into_response()
}

fn analytics_busy() -> Response {
    unavailable(Source::SalesIndex, "sales analytics is busy")
}

fn read_slot(slots: &tokio::sync::Semaphore) -> Result<tokio::sync::SemaphorePermit<'_>, Response> {
    slots.try_acquire().map_err(|_| analytics_busy())
}

async fn cached_analytics<T>(
    state: &MarketplaceState,
    slots: &tokio::sync::Semaphore,
    key: String,
    expires_at: chrono::DateTime<Utc>,
    fetch: impl std::future::Future<Output = Result<T, Response>>,
) -> Result<T, Response>
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let ttl = readers::Ttl::S30;
    let value = if let Some(value) = state.readers.fresh(ttl, &key) {
        value
    } else {
        // A cold request already holds a read transaction. Bound the number
        // that can wait for another request's cache fill as well as leaders.
        let _slot = read_slot(slots)?;
        readers::memoized(state, ttl, key, || async {
            let result = fetch.await?;
            ensure_fresh(expires_at)?;
            serde_json::to_value(result).map_err(db_error)
        })
        .await?
    };
    ensure_fresh(expires_at)?;
    serde_json::from_value(value.as_ref().clone()).map_err(db_error)
}

pub(crate) fn ensure_fresh(expires_at: DateTime<Utc>) -> Result<(), Response> {
    if Utc::now() > expires_at {
        return Err(unavailable(
            Source::SalesIndex,
            "the sales publication expired during the read",
        ));
    }
    Ok(())
}

pub(crate) async fn seller_slot(
    state: &MarketplaceState,
) -> Result<tokio::sync::SemaphorePermit<'_>, Response> {
    // The 7-day and 30-day dashboards each ask for two windows. Wait
    // briefly for a slot before opening a database connection.
    let slot = tokio::time::timeout(
        std::time::Duration::from_millis(250),
        state.sales_seller_slots.acquire(),
    )
    .await;
    match slot {
        Ok(Ok(permit)) => Ok(permit),
        _ => Err(unavailable(
            Source::SalesIndex,
            "seller sales queries are busy",
        )),
    }
}

pub(crate) fn enabled(state: &MarketplaceState) -> bool {
    state.settings.market.indexes.sales_enabled
        && crate::shared::network::ChainPair::from_eth_network(&state.settings.eth_network)
            == Some(crate::shared::network::ChainPair::Mainnet)
}

pub(crate) async fn snapshot(
    state: &MarketplaceState,
    network: Option<Network>,
) -> Result<(sale_fold::SalesSnapshot, &'static [ChainNet]), Response> {
    let nets: &'static [ChainNet] = match network {
        Some(Network::Ethereum) => &[ChainNet::Ethereum],
        Some(Network::Matic) => &[ChainNet::Matic],
        None => &ChainNet::ALL,
    };
    if !enabled(state) {
        return Err(unavailable(
            Source::SalesIndex,
            "the mainnet sales index is disabled",
        ));
    }
    let now = Utc::now();
    let snap = sale_fold::snapshot(&state.pool, nets, now)
        .await
        .map_err(db_error)?
        .ok_or_else(|| {
            unavailable(
                Source::SalesIndex,
                "the requested sales history has no fresh publication",
            )
        })?;
    Ok((snap, nets))
}

pub(crate) async fn require_metadata(
    snap: &mut sale_fold::SalesSnapshot,
    nets: &[ChainNet],
    mirror: &'static str,
    source: Source,
) -> Result<Vec<String>, Response> {
    let now = Utc::now();
    let proofs: Vec<(bool, Option<String>, Option<NaiveDateTime>)> = sqlx::query_as(
        "SELECT projection.owned_at IS NOT NULL,
           CASE WHEN projection.owned_at IS NOT NULL THEN
             CASE WHEN projection.pending=0 AND projection.projected_through>=sales.block_number
                        AND walk.folded_block>=sales.block_number AND walk.full_history
                        AND walk.chain_id=CASE WHEN sales.network='matic' THEN 137 ELSE 1 END
                        AND walk.folded_at BETWEEN $3 AND $4
                        AND walk.diverged_at IS NULL AND walk.quarantined_at IS NULL
                        AND walk.rebuild_requested_at IS NULL THEN concat('index:', projection.owned_at,
                            '/',walk.sale_input_epoch,'/',walk.sale_history_revision,'/',coalesce(collection.epoch,0)) END
           ELSE 'mirror:'||copy.last_complete_pass_at END AS proof,
           CASE WHEN projection.owned_at IS NOT NULL THEN walk.folded_at END
         FROM shop.sale_publication sales
         LEFT JOIN shop.collection_projection projection USING(network)
         LEFT JOIN shop.collection_walk collection USING(network)
         LEFT JOIN shop.chain_cursor walk ON walk.network=sales.network AND walk.indexer='collections'
         LEFT JOIN shop.mirror_state copy ON copy.network=sales.network AND copy.mirror=$1
         WHERE sales.network=ANY($2)
         ORDER BY sales.network",
    )
    .bind(mirror).bind(nets.iter().map(|n|n.db_name()).collect::<Vec<_>>())
    .bind((now-super::chain_walk::READY_WINDOW).naive_utc()).bind(now.naive_utc())
    .fetch_all(&mut *snap.transaction).await.map_err(db_error)?;
    proofs
        .into_iter()
        .map(|(native, proof, folded_at)| {
            if let Some(folded_at) = folded_at {
                snap.expires_at = snap
                    .expires_at
                    .min(folded_at.and_utc() + super::chain_walk::READY_WINDOW);
            }
            proof.ok_or_else(|| {
                if native {
                    unavailable(
                        Source::CollectionsIndex,
                        "the native catalogue does not cover the published sales",
                    )
                } else {
                    unavailable(
                        source,
                        "the requested marketplace metadata has no complete mirror pass",
                    )
                }
            })
        })
        .collect()
}

async fn require_collections(
    state: &MarketplaceState,
    snap: &mut sale_fold::SalesSnapshot,
) -> Result<String, Response> {
    require_polygon_cursor(
        snap,
        super::chain_walk::INDEXER_COLLECTIONS,
        super::collections_index::START_BLOCK,
        state.settings.market.indexes.collections_enabled,
        Source::CollectionsIndex,
    )
    .await
}

async fn require_polygon_cursor(
    snap: &mut sale_fold::SalesSnapshot,
    indexer: &'static str,
    start_block: u64,
    enabled: bool,
    source: Source,
) -> Result<String, Response> {
    use super::chain_walk;
    if !enabled {
        return Err(unavailable(source, "the Polygon index is disabled"));
    }
    let now = Utc::now();
    let (identity, folded_at): (String, NaiveDateTime) = sqlx::query_as(
        "SELECT concat(c.sale_input_epoch,'/',c.sale_history_revision,'/',coalesce(w.epoch,0)), c.folded_at
         FROM shop.sale_publication p JOIN shop.chain_cursor c USING(network)
         LEFT JOIN shop.collection_walk w ON w.network=c.network AND c.indexer='collections'
         WHERE p.network='matic' AND c.indexer=$1 AND c.chain_id=137 AND c.full_history
           AND c.start_block<=$2 AND c.folded_at BETWEEN $3 AND $4
           AND c.folded_block BETWEEN p.block_number AND c.last_block
           AND c.diverged_at IS NULL AND c.quarantined_at IS NULL AND c.rebuild_requested_at IS NULL
           AND c.last_block_hash IS NOT NULL AND (c.last_block>p.block_number OR c.last_block_hash=p.block_hash)",
    )
    .bind(indexer)
    .bind(start_block as i64)
    .bind((now - chain_walk::READY_WINDOW).naive_utc())
    .bind(now.naive_utc())
    .fetch_optional(&mut *snap.transaction)
    .await
    .map_err(db_error)?
    .ok_or_else(|| unavailable(source, "the Polygon index does not cover the published sales"))?;
    snap.expires_at = snap
        .expires_at
        .min(folded_at.and_utc() + chain_walk::READY_WINDOW);
    Ok(identity)
}

pub(crate) struct SellerStats {
    pub summary: super::sales::SalesSummary,
    pub window: BTreeMap<(String, String), (i64, String)>,
}

#[derive(serde::Deserialize)]
struct MintTotals {
    contract_address: String,
    item_id: String,
    lifetime: i64,
    sold: i64,
    earned: String,
}

#[derive(sqlx::FromRow)]
struct SellerTotals {
    total: i64,
    mints: i64,
    resales: i64,
    earned: String,
    royalty_resales: i64,
    volume: String,
    by_collection: Json<Vec<super::sales::SalesSummaryCollection>>,
    items: Json<Vec<MintTotals>>,
    incomplete: bool,
}

pub(crate) async fn seller_stats(
    snap: &mut sale_fold::SalesSnapshot,
    nets: &[ChainNet],
    f: &super::sales::SalesSummaryFilters,
) -> Result<SellerStats, Response> {
    use super::sales::{SalesSummary, SalesSummaryItem, SalesSummaryRoyalties};
    require_metadata(snap, nets, "items", Source::ItemsMirror).await?;
    let row: SellerTotals = sales_query!(nets, wallet "$7"; "summary.sql")
        .bind(f.seller.to_lowercase())
        .bind(f.from)
        .bind(f.to)
        .fetch_one(&mut *snap.transaction)
        .await
        .map_err(db_error)?;
    if row.incomplete {
        return Err(unavailable(
            Source::SalesIndex,
            "the published sales history contains an unresolved seller",
        ));
    }
    let mut by_item = Vec::with_capacity(row.items.len());
    let mut window = BTreeMap::new();
    for i in row.items.0 {
        window.insert(
            (i.contract_address.clone(), i.item_id.clone()),
            (i.sold, i.earned),
        );
        by_item.push(SalesSummaryItem {
            contract_address: i.contract_address,
            item_id: i.item_id,
            sold_lifetime: i.lifetime,
        });
    }
    ensure_fresh(snap.expires_at)?;
    Ok(SellerStats {
        summary: SalesSummary {
            total: row.total,
            mints: row.mints,
            resales: row.resales,
            earned_wei: row.earned,
            by_collection: row.by_collection.0,
            by_item,
            royalties: SalesSummaryRoyalties {
                resales: row.royalty_resales,
                volume_wei: row.volume,
            },
        },
        window,
    })
}

pub(super) async fn list(
    state: &MarketplaceState,
    f: &SaleFilters,
) -> Result<(Vec<Sale>, i64), Response> {
    let (first, skip) = readers::deep_page(f.first, f.skip, 100)?;
    for (field, value) in [
        ("minPrice", &f.min_price),
        ("maxPrice", &f.max_price),
        ("itemId", &f.item_id),
        ("tokenId", &f.token_id),
    ] {
        if let Some(value) = value
            && (value.is_empty() || value.len() > 78 || !value.bytes().all(|c| c.is_ascii_digit()))
        {
            return Err(
                ApiError::bad_request(format!("{field} must be an unsigned integer"))
                    .into_response(),
            );
        }
    }
    let _slot = read_slot(&state.sales_pages_slots)?;
    let (mut snap, nets) = snapshot(state, f.network).await?;
    if f.categories
        .iter()
        .any(|c| matches!(c.as_str(), "wearable" | "emote"))
    {
        require_metadata(&mut snap, nets, "items", Source::ItemsMirror).await?;
    }
    sale_page(&mut snap, nets, f, first, skip, false).await
}

pub(super) async fn activity_sales(
    state: &MarketplaceState,
    address: &str,
    limit: i64,
) -> (Vec<Sale>, Vec<Sale>) {
    if !enabled(state) {
        return (Vec::new(), Vec::new());
    }
    let Ok(_slot) = seller_slot(state).await else {
        tracing::warn!("activity omitted sales while seller sales queries were busy");
        return (Vec::new(), Vec::new());
    };
    let (mut snap, nets) = match snapshot(state, None).await {
        Ok(snapshot) => snapshot,
        Err(response) => {
            tracing::debug!(status=%response.status(), "activity omitted sales without a usable publication");
            return (Vec::new(), Vec::new());
        }
    };
    let bought = SaleFilters {
        buyer: Some(address.to_owned()),
        ..Default::default()
    };
    let sold = SaleFilters {
        seller: Some(address.to_owned()),
        ..Default::default()
    };
    let mut legs = [Vec::new(), Vec::new()];
    for (sales, filters) in legs.iter_mut().zip([bought, sold]) {
        match sale_page(&mut snap, nets, &filters, limit, 0, true).await {
            Ok((rows, _)) => *sales = rows,
            Err(response) => {
                tracing::warn!(
                    status=%response.status(),
                    buyer=filters.buyer.is_some(),
                    "activity omitted a failed sales read"
                );
            }
        }
    }
    let [buyers, sellers] = legs;
    (buyers, sellers)
}

async fn sale_page(
    snap: &mut sale_fold::SalesSnapshot,
    nets: &[ChainNet],
    f: &SaleFilters,
    first: i64,
    skip: i64,
    allow_partial: bool,
) -> Result<(Vec<Sale>, i64), Response> {
    let unfiltered = f.sale_type.is_none()
        && f.buyer.is_none()
        && f.seller.is_none()
        && f.contract_address.is_none()
        && f.token_id.is_none()
        && f.item_id.is_none()
        && f.from.is_none()
        && f.to.is_none()
        && f.min_price.is_none()
        && f.max_price.is_none()
        && f.categories.is_empty();
    let recent_lane = f.buyer.is_none()
        && f.seller.is_none()
        && f.contract_address.is_none()
        && f.token_id.is_none()
        && f.item_id.is_none()
        && f.min_price.is_none()
        && f.max_price.is_none()
        && f.categories.is_empty()
        && f.sort_by != Some(SaleSortBy::MostExpensive);
    let query = if f.seller.is_some() {
        sales_query!(nets, wallet "$9"; "list.sql")
    } else if recent_lane {
        sales_query!(@query nets, include_str!("recent_assets.sql"); "list.sql")
    } else {
        sales_query!(nets, "list.sql")
    };
    // A generic plan cannot distinguish a token lookup from a wide page
    // when the predicates are optional. Keep the index paths parameter-aware.
    sqlx::query("SET LOCAL plan_cache_mode = force_custom_plan")
        .execute(&mut *snap.transaction)
        .await
        .map_err(db_error)?;
    let (total, incomplete, rows): (i64, bool, Value) = query
        .bind(f.sale_type.map(|t| t.as_str()))
        .bind(&f.buyer)
        .bind(&f.seller)
        .bind(&f.contract_address)
        .bind(&f.item_id)
        .bind(&f.token_id)
        .bind(
            f.from
                .map(|t| t.div_euclid(1000) + i64::from(t.rem_euclid(1000) != 0)),
        )
        .bind(f.to.map(|t| t.div_euclid(1000)))
        .bind(&f.min_price)
        .bind(&f.max_price)
        .bind(&f.categories)
        .bind(f.sort_by == Some(SaleSortBy::MostExpensive))
        .bind(first)
        .bind(skip)
        .bind(unfiltered)
        .bind(readers::count_cap(first, skip))
        .bind(allow_partial)
        .fetch_one(&mut *snap.transaction)
        .await
        .map_err(db_error)?;
    if incomplete {
        return Err(unavailable(
            Source::SalesIndex,
            "the requested sales are missing asset or seller evidence",
        ));
    }
    ensure_fresh(snap.expires_at)?;
    Ok((serde_json::from_value(rows).map_err(db_error)?, total))
}

pub(super) async fn volume(
    state: &MarketplaceState,
    from_ms: i64,
) -> Result<super::rankings::VolumeData, Response> {
    let (mut snap, nets) = snapshot(state, None).await?;
    let key = format!("published-volume/{:?}/{from_ms}", snap.generations);
    cached_analytics(
        state,
        &state.sales_analytics_slots,
        key,
        snap.expires_at,
        async {
            let (sales, volume, creators_earnings, dao_earnings, incomplete): (
                i64,
                String,
                String,
                String,
                bool,
            ) = sales_query!(nets, "volume.sql")
                .bind(from_ms / 1000)
                .fetch_one(&mut *snap.transaction)
                .await
                .map_err(db_error)?;
            if incomplete {
                return Err(unavailable(
                    Source::SalesIndex,
                    "the requested volume contains an unresolved payment split",
                ));
            }
            Ok(super::rankings::VolumeData {
                sales,
                volume,
                creators_earnings,
                dao_earnings,
            })
        },
    )
    .await
}

mod rankings;
pub(super) use rankings::{collector_rankings, creator_rankings, item_rankings};

mod accounts;
pub(super) use accounts::accounts;
