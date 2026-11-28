// from: catalyrst/crates/catalyrst-builder/src/handlers/collections.rs (the by-id routes); the listing rule from builder-server's Collection.router.ts getCollections/getAddressCollections and Item.router.ts getItems/getAddressItems

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

/// Counts published-collection creator reads that failed, so the stored
/// (possibly previous) owner answered.
const CREATOR_READ_FAILURE_COUNTER: dcl_metrics::CounterFamily<0> = dcl_metrics::CounterFamily::new(
    CREATOR_READ_FAILURES,
    "Collection creator lookups that failed and used the stored owner.",
    Some(dcl_metrics::Unit::Count),
    &[],
);
pub const FAMILIES: &[dcl_metrics::Descriptor] = &[CREATOR_READ_FAILURE_COUNTER.descriptor];

pub const CREATOR_READ_FAILURES: &str = "builder_collection_creator_read_failures_total";

/// The collection with a published standard collection's owner taken from
/// its on-chain creator (builder-server's `syncOwnerFromChain`). A changed
/// creator moves the collection and its items to it when writes are on;
/// off, only this answer carries it. One read per contract is reused for
/// [`crate::CREATOR_MEMO_TTL`]. Without a Polygon RPC or with the read
/// failing, the stored owner answers.
pub(super) async fn synced_collection(
    state: &SharedState,
    id: &Uuid,
) -> Result<crate::items::CollectionMetaRow, ApiError> {
    let mut meta = state
        .items
        .collection_by_id(id)
        .await?
        .ok_or_else(|| ApiError::not_found("Not found"))?;
    let (true, Some(contract), Some(url)) = (
        meta.follows_chain(),
        meta.contract_address.clone(),
        state.settings.polygon_rpc_url.as_deref(),
    ) else {
        return Ok(meta);
    };
    let creator = state
        .creator_memo
        .get_or_fetch(contract.clone(), || async {
            let chain = crate::publication_chain::PublicationChain {
                http: &state.http,
                url,
            };
            let read = async {
                state
                    .polygon_checked
                    .get_or_try_init(|| chain.ensure_polygon())
                    .await?;
                chain.current_creator(&contract).await
            };
            Ok::<_, std::convert::Infallible>(match read.await {
                Ok(creator) => Some(creator),
                Err(error) => {
                    CREATOR_READ_FAILURE_COUNTER.handle([]).increment(1);
                    tracing::warn!(collection = %id, error = ?error, "collection creator read failed; the stored owner answers");
                    None
                }
            })
        })
        .await
        .unwrap_or_else(|never| match never {});
    if let Some(creator) = creator.filter(|c| *c != meta.eth_address) {
        if state.settings.writes_enabled {
            state.items.sync_collection_owner(id, &creator).await?;
        }
        meta.eth_address = creator;
    }
    Ok(meta)
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

    let meta = synced_collection(&state, &collection_id).await?;

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

    let meta = synced_collection(&state, &collection_id).await?;
    let owner = meta.eth_address.as_str();

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

    let (mut items, total) = state.items.items_for_collection(&collection_id, &q).await?;
    if meta.follows_chain() {
        // Rows keep the previous creator while writes are off.
        for item in &mut items {
            item.eth_address = owner.to_owned();
        }
    }
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
/// `BUILDER_ADMIN_ADDRESSES` and the current members of the Polygon
/// committee index ([`crate::committee`]); an unpublished index admits only
/// the configured addresses, and a failed read answers 503 to anyone else, as
/// the curation console does (a listing is never widened by an outage).
pub(super) async fn is_committee(state: &SharedState, address: &str) -> Result<bool, ApiError> {
    if state
        .settings
        .admin_addresses
        .iter()
        .any(|a| a.eq_ignore_ascii_case(address))
    {
        return Ok(true);
    }
    match crate::committee::current_members(state.items.pool()).await {
        Ok(Some(members)) => Ok(members.iter().any(|m| m.eq_ignore_ascii_case(address))),
        Ok(None) => Ok(false),
        Err(error) => {
            tracing::warn!(error = %error, "committee read failed");
            Err(ApiError::http(503, "committee unavailable"))
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
    if signer.as_str().eq_ignore_ascii_case(address) || is_committee(state, signer.as_str()).await?
    {
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
        Some(signer) if is_committee(&state, signer.as_str()).await? => Ok(Json(ApiData::ok(
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
    if !is_committee(&state, signer.as_str()).await? {
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
    use std::sync::Arc;

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
        Some(state_on(crate::db::scratch::pool(4).await?, committee).await)
    }

    async fn state_on(pool: sqlx::PgPool, committee: &str) -> SharedState {
        state_with(
            pool,
            crate::Settings {
                admin_addresses: vec![committee.to_string()],
                ..Default::default()
            },
        )
        .await
    }

    async fn state_with(pool: sqlx::PgPool, settings: crate::Settings) -> SharedState {
        Arc::new(
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
        )
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

    /// The committee is the index's current membership: a wallet whose latest
    /// MemberSet removed it is refused, a member that never curated is
    /// admitted, a MemberSet past the published block does not count yet,
    /// and an unpublished or stale index admits only the configured addresses.
    #[tokio::test]
    async fn committee_is_the_current_indexed_membership() {
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
            return;
        };
        let (removed, member, admin) = (
            address_of(STRANGER_KEY),
            address_of(COMMITTEE_KEY),
            address_of(OWNER_KEY),
        );
        let pending = "0x00000000000000000000000000000000000000cc";
        let state = state_on(pool.clone(), &admin).await;
        let app = crate::router(state.clone());
        let old = "0x71d9350ef44e1e451f00e447c0dff2d1fb75510a";
        let current = "0xaeec95a8aa671a6d3fec56594827d7804964fa70";
        for (block, contract, wallet, set) in [
            (10_i64, old, removed.as_str(), true),
            (20, current, removed.as_str(), false),
            (30, current, member.as_str(), true),
            (150, current, pending, true),
        ] {
            sqlx::query(
                "INSERT INTO shop.committee_event (network, block_number, log_index, block_time, contract_address, member, is_member) \
                 VALUES ('matic', $1, 0, $1, $2, $3, $4)",
            )
            .bind(block)
            .bind(contract)
            .bind(wallet)
            .bind(set)
            .execute(&pool)
            .await
            .unwrap();
        }
        let publish = |folded_at: &'static str| {
            let pool = pool.clone();
            async move {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, folded_block, fold_through, folded_at, full_history, chain_id) \
                     VALUES ('committee', 'matic', 0, 100, 100, 100, {folded_at}, true, 137) \
                     ON CONFLICT (indexer, network) DO UPDATE SET folded_at = EXCLUDED.folded_at, last_block = EXCLUDED.last_block"
                )))
                .execute(&pool)
                .await
                .unwrap();
            }
        };

        assert!(
            !is_committee(&state, &member).await.unwrap(),
            "no published index: not a member yet"
        );
        assert!(is_committee(&state, &admin).await.unwrap());

        publish("timezone('UTC', now())").await;
        assert!(is_committee(&state, &member).await.unwrap());
        assert!(
            !is_committee(&state, &removed).await.unwrap(),
            "removed member"
        );
        assert!(
            !is_committee(&state, pending).await.unwrap(),
            "past the published block"
        );
        assert!(is_committee(&state, &admin).await.unwrap());

        // A cursor not bound to Polygon, or in quarantine, admits no member.
        for (sql, why) in [
            (
                "UPDATE shop.chain_cursor SET chain_id = NULL WHERE indexer = 'committee'",
                "an unadopted cursor",
            ),
            (
                "UPDATE shop.chain_cursor SET chain_id = 137, quarantined_at = timezone('UTC', now()), quarantine_reason = 'test' WHERE indexer = 'committee'",
                "a cursor in quarantine",
            ),
        ] {
            sqlx::query(sql).execute(&pool).await.unwrap();
            assert!(!is_committee(&state, &member).await.unwrap(), "{why}");
        }
        sqlx::query("UPDATE shop.chain_cursor SET quarantined_at = NULL, quarantine_reason = NULL WHERE indexer = 'committee'")
            .execute(&pool)
            .await
            .unwrap();
        assert!(is_committee(&state, &member).await.unwrap());

        sqlx::query("UPDATE shop.chain_cursor SET last_block = 200 WHERE indexer = 'committee'")
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            is_committee(&state, &member).await.unwrap(),
            "a walk ahead of its last publish keeps the published membership"
        );
        assert!(
            !is_committee(&state, pending).await.unwrap(),
            "until it publishes"
        );

        let (status, _) = get(&app, "/v1/items", Some(COMMITTEE_KEY)).await;
        assert_eq!(status, StatusCode::OK, "a member that never curated");
        let (status, body) = get(&app, "/v1/items", Some(STRANGER_KEY)).await;
        assert_eq!(
            (status, body),
            (StatusCode::UNAUTHORIZED, refusal_of(&removed))
        );

        publish("timezone('UTC', now()) - interval '2 hours'").await;
        assert!(
            !is_committee(&state, &member).await.unwrap(),
            "a stale index"
        );
        assert!(is_committee(&state, &admin).await.unwrap());

        // A failed read answers 503, not a refusal; the admin addresses
        // need no read.
        sqlx::query("ALTER TABLE shop.committee_event RENAME TO committee_event_gone")
            .execute(&pool)
            .await
            .unwrap();
        let (status, body) = get(&app, "/v1/items", Some(COMMITTEE_KEY)).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        let (status, body) = get(&app, "/v1/items", Some(OWNER_KEY)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        pool.close().await;
    }

    /// On a deployment that walks the committee index, the curation console
    /// lists the published index's current members, and answers 503 while
    /// the publication is missing or stale (never an empty committee) and
    /// when it cannot be read. On one that does not walk it (the switch off,
    /// or a testnet, which schedules no committee walk), a missing
    /// publication lists the queue with no indexed members.
    #[tokio::test]
    async fn curation_console_lists_the_indexed_committee() {
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
            return;
        };
        let admin = address_of(OWNER_KEY);
        let state = state_with(
            pool.clone(),
            crate::Settings {
                admin_addresses: vec![admin.clone()],
                committee_index_walked: true,
                ..Default::default()
            },
        )
        .await;
        let app = crate::router(state);
        let index_off = crate::router(state_on(pool.clone(), &admin).await);
        let (removed, member) = (
            "0x00000000000000000000000000000000000000aa",
            "0x00000000000000000000000000000000000000bb",
        );
        for (block, wallet, set) in [
            (10_i64, removed, true),
            (20, member, true),
            (30, removed, false),
        ] {
            sqlx::query(
                "INSERT INTO shop.committee_event (network, block_number, log_index, block_time, contract_address, member, is_member) \
                 VALUES ('matic', $1, 0, $1, '0xaeec95a8aa671a6d3fec56594827d7804964fa70', $2, $3)",
            )
            .bind(block)
            .bind(wallet)
            .bind(set)
            .execute(&pool)
            .await
            .unwrap();
        }
        let committee = |body: &Value| {
            body["data"]["committee"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|m| m["address"].as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };

        let unpublished = |body: &Value| {
            body["message"]
                .as_str()
                .is_some_and(|m| m.starts_with("the curation committee index is not published"))
        };
        let (status, body) = get(&app, "/v1/collections/curation", Some(OWNER_KEY)).await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "unpublished index: {body}"
        );
        assert!(unpublished(&body), "{body}");
        let (status, body) = get(&index_off, "/v1/collections/curation", Some(OWNER_KEY)).await;
        assert_eq!(status, StatusCode::OK, "index off: {body}");
        assert!(committee(&body).is_empty(), "index off: {body}");

        sqlx::query(
            "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, folded_block, fold_through, folded_at, full_history, chain_id) \
             VALUES ('committee', 'matic', 0, 100, 100, 100, timezone('UTC', now()), true, 137)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let (status, body) = get(&app, "/v1/collections/curation", Some(OWNER_KEY)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(committee(&body), vec![member.to_string()], "{body}");
        // A current publication another process maintains is listed by a
        // node that does not walk the index too.
        let (status, body) = get(&index_off, "/v1/collections/curation", Some(OWNER_KEY)).await;
        assert_eq!(status, StatusCode::OK, "index off: {body}");
        assert_eq!(
            committee(&body),
            vec![member.to_string()],
            "index off: {body}"
        );

        sqlx::query("UPDATE shop.chain_cursor SET folded_at = timezone('UTC', now()) - interval '2 hours' WHERE indexer = 'committee'")
            .execute(&pool)
            .await
            .unwrap();
        let (status, body) = get(&app, "/v1/collections/curation", Some(OWNER_KEY)).await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "stale index: {body}"
        );
        assert!(unpublished(&body), "{body}");
        sqlx::query("UPDATE shop.chain_cursor SET folded_at = timezone('UTC', now()) WHERE indexer = 'committee'")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query("ALTER TABLE shop.committee_event RENAME TO committee_event_gone")
            .execute(&pool)
            .await
            .unwrap();
        let (status, body) = get(&app, "/v1/collections/curation", Some(OWNER_KEY)).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        pool.close().await;
    }

    #[derive(Default)]
    struct FakePolygon {
        chain_id: &'static str,
        creator: Option<String>,
        calls: usize,
    }

    /// A published collection follows its on-chain creator: after
    /// `transferCreatorship` the new creator reads it and its items and the
    /// previous one is refused. With writes on the rows move; off, only the
    /// answers carry the creator. Another chain, a zero creator or a failed
    /// read keep the stored owner, and one read per contract is reused.
    async fn owner_follows_creator(writes_enabled: bool) {
        use axum::extract::State as AxState;
        use axum::routing::post;
        use std::sync::Mutex;

        let Some(pool) = crate::db::scratch::pool(4).await else {
            eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
            return;
        };
        let recorder = dcl_metrics::PrometheusBuilder::new().build_recorder();
        let _recording = dcl_metrics::set_default_local_recorder(&recorder);
        let (owner, next) = (address_of(OWNER_KEY), address_of(STRANGER_KEY));
        let contract = format!("0x{}00000000", Uuid::new_v4().simple());
        let polygon = Arc::new(Mutex::new(FakePolygon {
            chain_id: "0x1",
            creator: Some(next.clone()),
            ..Default::default()
        }));
        async fn rpc(
            AxState(polygon): AxState<Arc<Mutex<FakePolygon>>>,
            Json(req): Json<serde_json::Value>,
        ) -> Json<serde_json::Value> {
            let mut polygon = polygon.lock().unwrap();
            polygon.calls += 1;
            if req["method"] == "eth_chainId" {
                return Json(json!({"jsonrpc":"2.0", "id":req["id"], "result": polygon.chain_id}));
            }
            assert_eq!(req["method"], "eth_call");
            assert_eq!(req["params"][1], "latest");
            Json(match polygon.creator.clone() {
                Some(c) => {
                    json!({"jsonrpc":"2.0", "id":req["id"], "result": format!("0x{:0>64}", &c[2..])})
                }
                None => {
                    json!({"jsonrpc":"2.0", "id":req["id"], "error": {"code": -32000, "message": "down"}})
                }
            })
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let rpc_app = Router::new()
            .route("/", post(rpc))
            .with_state(polygon.clone());
        tokio::spawn(async move { axum::serve(listener, rpc_app).await.unwrap() });
        let state = Arc::new(
            crate::State::new(
                pool,
                temp_storage().await,
                Arc::default(),
                dcl_http::Outbound::for_tests(),
                None,
                None,
                crate::Settings {
                    polygon_rpc_url: Some(url),
                    writes_enabled,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
        let set_creator = |creator: Option<&str>| {
            polygon.lock().unwrap().creator = creator.map(str::to_owned);
            state.creator_memo.clear();
        };
        let app = crate::router(state.clone());
        let (collection, item) = (Uuid::new_v4(), Uuid::new_v4());
        let store = &state.items;
        sqlx::query("INSERT INTO builder.collections (id, name, eth_address, contract_address, is_published) VALUES ($1, 'Moved', $2, $3, true)")
            .bind(collection).bind(&owner).bind(&contract).execute(store.pool()).await.unwrap();
        sqlx::query("INSERT INTO builder.items (id, collection_id, eth_address, name, type) VALUES ($1, $2, $3, 'Hat', 'wearable')")
            .bind(item).bind(collection).bind(&owner).execute(store.pool()).await.unwrap();
        let path = format!("/v1/collections/{collection}");
        let items_path = format!("/v1/collections/{collection}/items");
        let item_path = format!("/v1/items/{item}");
        let item_owner = || async {
            sqlx::query_scalar::<_, String>("SELECT eth_address FROM builder.items WHERE id = $1")
                .bind(item)
                .fetch_one(store.pool())
                .await
                .unwrap()
        };

        // Not Polygon Mainnet: the stored owner answers, counted once.
        assert_eq!(get(&app, &path, Some(OWNER_KEY)).await.0, StatusCode::OK);
        assert_eq!(
            get(&app, &path, Some(STRANGER_KEY)).await.0,
            StatusCode::UNAUTHORIZED
        );
        let rendered = recorder.handle().render();
        assert!(
            rendered
                .lines()
                .any(|l| l == format!("{CREATOR_READ_FAILURES} 1")),
            "{rendered}"
        );
        polygon.lock().unwrap().chain_id = "0x89";

        set_creator(Some("0x0000000000000000000000000000000000000000"));
        assert_eq!(
            get(&app, &path, Some(OWNER_KEY)).await.0,
            StatusCode::OK,
            "a zero creator keeps the stored owner"
        );
        let calls = polygon.lock().unwrap().calls;
        polygon.lock().unwrap().creator = Some(next.clone());
        assert_eq!(
            get(&app, &path, Some(STRANGER_KEY)).await.0,
            StatusCode::UNAUTHORIZED,
            "the read is reused for its TTL"
        );
        assert_eq!(polygon.lock().unwrap().calls, calls);

        set_creator(Some(&next));
        let (status, body) = get(&app, &path, Some(STRANGER_KEY)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["data"]["eth_address"], next.as_str());
        let (status, body) = get(&app, &items_path, Some(STRANGER_KEY)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["data"][0]["eth_address"], next.as_str());
        let (status, body) = get(&app, &item_path, Some(STRANGER_KEY)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["data"]["eth_address"], next.as_str());
        assert_eq!(
            item_owner().await,
            if writes_enabled { &next } else { &owner }.as_str()
        );
        assert_eq!(
            get(&app, &item_path, Some(OWNER_KEY)).await.0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            get(&app, &path, Some(OWNER_KEY)).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            get(&app, &items_path, Some(OWNER_KEY)).await.0,
            StatusCode::UNAUTHORIZED
        );

        set_creator(Some(&owner));
        assert_eq!(
            get(&app, &item_path, Some(OWNER_KEY)).await.0,
            StatusCode::OK,
            "an item read follows the creator too"
        );
        assert_eq!(item_owner().await, owner);
        assert_eq!(
            get(&app, &item_path, Some(STRANGER_KEY)).await.0,
            StatusCode::NOT_FOUND
        );

        set_creator(Some(&next));
        assert_eq!(
            get(&app, &item_path, Some(STRANGER_KEY)).await.0,
            StatusCode::OK
        );
        set_creator(None);
        let (stored, previous) = if writes_enabled {
            (STRANGER_KEY, OWNER_KEY)
        } else {
            (OWNER_KEY, STRANGER_KEY)
        };
        assert_eq!(
            get(&app, &path, Some(stored)).await.0,
            StatusCode::OK,
            "the stored owner answers"
        );
        assert_eq!(
            get(&app, &path, Some(previous)).await.0,
            StatusCode::UNAUTHORIZED
        );
        cleanup(store, &[collection], &[]).await;
    }

    #[tokio::test]
    async fn published_collection_owner_follows_the_onchain_creator() {
        owner_follows_creator(true).await;
    }

    #[tokio::test]
    async fn published_collection_owner_follows_the_onchain_creator_with_writes_off() {
        owner_follows_creator(false).await;
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
