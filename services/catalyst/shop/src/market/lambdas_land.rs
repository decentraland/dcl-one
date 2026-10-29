//! `/lambdas/users/{address}/lands`, `/lambdas/users/{address}/genesis-parcels`
//! and `/lambdas/names/{name}/owner`. The old handlers read the squid's `nft`,
//! `parcel`, `estate`, `data` and `order` tables; here the LAND index
//! (`land_authz.token_right`, `estate_land`, the transfer log) holds the
//! ownership and the transfer order, the `nft` index the LAND names and
//! descriptions (`shop.land_data`), the legacy orders
//! (`shop.nft_order`) and the DCL names (`shop.nft` rows of
//! the DCLRegistrar). Before the indexes are published `/lands` and the
//! name owner read the marketplace subgraph (`graph::owned_lands`,
//! `graph::name_owner`), as the old lambdas did before the squid; a route
//! with neither answers the unavailable envelope naming the index.

use axum::Json;
use axum::extract::{Path, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::Row;

use crate::land_authz::{ESTATE_REGISTRY_MAINNET, LAND_REGISTRY_MAINNET};
use crate::shared::query_params::{
    MAX_PAGE_SIZE, NonPositivePolicy, OversizePolicy, Pagination, parse_pagination_with,
    parse_query_string,
};

use super::chain_walk::{ChainNet, INDEXER_NFT, ready_cursor};
use super::graph::{self, OwnedLand};
use super::nft_index::DCL_REGISTRAR;
use super::nft_reads::land_ready;
use super::{SharedState, Source, bad_request, internal_server_error, unavailable};

/// The land routes' pagination: an oversized page clamps to 1000, a
/// non-positive page or size to 1.
fn parse_pagination(req: &Request) -> (i64, i64) {
    let params = parse_query_string(req.uri().query().unwrap_or(""));
    let p = parse_pagination_with(
        &params,
        MAX_PAGE_SIZE as i64,
        OversizePolicy::Clamp,
        NonPositivePolicy::ClampToOne,
    )
    .unwrap_or(Pagination {
        page_size: 1,
        page_num: 1,
        offset: 0,
        limit: 1,
    });
    (p.page_size, p.page_num)
}

fn page_offset(page_num: i64, page_size: i64) -> i64 {
    page_num.saturating_sub(1).saturating_mul(page_size)
}

fn land_index_unavailable() -> Response {
    unavailable(
        Source::LandAuthzIndex,
        "LAND ownership reads the land-authz index, which is not published yet",
    )
}

async fn nft_index_ready(pool: &sqlx::PgPool) -> bool {
    ready_cursor(pool, INDEXER_NFT, ChainNet::Ethereum, Utc::now())
        .await
        .is_some()
}

/// The owner's parcels and estates, newest transfer first (the old
/// `transferred_at DESC`): each with its LAND data (name, description) and
/// its open legacy order's price, the image the squid stored (the map
/// service's parcel or estate PNG, under `LAND_IMAGE_BASE_URL`).
const USER_LANDS_SQL: &str = "\
SELECT ld.name, tr.token_address AS contract_address, tr.token_id::text AS token_id, \
       CASE WHEN tr.token_address = $4 THEN 'parcel' ELSE 'estate' END AS category, \
       tr.x, tr.y, ld.description, (ld.token_id IS NOT NULL) AS has_data, o.price::text AS price, \
       count(*) OVER () AS total \
FROM land_authz.token_right tr \
LEFT JOIN shop.land_data ld ON ld.token_address = tr.token_address AND ld.token_id = tr.token_id \
LEFT JOIN shop.nft_order o ON o.network = 'ethereum' AND o.contract_address = tr.token_address AND o.token_id = tr.token_id \
      AND o.status = 'open' AND o.expires_at > $6 AND o.seller = tr.owner \
LEFT JOIN LATERAL (SELECT e.block_number, e.log_index FROM land_authz.authz_event e \
                   WHERE e.token_address = tr.token_address AND e.token_id = tr.token_id AND e.kind = 'transfer' \
                   ORDER BY e.block_number DESC, e.log_index DESC LIMIT 1) moved ON true \
WHERE tr.token_address IN ($4, $5) AND tr.owner = $1 \
ORDER BY moved.block_number DESC NULLS LAST, moved.log_index DESC NULLS LAST, tr.token_address, tr.token_id \
LIMIT $2 OFFSET $3";

const USER_LANDS_COUNT_SQL: &str = "SELECT count(*) FROM land_authz.token_right tr WHERE tr.token_address IN ($2, $3) AND tr.owner = $1";

fn land_image(
    base: &str,
    category: &str,
    token_id: &str,
    x: Option<i32>,
    y: Option<i32>,
) -> Option<String> {
    let base = base.trim_end_matches('/');
    match (category, x, y) {
        ("parcel", Some(x), Some(y)) => Some(format!("{base}/v1/parcels/{x}/{y}/map.png")),
        ("estate", _, _) => Some(format!("{base}/v1/estates/{token_id}/map.png")),
        _ => None,
    }
}

fn land_element(base: &str, land: &OwnedLand) -> Value {
    let mut obj = json!({
        "contractAddress": land.contract_address,
        "tokenId": land.token_id,
        "category": land.category,
    });
    if let Some(n) = &land.name {
        obj["name"] = json!(n);
    }
    if land.category == "parcel" {
        if let Some(x) = land.x {
            obj["x"] = json!(x.to_string());
        }
        if let Some(y) = land.y {
            obj["y"] = json!(y.to_string());
        }
    }
    if let Some(d) = &land.description {
        obj["description"] = json!(d);
    }
    if let Some(p) = &land.price {
        obj["price"] = json!(p);
    }
    if let Some(img) = land_image(base, &land.category, &land.token_id, land.x, land.y) {
        obj["image"] = json!(img);
    }
    obj
}

fn lands_page(base: &str, lands: &[OwnedLand], page_size: i64, page_num: i64) -> Response {
    let offset = page_offset(page_num, page_size).max(0) as usize;
    let elements: Vec<Value> = lands
        .iter()
        .skip(offset)
        .take(page_size.max(0) as usize)
        .map(|l| land_element(base, l))
        .collect();
    Json(json!({ "elements": elements, "totalAmount": lands.len() as i64, "pageNum": page_num, "pageSize": page_size })).into_response()
}

/// The owner's lands from the marketplace subgraph, paged here (the reader
/// holds the whole holding for 30 s, so `totalAmount` is the same count the
/// index path reports).
async fn user_lands_from_reader(
    state: &SharedState,
    owner: &str,
    page_size: i64,
    page_num: i64,
) -> Response {
    match graph::owned_lands(&state.index, owner).await {
        Ok(lands) => lands_page(
            &state.settings.land_image_base_url,
            &lands,
            page_size,
            page_num,
        ),
        Err(e) => {
            tracing::warn!(owner = %owner, error = %e, "marketplace LAND reader failed");
            unavailable(
                Source::MarketplaceSubgraph,
                "the marketplace LAND reader did not answer",
            )
        }
    }
}

/// `GET /lambdas/users/{address}/lands`: the indexes once both are
/// published, the marketplace subgraph reader before that.
pub async fn user_lands(
    State(state): State<SharedState>,
    Path(addr): Path<String>,
    req: Request,
) -> Response {
    let (page_size, page_num) = parse_pagination(&req);
    let owner = addr.to_lowercase();
    let land_ready = land_ready(&state.pool).await;
    if !land_ready || !nft_index_ready(&state.pool).await {
        if state.settings.marketplace_subgraph_url.is_some() {
            return user_lands_from_reader(&state, &owner, page_size, page_num).await;
        }
        if !land_ready {
            return land_index_unavailable();
        }
        return unavailable(
            Source::NftIndex,
            "LAND names, descriptions and orders read the ethereum nft index, which is not published yet",
        );
    }
    let rows = match sqlx::query(USER_LANDS_SQL)
        .bind(&owner)
        .bind(page_size)
        .bind(page_offset(page_num, page_size))
        .bind(LAND_REGISTRY_MAINNET)
        .bind(ESTATE_REGISTRY_MAINNET)
        .bind(Utc::now().timestamp_millis())
        .fetch_all(&state.pool)
        .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!(error = %e, "user lands query failed");
            return internal_server_error();
        }
    };
    let total: i64 = match rows.first() {
        Some(r) => r.get("total"),
        None => sqlx::query_scalar(USER_LANDS_COUNT_SQL)
            .bind(&owner)
            .bind(LAND_REGISTRY_MAINNET)
            .bind(ESTATE_REGISTRY_MAINNET)
            .fetch_one(&state.pool)
            .await
            .unwrap_or(0),
    };
    let base = &state.settings.land_image_base_url;
    let elements: Vec<Value> = rows
        .iter()
        .map(|r| {
            let land = OwnedLand {
                contract_address: r.get("contract_address"),
                token_id: r.get("token_id"),
                category: r.get("category"),
                name: r.get("name"),
                x: r.get("x"),
                y: r.get("y"),
                description: r.get::<bool, _>("has_data").then(|| {
                    r.get::<Option<String>, _>("description")
                        .unwrap_or_default()
                }),
                price: r.get("price"),
                transferred_at: 0,
            };
            land_element(base, &land)
        })
        .collect();
    Json(json!({ "elements": elements, "totalAmount": total, "pageNum": page_num, "pageSize": page_size })).into_response()
}

