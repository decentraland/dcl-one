use axum::Json;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};
use sqlx::types::Json as DbJson;

use super::{db_error, ensure_fresh, listings, nft_index, read_slot, require_metadata, snapshot};
use crate::land_authz::{ESTATE_REGISTRY_MAINNET, LAND_REGISTRY_MAINNET};
use crate::market::chain_walk::ChainNet;
use crate::market::wire::{ApiError, Params, strict_page, unsigned_decimal};
use crate::market::{MarketplaceState, SharedState, Source, is_valid_eth_address, unavailable};

const MAX_DATE_MS: i64 = 8_640_000_000_000_000;

pub(in crate::market) async fn get(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let filters = match parse(&Params::new(&pairs)) {
        Ok(filters) => filters,
        Err(error) => return error.into_response(),
    };
    match read(&state, filters).await {
        Ok((data, total, royalties)) => {
            Json(json!({ "data": data, "total": total, "royaltiesWei": royalties })).into_response()
        }
        Err(response) => response,
    }
}

struct RoyaltiesFilters {
    creator: String,
    from: Option<i64>,
    to: Option<i64>,
    first: i64,
    skip: i64,
}

fn parse(params: &Params<'_>) -> Result<RoyaltiesFilters, ApiError> {
    let creator = params
        .get_string("creator", None)
        .filter(|s| is_valid_eth_address(s))
        .ok_or_else(|| ApiError::bad_request("A valid creator address is required"))?;
    let bound = |key| match unsigned_decimal(params, key) {
        Ok(ms) if ms.is_none_or(|ms| ms <= MAX_DATE_MS) => Ok(ms),
        _ => Err(ApiError::bad_request(format!(
            "{key} must be an epoch timestamp in milliseconds"
        ))),
    };
    let (from, to) = (bound("from")?, bound("to")?);
    if from.zip(to).is_some_and(|(from, to)| from > to) {
        return Err(ApiError::bad_request(
            "from must be less than or equal to to",
        ));
    }
    let (first, skip) = strict_page(params)?;
    Ok(RoyaltiesFilters {
        creator: creator.to_lowercase(),
        from,
        to,
        first,
        skip,
    })
}

async fn read(
    state: &MarketplaceState,
    filters: RoyaltiesFilters,
) -> Result<(Vec<Value>, i64, String), Response> {
    let _slot = read_slot(&state.royalty_analytics_slots)?;
    let (mut snap, nets) = snapshot(state, None).await?;
    require_metadata(&mut snap, nets, "items", Source::ItemsMirror).await?;
    require_metadata(
        &mut snap,
        &[ChainNet::Ethereum],
        "collections",
        Source::CollectionsMirror,
    )
    .await?;
    let (DbJson(data), total, royalties, unresolved): (DbJson<Vec<Value>>, i64, String, bool) =
        sales_query!(nets, scope "creator_sales_scope.sql"; "settled_sales.sql", "royalties.sql")
            .bind(filters.creator)
            .bind(filters.from)
            .bind(filters.to)
            .bind(filters.first)
            .bind(filters.skip)
            .fetch_one(&mut *snap.transaction)
            .await
            .map_err(db_error)?;
    if unresolved {
        return Err(unavailable(
            Source::SalesIndex,
            "a resale is missing its seller or a proven royalty settlement",
        ));
    }
    ensure_fresh(snap.expires_at)?;
    Ok((data, total, royalties))
}
