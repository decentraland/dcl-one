//! Callers of the builder routes, each with a budget of requests, through
//! the router at both mounts as `serve` mounts them and with the node's
//! clock driven by the test: a source address, a signer, a review caller
//! and a listing reader over their budget get 429 with `Retry-After`, before
//! the body is validated and before the database is asked; another caller is
//! unaffected; the same caller is served again once the window refills.

use std::sync::Arc;
use std::time::Duration as Window;

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use chrono::Duration;
use tower::ServiceExt;
use uuid::Uuid;

use super::support::{OWNER_KEY, lazy_pool, signed_map, temp_storage};
use super::validator::{advance, clock, set_clock};

const OTHER_KEY: &str = "0x5555555555555555555555555555555555555555555555555555555555555555";
const SOURCE: &str = "203.0.113.9";
const OTHER_SOURCE: &str = "198.51.100.4";
const NOT_A_CID: &str = "not-a-cid";
const TOO_MANY: &str =
    r#"{"data":{},"error":"Too many requests, try again in a minute","ok":false}"#;

pub(super) async fn app_over(pool: sqlx::PgPool, tune: impl FnOnce(&mut crate::State)) -> Router {
    node_over(pool, tune).await.0
}

pub(super) async fn node_over(
    pool: sqlx::PgPool,
    tune: impl FnOnce(&mut crate::State),
) -> (Router, crate::SharedState) {
    set_clock(
        chrono::DateTime::from_timestamp_millis(chrono::Utc::now().timestamp_millis()).unwrap(),
    );
    let mut state = crate::State::new(
        pool,
        temp_storage().await,
        Arc::default(),
        dcl_http::Outbound::for_tests(),
        None,
        None,
        crate::Settings {
            writes_enabled: true,
            ..crate::Settings::default()
        },
    )
    .unwrap();
    state.now = clock;
    tune(&mut state);
    let state = Arc::new(state);
    let app = Router::new()
        .merge(crate::root_router(state.clone()))
        .nest("/builder-api", crate::router(state.clone()));
    (app, state)
}

pub(super) struct Call<'a> {
    pub(super) method: &'a str,
    pub(super) path: String,
    pub(super) key: Option<&'a str>,
    pub(super) source: Option<&'a str>,
    pub(super) body: Vec<u8>,
}

impl Call<'_> {
    pub(super) fn get(path: impl Into<String>) -> Self {
        Self {
            method: "GET",
            path: path.into(),
            key: None,
            source: None,
            body: Vec::new(),
        }
    }
}

pub(super) async fn send(app: &Router, call: &Call<'_>) -> (u16, String, Option<String>) {
    let mut request = Request::builder().method(call.method).uri(&call.path);
    if let Some(key) = call.key {
        for (name, value) in signed_map(
            key,
            &call.method.to_lowercase(),
            call.path.split('?').next().unwrap_or_default(),
            "{}",
            false,
        )
        .await
        {
            request = request.header(name, value);
        }
    }
    if let Some(source) = call.source {
        request = request.header("x-real-ip", source);
    }
    if !call.body.is_empty() {
        request = request.header("content-type", "application/json");
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(call.body.clone())).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let retry = response
        .headers()
        .get("retry-after")
        .map(|v| v.to_str().unwrap().to_string());
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned(), retry)
}

fn per_minute(n: u32) -> crate::shared::limits::RateLimiter {
    crate::limits::budget(n, Window::from_secs(60))
}

fn save<'a>(key: &'a str, mount: &str) -> Call<'a> {
    Call {
        method: "PUT",
        path: format!("{mount}/v1/items/{}", Uuid::new_v4()),
        key: Some(key),
        source: None,
        body: b"not json".to_vec(),
    }
}

fn file_read<'a>(key: Option<&'a str>, source: Option<&'a str>, mount: &str) -> Call<'a> {
    Call {
        method: "GET",
        path: format!("{mount}/v1/storage/contents/{NOT_A_CID}"),
        key,
        source,
        body: Vec::new(),
    }
}

#[tokio::test]
async fn a_signer_over_its_write_budget_waits_before_its_body_is_read() {
    let app = app_over(lazy_pool(), |state| {
        state.limits.wallet_writes = per_minute(3)
    })
    .await;
    for mount in ["", "/builder-api", ""] {
        assert_eq!(send(&app, &save(OWNER_KEY, mount)).await.0, 400, "{mount}");
    }
    let (status, body, retry) = send(&app, &save(OWNER_KEY, "/builder-api")).await;
    assert_eq!(
        (status, body.as_str(), retry.as_deref()),
        (429, TOO_MANY, Some("20"))
    );
    assert_eq!(
        send(&app, &save(OTHER_KEY, "")).await.0,
        400,
        "another signer keeps its budget"
    );
    assert_eq!(
        send(&app, &file_read(Some(OWNER_KEY), None, "")).await.0,
        404,
        "reads keep their own budget"
    );
    advance(Duration::seconds(19));
    assert_eq!(send(&app, &save(OWNER_KEY, "")).await.0, 429);
    advance(Duration::seconds(1));
    assert_eq!(send(&app, &save(OWNER_KEY, "")).await.0, 400);
    assert_eq!(send(&app, &save(OWNER_KEY, "")).await.0, 429);
}

#[tokio::test]
async fn a_signer_over_its_read_budget_waits_and_a_failed_signature_spends_no_signers_budget() {
    let app = app_over(lazy_pool(), |state| {
        state.limits.wallet_reads = per_minute(2)
    })
    .await;
    for mount in ["", "/builder-api"] {
        assert_eq!(
            send(&app, &file_read(Some(OWNER_KEY), None, mount)).await.0,
            404
        );
    }
    let (status, body, retry) = send(&app, &file_read(Some(OWNER_KEY), None, "")).await;
    assert_eq!(
        (status, body.as_str(), retry.as_deref()),
        (429, TOO_MANY, Some("30"))
    );
    assert_eq!(
        send(&app, &file_read(None, None, "")).await.0,
        404,
        "unsigned reads spend no signer's budget"
    );
    assert_eq!(
        send(&app, &file_read(Some(OTHER_KEY), None, "")).await.0,
        404
    );

    let mut forged = Request::builder().uri(format!("/v1/storage/contents/{NOT_A_CID}"));
    for (name, value) in signed_map(OTHER_KEY, "get", "/v1/somewhere-else", "{}", false).await {
        forged = forged.header(name, value);
    }
    for _ in 0..3 {
        let response = app.clone().oneshot(forged_request(&forged)).await.unwrap();
        assert_eq!(response.status().as_u16(), 401);
    }
    assert_eq!(
        send(&app, &file_read(Some(OTHER_KEY), None, "")).await.0,
        404,
        "requests that claim a signer without its signature spend nothing of its budget"
    );
}