/// Every Genesis parcel owned directly or through an owned estate: a parcel
/// in an estate belongs to the estate registry on chain, so the two legs are
/// disjoint.
const GENESIS_PARCELS_SQL: &str = "\
WITH owned AS ( \
    SELECT tr.token_id, tr.x, tr.y, false AS via_estate FROM land_authz.token_right tr \
    WHERE tr.token_address = $2 AND tr.owner = $1 AND tr.x IS NOT NULL \
  UNION ALL \
    SELECT p.token_id, p.x, p.y, true AS via_estate FROM land_authz.token_right e \
    JOIN land_authz.estate_land el ON el.estate_token_id = e.token_id \
    JOIN land_authz.token_right p ON p.token_address = $2 AND p.token_id = el.land_token_id \
    WHERE e.token_address = $3 AND e.owner = $1 AND p.x IS NOT NULL \
) \
SELECT token_id::text AS token_id, x, y, via_estate, count(*) OVER () AS total FROM owned ORDER BY x, y LIMIT $4 OFFSET $5";

const GENESIS_PARCELS_COUNT_SQL: &str = "\
SELECT (SELECT count(*) FROM land_authz.token_right tr WHERE tr.token_address = $2 AND tr.owner = $1 AND tr.x IS NOT NULL) \
     + (SELECT count(*) FROM land_authz.token_right e \
        JOIN land_authz.estate_land el ON el.estate_token_id = e.token_id \
        JOIN land_authz.token_right p ON p.token_address = $2 AND p.token_id = el.land_token_id \
        WHERE e.token_address = $3 AND e.owner = $1 AND p.x IS NOT NULL)";

