use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;

use crate::SharedState;
use crate::http::{ApiData, ApiError};
use crate::onchain::{self, BuilderCollectionOut, MARKETPLACE_DOWN, OrphanItemOut};

#[derive(Debug, Default, Deserialize)]
pub struct ItemsParams {
    #[serde(rename = "onlyOrphans")]
    pub only_orphans: Option<bool>,
}

fn valid_address(addr: &str) -> bool {
    let a = addr.strip_prefix("0x").unwrap_or(addr);
    a.len() == 40 && a.chars().all(|c| c.is_ascii_hexdigit())
}

fn reader(state: &SharedState) -> Result<&crate::chain_index::ChainIndex, ApiError> {
    state
        .market
        .as_deref()
        .filter(|m| onchain::configured(Some(m)))
        .ok_or_else(|| ApiError::service_unavailable(MARKETPLACE_DOWN))
}

/// The readers behind `collections::get_address_collections` and
/// `get_address_items`, which admit the wallet itself or the committee
/// first (builder-server's rule; the old route answered anonymously).
pub async fn get_address_collections(
    State(state): State<SharedState>,
    Path(address): Path<String>,
) -> Result<Json<ApiData<Vec<BuilderCollectionOut>>>, ApiError> {
    if !valid_address(&address) {
        return Err(ApiError::bad_request("Invalid address"));
    }
    let market = reader(&state)?;
    let data = onchain::collections_for_address(market, &address).await?;
    Ok(Json(ApiData::ok(data)))
}

pub async fn get_address_items(
    State(state): State<SharedState>,
    Path(address): Path<String>,
    Query(params): Query<ItemsParams>,
) -> Result<Json<ApiData<Vec<OrphanItemOut>>>, ApiError> {
    if !valid_address(&address) {
        return Err(ApiError::bad_request("Invalid address"));
    }
    let only_orphans = params.only_orphans.unwrap_or(false);
    let market = reader(&state)?;
    let data = onchain::items_for_address(market, &address, only_orphans).await?;
    Ok(Json(ApiData::ok(data)))
}

#[cfg(test)]
mod tests {
    use axum::extract::{Path, Query, State};

    use super::*;
    use crate::tests::support::state_without_db;

    const ADDR: &str = "0x797066a17f83425c1b4c7a8cca52d19095520a52";

    #[tokio::test]
    async fn collections_return_503_when_no_collections_reader_is_configured() {
        for market in [
            None,
            Some(crate::tests::support::market_without_db(
                crate::chain_index::IndexSettings::for_tests(),
            )),
        ] {
            let state = state_without_db(market).await;
            let err = get_address_collections(State(state), Path(ADDR.to_string()))
                .await
                .expect_err("must be an error, not an empty 200");
            assert_eq!(err.status(), 503);
        }
    }

    #[tokio::test]
    async fn items_return_503_when_no_collections_reader_is_configured() {
        let state = state_without_db(None).await;
        let err = get_address_items(
            State(state),
            Path(ADDR.to_string()),
            Query(ItemsParams::default()),
        )
        .await
        .expect_err("must be an error, not an empty 200");
        assert_eq!(err.status(), 503);
    }

    #[tokio::test]
    async fn a_failing_subgraph_is_a_5xx_not_an_empty_200() {
        let settings = crate::chain_index::IndexSettings {
            matic_collections_subgraph_url: Some("https://subgraph.invalid/graphql".to_string()),
            ..crate::chain_index::IndexSettings::for_tests()
        };
        let market = crate::tests::support::market_without_db(settings);
        let state = state_without_db(Some(market)).await;
        let err = get_address_collections(State(state.clone()), Path(ADDR.to_string()))
            .await
            .expect_err("an unreachable subgraph must error, never yield an empty list");
        assert!(err.status() >= 500, "{err}");
        let err = get_address_items(
            State(state),
            Path(ADDR.to_string()),
            Query(ItemsParams::default()),
        )
        .await
        .unwrap_err();
        assert!(err.status() >= 500, "{err}");
    }

    #[tokio::test]
    async fn invalid_address_is_still_a_400() {
        let state = state_without_db(None).await;
        let res = get_address_collections(State(state), Path("nope".into())).await;
        assert_eq!(res.expect_err("invalid address must 400").status(), 400);
    }
}
