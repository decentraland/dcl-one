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