fn forged_request(template: &axum::http::request::Builder) -> Request<Body> {
    let mut request = Request::builder().uri(template.uri_ref().unwrap().clone());
    for (name, value) in template.headers_ref().unwrap() {
        request = request.header(name, value);
    }
    request.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn a_source_over_its_budget_waits_whoever_signs_and_an_internal_source_has_none() {
    let app = app_over(lazy_pool(), |state| {
        state.limits.source_reads = per_minute(4);
        state.limits.source_writes = per_minute(1);
    })
    .await;
    for (key, mount) in [
        (None, ""),
        (Some(OWNER_KEY), "/builder-api"),
        (Some(OTHER_KEY), ""),
        (None, ""),
    ] {
        assert_eq!(
            send(&app, &file_read(key, Some(SOURCE), mount)).await.0,
            404
        );
    }
    let (status, body, retry) = send(&app, &file_read(Some(OTHER_KEY), Some(SOURCE), "")).await;
    assert_eq!(
        (status, body.as_str(), retry.as_deref()),
        (429, TOO_MANY, Some("15"))
    );
    let mut write = save(OWNER_KEY, "");
    write.source = Some(SOURCE);
    assert_eq!(
        send(&app, &write).await.0,
        400,
        "writes keep their own budget"
    );
    assert_eq!(send(&app, &write).await.0, 429);
    assert_eq!(
        send(&app, &file_read(None, Some(OTHER_SOURCE), "")).await.0,
        404
    );
    let ipv6 = |n: u8| format!("2001:db8:1:2::{n}");
    for n in 0..4 {
        assert_eq!(
            send(&app, &file_read(None, Some(&ipv6(n)), "")).await.0,
            404
        );
    }
    assert_eq!(
        send(&app, &file_read(None, Some(&ipv6(9)), "")).await.0,
        429,
        "one IPv6 /64 is one source"
    );
    for internal in ["10.0.0.7", "127.0.0.1", "fd00::1"] {
        for _ in 0..6 {
            assert_eq!(
                send(&app, &file_read(None, Some(internal), "")).await.0,
                404,
                "{internal}"
            );
        }
    }
    for _ in 0..6 {
        assert_eq!(send(&app, &file_read(None, None, "")).await.0, 404);
    }
    advance(Duration::seconds(15));
    assert_eq!(send(&app, &file_read(None, Some(SOURCE), "")).await.0, 404);
}

#[tokio::test]
async fn the_production_budgets_hold_at_their_bounds() {
    let app = app_over(lazy_pool(), |_| ()).await;
    let signed = save(OWNER_KEY, "");
    let mut request = Request::builder().method("PUT").uri(&signed.path);
    for (name, value) in signed_map(OWNER_KEY, "put", &signed.path, "{}", false).await {
        request = request.header(name, value);
    }
    let template = request;
    let mut statuses = Vec::new();
    for _ in 0..=crate::limits::WALLET_WRITES {
        let mut request = Request::builder()
            .method("PUT")
            .uri(template.uri_ref().unwrap().clone());
        for (name, value) in template.headers_ref().unwrap() {
            request = request.header(name, value);
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::from("x")).unwrap())
            .await
            .unwrap();
        statuses.push(response.status().as_u16());
    }
    assert!(statuses[..statuses.len() - 1].iter().all(|s| *s == 400));
    assert_eq!(statuses.last(), Some(&429));

    let mut statuses = Vec::new();
    for _ in 0..=crate::limits::SOURCE_READS {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/storage/contents/{NOT_A_CID}"))
                    .header("x-real-ip", OTHER_SOURCE)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        statuses.push(response.status().as_u16());
    }
    assert!(statuses[..statuses.len() - 1].iter().all(|s| *s == 404));
    assert_eq!(statuses.last(), Some(&429));
}

#[tokio::test]
async fn an_item_save_larger_than_the_body_cap_is_refused_before_its_keys_are_read() {
    let app = app_over(lazy_pool(), |_| ()).await;
    let large =
        serde_json::json!({ "item": { "id": Uuid::new_v4(), "unknown": "x".repeat(3 << 20) } });
    let mut call = save(OWNER_KEY, "");
    call.body = large.to_string().into_bytes();
    assert_eq!(send(&app, &call).await.0, 413);
}

#[tokio::test]
async fn each_route_takes_the_body_its_clients_send_and_refuses_more_unread() {
    let app = app_over(lazy_pool(), |_| ()).await;
    let padded = |len: usize| {
        let mut body = br#"{"item":{"id":""#.to_vec();
        body.resize(len - 3, b'x');
        body.extend_from_slice(br#""}}"#);
        body
    };
    for mount in ["", "/builder-api"] {
        let id = Uuid::new_v4();
        for (method, path, bound) in [
            (
                "PUT",
                format!("{mount}/v1/items/{id}"),
                crate::ITEM_BODY_LIMIT,
            ),
            (
                "PATCH",
                format!("{mount}/v1/items/{id}/curation"),
                crate::BODY_LIMIT,
            ),
            (
                "POST",
                format!("{mount}/v1/collections/{id}/comments"),
                crate::BODY_LIMIT,
            ),
        ] {
            for (len, refused) in [(bound, false), (bound + 1, true)] {
                let call = Call {
                    method,
                    path: path.clone(),
                    key: Some(OWNER_KEY),
                    source: None,
                    body: padded(len),
                };
                let status = send(&app, &call).await.0;
                assert_eq!(status == 413, refused, "{method} {path} {len}: {status}");
            }
        }
        for (method, path) in [
            ("PUT", format!("{mount}/v1/collections/{id}")),
            ("POST", format!("{mount}/v1/collections/{id}/tos")),
            ("POST", format!("{mount}/v1/collections/{id}/publish")),
        ] {
            let call = Call {
                method,
                path: path.clone(),
                key: Some(OWNER_KEY),
                source: None,
                body: padded(crate::LARGE_BODY_LIMIT),
            };
            assert_ne!(send(&app, &call).await.0, 413, "{method} {path}");
        }
    }
}

#[tokio::test]
async fn a_review_caller_over_its_minute_waits_even_with_the_flow_off() {
    let Some(pool) = super::validator::own_pool().await else {
        return;
    };
    let app = app_over(pool, |_| ()).await;
    let id = Uuid::new_v4();
    let write = |key: &'static str, n: usize| Call {
        method: "POST",
        path: format!(
            "/v1/collections/{id}/{}",
            ["validations", "curation/appeal", "comments"][n % 3]
        ),
        key: Some(key),
        source: None,
        body: Vec::new(),
    };
    for n in 0..crate::limits::REVIEW_WRITES as usize {
        assert_eq!(send(&app, &write(OWNER_KEY, n)).await.0, 404, "{n}");
    }
    let (status, body, retry) = send(&app, &write(OWNER_KEY, 0)).await;
    assert_eq!(
        (status, body.as_str(), retry.as_deref()),
        (429, TOO_MANY, Some("2"))
    );
    assert_eq!(send(&app, &write(OTHER_KEY, 0)).await.0, 404);

    let events = |key: &'static str| Call {
        key: Some(key),
        ..Call::get(format!("/builder-api/v1/collections/{id}/events"))
    };
    for n in 0..crate::limits::REVIEW_READS {
        assert_eq!(send(&app, &events(OWNER_KEY)).await.0, 404, "{n}");
    }
    let (status, _, retry) = send(&app, &events(OWNER_KEY)).await;
    assert_eq!((status, retry.as_deref()), (429, Some("1")));
    advance(Duration::seconds(2));
    assert_eq!(send(&app, &write(OWNER_KEY, 1)).await.0, 404);
}

async fn seeded(state: &crate::SharedState, n: usize) -> Uuid {
    let owner = super::validator::address(OWNER_KEY);
    let collection = Uuid::new_v4();
    super::support::save_collection(
        &state.items,
        &owner,
        serde_json::json!({ "id": collection, "name": format!("Listing {}", &collection.simple().to_string()[..8]) }),
    )
    .await
    .unwrap();
    for i in 0..n {
        super::support::save_item(
            &state.items,
            &owner,
            &serde_json::json!({ "id": Uuid::new_v4(), "name": format!("Item {i}"), "collection_id": collection }),
        )
        .await
        .unwrap();
    }
    sqlx::query(
        "WITH made AS (INSERT INTO builder.collections (id, name, eth_address) \
           SELECT gen_random_uuid(), 'Curated ' || g, $1 FROM generate_series(1, $2) g RETURNING id) \
         INSERT INTO builder.collection_curations (id, collection_id, status) \
         SELECT gen_random_uuid(), id, 'pending' FROM made",
    )
    .bind(&owner)
    .bind(n as i32)
    .execute(state.items.pool())
    .await
    .unwrap();
    collection
}