/// `GET /lambdas/users/{address}/genesis-parcels`: an ownership list, not a
/// claim about delegated deployment permissions.
pub async fn user_genesis_parcels(
    State(state): State<SharedState>,
    Path(addr): Path<String>,
    req: Request,
) -> Response {
    let owner = addr.to_lowercase();
    if !super::is_valid_eth_address(&owner) {
        return bad_request("Address must be a valid Ethereum address");
    }
    if !land_ready(&state.pool).await {
        return land_index_unavailable();
    }
    let (page_size, page_num) = parse_pagination(&req);
    let rows = match sqlx::query(GENESIS_PARCELS_SQL)
        .bind(&owner)
        .bind(LAND_REGISTRY_MAINNET)
        .bind(ESTATE_REGISTRY_MAINNET)
        .bind(page_size)
        .bind(page_offset(page_num, page_size))
        .fetch_all(&state.pool)
        .await
    {
        Ok(rows) => rows,
        Err(err) => {
            tracing::warn!(owner = %owner, error = %err, "owned Genesis parcels lookup failed");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let total: i64 = match rows.first() {
        Some(r) => r.get("total"),
        None => match sqlx::query_scalar::<_, i64>(GENESIS_PARCELS_COUNT_SQL)
            .bind(&owner)
            .bind(LAND_REGISTRY_MAINNET)
            .bind(ESTATE_REGISTRY_MAINNET)
            .fetch_one(&state.pool)
            .await
        {
            Ok(total) => total,
            Err(err) => {
                tracing::warn!(owner = %owner, error = %err, "owned Genesis parcels count failed");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        },
    };
    let elements: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "tokenId": r.get::<String, _>("token_id"),
                "x": r.get::<i32, _>("x").to_string(),
                "y": r.get::<i32, _>("y").to_string(),
                "viaEstate": r.get::<bool, _>("via_estate"),
            })
        })
        .collect();
    Json(json!({ "elements": elements, "totalAmount": total, "pageNum": page_num, "pageSize": page_size })).into_response()
}

/// `GET /lambdas/names/{name}/owner`: the holder of the DCL name (`.dcl.eth`
/// optional), matched case-insensitively; 404 when nobody holds it.
pub async fn name_owner(State(state): State<SharedState>, Path(name): Path<String>) -> Response {
    // Preserve the public route's existing fallback posture on cursor faults.
    let ready = nft_index_ready(&state.pool).await;
    match read_name_owner(&state, &name, ready).await {
        Ok(Some(owner)) => Json(json!({"owner":owner.as_str()})).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(NameOwnerError::Unavailable(source, message)) => unavailable(source, message),
        Err(NameOwnerError::Store(error)) => {
            tracing::error!(%error, "name owner query failed");
            internal_server_error()
        }
    }
}

