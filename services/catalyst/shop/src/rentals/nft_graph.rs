//! Marketplace asset reader for nodes that do not walk the Ethereum NFT index.

use super::nft::{LandNft, NftReadError};
use crate::land_authz::LandNetwork;
use crate::market::{MarketplaceState, graph::graph_query};
use crate::shared::cache::TtlMap;
use serde_json::{Value, json};
use std::time::Duration;

pub(super) type Cache = TtlMap<(String, String), Option<LandNft>>;

pub(super) fn cache() -> Cache {
    Cache::bounded(Duration::from_secs(30), 10_000)
}

const QUERY: &str = r#"
query nft($contract: Bytes!, $tokenId: BigInt!) {
  _meta { block { number } }
  nfts(first: 1, where: { contractAddress: $contract, tokenId: $tokenId }) {
    contractAddress tokenId category owner { id } searchText searchDistanceToPlaza
    searchAdjacentToRoad searchEstateSize createdAt updatedAt transferredAt
  }
}"#;

fn integer(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| value.as_str()?.parse().ok())
}

fn parse(
    data: &Value,
    contract: &str,
    token: &str,
    estate: bool,
) -> Result<Option<LandNft>, NftReadError> {
    let incomplete = || NftReadError::Subgraph("incomplete listing asset response".into());
    let rows = data["nfts"].as_array().ok_or_else(incomplete)?;
    let Some(nft) = rows.first() else {
        return Ok(None);
    };
    let category = if estate { "estate" } else { "parcel" };
    if nft["contractAddress"]
        .as_str()
        .is_none_or(|address| !contract.eq_ignore_ascii_case(address))
        || nft["tokenId"].as_str() != Some(token)
        || nft["category"].as_str() != Some(category)
    {
        return Err(incomplete());
    }
    let time = |key: &str| integer(&nft[key]).filter(|v| *v > 0).ok_or_else(incomplete);
    let updated_at = time("updatedAt")?;
    Ok(Some(LandNft {
        nft_id: format!("{contract}-{token}"),
        category: category.into(),
        owner_address: nft["owner"]["id"]
            .as_str()
            .ok_or_else(incomplete)?
            .to_ascii_lowercase(),
        search_text: nft["searchText"].as_str().unwrap_or_default().into(),
        distance_to_plaza: integer(&nft["searchDistanceToPlaza"]).and_then(|v| v.try_into().ok()),
        adjacent_to_road: nft["searchAdjacentToRoad"].as_bool(),
        estate_size: integer(&nft["searchEstateSize"]).and_then(|v| v.try_into().ok()),
        created_at: time("createdAt")?,
        updated_at,
        transferred_at: time("transferredAt")?,
        transfer_block: None,
        membership_block: if estate {
            integer(&data["_meta"]["block"]["number"]).ok_or_else(incomplete)?
        } else {
            0
        },
        membership_checked_at: estate.then_some(updated_at),
    }))
}

pub(super) async fn read(
    market: &MarketplaceState,
    cache: &Cache,
    net: LandNetwork,
    address: &str,
    token: &str,
) -> Result<Option<LandNft>, NftReadError> {
    let url = market
        .settings
        .marketplace_subgraph_url
        .as_deref()
        .ok_or_else(|| NftReadError::Subgraph("MARKETPLACE_SUBGRAPH_URL is unset".into()))?;
    let contract = address.to_ascii_lowercase();
    let estate = contract == net.estate_registry;
    if !estate && contract != net.land_registry {
        return Ok(None);
    }
    // Graph BigInt results use canonical decimals even for a zero-padded input.
    let decimal = token.trim_start_matches('0');
    let token = if decimal.is_empty() { "0" } else { decimal };
    cache
        .get_or_fetch((contract.clone(), token.into()), || async {
            let data = graph_query(
                &market.index,
                url,
                QUERY,
                json!({"contract": contract, "tokenId": token}),
            )
            .await
            .map_err(NftReadError::Subgraph)?;
            parse(&data, &contract, token, estate)
        })
        .await
}
