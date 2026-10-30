//! The LAND/Estate reader behind the create and refresh routes: the old crate
//! probed `squid_marketplace.nft` for the asset's owner, category and search
//! columns; v2 has no squid database, so the same row comes from the
//! marketplace subgraph (`nfts(where: {contractAddress, tokenId})`) through
//! the stage-4 reader (`marketplace::graph::graph_query`: the ONE client, the
//! SSRF guard, the retry ladder, the body cap), memoized per asset for
//! [`NFT_TTL`]. Without `MARKETPLACE_SUBGRAPH_URL` the caller answers the
//! unavailable envelope: an ownership check that cannot run refuses, never
//! guesses.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use crate::market::MarketplaceState;
use crate::market::graph::graph_query;

/// How long one asset's row is reused; the same budget as the other
/// per-owner readers (`graph::OWNED_TTL`).
pub const NFT_TTL: Duration = Duration::from_secs(30);
pub const NFT_CACHE_MAX_ENTRIES: usize = 10_000;

/// The marketplace subgraph's `nft` row as the listing routes read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandNft {
    /// `<contract>-<tokenId>`, the wire's `nftId`.
    pub nft_id: String,
    /// `parcel` | `estate` | `ens` | ... (the subgraph's spelling).
    pub category: String,
    pub owner_address: String,
    pub search_text: String,
    pub distance_to_plaza: Option<i32>,
    pub adjacent_to_road: Option<bool>,
    pub estate_size: Option<i32>,
    /// Seconds since the epoch.
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug)]
pub enum NftReadError {
    /// No `MARKETPLACE_SUBGRAPH_URL` (or no marketplace state at all).
    Unconfigured,
    Upstream(String),
}

const QUERY_NFT: &str = r#"
query nft($contract: Bytes!, $tokenId: BigInt!) {
  nfts(first: 1, where: { contractAddress: $contract, tokenId: $tokenId }) {
    id
    category
    owner { id }
    searchText
    searchDistanceToPlaza
    searchAdjacentToRoad
    searchEstateSize
    createdAt
    updatedAt
  }
}"#;

fn int_of(v: &Value, key: &str) -> Option<i64> {
    match v.get(key)? {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_i64(),
        _ => None,
    }
}

pub(crate) fn parse_nft(data: &Value, contract_address: &str, token_id: &str) -> Option<LandNft> {
    let nft = data.get("nfts")?.as_array()?.first()?;
    let owner = nft.get("owner")?.get("id")?.as_str()?.to_lowercase();
    Some(LandNft {
        nft_id: format!("{}-{}", contract_address.to_lowercase(), token_id),
        category: nft
            .get("category")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_lowercase(),
        owner_address: owner,
        search_text: nft
            .get("searchText")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        distance_to_plaza: int_of(nft, "searchDistanceToPlaza").and_then(|v| i32::try_from(v).ok()),
        adjacent_to_road: nft.get("searchAdjacentToRoad").and_then(Value::as_bool),
        estate_size: int_of(nft, "searchEstateSize").and_then(|v| i32::try_from(v).ok()),
        created_at: int_of(nft, "createdAt").unwrap_or(0),
        updated_at: int_of(nft, "updatedAt").unwrap_or(0),
    })
}

/// One asset's row from the marketplace subgraph; `Ok(None)` when the
/// subgraph has no such token. Cached per `(contract, token)` for [`NFT_TTL`].
pub async fn nft_by_contract_token(
    market: Option<&Arc<MarketplaceState>>,
    cache: &crate::shared::cache::TtlMap<(String, String), Option<Arc<LandNft>>>,
    contract_address: &str,
    token_id: &str,
) -> Result<Option<Arc<LandNft>>, NftReadError> {
    let Some(market) = market else {
        return Err(NftReadError::Unconfigured);
    };
    let Some(url) = market.settings.marketplace_subgraph_url.clone() else {
        return Err(NftReadError::Unconfigured);
    };
    let contract = contract_address.to_lowercase();
    let token = token_id.to_string();
    cache
        .get_or_fetch((contract.clone(), token.clone()), || async move {
            let data = graph_query(
                &market.index,
                &url,
                QUERY_NFT,
                json!({ "contract": contract, "tokenId": token }),
            )
            .await
            .map_err(NftReadError::Upstream)?;
            Ok(parse_nft(&data, &contract, &token).map(Arc::new))
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_subgraph_row_and_lowercases_the_owner() {
        let data = json!({ "nfts": [ {
            "id": "0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d-42",
            "category": "parcel",
            "owner": { "id": "0xF39Fd6e51aad88F6F4ce6aB8827279cffFb92266" },
            "searchText": "-10,20",
            "searchDistanceToPlaza": 3,
            "searchAdjacentToRoad": true,
            "searchEstateSize": null,
            "createdAt": "1600000000",
            "updatedAt": 1700000000
        } ] });
        let nft = parse_nft(&data, "0xF87E31492FAF9A91B02EE0DEAAD50D51D56D5D4D", "42").unwrap();
        assert_eq!(
            nft,
            LandNft {
                nft_id: "0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d-42".into(),
                category: "parcel".into(),
                owner_address: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266".into(),
                search_text: "-10,20".into(),
                distance_to_plaza: Some(3),
                adjacent_to_road: Some(true),
                estate_size: None,
                created_at: 1_600_000_000,
                updated_at: 1_700_000_000,
            }
        );
    }

    #[test]
    fn an_empty_page_or_a_row_without_an_owner_is_none() {
        assert!(parse_nft(&json!({ "nfts": [] }), "0xabc", "1").is_none());
        assert!(
            parse_nft(
                &json!({ "nfts": [ { "id": "x", "category": "parcel" } ] }),
                "0xabc",
                "1"
            )
            .is_none()
        );
        assert!(parse_nft(&json!({}), "0xabc", "1").is_none());
    }

    #[tokio::test]
    async fn without_a_marketplace_subgraph_the_reader_reports_unconfigured() {
        let cache = crate::shared::cache::TtlMap::bounded(NFT_TTL, 10);
        assert!(matches!(
            nft_by_contract_token(None, &cache, "0xabc", "1").await,
            Err(NftReadError::Unconfigured)
        ));
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://nobody@127.0.0.1:1/nowhere")
            .unwrap();
        let market = Arc::new(MarketplaceState::new(
            pool,
            crate::http::Outbound::for_tests(),
            crate::market::Settings::for_tests(),
        ));
        assert!(matches!(
            nft_by_contract_token(Some(&market), &cache, "0xabc", "1").await,
            Err(NftReadError::Unconfigured)
        ));
    }
}