async fn a_listing_charges_its_rows(
    listing: impl Fn(Uuid) -> String,
    committee: bool,
    item_curations: bool,
) {
    for by_source in [false, true] {
        let Some(pool) = super::validator::own_pool().await else {
            return;
        };
        let registry = match committee {
            true => {
                super::support::committee_index(&super::support::FakeCommittee::with(&[
                    &super::validator::address(OWNER_KEY),
                ]))
                .await
            }
            false => super::support::registry_only_index().await,
        };
        let (app, state) = node_over(pool, |state| {
            state.market = Some(registry);
            state.limits = crate::limits::Limits::unbounded();
            if by_source {
                state.limits.source_reads = per_minute(5);
            } else {
                state.limits.wallet_reads = per_minute(5);
            }
        })
        .await;
        let collection = seeded(&state, 60).await;
        if item_curations {
            sqlx::query(
                "INSERT INTO builder.item_curations (id, item_id, status, content_hash) \
                 SELECT gen_random_uuid(), id, 'pending', repeat('a', 64) FROM builder.items WHERE collection_id = $1",
            )
            .bind(collection)
            .execute(state.items.pool())
            .await
            .unwrap();
        }
        let source = by_source.then_some(SOURCE);
        let call = |path: String, key| Call {
            key: Some(key),
            source,
            ..Call::get(path)
        };
        let (status, body, _) = send(&app, &call(listing(collection), OWNER_KEY)).await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(
            send(
                &app,
                &call(format!("/v1/storage/contents/{NOT_A_CID}"), OWNER_KEY)
            )
            .await
            .0,
            404
        );
        let next = if by_source { OTHER_KEY } else { OWNER_KEY };
        let (status, _, retry) = send(
            &app,
            &call(format!("/v1/storage/contents/{NOT_A_CID}"), next),
        )
        .await;
        assert_eq!(
            (status, retry.as_deref()),
            (429, Some("12")),
            "{}",
            listing(collection)
        );
    }
}

#[tokio::test]
async fn a_collections_items_cost_their_reader_by_the_row() {
    a_listing_charges_its_rows(|id| format!("/v1/collections/{id}/items"), false, false).await;
}

#[tokio::test]
async fn an_addresss_items_cost_their_reader_by_the_row() {
    let owner = super::validator::address(OWNER_KEY);
    a_listing_charges_its_rows(
        move |_| format!("/builder-api/v1/{owner}/items"),
        false,
        false,
    )
    .await;
}

#[tokio::test]
async fn every_item_costs_the_committee_member_by_the_row() {
    a_listing_charges_its_rows(|_| "/v1/items".to_string(), true, false).await;
}

#[tokio::test]
async fn the_curations_cost_their_reader_by_the_row() {
    a_listing_charges_its_rows(|_| "/v1/curations".to_string(), true, false).await;
}

#[tokio::test]
async fn an_addresss_collections_cost_their_reader_by_the_row() {
    let owner = super::validator::address(OWNER_KEY);
    a_listing_charges_its_rows(move |_| format!("/v1/{owner}/collections"), false, false).await;
}

#[tokio::test]
async fn a_collections_item_curations_cost_their_reader_by_the_row() {
    a_listing_charges_its_rows(
        |id| format!("/v1/collections/{id}/itemCurations"),
        true,
        true,
    )
    .await;
}

#[tokio::test]
async fn the_public_collection_search_costs_its_source_as_fifty_reads() {
    let Some(pool) = super::validator::own_pool().await else {
        return;
    };
    let app = app_over(pool, |state| state.limits.source_reads = per_minute(102)).await;
    let search = |query: &str| Call {
        source: Some(SOURCE),
        ..Call::get(format!("/v1/collections?is_published=true&q=a{query}"))
    };
    assert_eq!(send(&app, &search("")).await.0, 200);
    assert_eq!(send(&app, &search("&tag=hat")).await.0, 200);
    let (status, _, retry) = send(&app, &search("&tag=other")).await;
    assert_eq!((status, retry.as_deref()), (429, Some("1")));
    assert_eq!(
        send(
            &app,
            &Call {
                source: Some(OTHER_SOURCE),
                ..Call::get("/v1/collections?is_published=true&q=a")
            }
        )
        .await
        .0,
        200
    );
}

#[tokio::test]
async fn the_addresses_listing_costs_its_source_as_fifty_reads() {
    let Some(pool) = super::validator::own_pool().await else {
        return;
    };
    let app = app_over(pool, |state| state.limits.source_reads = per_minute(102)).await;
    let addresses = |query: &str| Call {
        source: Some(SOURCE),
        ..Call::get(format!("/v1/addresses{query}"))
    };
    assert_eq!(send(&app, &addresses("")).await.0, 200);
    assert_eq!(send(&app, &addresses("?tag=hat")).await.0, 200);
    let (status, _, retry) = send(&app, &addresses("?tag=other")).await;
    assert_eq!((status, retry.as_deref()), (429, Some("1")));
    assert_eq!(
        send(
            &app,
            &Call {
                source: Some(OTHER_SOURCE),
                ..Call::get("/v1/addresses")
            }
        )
        .await
        .0,
        200
    );
}

#[tokio::test]
async fn a_listing_past_its_bytes_or_its_readers_budget_is_refused_before_its_items_load() {
    let Some(pool) = super::validator::own_pool().await else {
        return;
    };
    let (_, state) = node_over(pool.clone(), |state| {
        state.limits = crate::limits::Limits::unbounded();
    })
    .await;
    let collection = seeded(&state, 60).await;
    let ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM builder.items WHERE collection_id = $1")
            .bind(collection)
            .fetch_all(&pool)
            .await
            .unwrap();
    let bytes: i64 = sqlx::query_scalar(crate::items::listing::DOCUMENT_BYTES)
        .bind(&ids)
        .fetch_one(&pool)
        .await
        .unwrap();
    let items = |query: &str| Call {
        key: Some(OWNER_KEY),
        ..Call::get(format!("/v1/collections/{collection}/items{query}"))
    };
    for (bound, query, expected) in [
        (bytes, "", 200),
        (bytes - 1, "", 413),
        (bytes - 1, "?page=1&limit=59", 200),
    ] {
        let app = app_over(pool.clone(), |state| {
            state.limits = crate::limits::Limits::unbounded();
            state.limits.listing_bytes = bound;
        })
        .await;
        let (status, body, _) = send(&app, &items(query)).await;
        assert_eq!(status, expected, "{bound} {query}: {body}");
    }
    let app = app_over(pool.clone(), |state| {
        state.limits = crate::limits::Limits::unbounded();
        state.limits.wallet_reads = per_minute(3);
    })
    .await;
    let (status, _, retry) = send(&app, &items("")).await;
    assert_eq!((status, retry.is_some()), (429, true));
    let (status, _, _) = send(&app, &items("?page=1&limit=19")).await;
    assert_eq!(status, 200);
    let (app, state) = node_over(pool.clone(), |state| {
        state.limits = crate::limits::Limits::unbounded();
        state.limits.source_reads = per_minute(3);
        state.limits.wallet_reads = crate::limits::budget(1_000, Window::from_secs(86_400));
    })
    .await;
    let from_source = Call {
        source: Some("198.51.100.90"),
        ..items("")
    };
    let (status, _, _) = send(&app, &from_source).await;
    assert_eq!(status, 429);
    let wallet = super::validator::address(OWNER_KEY);
    let spent = state
        .limits
        .wallet_reads
        .spent_at(&wallet, std::time::Instant::now());
    assert!(
        spent < 2.0,
        "the wallet paid {spent} read tokens for a listing its address refused"
    );
}