enum NameOwnerError {
    Unavailable(Source, &'static str),
    Store(String),
}

/// One source-complete owner lookup shared by the public lambdas route and
/// comms. Shop retains ownership of its NFT SQL and source selection.
async fn read_name_owner(
    state: &super::MarketplaceState,
    name: &str,
    ready: bool,
) -> Result<Option<crate::shared::identity::Address>, NameOwnerError> {
    let normalized = name.to_lowercase();
    let dcl_name = normalized.strip_suffix(".dcl.eth").unwrap_or(&normalized);
    if !ready {
        if state.settings.marketplace_subgraph_url.is_none() {
            return Err(NameOwnerError::Unavailable(
                Source::NftIndex,
                "DCL names read the ethereum nft index, which is not published yet",
            ));
        }
        return match graph::name_owner(&state.index, dcl_name).await {
            Ok(Some(owner)) => crate::shared::identity::Address::parse(&owner)
                .map(Some)
                .map_err(|error| NameOwnerError::Store(error.to_string())),
            Ok(None) => Ok(None),
            Err(e) => {
                tracing::warn!(name = %dcl_name, error = %e, "marketplace names reader failed");
                Err(NameOwnerError::Unavailable(
                    Source::MarketplaceSubgraph,
                    "the marketplace names reader did not answer",
                ))
            }
        };
    }
    let owner: Result<Option<String>, sqlx::Error> = sqlx::query_scalar(
        "SELECT n.owner FROM shop.nft n \
         WHERE lower(n.ens_subdomain) = lower($1) AND n.ens_subdomain IS NOT NULL \
           AND n.network = 'ethereum' AND n.contract_address = $2 AND n.owner IS NOT NULL \
         ORDER BY n.token_id LIMIT 1",
    )
    .bind(dcl_name)
    .bind(DCL_REGISTRAR)
    .fetch_optional(&state.pool)
    .await
    .map(Option::flatten);
    match owner {
        Ok(Some(owner)) if !owner.is_empty() => crate::shared::identity::Address::parse(&owner)
            .map(Some)
            .map_err(|error| NameOwnerError::Store(error.to_string())),
        Ok(_) => Ok(None),
        Err(error) => Err(NameOwnerError::Store(error.to_string())),
    }
}

#[async_trait::async_trait]
impl crate::comms::interfaces::NameOwnerReader for super::MarketplaceState {
    async fn name_owner(
        &self,
        name: &str,
    ) -> Result<Option<crate::shared::identity::Address>, String> {
        // A moderation write cannot infer an absent/unready source from a
        // failed readiness read, even when a subgraph fallback is configured.
        let ready = super::chain_walk::cursor(&self.pool, INDEXER_NFT, ChainNet::Ethereum)
            .await
            .map_err(|error| error.to_string())?
            .is_some_and(|cursor| super::chain_walk::is_ready(&cursor, Utc::now()));
        read_name_owner(self, name, ready)
            .await
            .map_err(|error| match error {
                NameOwnerError::Unavailable(_, message) => message.to_string(),
                NameOwnerError::Store(error) => error,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn moderation_name_owner_store_fault_is_unavailable_even_with_a_fallback() {
        use crate::comms::interfaces::NameOwnerReader;
        let pool = sqlx::PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap();
        pool.close().await;
        let state = super::super::MarketplaceState::new(
            pool,
            crate::http::Outbound::for_tests(),
            super::super::Settings {
                marketplace_subgraph_url: Some("https://reader.example/subgraph".into()),
                ..super::super::Settings::for_tests()
            },
        );
        assert!(NameOwnerReader::name_owner(&state, "Alice").await.is_err());
    }

    #[test]
    fn land_images_are_the_map_service_pngs() {
        assert_eq!(
            land_image(
                "https://api.decentraland.org/",
                "parcel",
                "1",
                Some(-3),
                Some(12)
            )
            .as_deref(),
            Some("https://api.decentraland.org/v1/parcels/-3/12/map.png")
        );
        assert_eq!(
            land_image("https://maps.example", "estate", "42", None, None).as_deref(),
            Some("https://maps.example/v1/estates/42/map.png")
        );
        assert!(land_image("https://x", "parcel", "1", None, None).is_none());
    }

    #[test]
    fn pages_clamp_like_the_other_land_routes() {
        let req = |q: &str| {
            Request::get(format!("/x?{q}"))
                .body(axum::body::Body::empty())
                .unwrap()
        };
        assert_eq!(parse_pagination(&req("pageSize=5000&pageNum=0")), (1000, 1));
        assert_eq!(parse_pagination(&req("pageSize=10&pageNum=3")), (10, 3));
        assert_eq!(page_offset(3, 10), 20);
    }
}
