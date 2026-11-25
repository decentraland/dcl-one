//! Marketplace sales served from a common published prefix of the requested chains.
use axum::response::{IntoResponse, Response};
use chrono::Utc;
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
    if error
        .downcast_ref::<sqlx::Error>()
        .and_then(sqlx::Error::as_database_error)
        .is_some_and(|e| e.code().as_deref() == Some("57014"))
    {
        return super::nft_reads::timed_out();
    }
    tracing::error!(%error, "reading published marketplace sales");
    ApiError::internal("database error").into_response()
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
    if !state.settings.market.indexes.sales_enabled
        || state.settings.eth_network.trim() != "mainnet"
    {
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

pub(crate) async fn require_mirror(
    tx: &mut sqlx::PgConnection,
    nets: &[ChainNet],
    mirror: &'static str,
    source: Source,
) -> Result<(), Response> {
    let ready:bool=sqlx::query_scalar("SELECT count(*)=cardinality($2::text[]) FROM shop.mirror_state WHERE mirror=$1 AND network=ANY($2) AND last_complete_pass_at IS NOT NULL")
        .bind(mirror).bind(nets.iter().map(|n|n.db_name()).collect::<Vec<_>>()).fetch_one(tx).await.map_err(db_error)?;
    if !ready {
        return Err(unavailable(
            source,
            "the requested marketplace metadata has no complete mirror pass",
        ));
    }
    Ok(())
}

async fn require_collections(
    state: &MarketplaceState,
    snap: &mut sale_fold::SalesSnapshot,
) -> Result<(), Response> {
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
) -> Result<(), Response> {
    use super::chain_walk;
    if !enabled {
        return Err(unavailable(source, "the Polygon index is disabled"));
    }
    let cursor = chain_walk::cursor(&mut *snap.transaction, indexer, ChainNet::Matic)
        .await
        .map_err(db_error)?;
    let (height, hash): (i64, String) = sqlx::query_as(
        "SELECT block_number,block_hash FROM shop.sale_publication WHERE network='matic'",
    )
    .fetch_one(&mut *snap.transaction)
    .await
    .map_err(db_error)?;
    let ready = cursor.is_some_and(|c| {
        chain_walk::is_ready_on(&c, 137, Utc::now())
            && c.start_block <= start_block as i64
            && c.diverged_at.is_none()
            && c.rebuild_requested_at.is_none()
            && c.last_block_hash.is_some()
            && c.last_block >= height
            && (c.last_block != height || c.last_block_hash.as_deref() == Some(hash.as_str()))
    });
    if !ready {
        return Err(unavailable(
            source,
            "the Polygon index does not cover the published sales",
        ));
    }
    Ok(())
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
    require_mirror(&mut snap.transaction, nets, "items", Source::ItemsMirror).await?;
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
    for price in [f.min_price.as_deref(), f.max_price.as_deref()]
        .into_iter()
        .flatten()
    {
        if price.is_empty() || price.len() > 78 || !price.bytes().all(|c| c.is_ascii_digit()) {
            return Err(
                ApiError::bad_request("price must be an unsigned integer in base units")
                    .into_response(),
            );
        }
    }
    let (mut snap, nets) = snapshot(state, f.network).await?;
    if f.categories
        .iter()
        .any(|c| matches!(c.as_str(), "wearable" | "emote"))
    {
        require_mirror(&mut snap.transaction, nets, "items", Source::ItemsMirror).await?;
    }
    sale_page(&mut snap, nets, f, first, skip).await
}

pub(super) async fn activity_sales(
    state: &MarketplaceState,
    address: &str,
    limit: i64,
) -> (Vec<Sale>, Vec<Sale>) {
    let Ok((mut snap, nets)) = snapshot(state, None).await else {
        return (Vec::new(), Vec::new());
    };
    let bought = SaleFilters {
        buyer: Some(address.to_owned()),
        ..Default::default()
    };
    let sold = SaleFilters {
        seller: Some(address.to_owned()),
        ..Default::default()
    };
    let buyers = sale_page(&mut snap, nets, &bought, limit, 0)
        .await
        .unwrap_or_default()
        .0;
    let sellers = sale_page(&mut snap, nets, &sold, limit, 0)
        .await
        .unwrap_or_default()
        .0;
    (buyers, sellers)
}

async fn sale_page(
    snap: &mut sale_fold::SalesSnapshot,
    nets: &[ChainNet],
    f: &SaleFilters,
    first: i64,
    skip: i64,
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
    let query = if f.seller.is_some() {
        sales_query!(nets, wallet "$9"; "list.sql")
    } else if unfiltered && f.sort_by != Some(SaleSortBy::MostExpensive) {
        sales_query!(@query nets, include_str!("recent_assets.sql"); "list.sql")
    } else {
        sales_query!(nets, "list.sql")
    };
    let (total, incomplete, rows): (i64, bool, Value) = query
        .bind(f.sale_type.map(|t| t.as_str()))
        .bind(&f.buyer)
        .bind(&f.seller)
        .bind(&f.contract_address)
        .bind(&f.item_id)
        .bind(&f.token_id)
        .bind(f.from.map(|t| t / 1000))
        .bind(f.to.map(|t| t / 1000))
        .bind(&f.min_price)
        .bind(&f.max_price)
        .bind(&f.categories)
        .bind(f.sort_by == Some(SaleSortBy::MostExpensive))
        .bind(first)
        .bind(skip)
        .bind(unfiltered)
        .fetch_one(&mut *snap.transaction)
        .await
        .map_err(db_error)?;
    if incomplete {
        return Err(unavailable(
            Source::SalesIndex,
            "the requested sales are missing asset or seller evidence",
        ));
    }
    Ok((serde_json::from_value(rows).map_err(db_error)?, total))
}

pub(super) async fn volume(
    state: &MarketplaceState,
    from_ms: i64,
) -> Result<super::rankings::VolumeData, Response> {
    let (mut snap, nets) = snapshot(state, None).await?;
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
}

mod rankings;
pub(super) use rankings::{collector_rankings, creator_rankings, item_rankings};

mod accounts;
pub(super) use accounts::accounts;
