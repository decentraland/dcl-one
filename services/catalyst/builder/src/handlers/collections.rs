// from: catalyrst/crates/catalyrst-builder/src/handlers/collections.rs (the by-id routes); the listing rule from builder-server's Collection.router.ts getCollections/getAddressCollections and Item.router.ts getItems/getAddressItems
use std::sync::Arc;

use axum::Json;
use axum::extract::{OriginalUri, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::SharedState;
use crate::auth::{signed_path, try_extract_signer};
use crate::http::{ApiData, ApiError};
use crate::items::{CollectionMetaOut, FullItemOut, ItemQuery};
use crate::onchain;

use super::drafts::signer;
use super::onchain::ItemsParams;

const CURATION_STATUSES: [&str; 3] = ["pending", "approved", "rejected"];

const DEFAULT_LIMIT: i64 = 100_000;

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct PaginatedFullItemsOut {
    pub total: i64,
    pub limit: i64,
    pub pages: i64,
    pub page: i64,
    pub results: Vec<FullItemOut>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub enum CollectionItemsOut {
    Plain(Vec<FullItemOut>),
    Paginated(PaginatedFullItemsOut),
}

fn should_paginate(page: Option<i64>, limit: Option<i64>) -> bool {
    matches!((page, limit), (Some(p), Some(l)) if p != 0 && l != 0 && l < DEFAULT_LIMIT)
}

/// The owner or an admin address reads a collection by id.
pub async fn get_collection(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Json<ApiData<CollectionMetaOut>>, ApiError> {
    let signer = signer(&state, &headers, "get", &original, &uri).await?;

    let collection_id = Uuid::parse_str(id.trim()).map_err(|_| ApiError::not_found("Not found"))?;

    let meta = state
        .items
        .collection_by_id(&collection_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Not found"))?;

    let is_admin = state
        .settings
        .admin_addresses
        .iter()
        .any(|a| a == signer.as_str());
    if signer.as_str() != meta.eth_address.to_ascii_lowercase() && !is_admin {
        return Err(ApiError::unauthorized("Unauthorized"));
    }

    Ok(Json(ApiData::ok(CollectionMetaOut::from(&meta))))
}

#[derive(Debug, Default, Deserialize)]
pub struct CollectionItemsParams {
    pub status: Option<String>,
    #[serde(rename = "mappingStatus")]
    pub mapping_status: Option<String>,
    pub synced: Option<bool>,
    pub name: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

/// The explorer's primary route: the collection's items as `FullItem`s, for
/// the owner or an admin address; paginated only when `page` and `limit`
/// are both truthy and `limit` is below the upstream default.
pub async fn get_collection_items(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Query(params): Query<CollectionItemsParams>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Json<ApiData<CollectionItemsOut>>, ApiError> {
    let signer = signer(&state, &headers, "get", &original, &uri).await?;

    if let Some(status) = &params.status
        && !CURATION_STATUSES.contains(&status.as_str())
    {
        return Err(ApiError::bad_request("Invalid Status provided"));
    }

    let collection_id = Uuid::parse_str(id.trim()).map_err(|_| ApiError::not_found("Not found"))?;

    let owner = state
        .items
        .collection_owner(&collection_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Not found"))?;

    let is_admin = state
        .settings
        .admin_addresses
        .iter()
        .any(|a| a == signer.as_str());
    if signer.as_str() != owner && !is_admin {
        return Err(ApiError::unauthorized("Unauthorized"));
    }

    let paginate = should_paginate(params.page, params.limit);

    let q = ItemQuery {
        status: params.status,
        mapping_status: params.mapping_status,
        synced: params.synced,
        name: params.name,
        page: params.page,
        limit: params.limit,
    };

    let (items, total) = state.items.items_for_collection(&collection_id, &q).await?;
    let results: Vec<FullItemOut> = items.iter().map(|i| i.to_out()).collect();

    let data = if paginate {
        let limit = params.limit.unwrap_or(0);
        let page = params.page.unwrap_or(0);
        let pages = if limit > 0 {
            (total + limit - 1) / limit
        } else {
            0
        };
        CollectionItemsOut::Paginated(PaginatedFullItemsOut {
            total,
            limit,
            pages,
            page,
            results,
        })
    } else {
        CollectionItemsOut::Plain(results)
    };

    Ok(Json(ApiData::ok(data)))
}

/// builder-server's `HTTPError('Unauthorized', { eth_address }, 401)`: a
/// listing refusal names the wallet the request was about (the path's
/// address, the signer's, `""` anonymously), with no `message` key.
fn unauthorized_for(eth_address: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(
            json!({ "ok": false, "data": { "eth_address": eth_address }, "error": "Unauthorized" }),
        ),
    )
        .into_response()
}

/// The curation committee (builder-server's `isCommitteeMember`):
/// `BUILDER_ADMIN_ADDRESSES` and the curators the collections subgraphs
/// list, memoised five minutes as `/v1/collections/curation` reads them;
/// without a reader only the configured addresses, and a failed lookup
/// refuses (a listing is never widened by an outage).
pub(super) async fn is_committee(state: &SharedState, address: &str) -> bool {
    if state
        .settings
        .admin_addresses
        .iter()
        .any(|a| a.eq_ignore_ascii_case(address))
    {
        return true;
    }
    let Some(market) = state
        .market
        .as_deref()
        .filter(|m| onchain::configured(Some(m)))
    else {
        return false;
    };
    match state
        .committee_memo
        .get_or_fetch((), || async {
            onchain::committee_members(market).await.map(Arc::new)
        })
        .await
    {
        Ok(members) => members
            .iter()
            .any(|m| m.address.eq_ignore_ascii_case(address)),
        Err(error) => {
            tracing::warn!(error = %error, "committee lookup failed; the listing is refused");
            false
        }
    }
}

/// `Some(refusal)` unless the request is signed by `address` itself or by
/// the committee; an unsigned or unverifiable chain is the signer's own
/// error (400/401/503, as every signed builder route).
async fn refusal_unless_own_or_committee(
    state: &SharedState,
    headers: &HeaderMap,
    original: &Uri,
    local: &Uri,
    address: &str,
) -> Result<Option<Response>, ApiError> {
    let signer = signer(state, headers, "get", original, local).await?;
    if signer.as_str().eq_ignore_ascii_case(address) || is_committee(state, signer.as_str()).await {
        return Ok(None);
    }
    Ok(Some(unauthorized_for(address)))
}

#[derive(Debug, Default, Deserialize)]
pub struct CollectionsParams {
    pub q: Option<String>,
    pub is_published: Option<String>,
}

/// `GET /v1/collections` (`getCollections`): `q` with `is_published=true`
/// is the public search over the published collections, anonymous;
/// otherwise the committee lists every wallet's drafts and anyone else,
/// signed or not, is refused with the signer (`""` anonymously). A wallet
/// reads its own set at `/v1/{address}/collections`.
pub async fn get_collections(
    State(state): State<SharedState>,
    Query(params): Query<CollectionsParams>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let q = params.q.as_deref().map(str::trim).filter(|q| !q.is_empty());
    if let Some(q) = q
        && params.is_published.as_deref() == Some("true")
    {
        return Ok(Json(ApiData::ok(
            state.items.published_collections_named(q).await?,
        ))
        .into_response());
    }
    match try_extract_signer(
        &headers,
        "get",
        &signed_path(&original, &uri),
        state.eip1654.as_deref(),
    )
    .await
    {
        Some(signer) if is_committee(&state, signer.as_str()).await => Ok(Json(ApiData::ok(
            state.items.collection_drafts_of_everyone().await?,
        ))
        .into_response()),
        Some(signer) => Ok(unauthorized_for(signer.as_str())),
        None => Ok(unauthorized_for("")),
    }
}

/// `GET /v1/items` (`getItems`): the committee lists every wallet's item
/// drafts; a signed wallet that is not on it is refused with its address.
pub async fn get_items(
    State(state): State<SharedState>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let signer = signer(&state, &headers, "get", &original, &uri).await?;
    if !is_committee(&state, signer.as_str()).await {
        return Ok(unauthorized_for(signer.as_str()));
    }
    Ok(Json(ApiData::ok(state.items.item_drafts_of_everyone().await?)).into_response())
}

/// `GET /v1/{address}/collections` (`getAddressCollections`): the wallet
/// itself or the committee, signed; anyone else is refused with the path's
/// address. The set is the on-chain reader's (`onchain::get_address_collections`).
pub async fn get_address_collections(
    State(state): State<SharedState>,
    Path(address): Path<String>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if let Some(refused) =
        refusal_unless_own_or_committee(&state, &headers, &original, &uri, &address).await?
    {
        return Ok(refused);
    }
    Ok(
        super::onchain::get_address_collections(State(state), Path(address))
            .await?
            .into_response(),
    )
}

/// `GET /v1/{address}/items` (`getAddressItems`): the same rule over the
/// on-chain items (`onchain::get_address_items`).
pub async fn get_address_items(
    State(state): State<SharedState>,
    Path(address): Path<String>,
    Query(params): Query<ItemsParams>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if let Some(refused) =
        refusal_unless_own_or_committee(&state, &headers, &original, &uri, &address).await?
    {
        return Ok(refused);
    }
    Ok(
        super::onchain::get_address_items(State(state), Path(address), Query(params))
            .await?
            .into_response(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::Router;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use tower::ServiceExt;

    use crate::shared::signed_fetch::test_support::signed_headers_by;
    use crate::tests::support::{OWNER_KEY, cleanup, temp_storage, wallet};

    const STRANGER_KEY: &str = "0x2222222222222222222222222222222222222222222222222222222222222222";
    const COMMITTEE_KEY: &str =
        "0x3333333333333333333333333333333333333333333333333333333333333333";

    fn address_of(key: &str) -> String {
        format!("{:#x}", wallet(key).address())
    }

    async fn state(committee: &str) -> Option<SharedState> {
        let pool = crate::db::scratch::pool(4).await?;
        let settings = crate::Settings {
            admin_addresses: vec![committee.to_string()],
            ..Default::default()
        };
        Some(Arc::new(
            crate::State::new(
                pool,
                temp_storage().await,
                Arc::default(),
                dcl_http::Outbound::for_tests(),
                None,
                None,
                settings,
            )
            .unwrap(),
        ))
    }

    async fn get(app: &Router, path: &str, key: Option<&str>) -> (StatusCode, Value) {
        let mut request = Request::get(path);
        if let Some(key) = key {
            let now = chrono::Utc::now().timestamp_millis().to_string();
            let local = path.split('?').next().unwrap();
            for (name, value) in signed_headers_by(&wallet(key), "get", local, "{}", true, &now)
                .await
                .iter()
            {
                request = request.header(name, value);
            }
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        if path.starts_with("/v1/drafts/") {
            assert_eq!(
                response.headers()[axum::http::header::CACHE_CONTROL],
                "private, no-store"
            );
        }
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .unwrap();
        (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
    }

    fn refusal_of(address: &str) -> Value {
        json!({ "ok": false, "data": { "eth_address": address }, "error": "Unauthorized" })
    }

    fn ids(body: &Value) -> Vec<String> {
        body["data"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|c| c["id"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The four listings, builder-server's rule: a wallet that owns nothing
    /// and is not on the committee gets the upstream 401 envelope naming the
    /// wallet the request was about, the owner its own set, the committee
    /// every set, the anonymous published search its matches; v2 answered
    /// `{ok:true,data:[]}` to the stranger and served the address routes unsigned.
    #[tokio::test]
    async fn listings_are_visible_to_the_owner_and_the_committee_only() {
        let (owner, stranger, committee) = (
            address_of(OWNER_KEY),
            address_of(STRANGER_KEY),
            address_of(COMMITTEE_KEY),
        );
        let Some(state) = state(&committee).await else {
            eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
            return;
        };
        let app = crate::router(state.clone());
        let tag = Uuid::new_v4().simple().to_string();
        let (draft, published) = (Uuid::new_v4(), Uuid::new_v4());
        let saved = state
            .items
            .save_collection_draft(
                &owner,
                &serde_json::from_value(json!({ "id": draft, "name": format!("draft {tag}") }))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(saved.eth_address, owner);
        state
            .items
            .save_collection_draft(
                &owner,
                &serde_json::from_value(
                    json!({ "id": published, "name": format!("SheFi {tag}%_") }),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        sqlx::query("UPDATE builder.collections SET is_published = TRUE WHERE id = $1")
            .bind(published)
            .execute(state.items.pool())
            .await
            .unwrap();
        let item = Uuid::new_v4();
        let item_draft = serde_json::from_value(json!({
            "id": item, "name": "My wearable", "collection_id": draft,
            "type": "wearable", "data": { "category": "upper_body" }, "rarity": "common", "price": "0",
            "metrics": { "triangles": 42, "meshes": 1 }
        }))
        .unwrap();
        state
            .items
            .save_item_draft(&owner, &item_draft)
            .await
            .unwrap();

        for (kind, expected) in [("items", item), ("collections", draft)] {
            let path = format!("/v1/drafts/{kind}");
            let (status, body) = get(&app, &path, Some(OWNER_KEY)).await;
            assert_eq!(
                status,
                StatusCode::OK,
                "local drafts need no upstream reader: {body}"
            );
            assert!(ids(&body).contains(&expected.to_string()), "{body}");
            for key in [STRANGER_KEY, COMMITTEE_KEY] {
                let (status, body) = get(&app, &path, Some(key)).await;
                assert_eq!(status, StatusCode::OK);
                assert!(
                    !ids(&body).contains(&expected.to_string()),
                    "local drafts are always signer-scoped: {body}"
                );
            }
            let (status, _) = get(&app, &path, None).await;
            assert_eq!(
                status,
                StatusCode::BAD_REQUEST,
                "missing signed-fetch headers are malformed"
            );
        }

        // /v1/items: the committee, or the upstream refusal with the signer
        let (status, body) = get(&app, "/v1/items", Some(STRANGER_KEY)).await;
        assert_eq!(
            (status, body),
            (StatusCode::UNAUTHORIZED, refusal_of(&stranger))
        );
        let (status, body) = get(&app, "/v1/items", Some(OWNER_KEY)).await;
        assert_eq!(
            (status, body),
            (StatusCode::UNAUTHORIZED, refusal_of(&owner)),
            "an owner is not the committee either"
        );
        let (status, body) = get(&app, "/v1/items", Some(COMMITTEE_KEY)).await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            ids(&body).contains(&item.to_string()),
            "the committee sees another wallet's item draft: {body}"
        );
        let (status, body) = get(&app, "/v1/items", None).await;
        assert_ne!(
            status,
            StatusCode::OK,
            "unsigned is the signer's error: {body}"
        );
        assert_eq!(body["ok"], false);

        // /v1/collections: the committee, the anonymous published search, else the refusal
        let (status, body) = get(&app, "/v1/collections", None).await;
        assert_eq!((status, body), (StatusCode::UNAUTHORIZED, refusal_of("")));
        let (status, body) = get(&app, "/v1/collections", Some(STRANGER_KEY)).await;
        assert_eq!(
            (status, body),
            (StatusCode::UNAUTHORIZED, refusal_of(&stranger))
        );
        let (status, body) = get(&app, "/v1/collections", Some(COMMITTEE_KEY)).await;
        assert_eq!(status, StatusCode::OK);
        let listed = ids(&body);
        assert!(
            listed.contains(&draft.to_string()) && !listed.contains(&published.to_string()),
            "{body}"
        );
        let search = format!("/v1/collections?q={tag}&is_published=true");
        let (status, body) = get(&app, &search, None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let found = ids(&body);
        assert!(
            found.contains(&published.to_string()) && !found.contains(&draft.to_string()),
            "{body}"
        );
        let (status, body) = get(
            &app,
            &format!("/v1/collections?q={tag}%25_&is_published=true"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            ids(&body).contains(&published.to_string()),
            "the pattern characters of q are literal: {body}"
        );
        let (status, body) = get(
            &app,
            &format!("/v1/collections?q=nobody-{tag}&is_published=true"),
            None,
        )
        .await;
        assert_eq!((status, ids(&body)), (StatusCode::OK, vec![]));
        let (status, body) = get(&app, &format!("/v1/collections?q={tag}"), None).await;
        assert_eq!(
            (status, body),
            (StatusCode::UNAUTHORIZED, refusal_of("")),
            "q without is_published=true is no search"
        );

        // /v1/{address}/collections and /items: signed by the wallet or the committee
        for route in ["collections", "items"] {
            let path = format!("/v1/{owner}/{route}");
            let (status, body) = get(&app, &path, None).await;
            assert_ne!(status, StatusCode::OK, "unsigned {route}: {body}");
            assert_eq!(body["ok"], false);
            let (status, body) = get(&app, &path, Some(STRANGER_KEY)).await;
            assert_eq!(
                (status, body),
                (StatusCode::UNAUTHORIZED, refusal_of(&owner)),
                "{route}: the refusal names the path's wallet"
            );
            for key in [OWNER_KEY, COMMITTEE_KEY] {
                let (status, body) = get(&app, &path, Some(key)).await;
                assert_eq!(
                    status,
                    StatusCode::SERVICE_UNAVAILABLE,
                    "{route}: admitted, then the reader's 503 without a subgraph: {body}"
                );
            }
            let (status, _) = get(
                &app,
                &format!("/v1/{}/{route}", owner.to_uppercase().replace("0X", "0x")),
                Some(OWNER_KEY),
            )
            .await;
            assert_eq!(
                status,
                StatusCode::SERVICE_UNAVAILABLE,
                "{route}: the path's case does not matter"
            );
        }

        cleanup(&state.items, &[draft, published], &[]).await;
    }

    #[test]
    fn pagination_truthiness_matches_upstream_js() {
        assert!(should_paginate(Some(1), Some(10)));
        assert!(should_paginate(Some(1), Some(99_999)));
        assert!(!should_paginate(None, Some(10)));
        assert!(!should_paginate(Some(1), None));
        assert!(!should_paginate(None, None));
        assert!(!should_paginate(Some(0), Some(10)));
        assert!(!should_paginate(Some(1), Some(0)));
        assert!(!should_paginate(Some(1), Some(100_000)));
        assert!(!should_paginate(Some(1), Some(100_001)));
    }

    #[test]
    fn collection_items_union_arms_serialize_correctly() {
        let plain = ApiData::ok(CollectionItemsOut::Plain(vec![]));
        assert_eq!(
            serde_json::to_value(&plain).unwrap(),
            json!({ "ok": true, "data": [] }),
        );

        let paginated = ApiData::ok(CollectionItemsOut::Paginated(PaginatedFullItemsOut {
            total: 5,
            limit: 2,
            pages: 3,
            page: 1,
            results: vec![],
        }));
        assert_eq!(
            serde_json::to_value(&paginated).unwrap(),
            json!({
                "ok": true,
                "data": { "total": 5, "limit": 2, "pages": 3, "page": 1, "results": [] },
            }),
        );
    }
}
