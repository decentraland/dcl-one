//! The collection and item reads: a collection's body and items, the
//! committee's listings, an address's listings, the public search of
//! published collections and the curation committee.

use axum::Json;
use axum::extract::{OriginalUri, Path, RawQuery, State};
use axum::http::{HeaderMap, Uri};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::SharedState;
use crate::access;
use crate::auth::{Signed, signed_path, try_extract_signer};
use crate::collections::listing::{CommitteeListing, ListQuery, OwnListing, PublicSearch};
use crate::committee::Member;
use crate::http::{ApiData, ApiError};
use crate::items::listing::ItemsOut;
use crate::kept::Since;
use crate::shared::signed_fetch::AUTH_CHAIN_HEADER_PREFIX;

/// `GET /v1/collections/:id`, for whoever `access::read_collection` admits.
pub async fn get_collection(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
) -> Result<Json<ApiData<Value>>, ApiError> {
    let read = access::read_collection(&state, &signer, &id).await?;
    Ok(Json(ApiData::ok(state.items.collection(&read).await?)))
}

/// `GET /v1/collections/:idOrContractAddress/items` (`getCollectionItems`):
/// every item for a role holder, a minter or the committee, the published
/// ones for any other signer of a published collection
/// (`access::collection_items`, which checks `status` once the collection
/// is found); paginated only when `page` and `limit` are both truthy.
pub async fn get_collection_items(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> Result<Json<ApiData<ItemsOut>>, ApiError> {
    let q = ListQuery::parse(raw.as_deref());
    let status = q.non_empty("status")?.map(str::to_string);
    let read = access::collection_items(&state, &signer, &id, status.as_deref()).await?;
    let listing = crate::limits::Listing {
        state: &state,
        headers: &headers,
        signer: signer.as_str(),
    };
    Ok(Json(ApiData::ok(
        state
            .items
            .collection_items(&read, &q, Some(&listing))
            .await?,
    )))
}

/// Whether a listing's `status` filter and MOST_RELEVANT sort read the
/// review flow as on: only a listing that filters by status or sorts by
/// relevance asks.
pub(crate) async fn review_flow_asked(
    state: &SharedState,
    q: &ListQuery,
) -> Result<bool, ApiError> {
    if q.all("status").is_empty() && q.all("sort") != ["MOST_RELEVANT"] {
        return Ok(false);
    }
    let mut conn = state.items.pool().acquire().await?;
    crate::review::enabled(&mut conn, state).await
}

/// builder-server's `HTTPError('Unauthorized', { eth_address }, 401)` for
/// an anonymous listing request; `eth_address` is `""`.
fn unauthorized_for(eth_address: &str) -> ApiError {
    ApiError::unauthorized("Unauthorized").with_data(json!({ "eth_address": eth_address }))
}

/// `GET /v1/collections` (`getCollections`): a `q` with `is_published=true`
/// is the public search, open to anyone and answered in full to the
/// committee (from the read at hand, without waiting); any other request is
/// the committee's listing, and anyone else, signed or not, is refused with
/// the signer (`""` anonymously). The filters are checked after the caller.
pub async fn get_collections(
    State(state): State<SharedState>,
    RawQuery(raw): RawQuery,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    let signed = headers.contains_key(format!("{AUTH_CHAIN_HEADER_PREFIX}0"));
    let answer = list_collections(&state, raw.as_deref(), &original, &uri, &headers)
        .await
        .into_response();
    if signed {
        crate::http::private_response(answer).await
    } else {
        answer
    }
}

async fn list_collections(
    state: &SharedState,
    raw: Option<&str>,
    original: &Uri,
    uri: &Uri,
    headers: &HeaderMap,
) -> Result<Response, ApiError> {
    let q = ListQuery::parse(raw);
    let signer = try_extract_signer(
        headers,
        "get",
        &signed_path(original, uri),
        state.eip1654.as_deref(),
    )
    .await;
    if let Some(signer) = &signer {
        crate::limits::spend_wallet(state, signer.as_str(), &axum::http::Method::GET)?;
    }
    let review_flow = review_flow_asked(state, &q).await?;
    let committee = match (&signer, q.is_public_search()) {
        (Some(signer), true) => access::known_committee_member(state, signer).await,
        (Some(signer), false) => Some(access::committee(state, signer).await?),
        (None, true) => None,
        (None, false) => return Err(unauthorized_for("")),
    };
    let data = match committee {
        Some(member) => {
            state
                .items
                .committee_collections(&CommitteeListing::new(&member, &q, review_flow)?, state)
                .await?
        }
        None => {
            let search = PublicSearch::new(&q, review_flow)?.ok_or_else(|| unauthorized_for(""))?;
            let found = state.items.public_collections(&search, state).await?;
            crate::limits::charge(
                state,
                headers,
                signer.as_ref().map(|s| s.as_str()),
                crate::limits::ADDRESSES_TOKENS,
            );
            found
        }
    };
    Ok(Json(ApiData::ok(data)).into_response())
}

/// `GET /v1/items` (`getItems`): the committee lists every item; a signed
/// wallet that is not on it is refused with its address.
pub async fn get_items(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let committee = access::committee(&state, &signer).await?;
    let listing = crate::limits::Listing {
        state: &state,
        headers: &headers,
        signer: signer.as_str(),
    };
    let items = state.items.every_item(&committee, &listing).await?;
    Ok(Json(ApiData::ok(items)).into_response())
}

/// `GET /v1/{address}/collections` (`getAddressCollections`): the wallet
/// itself, signed; anyone else is refused with the path's address. Its
/// collections from the builder's rows and the chain copy
/// (`collections::listing`).
pub async fn get_address_collections(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(address): Path<String>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> Result<Json<ApiData<Value>>, ApiError> {
    let wallet = access::own_address(&state, &signer, &address).await?;
    let listing = OwnListing::new(&wallet, &ListQuery::parse(raw.as_deref()))?;
    let collections = state.items.address_collections(&listing).await?;
    let rows = collections
        .get("results")
        .unwrap_or(&collections)
        .as_array()
        .map_or(0, Vec::len);
    crate::limits::charge_rows(&state, &headers, Some(signer.as_str()), rows);
    Ok(Json(ApiData::ok(collections)))
}

/// `GET /v1/{address}/items` (`getAddressItems`): the wallet itself,
/// signed; anyone else is refused with the path's address. Its items from
/// the builder's rows, merged with the chain copy (`items::listing`).
pub async fn get_address_items(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(address): Path<String>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> Result<Json<ApiData<ItemsOut>>, ApiError> {
    let wallet = access::own_address(&state, &signer, &address).await?;
    let listing = crate::limits::Listing {
        state: &state,
        headers: &headers,
        signer: signer.as_str(),
    };
    let items = state
        .items
        .address_items(&wallet, &ListQuery::parse(raw.as_deref()), &listing)
        .await?;
    Ok(Json(ApiData::ok(items)))
}

/// `GET /v1/committee`: the current committee members, unauthenticated;
/// 503 while the committee can't tell.
pub async fn get_committee(
    State(state): State<SharedState>,
) -> Result<Json<ApiData<Vec<Member>>>, ApiError> {
    let members = state.committee.members(&state, Since::read()).await?;
    Ok(Json(ApiData::ok(members.to_vec())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use uuid::Uuid;

    use axum::Router;
    use axum::body::Body;
    use axum::http::StatusCode;
    use serde_json::Value;
    use tower::ServiceExt;

    use crate::tests::support::{
        FakeCommittee, OWNER_KEY, address, cleanup, committee_index, request,
    };

    const STRANGER_KEY: &str = "0x2222222222222222222222222222222222222222222222222222222222222222";
    const COMMITTEE_KEY: &str =
        "0x3333333333333333333333333333333333333333333333333333333333333333";

    async fn state(committee: &str) -> Option<SharedState> {
        let pool = crate::tests::support::scratch_pool(4).await?;
        let fake = FakeCommittee::with(&[committee]);
        let market = committee_index(&fake).await;
        Some(Arc::new(
            crate::tests::support::state(pool, Some(market), crate::Settings::default()).await,
        ))
    }

    async fn get(app: &Router, path: &str, key: Option<&str>) -> (StatusCode, Value) {
        let request = request(key, "GET", path).await;
        let response = app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let cache = response.headers().get(axum::http::header::CACHE_CONTROL);
        if path.starts_with("/v1/drafts/") {
            assert_eq!(cache.unwrap(), "private, no-store");
        }
        if path.starts_with("/v1/collections?") && key.is_none() {
            assert_eq!(cache, None, "an unsigned search stays cacheable");
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

    fn names(body: &Value) -> Vec<String> {
        body["data"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|c| c["name"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The four listings, builder-server's rule: a wallet that owns nothing
    /// and is not on the committee gets the 403 envelope naming the
    /// wallet the request was about, the owner its own set, the committee
    /// every wallet's drafts but no other wallet's address listing, the
    /// anonymous published search its matches.
    #[tokio::test]
    async fn listings_are_visible_to_the_owner_and_the_committee_only() {
        let (owner, stranger, committee) = (
            address(OWNER_KEY),
            address(STRANGER_KEY),
            address(COMMITTEE_KEY),
        );
        let Some(state) = state(&committee).await else {
            eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
            return;
        };
        let app = crate::router(state.clone());
        let tag = Uuid::new_v4().simple().to_string();
        let (draft, published) = (Uuid::new_v4(), Uuid::new_v4());
        let saved = crate::tests::support::save_collection(
            &state.items,
            &owner,
            json!({ "id": draft, "name": format!("draft {tag}") }),
        )
        .await
        .unwrap();
        assert_eq!(saved["eth_address"], owner.as_str());
        crate::tests::support::save_collection(
            &state.items,
            &owner,
            json!({ "id": published, "name": format!("SheFi {tag}%_") }),
        )
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO builder.items (id, collection_id, eth_address, name, type, blockchain_item_id) \
             VALUES ($1, $2, $3, 'Minted', 'wearable', '0')",
        )
        .bind(Uuid::new_v4())
        .bind(published)
        .bind(&owner)
        .execute(state.items.pool())
        .await
        .unwrap();
        let item = Uuid::new_v4();
        crate::tests::support::save_item(
            &state.items,
            &owner,
            &json!({
                "id": item, "name": "My wearable", "collection_id": draft,
                "type": "wearable", "data": { "category": "upper_body" }, "rarity": "common", "price": "0",
            }),
        )
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
            let (status, body) = get(&app, &path, None).await;
            assert_eq!(
                (status, body),
                (
                    StatusCode::UNAUTHORIZED,
                    json!({ "ok": false, "data": { "message": "Invalid auth chain" }, "error": "Unauthenticated" })
                ),
                "missing signed-fetch headers are upstream's Unauthenticated"
            );
        }

        let (status, body) = get(&app, "/v1/items", Some(STRANGER_KEY)).await;
        assert_eq!(
            (status, body),
            (StatusCode::FORBIDDEN, refusal_of(&stranger))
        );
        let (status, body) = get(&app, "/v1/items", Some(OWNER_KEY)).await;
        assert_eq!(
            (status, body),
            (StatusCode::FORBIDDEN, refusal_of(&owner)),
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

        let (status, body) = get(&app, "/v1/collections", None).await;
        assert_eq!((status, body), (StatusCode::UNAUTHORIZED, refusal_of("")));
        let (status, body) = get(&app, "/v1/collections", Some(STRANGER_KEY)).await;
        assert_eq!(
            (status, body),
            (StatusCode::FORBIDDEN, refusal_of(&stranger))
        );
        let (status, body) = get(&app, "/v1/collections", Some(COMMITTEE_KEY)).await;
        assert_eq!(status, StatusCode::OK);
        let listed = ids(&body);
        assert!(
            listed.contains(&draft.to_string()) && listed.contains(&published.to_string()),
            "the committee lists every collection: {body}"
        );
        let search = format!("/v1/collections?q={tag}&is_published=true");
        let (status, body) = get(&app, &search, None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let found = names(&body);
        assert!(
            found.contains(&format!("SheFi {tag}%_")) && !found.contains(&format!("draft {tag}")),
            "{body}"
        );
        assert!(
            body["data"][0].get("id").is_none(),
            "the public search answers no id: {body}"
        );
        let (status, body) = get(
            &app,
            &format!("/v1/collections?q={tag}%25_&is_published=true"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            names(&body).contains(&format!("SheFi {tag}%_")),
            "the pattern characters of q are literal: {body}"
        );
        let (status, body) = get(
            &app,
            &format!("/v1/collections?q=nobody-{tag}&is_published=true"),
            None,
        )
        .await;
        assert_eq!((status, names(&body)), (StatusCode::OK, vec![]));
        let (status, body) = get(&app, &format!("/v1/collections?q={tag}"), None).await;
        assert_eq!(
            (status, body),
            (StatusCode::UNAUTHORIZED, refusal_of("")),
            "q without is_published=true is no search"
        );

        for route in ["collections", "items"] {
            let path = format!("/v1/{owner}/{route}");
            let (status, body) = get(&app, &path, None).await;
            assert_ne!(status, StatusCode::OK, "unsigned {route}: {body}");
            assert_eq!(body["ok"], false);
            for key in [STRANGER_KEY, COMMITTEE_KEY] {
                let (status, body) = get(&app, &path, Some(key)).await;
                assert_eq!(
                    (status, body),
                    (StatusCode::FORBIDDEN, refusal_of(&owner)),
                    "{route}: the refusal names the path's wallet; the committee is no exception"
                );
            }
            let (status, body) = get(&app, &path, Some(OWNER_KEY)).await;
            assert_eq!(status, StatusCode::OK, "{route}: {body}");
            let (status, _) = get(
                &app,
                &format!("/v1/{}/{route}", owner.to_uppercase().replace("0X", "0x")),
                Some(OWNER_KEY),
            )
            .await;
            assert_eq!(
                status,
                StatusCode::OK,
                "{route}: the path's case does not matter"
            );
        }

        cleanup(&state.items, &[draft, published], &[]).await;
    }

    /// The query parameters builder-server's `getCollections` reads
    /// (`getPaginationParams`, `getReviewFilters`, `sort`, `q`,
    /// `is_published`, `tag`, `type`).
    const COLLECTIONS_PARAMS: [&str; 10] = [
        "page",
        "limit",
        "sort",
        "q",
        "is_published",
        "tag",
        "type",
        "status",
        "review_type",
        "assignee",
    ];
    /// The query parameters builder-server's `getAddressCollections` reads
    /// (`address` through `extractFromReq`, which reads the query first).
    const ADDRESS_PARAMS: [&str; 9] = [
        "address",
        "page",
        "limit",
        "is_published",
        "sort",
        "q",
        "type",
        "status",
        "contract_address",
    ];

    fn names_or_body(body: &Value) -> Value {
        match &body["data"] {
            Value::Array(rows) => rows.iter().map(|r| r["name"].clone()).collect(),
            other => other.clone(),
        }
    }

    /// Every parameter builder-server's listing handlers read changes what
    /// the node answers: none is accepted and silently ignored.
    #[tokio::test]
    async fn every_listing_parameter_upstream_reads_is_applied_or_deliberately_ignored() {
        let (owner, committee) = (address(OWNER_KEY), address(COMMITTEE_KEY));
        let Some(state) = state(&committee).await else {
            return;
        };
        let app = crate::router(state.clone());
        let tag = Uuid::new_v4().simple().to_string()[..10].to_string();
        let ids = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        let mut sorted = ids;
        sorted.sort();
        let [draft, linked, published] = sorted;
        let save = |id: Uuid, name: String, urn: Option<String>| {
            let items = &state.items;
            let owner = owner.clone();
            async move {
                crate::tests::support::save_collection(
                    items,
                    &owner,
                    json!({ "id": id, "name": name, "urn": urn }),
                )
                .await
                .unwrap()
            }
        };
        save(draft, format!("{tag} a draft"), None).await;
        save(
            linked,
            format!("{tag} b linked"),
            Some(format!(
                "urn:decentraland:matic:collections-thirdparty:p{tag}:c"
            )),
        )
        .await;
        let saved = save(published, format!("{tag} c published"), None).await;
        sqlx::query(
            "INSERT INTO builder.items (id, collection_id, eth_address, name, type, blockchain_item_id, data) \
             VALUES ($1, $2, $3, 'Minted', 'wearable', '0', jsonb_build_object('tags', jsonb_build_array($4::text)))",
        )
        .bind(Uuid::new_v4())
        .bind(published)
        .bind(&owner)
        .bind(&tag)
        .execute(state.items.pool())
        .await
        .unwrap();
        let contract = saved["contract_address"].as_str().unwrap().to_string();
        let routes = [
            (
                "/v1/collections".to_string(),
                COMMITTEE_KEY,
                COLLECTIONS_PARAMS.as_slice(),
                vec![
                    ("page", "page=1&limit=1".to_string()),
                    ("limit", "limit=1".to_string()),
                    ("sort", "sort=NAME_DESC".to_string()),
                    ("q", format!("q={tag}%20a")),
                    ("is_published", "is_published=true".to_string()),
                    ("tag", format!("tag={tag}")),
                    ("type", "type=third_party".to_string()),
                    ("status", "status=to_review".to_string()),
                    ("review_type", "review_type=updates".to_string()),
                    ("assignee", format!("assignee={committee}")),
                ],
            ),
            (
                format!("/v1/{owner}/collections"),
                OWNER_KEY,
                ADDRESS_PARAMS.as_slice(),
                vec![
                    ("page", "page=1&limit=1".to_string()),
                    ("limit", "limit=1".to_string()),
                    ("is_published", "is_published=true".to_string()),
                    ("sort", "sort=NAME_DESC".to_string()),
                    ("q", format!("q={tag}%20a")),
                    ("type", "type=third_party".to_string()),
                    ("status", "status=draft".to_string()),
                    ("contract_address", format!("contract_address={contract}")),
                    ("address", format!("address={committee}")),
                ],
            ),
        ];
        for (path, key, params, cases) in routes {
            let mut covered: Vec<&str> = cases.iter().map(|(p, _)| *p).collect();
            covered.sort();
            let mut expected = params.to_vec();
            expected.sort();
            assert_eq!(
                covered, expected,
                "{path}: every parameter upstream reads has a case"
            );
            let base_query = format!("q={tag}&sort=CREATED_AT_ASC");
            let (status, base) = get(&app, &format!("{path}?{base_query}"), Some(key)).await;
            assert_eq!(status, StatusCode::OK, "{path}: {base}");
            for (param, query) in cases {
                let query = if query.starts_with("q=") || query.starts_with("sort=") {
                    query
                } else {
                    format!("{base_query}&{query}")
                };
                let (status, body) = get(&app, &format!("{path}?{query}"), Some(key)).await;
                assert_eq!(status, StatusCode::OK, "{path}?{query}: {body}");
                if param == "address" {
                    assert_eq!(
                        names_or_body(&body),
                        names_or_body(&base),
                        "{path}: the path names the address, never the query (deliberate gap query-never-overrides)"
                    );
                    continue;
                }
                assert_ne!(
                    names_or_body(&body),
                    names_or_body(&base),
                    "{path}: {param} ({query}) changes nothing"
                );
            }
        }
        cleanup(&state.items, &ids, &[]).await;
    }
}