/// An owner's draft past the 50 items a publication takes: its preview and
/// its begin answer 400 before any item document loads, any named file is
/// looked up in the store or any item row is locked, and nothing is
/// recorded.
#[tokio::test]
async fn a_draft_past_the_publication_cap_is_refused_before_its_items_load() {
    let Some(pool) = super::validator::own_pool().await else {
        return;
    };
    let (app, state) = node_over(pool.clone(), |state| {
        state.limits = crate::limits::Limits::unbounded();
        state.settings.polygon_rpc_url = Some("https://rpc.example".into());
    })
    .await;
    let collection = seeded(&state, 51).await;
    let faulty = dcl_auth::hash_bytes_v1(Uuid::new_v4().as_bytes());
    tokio::fs::create_dir_all(
        crate::storage::resolve_file_path(state.items.blobs.root(), &faulty).unwrap(),
    )
    .await
    .unwrap();
    assert!(state.items.blobs.exist(&faulty).await.is_err());
    sqlx::query(
        "INSERT INTO builder.item_contents (item_id, file, hash, named_by) \
         SELECT id, 'model.glb', $2, eth_address FROM builder.items WHERE collection_id = $1",
    )
    .bind(collection)
    .bind(&faulty)
    .execute(&pool)
    .await
    .unwrap();
    let mut holder = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM builder.items WHERE collection_id = $1 FOR UPDATE")
        .bind(collection)
        .fetch_all(&mut *holder)
        .await
        .unwrap();
    let path = format!("/v1/collections/{collection}/publication");
    for call in [
        Call {
            key: Some(OWNER_KEY),
            ..Call::get(path.clone())
        },
        Call {
            method: "POST",
            key: Some(OWNER_KEY),
            body: br#"{"revision":"any"}"#.to_vec(),
            ..Call::get(path.clone())
        },
    ] {
        let (status, body, _) = tokio::time::timeout(Window::from_secs(10), send(&app, &call))
            .await
            .unwrap_or_else(|_| panic!("{} waited for the locked item rows", call.method));
        assert_eq!(status, 400, "{}: {body}", call.method);
        assert!(
            body.contains("Publish a collection containing 1 to 50 items"),
            "{}: {body}",
            call.method
        );
    }
    holder.rollback().await.unwrap();
    let recorded: (i64, bool) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM builder.collection_publications WHERE collection_id = $1), \
                (SELECT publication_pending FROM builder.collections WHERE id = $1)",
    )
    .bind(collection)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(recorded, (0, false));
}

#[tokio::test]
async fn every_item_answers_the_committee_member_at_most_its_cap() {
    let Some(pool) = super::validator::own_pool().await else {
        return;
    };
    let member = super::validator::address(OWNER_KEY);
    sqlx::query(
        "INSERT INTO builder.items (id, name, eth_address, type) \
         SELECT gen_random_uuid(), 'Item ' || g, $1, 'wearable' \
         FROM generate_series(1, $2) g",
    )
    .bind(&member)
    .bind(crate::items::listing::EVERY_ITEM_CAP as i32 + 1)
    .execute(&pool)
    .await
    .unwrap();
    let registry =
        super::support::committee_index(&super::support::FakeCommittee::with(&[&member])).await;
    let app = app_over(pool, |state| {
        state.market = Some(registry);
        state.limits = crate::limits::Limits::unbounded();
    })
    .await;
    let (status, body, _) = send(
        &app,
        &Call {
            key: Some(OWNER_KEY),
            ..Call::get("/v1/items")
        },
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let answered: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        answered["data"].as_array().unwrap().len(),
        crate::items::listing::EVERY_ITEM_CAP as usize
    );
}

#[tokio::test]
async fn a_wallets_drafts_answer_at_most_their_cap_and_cost_it_by_the_row() {
    let Some(pool) = super::validator::own_pool().await else {
        return;
    };
    let owner = super::validator::address(OWNER_KEY);
    sqlx::query(
        "INSERT INTO builder.items (id, name, eth_address, type) \
         SELECT gen_random_uuid(), 'Draft ' || g, $1, 'wearable' \
         FROM generate_series(1, $2) g",
    )
    .bind(&owner)
    .bind(crate::drafts::DRAFTS_CAP as i32 + 1)
    .execute(&pool)
    .await
    .unwrap();
    let reads = crate::limits::WALLET_READS;
    let registry = super::support::registry_only_index().await;
    let app = app_over(pool, |state| {
        state.market = Some(registry);
        state.limits = crate::limits::Limits::unbounded();
        state.limits.wallet_reads = per_minute(reads);
    })
    .await;
    let drafts = Call {
        key: Some(OWNER_KEY),
        ..Call::get("/v1/drafts/items")
    };
    let (status, body, _) = send(&app, &drafts).await;
    assert_eq!(status, 200);
    let answered: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        answered["data"].as_array().unwrap().len(),
        crate::drafts::DRAFTS_CAP as usize
    );
    let spent = 1 + crate::drafts::DRAFTS_CAP as u32 / crate::limits::ROWS_PER_TOKEN as u32;
    for _ in spent..reads {
        assert_eq!(
            send(
                &app,
                &Call {
                    key: Some(OWNER_KEY),
                    ..Call::get(format!("/v1/storage/contents/{NOT_A_CID}"))
                }
            )
            .await
            .0,
            404
        );
    }
    let (status, _, _) = send(&app, &drafts).await;
    assert_eq!(status, 429);
}

/// Incompressible text of `len` characters, different for each `seed`.
fn noise(seed: u64, len: usize) -> String {
    let mut x = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            char::from(b'a' + (x % 26) as u8)
        })
        .collect()
}

/// `PUT /v1/items/{id}` of a new or stored loose item of `key`'s from
/// `source`, carrying `filler` characters of permissions.
async fn put_item(
    app: &Router,
    state: &crate::SharedState,
    key: &str,
    source: &str,
    id: Uuid,
    filler: usize,
) -> (u16, String) {
    let permissions: Vec<String> = (0..filler.div_ceil(1_000))
        .map(|n| noise(id.as_u128() as u64 ^ n as u64, 1_000.min(filler)))
        .collect();
    let body = super::support::item_body(
        &state.items,
        &super::validator::address(key),
        &serde_json::json!({ "id": id, "name": "Big hat", "rarity": "common",
            "contents": super::support::contents_of(&[("model.glb", b"model")]),
            "data": { "requiredPermissions": permissions } }),
    )
    .await;
    let (status, body, _) = send(
        app,
        &Call {
            method: "PUT",
            path: format!("/v1/items/{id}"),
            key: Some(key),
            source: Some(source),
            body: serde_json::to_vec(&body).unwrap(),
        },
    )
    .await;
    (status, body)
}

