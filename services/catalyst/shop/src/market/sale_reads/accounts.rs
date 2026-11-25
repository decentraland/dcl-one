use super::{
    analytics_slot, cached_analytics, db_error, listings, nft_index, require_collections,
    require_polygon_cursor, snapshot,
};
use crate::land_authz::{ESTATE_REGISTRY_MAINNET, LAND_REGISTRY_MAINNET};
use crate::market::{
    MarketplaceState, Source,
    accounts::{Account, AccountFilters, AccountSortBy},
    chain_walk::{ChainNet, INDEXER_COMMITTEE},
    committee_index, readers, unavailable,
};
use axum::response::Response;
use sqlx::types::Json;

pub(in crate::market) async fn accounts(
    state: &MarketplaceState,
    filters: &AccountFilters,
) -> Result<(Vec<Account>, i64), Response> {
    let (first, skip) = readers::deep_page(filters.first, filters.skip, 100)?;
    let mut addresses: Vec<_> = filters.address.iter().map(|a| a.to_lowercase()).collect();
    if let Some(id) = &filters.id {
        addresses.push(id.split('-').next().unwrap_or(id).to_lowercase());
    }
    let _slot = analytics_slot(state)?;
    let (mut snap, nets) = snapshot(state, filters.network).await?;
    let polygon = if nets.contains(&ChainNet::Matic) {
        let collections = require_collections(state, &mut snap).await?;
        let committee = require_polygon_cursor(
            &mut snap,
            INDEXER_COMMITTEE,
            committee_index::START_BLOCK,
            state.settings.market.indexes.committee_enabled,
            Source::CommitteeIndex,
        )
        .await?;
        Some((collections, committee))
    } else {
        None
    };
    let key = format!(
        "published-accounts/{:?}/{polygon:?}/{filters:?}",
        snap.generations
    );
    cached_analytics(state, key, snap.expires_at, async {
        let (total, missing_payment, missing_creator, Json(rows)): (
            i64,
            bool,
            bool,
            Json<Vec<Account>>,
        ) = sales_query!(
            nets,
            "account_values.sql",
            "collection_counts.sql",
            "accounts.sql"
        )
        .bind(&addresses)
        .bind(
            filters
                .sort_by
                .unwrap_or(AccountSortBy::MostEarned)
                .order_by(),
        )
        .bind(first)
        .bind(skip)
        .fetch_one(&mut *snap.transaction)
        .await
        .map_err(db_error)?;
        if missing_payment {
            return Err(unavailable(
                Source::SalesIndex,
                "the account totals lack a proven payment split or seller",
            ));
        }
        if missing_creator {
            return Err(unavailable(
                Source::CollectionsIndex,
                "the original collection creator is unavailable",
            ));
        }
        Ok((rows, total))
    })
    .await
}
