use super::{
    analytics_slot, cached_analytics, db_error, listings, nft_index, require_collections,
    require_metadata, snapshot,
};
use crate::land_authz::{ESTATE_REGISTRY_MAINNET, LAND_REGISTRY_MAINNET};
use crate::market::{MarketplaceState, Source, unavailable};
use axum::response::Response;
use sqlx::types::Json;

pub(in crate::market) async fn item_rankings(
    state: &MarketplaceState,
    entity: crate::market::rankings::RankingEntity,
    filters: &crate::market::rankings::RankingsFilters,
) -> Result<crate::market::rankings::RankingResponse, Response> {
    use crate::market::rankings::{RankingEntity, RankingResponse, RankingsSortBy};

    let _slot = analytics_slot(state)?;
    let (mut snap, nets) = snapshot(state, None).await?;
    let items = require_metadata(&mut snap.transaction, nets, "items", Source::ItemsMirror).await?;
    let key = format!(
        "published-items-rank/{entity:?}/{filters:?}/{:?}/{items:?}",
        snap.generations
    );
    let rows = cached_analytics(state, key, snap.expires_at, async {
        sales_query!(nets, "item_ranks.sql")
            .bind(filters.from / 1000)
            .bind(entity == RankingEntity::Emotes)
            .bind(&filters.rarity)
            .bind(filters.sort_by == Some(RankingsSortBy::MostSales))
            .bind(ranking_limit(filters))
            .fetch_all(&mut *snap.transaction)
            .await
            .map_err(db_error)
    })
    .await?;
    Ok(RankingResponse::Items(rows))
}

fn ranking_limit(filters: &crate::market::rankings::RankingsFilters) -> i64 {
    use crate::market::rankings::LIFETIME_MAX_ROWS;
    filters
        .first
        .unwrap_or(LIFETIME_MAX_ROWS)
        .clamp(0, LIFETIME_MAX_ROWS)
}

pub(in crate::market) async fn collector_rankings(
    state: &MarketplaceState,
    filters: &crate::market::rankings::RankingsFilters,
) -> Result<crate::market::rankings::RankingResponse, Response> {
    use crate::market::rankings::{RankingResponse, RankingsSortBy};
    let _slot = analytics_slot(state)?;
    let (mut snap, nets) = snapshot(state, None).await?;
    let items = require_metadata(&mut snap.transaction, nets, "items", Source::ItemsMirror).await?;
    let key = format!(
        "published-collector-rank/{filters:?}/{:?}/{items:?}",
        snap.generations
    );
    let rows = cached_analytics(state, key, snap.expires_at, async {
        let (Json(rows), incomplete): (Json<Vec<crate::market::rankings::CollectorRank>>, bool) =
            sales_query!(nets, "collector_ranks.sql")
                .bind(filters.from / 1000)
                .bind(filters.sort_by == Some(RankingsSortBy::MostSales))
                .bind(ranking_limit(filters))
                .fetch_one(&mut *snap.transaction)
                .await
                .map_err(db_error)?;
        if incomplete {
            return Err(unavailable(
                Source::SalesIndex,
                "the collector's purchase history is missing a seller",
            ));
        }
        Ok(rows)
    })
    .await?;
    Ok(RankingResponse::Collectors(rows))
}

pub(in crate::market) async fn creator_rankings(
    state: &MarketplaceState,
    filters: &crate::market::rankings::RankingsFilters,
) -> Result<crate::market::rankings::RankingResponse, Response> {
    use crate::market::rankings::{CreatorRank, RankingResponse, RankingsSortBy};
    let _slot = analytics_slot(state)?;
    let (mut snap, nets) = snapshot(state, None).await?;
    let dependencies = if filters.from == 0 {
        let items =
            require_metadata(&mut snap.transaction, nets, "items", Source::ItemsMirror).await?;
        Some((items, require_collections(state, &mut snap).await?))
    } else {
        None
    };
    let cache_key = format!(
        "published-creator-rank/{filters:?}/{:?}/{dependencies:?}",
        snap.generations
    );
    let rows = cached_analytics(state, cache_key, snap.expires_at, async {
        let key = match filters.sort_by {
            Some(RankingsSortBy::MostSales) => "sales",
            Some(RankingsSortBy::MostVolume) => "volume",
            None => "earned",
        };
        let (Json(rows), unknown_money, unknown_creator): (Json<Vec<CreatorRank>>, bool, bool) =
            sales_query!(
                nets,
                "settled_sales.sql",
                "account_values.sql",
                "collection_counts.sql",
                "creator_ranks.sql"
            )
            .bind(filters.from / 1000)
            .bind(key)
            .bind(ranking_limit(filters))
            .fetch_one(&mut *snap.transaction)
            .await
            .map_err(db_error)?;
        if unknown_money {
            return Err(unavailable(
                Source::SalesIndex,
                "the requested ranking contains an unresolved sale",
            ));
        }
        if unknown_creator {
            return Err(unavailable(
                Source::CollectionsIndex,
                "the original collection creator is unavailable",
            ));
        }
        Ok(rows)
    })
    .await?;
    Ok(RankingResponse::Creators(rows))
}