async fn stored_items(pool: &sqlx::PgPool, ids: &[Uuid]) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM builder.items WHERE id = ANY($1)")
        .bind(ids)
        .fetch_one(pool)
        .await
        .unwrap()
}

const DOCUMENT_KEYS: [&str; 8] = [
    "0x5611111111111111111111111111111111111111111111111111111111111111",
    "0x5622222222222222222222222222222222222222222222222222222222222222",
    "0x5633333333333333333333333333333333333333333333333333333333333333",
    "0x5644444444444444444444444444444444444444444444444444444444444444",
    "0x5655555555555555555555555555555555555555555555555555555555555555",
    "0x5666666666666666666666666666666666666666666666666666666666666666",
    "0x5677777777777777777777777777777777777777777777777777777777777777",
    "0x5688888888888888888888888888888888888888888888888888888888888888",
];

/// A wallet keeps at most its bound of draft items: the next new one is
/// refused and not stored, while it still edits the ones it has and
/// another wallet saves on.
#[tokio::test]
async fn a_wallet_keeps_at_most_its_bound_of_draft_items_and_still_edits_them() {
    let Some(pool) = super::support::scratch_pool(8).await else {
        return;
    };
    let (app, state) = node_over(pool.clone(), |state| state.items.documents.items = 3).await;
    let ids: Vec<Uuid> = (0..4).map(|_| Uuid::new_v4()).collect();
    let mut statuses = Vec::new();
    for id in &ids {
        statuses.push(put_item(&app, &state, DOCUMENT_KEYS[0], SOURCE, *id, 10).await);
    }
    let edited = put_item(&app, &state, DOCUMENT_KEYS[0], SOURCE, ids[0], 20)
        .await
        .0;
    let other = Uuid::new_v4();
    let others = put_item(&app, &state, DOCUMENT_KEYS[3], SOURCE, other, 10)
        .await
        .0;
    let stored = stored_items(&pool, &ids).await;
    let (status, body) = &statuses[3];
    assert_eq!(
        statuses[..3].iter().map(|s| s.0).collect::<Vec<_>>(),
        [200, 200, 200]
    );
    assert_eq!(*status, 413, "{body}");
    let body: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(body["data"], serde_json::json!({ "maxItems": 3 }));
    assert_eq!((stored, edited, others), (3, 200, 200));
}

/// A wallet keeps at most its bound of draft collections.
#[tokio::test]
async fn a_wallet_keeps_at_most_its_bound_of_draft_collections() {
    let Some(pool) = super::support::scratch_pool(8).await else {
        return;
    };
    let (app, _) = node_over(pool.clone(), |state| state.items.documents.collections = 2).await;
    let owner = super::validator::address(DOCUMENT_KEYS[2]);
    let mut statuses = Vec::new();
    for n in 0..3 {
        let id = Uuid::new_v4();
        let body = super::support::collection_body(
            serde_json::json!({ "id": id, "name": format!("Kept {n} {}", &id.simple().to_string()[..8]) }),
            &owner,
        );
        let (status, body, _) = send(
            &app,
            &Call {
                method: "PUT",
                path: format!("/v1/collections/{id}"),
                key: Some(DOCUMENT_KEYS[2]),
                source: Some(SOURCE),
                body: serde_json::to_vec(&body).unwrap(),
            },
        )
        .await;
        statuses.push((status, body));
    }
    let stored: i64 =
        sqlx::query_scalar("SELECT count(*) FROM builder.collections WHERE eth_address = $1")
            .bind(&owner)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        statuses.iter().map(|s| s.0).collect::<Vec<_>>(),
        [200, 200, 413],
        "{:?}",
        statuses[2].1
    );
    assert!(
        statuses[2].1.contains(r#""maxCollections":2"#),
        "{}",
        statuses[2].1
    );
    assert_eq!(stored, 2);
}

/// A wallet's draft item documents are bounded in stored bytes as in
/// number: a save that would pass the bound is refused and not stored.
#[tokio::test]
async fn a_wallets_draft_item_documents_are_bounded_in_bytes() {
    let Some(pool) = super::support::scratch_pool(8).await else {
        return;
    };
    let (app, state) = node_over(pool.clone(), |state| state.items.documents.bytes = 20_000).await;
    let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
    let mut statuses = Vec::new();
    for id in &ids {
        statuses.push(put_item(&app, &state, DOCUMENT_KEYS[1], OTHER_SOURCE, *id, 8_000).await);
    }
    let stored = stored_items(&pool, &ids).await;
    assert_eq!(
        statuses.iter().map(|s| s.0).collect::<Vec<_>>(),
        [200, 200, 413],
        "{}",
        statuses[2].1
    );
    assert!(
        statuses[2].1.contains(r#""maxBytes":20000"#),
        "{}",
        statuses[2].1
    );
    assert_eq!(stored, 2);
}

/// Document saves spend their bytes of their address's and their wallet's
/// day: past either, the next save waits, while another address and wallet
/// save on.
#[tokio::test]
async fn document_saves_spend_their_bytes_of_the_day() {
    let Some(pool) = super::support::scratch_pool(8).await else {
        return;
    };
    let day = || crate::limits::budget(600, Window::from_secs(86_400));
    let (app, state) = node_over(pool.clone(), |state| {
        state.limits.source_document_kib = day();
        state.limits.wallet_document_kib = day();
    })
    .await;
    let big = 240_000;
    let a = put_item(
        &app,
        &state,
        DOCUMENT_KEYS[4],
        "198.51.100.30",
        Uuid::new_v4(),
        big,
    )
    .await;
    let b = put_item(
        &app,
        &state,
        DOCUMENT_KEYS[5],
        "198.51.100.30",
        Uuid::new_v4(),
        big,
    )
    .await;
    let c = put_item(
        &app,
        &state,
        DOCUMENT_KEYS[6],
        "198.51.100.30",
        Uuid::new_v4(),
        big,
    )
    .await;
    let d = put_item(
        &app,
        &state,
        DOCUMENT_KEYS[6],
        "198.51.100.31",
        Uuid::new_v4(),
        big,
    )
    .await;
    let e = put_item(
        &app,
        &state,
        DOCUMENT_KEYS[4],
        "198.51.100.32",
        Uuid::new_v4(),
        big,
    )
    .await;
    let f = put_item(
        &app,
        &state,
        DOCUMENT_KEYS[4],
        "198.51.100.33",
        Uuid::new_v4(),
        big,
    )
    .await;
    assert_eq!(
        (a.0, b.0, c.0, d.0, e.0, f.0),
        (200, 200, 429, 200, 200, 429),
        "{} {}",
        c.1,
        f.1
    );
}

/// Under the production budgets one wallet behind one address can't keep
/// writing item documents of the size bound as fast as it asks.
#[tokio::test]
async fn under_the_production_budgets_one_wallet_cannot_write_documents_at_will() {
    let Some(pool) = super::support::scratch_pool(8).await else {
        return;
    };
    let (app, state) = node_over(pool.clone(), |_| {}).await;
    let mut statuses = std::collections::BTreeMap::new();
    for _ in 0..300 {
        let status = put_item(&app, &state, OTHER_KEY, SOURCE, Uuid::new_v4(), 245_000)
            .await
            .0;
        *statuses.entry(status).or_insert(0) += 1;
        if status == 429 {
            break;
        }
    }
    sqlx::query("DELETE FROM builder.items WHERE eth_address = $1")
        .bind(super::validator::address(OTHER_KEY))
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        statuses.get(&429).is_some_and(|refused| *refused > 0),
        "{statuses:?}"
    );
}

/// Saves of new drafts sent at once still leave the wallet within its bound.
#[tokio::test]
async fn new_drafts_saved_at_once_stay_within_the_wallets_bound() {
    let Some(pool) = super::support::scratch_pool(8).await else {
        return;
    };
    let (app, state) = node_over(pool.clone(), |state| state.items.documents.items = 3).await;
    let ids: Vec<Uuid> = (0..12).map(|_| Uuid::new_v4()).collect();
    let saves = ids.iter().map(|id| {
        let (app, state, id) = (app.clone(), state.clone(), *id);
        tokio::spawn(async move {
            put_item(&app, &state, DOCUMENT_KEYS[7], SOURCE, id, 10)
                .await
                .0
        })
    });
    let mut statuses = Vec::new();
    for save in saves.collect::<Vec<_>>() {
        statuses.push(save.await.unwrap());
    }
    statuses.sort();
    assert_eq!(stored_items(&pool, &ids).await, 3, "{statuses:?}");
    assert_eq!(
        statuses,
        [[200; 3].as_slice(), [413; 9].as_slice()].concat()
    );
}

/// The bounds table of `services/catalyst/docs/builder.md` states the value
/// of each constant its rows name and names no constant this test doesn't
/// check; every numeric constant of the builder crate is a row or listed
/// here as no bound of the table; and the node's budgets take the
/// constants' values.
#[test]
fn the_builder_doc_states_each_bound_the_code_enforces() {
    use crate::collections::{listing as cl, publish_third_party, save as cs};
    use crate::handlers::{curation as hc, newsletter, third_parties as htp, validator as hv};
    use crate::items::{listing as il, places, save as is, schema, upload, video};
    use crate::limits as l;
    fn grouped(n: u64) -> String {
        let digits = n.to_string();
        digits
            .chars()
            .enumerate()
            .fold(String::new(), |mut out, (i, digit)| {
                if i > 0 && (digits.len() - i).is_multiple_of(3) {
                    out.push(',');
                }
                out.push(digit);
                out
            })
    }
    let count = grouped;
    let bytes = |n: u64| format!("{} bytes", grouped(n));
    let binary = |n: u64| {
        let (unit, size) = [("GiB", 1u64 << 30), ("MiB", 1 << 20), ("KiB", 1 << 10)]
            .into_iter()
            .find(|(_, size)| n.is_multiple_of(*size))
            .unwrap_or(("bytes", 1));
        format!("{} {unit}", grouped(n / size))
    };
    let span = |d: std::time::Duration| {
        let secs = d.as_secs();
        let (unit, size) = [("hour", 3_600), ("minute", 60), ("second", 1)]
            .into_iter()
            .find(|(_, size)| secs.is_multiple_of(*size))
            .unwrap();
        let n = secs / size;
        format!("{n} {unit}{}", if n == 1 { "" } else { "s" })
    };
    let mut ipv6 = axum::http::HeaderMap::new();
    ipv6.insert("x-real-ip", "2001:db8:5:1::1".parse().unwrap());
    let block = l::upload_source(&ipv6).unwrap();
    let request_block = l::source(&ipv6).unwrap();
    let enforced = [
        ("limits::SOURCE_READS", count(l::SOURCE_READS.into())),
        ("limits::SOURCE_WRITES", count(l::SOURCE_WRITES.into())),
        ("limits::WALLET_READS", count(l::WALLET_READS.into())),
        ("limits::WALLET_WRITES", count(l::WALLET_WRITES.into())),
        ("ITEM_BODY_LIMIT", binary(crate::ITEM_BODY_LIMIT as u64)),
        ("LARGE_BODY_LIMIT", binary(crate::LARGE_BODY_LIMIT as u64)),
        ("BODY_LIMIT", binary(crate::BODY_LIMIT as u64)),
        ("limits::WALLET_BYTES", bytes(l::WALLET_BYTES)),
        ("limits::WALLET_FILES", count(l::WALLET_FILES)),
        ("limits::NODE_BYTES", bytes(l::NODE_BYTES)),
        ("limits::NODE_FILES", count(l::NODE_FILES)),
        ("limits::UNPUBLISHED_KEPT", span(l::UNPUBLISHED_KEPT)),
        (
            "limits::SOURCE_UPLOAD_MIB",
            binary(u64::from(l::SOURCE_UPLOAD_MIB) << 20),
        ),
        (
            "limits::SOURCE_UPLOAD_FILES",
            count(l::SOURCE_UPLOAD_FILES.into()),
        ),
        (
            "limits::WALLET_UPLOAD_FILES",
            count(l::WALLET_UPLOAD_FILES.into()),
        ),
        (
            "limits::NEAR_BOUND_PRICE",
            count(l::NEAR_BOUND_PRICE.into()),
        ),
        (
            "limits::upload_source",
            format!("/{}", block.rsplit('/').next().unwrap()),
        ),
        ("items::upload::MAX_FILES", count(upload::MAX_FILES as u64)),
        ("items::upload::MAX_BYTES", binary(upload::MAX_BYTES as u64)),
        ("UPLOAD_BODY_LIMIT", binary(crate::UPLOAD_BODY_LIMIT as u64)),
        ("items::upload::DEADLINE", span(upload::DEADLINE)),
        (
            "storage::gc::SHARED_LOCK_WAIT",
            span(crate::storage::gc::SHARED_LOCK_WAIT),
        ),
        ("FILE_UPLOADS", count(crate::FILE_UPLOADS as u64)),
        (
            "FILE_UPLOADS_PER_SIGNER",
            count(crate::FILE_UPLOADS_PER_SIGNER as u64),
        ),
        (
            "FILE_UPLOADS_PER_SOURCE",
            count(crate::FILE_UPLOADS_PER_SOURCE as u64),
        ),
        (
            "items::places::RETRY_AFTER_SECS",
            span(std::time::Duration::from_secs(
                places::RETRY_AFTER_SECS.into(),
            )),
        ),
        (
            "items::video::DEFAULT_MAX_BYTES",
            bytes(video::DEFAULT_MAX_BYTES),
        ),
        (
            "items::video::DEFAULT_DEADLINE",
            span(video::DEFAULT_DEADLINE),
        ),
        (
            "items::places::IDLE_TIMEOUT",
            span(crate::items::places::IDLE_TIMEOUT),
        ),
        (
            "items::video::MAX_UPLOADS",
            count(video::MAX_UPLOADS as u64),
        ),
        (
            "MAX_VIDEO_UPLOADS_SETTING",
            count(crate::MAX_VIDEO_UPLOADS_SETTING as u64),
        ),
        (
            "items::video::MAX_UPLOADS_PER_SIGNER",
            count(video::MAX_UPLOADS_PER_SIGNER as u64),
        ),
        (
            "items::video::MAX_UPLOADS_PER_SOURCE",
            count(video::MAX_UPLOADS_PER_SOURCE as u64),
        ),
        (
            "items::video::WALLET_VIDEO_SECS_A_DAY",
            span(video::WALLET_VIDEO_SECS_A_DAY),
        ),
        (
            "items::video::SOURCE_VIDEO_SECS_A_DAY",
            span(video::SOURCE_VIDEO_SECS_A_DAY),
        ),
        ("limits::WALLET_ITEMS", count(l::WALLET_ITEMS)),
        ("limits::WALLET_COLLECTIONS", count(l::WALLET_COLLECTIONS)),
        (
            "limits::WALLET_DOCUMENT_BYTES",
            binary(l::WALLET_DOCUMENT_BYTES),
        ),
        (
            "limits::WALLET_DOCUMENT_KIB",
            binary(u64::from(l::WALLET_DOCUMENT_KIB) << 10),
        ),
        (
            "limits::SOURCE_DOCUMENT_KIB",
            binary(u64::from(l::SOURCE_DOCUMENT_KIB) << 10),
        ),
        (
            "limits::DOCUMENT_MIN_KIB",
            binary(u64::from(l::DOCUMENT_MIN_KIB) << 10),
        ),
        (
            "limits::BYTES_PER_WRITE_TOKEN",
            binary(l::BYTES_PER_WRITE_TOKEN as u64),
        ),
        ("limits::REVIEW_WRITES", count(l::REVIEW_WRITES.into())),
        ("limits::REVIEW_READS", count(l::REVIEW_READS.into())),
        (
            "limits::NEWSLETTER_PER_SOURCE",
            count(l::NEWSLETTER_PER_SOURCE.into()),
        ),
        (
            "limits::NEWSLETTER_PER_EMAIL",
            count(l::NEWSLETTER_PER_EMAIL.into()),
        ),
        (
            "limits::NEWSLETTER_PER_NODE",
            count(l::NEWSLETTER_PER_NODE.into()),
        ),
        (
            "limits::ADDRESSES_TOKENS",
            count(l::ADDRESSES_TOKENS.into()),
        ),
        ("limits::ROWS_PER_TOKEN", count(l::ROWS_PER_TOKEN as u64)),
        ("limits::BYTES_PER_TOKEN", binary(l::BYTES_PER_TOKEN as u64)),
        ("limits::LISTING_BYTES", binary(l::LISTING_BYTES as u64)),
        ("limits::KEYS", count(l::KEYS as u64)),
        (
            "limits::UNPUBLISHED_COUNT_TIMEOUT",
            span(l::UNPUBLISHED_COUNT_TIMEOUT),
        ),
        (
            "limits::source",
            format!("/{}", request_block.rsplit('/').next().unwrap()),
        ),
        ("MIN_SECRET_BYTES", bytes(crate::MIN_SECRET_BYTES as u64)),
        (
            "drafts::DRAFTS_CAP",
            count(crate::drafts::DRAFTS_CAP as u64),
        ),
        ("items::listing::ROWS_CAP", count(il::ROWS_CAP as u64)),
        (
            "items::listing::EVERY_ITEM_CAP",
            count(il::EVERY_ITEM_CAP as u64),
        ),
        (
            "collections::listing::COMMITTEE_CAP",
            count(cl::COMMITTEE_CAP as u64),
        ),
        (
            "collections::listing::ADDRESS_CAP",
            count(cl::ADDRESS_CAP as u64),
        ),
        ("kept::READ_WINDOW", span(crate::kept::READ_WINDOW)),
        (
            "committee::MAX_MEMBERS",
            count(crate::committee::MAX_MEMBERS as u64),
        ),
        ("collections::listing::Q_MAX", count(cl::Q_MAX as u64)),
        (
            "collections::listing::MAX_ASSIGNEES",
            count(cl::MAX_ASSIGNEES as u64),
        ),
        ("collections::listing::MAX_TAGS", count(cl::MAX_TAGS as u64)),
        ("collections::listing::TAG_MAX", count(cl::TAG_MAX as u64)),
        (
            "curation::rows::MAX_ROWS",
            count(crate::curation::rows::MAX_ROWS as u64),
        ),
        (
            "collections::publish_third_party::MAX_ITEMS",
            count(publish_third_party::MAX_ITEMS as u64),
        ),
        ("terms::MAX_HASHES", count(crate::terms::MAX_HASHES as u64)),
        (
            "terms::MAX_HASH_LEN",
            count(crate::terms::MAX_HASH_LEN as u64),
        ),
        ("items::save::MAX_TAGS", count(is::MAX_TAGS as u64)),
        ("items::save::MAX_OUTCOMES", count(is::MAX_OUTCOMES as u64)),
        (
            "items::schema::MAX_CONTENTS",
            count(schema::MAX_CONTENTS as u64),
        ),
        ("items::schema::LIST_MAX", count(schema::LIST_MAX as u64)),
        ("items::schema::TEXT_MAX", count(schema::TEXT_MAX as u64)),
        (
            "items::schema::REPRESENTATIONS_MAX",
            count(schema::REPRESENTATIONS_MAX as u64),
        ),
        (
            "items::schema::HIDDEN_MAX",
            count(schema::HIDDEN_MAX as u64),
        ),
        ("items::schema::BONES_MAX", count(schema::BONES_MAX as u64)),
        (
            "items::schema::MAX_MAPPINGS",
            count(schema::MAX_MAPPINGS as u64),
        ),
        (
            "items::schema::MAX_MAPPING_IDS",
            count(schema::MAX_MAPPING_IDS as u64),
        ),
        ("collections::save::NAME_MAX", count(cs::NAME_MAX as u64)),
        ("collections::save::TEXT_MAX", count(cs::TEXT_MAX as u64)),
        ("collections::save::LIST_MAX", count(cs::LIST_MAX as u64)),
        ("collections::save::DATA_MAX", binary(cs::DATA_MAX as u64)),
        (
            "handlers::curation::MAX_BULK_ITEMS",
            count(hc::MAX_BULK_ITEMS as u64),
        ),
        (
            "handlers::newsletter::MAX_BODY",
            binary(newsletter::MAX_BODY as u64),
        ),
        (
            "handlers::newsletter::MAX_SOURCE",
            count(newsletter::MAX_SOURCE as u64),
        ),
        (
            "handlers::validator::SMALL_BODY",
            binary(hv::SMALL_BODY as u64),
        ),
        (
            "handlers::validator::VERDICT_BODY",
            binary(hv::VERDICT_BODY as u64),
        ),
        ("handlers::third_parties::ID_MAX", count(htp::ID_MAX as u64)),
        ("items::video::BODY_LIMIT", bytes(video::BODY_LIMIT as u64)),
        ("items::video::FRAMING_BYTES", binary(video::FRAMING_BYTES)),
        (
            "items::video::SPOOL_ORPHAN_AGE",
            span(video::SPOOL_ORPHAN_AGE),
        ),
    ];
    let not_in_table = [
        "limits::MINUTE",
        "limits::HOUR",
        "limits::DAY",
        "limits::MIB",
        "limits::UPLOAD_SOURCE_V6_BITS",
        "items::video::SLICE_BYTES",
        "handlers::storage::SNIFF_BYTES",
        "collections::listing::UNPAGINATED_LIMIT",
        "terms::MAX_ATTEMPTS",
        "terms::PASS_LIMIT",
        "access::FILE_PROVIDERS",
        "access::LOCK_HOLDS",
        "chain_state::STALE_AFTER",
        "collections::deployment::DEPLOYMENT_TTL",
        "collections::deployment::KEPT_CONTRACTS",
        "kept::FAILURE_TTL",
        "kept::FETCH_TIMEOUT",
        "onchain::MAX_REVIEW_ROWS",
        "onchain::CURATION_CHUNK",
        "rarities::RARITIES_TTL",
        "rarities::MAX_RARITIES",
        "rarities::NAME_MAX",
        "rarities::AMOUNT_MAX",
        "curation::events::JUDGED_RUNS",
        "curation::forum::TIMEOUT",
        "curation::forum::MAX_ANSWER",
        "curation::forum::MAX_LINK",
        "curation::forum_posts::CLAIM_HOLDS",
        "curation::forum_posts::ATTEMPT",
        "curation::forum_posts::MAX_REPLY",
        "curation::forum_text::MAX_ITEMS",
        "curation::slack::TIMEOUT",
        "curation::slack::MAX_NAME",
        "curation::slack::MAX_QUOTE",
        "curation::writes::MAX_MESSAGE",
        "curation::writes::MAX_NOTE",
        "review::COMMENTS_PER_COLLECTION",
        "review::COMMENTS_PER_CALLER",
        "review::APPEALS_PER_COLLECTION",
        "review::APPEALS_PER_CALLER",
        "review::MAX_TEXT",
        "review::DEFAULT_EVENTS",
        "review::MAX_EVENTS",
        "review::MAX_PAGE",
        "publication::MAX_ITEMS",
        "publication_fee::WEI_PER_USD_CENT",
        "validator::MAX_ITEMS",
        "validator::DAILY_REQUESTS",
        "validator::LEASE",
        "validator::LEASE_LIMIT",
        "validator::UNSETTLED_FOR",
        "validator::KEPT_FOR",
        "validator::claims::TOKEN",
        "validator::claims::SHORT_TEXT",
        "validator::claims::LONG_TEXT",
        "validator::claims::MAX_FINDINGS",
        "validator::claims::LOGGED_ERROR",
    ];
    let mut wrong = numeric_constants(std::path::Path::new(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/builder/src"
    )))
    .into_iter()
    .filter(|name| {
        !enforced.iter().any(|(code, _)| code == name) && !not_in_table.contains(&name.as_str())
    })
    .map(|name| format!("{name}: a constant neither in the table nor listed as no bound of it"))
    .collect::<Vec<_>>();
    let defaults = l::Limits::default();
    let now = std::time::Instant::now();
    for (name, budget, per) in [
        ("SOURCE_READS", &defaults.source_reads, l::SOURCE_READS),
        ("SOURCE_WRITES", &defaults.source_writes, l::SOURCE_WRITES),
        ("WALLET_READS", &defaults.wallet_reads, l::WALLET_READS),
        ("WALLET_WRITES", &defaults.wallet_writes, l::WALLET_WRITES),
        ("REVIEW_WRITES", &defaults.review_writes, l::REVIEW_WRITES),
        ("REVIEW_READS", &defaults.review_reads, l::REVIEW_READS),
        (
            "NEWSLETTER_PER_SOURCE",
            &defaults.newsletter_sources,
            l::NEWSLETTER_PER_SOURCE,
        ),
        (
            "NEWSLETTER_PER_EMAIL",
            &defaults.newsletter_emails,
            l::NEWSLETTER_PER_EMAIL,
        ),
        (
            "NEWSLETTER_PER_NODE",
            &defaults.newsletter_node,
            l::NEWSLETTER_PER_NODE,
        ),
        (
            "SOURCE_UPLOAD_MIB",
            &defaults.source_upload_mib,
            l::SOURCE_UPLOAD_MIB,
        ),
        (
            "SOURCE_UPLOAD_FILES",
            &defaults.source_upload_files,
            l::SOURCE_UPLOAD_FILES,
        ),
        (
            "WALLET_UPLOAD_FILES",
            &defaults.wallet_upload_files,
            l::WALLET_UPLOAD_FILES,
        ),
        (
            "SOURCE_DOCUMENT_KIB",
            &defaults.source_document_kib,
            l::SOURCE_DOCUMENT_KIB,
        ),
        (
            "WALLET_DOCUMENT_KIB",
            &defaults.wallet_document_kib,
            l::WALLET_DOCUMENT_KIB,
        ),
    ] {
        if budget.take_at("probe", per, now).is_err() || budget.take_at("probe", 1, now).is_ok() {
            wrong.push(format!("{name}: the node's budget holds another value"));
        }
    }
    for (name, held, bound) in [
        ("WALLET_BYTES", defaults.wallet_bytes, l::WALLET_BYTES),
        ("WALLET_FILES", defaults.wallet_files, l::WALLET_FILES),
        ("NODE_BYTES", defaults.node_bytes, l::NODE_BYTES),
        ("NODE_FILES", defaults.node_files, l::NODE_FILES),
        (
            "LISTING_BYTES",
            defaults.listing_bytes as u64,
            l::LISTING_BYTES as u64,
        ),
    ] {
        if held != bound {
            wrong.push(format!("{name}: the node's bound holds {held}"));
        }
    }
    let doc = std::fs::read_to_string(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/docs/builder.md"
    ))
    .unwrap();
    let rows: Vec<(String, String)> = doc
        .split_once("\n### Bounds\n")
        .expect("builder.md has a Bounds table")
        .1
        .lines()
        .take_while(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            let code = cells.get(3)?.strip_prefix('`')?.strip_suffix('`')?;
            Some((code.to_string(), cells[2].to_string()))
        })
        .collect();
    let stated: std::collections::BTreeMap<&str, &str> = rows
        .iter()
        .map(|(code, value)| (code.as_str(), value.as_str()))
        .collect();
    wrong.extend(
        enforced
            .iter()
            .filter(|(code, value)| stated.get(code) != Some(&value.as_str()))
            .map(|(code, value)| {
                format!(
                    "{code}: the doc states {:?}, the code {value}",
                    stated.get(code)
                )
            }),
    );
    wrong.extend(
        stated
            .keys()
            .filter(|code| !enforced.iter().any(|(checked, _)| checked == *code))
            .map(|code| format!("{code}: no bound of this test")),
    );
    if rows.len() != stated.len() {
        wrong.push("a constant is named by two rows".to_string());
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// Every numeric constant outside the tests under `dir`, by its path in the
/// crate: `items::video::MAX_UPLOADS`, `BODY_LIMIT` for `lib.rs`.
fn numeric_constants(dir: &std::path::Path) -> Vec<String> {
    const NUMERIC: [&str; 10] = [
        "u8",
        "u16",
        "u32",
        "u64",
        "usize",
        "i32",
        "i64",
        "f64",
        "Duration",
        "TimeDelta",
    ];
    let root = std::path::Path::new(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/builder/src"
    ));
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if path.file_name() != Some(std::ffi::OsStr::new("tests")) {
                found.extend(numeric_constants(&path));
            }
            continue;
        }
        if path.extension() != Some(std::ffi::OsStr::new("rs")) {
            continue;
        }
        let module = path
            .strip_prefix(root)
            .unwrap()
            .with_extension("")
            .to_string_lossy()
            .replace('/', "::");
        let module = module
            .strip_suffix("::mod")
            .or_else(|| module.strip_suffix("lib"))
            .unwrap_or(&module)
            .to_string();
        let source = std::fs::read_to_string(&path).unwrap();
        let code = source.split("#[cfg(test)]").next().unwrap();
        for line in code.lines() {
            let line = line.trim_start();
            let line = ["pub(crate) ", "pub "]
                .iter()
                .find_map(|vis| line.strip_prefix(vis))
                .unwrap_or(line);
            let Some((name, kind)) = line
                .strip_prefix("const ")
                .and_then(|rest| rest.split_once(": "))
            else {
                continue;
            };
            let kind = kind.split([' ', '=']).next().unwrap_or("");
            if NUMERIC.contains(&kind.rsplit("::").next().unwrap_or(kind)) {
                found.push(if module.is_empty() {
                    name.to_string()
                } else {
                    format!("{module}::{name}")
                });
            }
        }
    }
    found
}
