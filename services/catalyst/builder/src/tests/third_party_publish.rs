//! `POST /v1/collections/:id/publish` of a third-party collection, against
//! the router as `serve` mounts it: a manager of the collection's provider
//! submits the items it names for curation with a slot cheque it signed;
//! every refusal writes nothing.

use std::sync::Arc;

use alloy::signers::SignerSync;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, header};
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use super::support::{FakeCommittee, FakeRegistry, registry_index, temp_storage, wallet};
use crate::shared::signed_fetch::test_support::signed_headers_by;

const MANAGER_KEY: &str = "0x8101010101010101010101010101010101010101010101010101010101010101";
const OTHER_KEY: &str = "0x8202020202020202020202020202020202020202020202020202020202020202";
const STRANGER_KEY: &str = "0x8303030303030303030303030303030303030303030303030303030303030303";
const COMMITTEE_KEY: &str = "0x8404040404040404040404040404040404040404040404040404040404040404";
const HOLDER_KEY: &str = "0x8505050505050505050505050505050505050505050505050505050505050505";

fn address(key: &str) -> String {
    format!("{:#x}", wallet(key).address())
}

struct World {
    pool: PgPool,
    app: Router,
    registry: FakeRegistry,
    committee: FakeCommittee,
    state: crate::SharedState,
}

async fn world() -> Option<World> {
    world_with(crate::Settings::default()).await
}

async fn world_with(settings: crate::Settings) -> Option<World> {
    let pool = crate::db::scratch::isolated_pool(8).await?;
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at, pass_started_at) \
         VALUES ('collections', 'matic', 'https://subgraph.example', timezone('UTC', now()), timezone('UTC', now()))",
    )
    .execute(&pool)
    .await
    .unwrap();
    let committee = FakeCommittee::with(&[&address(COMMITTEE_KEY)]);
    let registry = FakeRegistry::up();
    let mut state = crate::State::new(
        pool.clone(),
        temp_storage().await,
        Arc::default(),
        dcl_http::Outbound::for_tests(),
        Some(registry_index(&committee, &registry).await),
        None,
        crate::Settings {
            writes_enabled: true,
            ..settings
        },
    )
    .unwrap();
    state.guarded = dcl_http::Outbound::for_tests();
    let state = Arc::new(state);
    let app = Router::new()
        .merge(crate::root_router(state.clone()))
        .nest("/builder-api", crate::router(state.clone()));
    Some(World {
        pool,
        app,
        registry,
        committee,
        state,
    })
}

/// A provider the registry lists with `managers` and `max_items` slots.
fn listed(w: &World, managers: &[&str], max_items: &str) -> String {
    let id = format!(
        "urn:decentraland:matic:collections-thirdparty:p{}",
        Uuid::new_v4().simple()
    );
    w.registry.put(&id, managers, true, false);
    w.registry.set(&id, "maxItems", json!(max_items));
    id
}

/// A collection of `provider` with `items` items, each with a content hash.
async fn collection(w: &World, provider: &str, items: usize) -> (Uuid, Vec<Uuid>) {
    let id = Uuid::new_v4();
    let urn = format!("{provider}:c{}", id.simple());
    sqlx::query(
        "INSERT INTO builder.collections (id, name, eth_address, third_party_id, urn_suffix) \
         VALUES ($1, $2, '', $3, $4)",
    )
    .bind(id)
    .bind(format!("Linked {}", id.simple()))
    .bind(provider)
    .bind(&urn)
    .execute(&w.pool)
    .await
    .unwrap();
    let mut ids = Vec::new();
    for n in 0..items {
        ids.push(item(w, id, &format!("{urn}:i{n}"), Some(n)).await);
    }
    (id, ids)
}

async fn item(w: &World, collection: Uuid, urn: &str, hash: Option<usize>) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO builder.items (id, collection_id, eth_address, name, type, urn_suffix, local_content_hash) \
         VALUES ($1, $2, $3, 'Hat', 'wearable', $4, $5)",
    )
    .bind(id)
    .bind(collection)
    .bind(address(MANAGER_KEY))
    .bind(urn)
    .bind(hash.map(|n| format!("{n:064x}")))
    .execute(&w.pool)
    .await
    .unwrap();
    id
}

fn salt() -> String {
    format!("0x{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

/// The cheque `key` signs for `qty` slots of `provider` under `salt`.
fn cheque(key: &str, provider: &str, qty: i32, salt: &str) -> Value {
    let digest = crate::cheques::digest(
        &crate::network::POLYGON,
        provider,
        qty,
        salt.parse().unwrap(),
    );
    let signature = wallet(key)
        .sign_hash_sync(&alloy_primitives::B256::from(digest))
        .unwrap();
    json!({ "qty": qty, "salt": salt, "signature": format!("0x{}", hex::encode(signature.as_bytes())) })
}

async fn call(app: &Router, key: Option<&str>, path: String, body: Option<Value>) -> (u16, Value) {
    let mut request = Request::builder().method("POST").uri(&path);
    if let Some(key) = key {
        let now = Utc::now().timestamp_millis().to_string();
        for (name, value) in signed_headers_by(&wallet(key), "post", &path, "{}", true, &now)
            .await
            .iter()
        {
            request = request.header(name, value);
        }
    }
    let body = match body {
        Some(value) => {
            request = request.header(header::CONTENT_TYPE, "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), 64 << 20)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn publish(id: Uuid) -> String {
    format!("/v1/collections/{id}/publish")
}

async fn written(pool: &PgPool) -> Value {
    sqlx::query_scalar(
        "SELECT jsonb_build_object( \
           'cheques', (SELECT COALESCE(jsonb_agg(to_jsonb(s) ORDER BY s.id), '[]') FROM builder.slot_usage_cheques s), \
           'items', (SELECT COALESCE(jsonb_agg(to_jsonb(k) ORDER BY k.id), '[]') FROM builder.item_curations k), \
           'collections', (SELECT COALESCE(jsonb_agg(to_jsonb(c) ORDER BY c.id), '[]') FROM builder.collection_curations c), \
           'events', (SELECT count(*) FROM builder.collection_events))",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

/// A refusal case: its name, signer, body and expected status and error.
type Case<'a> = (&'a str, Option<&'a str>, Value, (u16, String));

fn error((status, body): (u16, Value)) -> (u16, String) {
    (status, body["error"].as_str().unwrap_or_default().into())
}

#[tokio::test]
async fn a_provider_manager_submits_the_named_items_with_its_cheque() {
    let Some(w) = world().await else { return };
    let manager = address(MANAGER_KEY);
    let provider = listed(&w, &[&manager], "10");
    let (id, items) = collection(&w, &provider, 3).await;
    let first_salt = salt();
    let mut offered = cheque(MANAGER_KEY, &provider, 2, &first_salt);
    let compact = {
        let full = hex::decode(&offered["signature"].as_str().unwrap()[2..]).unwrap();
        let mut c = full[..64].to_vec();
        c[32] |= (full[64] - 27) << 7;
        format!("0x{}", hex::encode(c))
    };
    let canonical = offered["signature"].as_str().unwrap().to_string();
    offered["signature"] = json!(compact);
    let (status, body) = call(
        &w.app,
        Some(MANAGER_KEY),
        publish(id),
        Some(json!({ "itemIds": [items[0], items[1]], "cheque": offered })),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["ok"], true);
    let data = &body["data"];
    assert_eq!(data["collection"]["id"], json!(id));
    assert_eq!(data["collection"]["is_published"], true);
    let published: Vec<&str> = data["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap())
        .collect();
    assert_eq!(published.len(), 2);
    for item in &items[..2] {
        assert!(published.contains(&item.to_string().as_str()));
    }
    assert!(
        data["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["is_published"] == true)
    );
    let curations = data["itemCurations"].as_array().unwrap();
    assert_eq!(curations.len(), 2);
    assert!(
        curations
            .iter()
            .any(|c| c["created_at"] == data["collection"]["created_at"]),
        "the collection's dates are its first curated item's curation: {data}"
    );
    for curation in curations {
        assert_eq!(curation["status"], "pending");
        assert_eq!(curation["is_mapping_complete"], false);
        let n = items
            .iter()
            .position(|i| curation["item_id"] == json!(i))
            .unwrap();
        assert_eq!(curation["content_hash"], json!(format!("{n:064x}")));
    }
    let stored: (String, String, i32, String, Uuid) = sqlx::query_as(
        "SELECT signature, salt, qty, third_party_id, collection_id FROM builder.slot_usage_cheques",
    )
    .fetch_one(&w.pool)
    .await
    .unwrap();
    assert_eq!(stored, (canonical, first_salt, 2, provider.clone(), id));
    let reviews: Vec<String> = sqlx::query_scalar(
        "SELECT status FROM builder.collection_curations WHERE collection_id = $1",
    )
    .bind(id)
    .fetch_all(&w.pool)
    .await
    .unwrap();
    assert_eq!(reviews, ["pending"]);

    let again = cheque(MANAGER_KEY, &provider, 1, &salt());
    assert_eq!(
        error(
            call(
                &w.app,
                Some(MANAGER_KEY),
                publish(id),
                Some(json!({ "itemIds": [items[2]], "cheque": again })),
            )
            .await
        ),
        (
            409,
            "The third party collection already has published items. It can't be updated.".into()
        )
    );
    sqlx::query("UPDATE builder.item_curations SET status = 'approved' WHERE item_id = ANY($1)")
        .bind(&items[..2])
        .execute(&w.pool)
        .await
        .unwrap();
    let (status, body) = call(
        &w.app,
        Some(MANAGER_KEY),
        format!("/builder-api{}", publish(id)),
        Some(json!({ "itemIds": [items[2]], "cheque": again })),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let counts: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM builder.slot_usage_cheques), \
                (SELECT count(*) FROM builder.collection_curations WHERE collection_id = $1), \
                (SELECT count(*) FROM builder.item_curations)",
    )
    .bind(id)
    .fetch_one(&w.pool)
    .await
    .unwrap();
    assert_eq!(counts, (2, 2, 3));
}

#[tokio::test]
async fn every_refusal_answers_before_anything_is_written() {
    let Some(w) = world().await else { return };
    let manager = address(MANAGER_KEY);
    let other = address(OTHER_KEY);
    let provider = listed(&w, &[&manager], "10");
    let elsewhere = provider_of_other(&w, &other);
    let (id, items) = collection(&w, &provider, 2).await;
    let (sibling, sibling_items) = collection(&w, &provider, 1).await;
    let hashless = item(&w, id, &format!("{provider}:c{}:none", id.simple()), None).await;
    let unuploaded = item(
        &w,
        id,
        &format!("{provider}:c{}:files", id.simple()),
        Some(7),
    )
    .await;
    let cid_hashed = item(&w, id, &format!("{provider}:c{}:cid", id.simple()), Some(8)).await;
    sqlx::query("UPDATE builder.items SET local_content_hash = $2 WHERE id = $1")
        .bind(cid_hashed)
        .bind(dcl_auth::hash_bytes_v1(b"not a keccak digest"))
        .execute(&w.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE builder.items SET name = 'Unuploaded' WHERE id = $1")
        .bind(unuploaded)
        .execute(&w.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO builder.item_contents (item_id, file, hash, named_by) VALUES ($1, 'model.glb', $2, $3)",
    )
    .bind(unuploaded)
    .bind(dcl_auth::hash_bytes_v1(b"never uploaded"))
    .bind(address(MANAGER_KEY))
    .execute(&w.pool)
    .await
    .unwrap();
    let unchecked = item(
        &w,
        id,
        &format!("{provider}:c{}:video", id.simple()),
        Some(9),
    )
    .await;
    let page = dcl_auth::hash_bytes_v1(b"<p>not a video</p>");
    sqlx::query(
        "WITH named AS (UPDATE builder.items SET name = 'Unchecked' WHERE id = $1) \
         INSERT INTO builder.item_contents (item_id, file, hash, named_by) VALUES ($1, 'video.mp4', $2, $3)",
    )
    .bind(unchecked)
    .bind(&page)
    .bind(address(MANAGER_KEY))
    .execute(&w.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO builder.uploaded_contents (hash, size, uploaded_by) VALUES ($1, 1, $2)",
    )
    .bind(&page)
    .bind(address(MANAGER_KEY))
    .execute(&w.pool)
    .await
    .unwrap();
    let lost = item(
        &w,
        id,
        &format!("{provider}:c{}:lost", id.simple()),
        Some(10),
    )
    .await;
    let gone = dcl_auth::hash_bytes_v1(b"recorded, never stored");
    sqlx::query(
        "WITH named AS (UPDATE builder.items SET name = 'Lost' WHERE id = $1), \
           recorded AS (INSERT INTO builder.uploaded_contents (hash, size, uploaded_by) VALUES ($2, 1, $3)) \
         INSERT INTO builder.item_contents (item_id, file, hash, named_by) VALUES ($1, 'model.glb', $2, $3)",
    )
    .bind(lost)
    .bind(&gone)
    .bind(address(MANAGER_KEY))
    .execute(&w.pool)
    .await
    .unwrap();
    let unreadable = item(
        &w,
        id,
        &format!("{provider}:c{}:unreadable", id.simple()),
        Some(11),
    )
    .await;
    let faulty = dcl_auth::hash_bytes_v1(b"a stored file the store can't read");
    tokio::fs::create_dir_all(
        crate::storage::resolve_file_path(w.state.items.blobs.root(), &faulty).unwrap(),
    )
    .await
    .unwrap();
    sqlx::query(
        "WITH recorded AS (INSERT INTO builder.uploaded_contents (hash, size, uploaded_by) VALUES ($2, 1, $3)) \
         INSERT INTO builder.item_contents (item_id, file, hash, named_by) VALUES ($1, 'model.glb', $2, $3)",
    )
    .bind(unreadable)
    .bind(&faulty)
    .bind(address(MANAGER_KEY))
    .execute(&w.pool)
    .await
    .unwrap();
    let used = salt();
    let (status, _) = call(
        &w.app,
        Some(MANAGER_KEY),
        publish(sibling),
        Some(
            json!({ "itemIds": sibling_items, "cheque": cheque(MANAGER_KEY, &provider, 1, &used) }),
        ),
    )
    .await;
    assert_eq!(status, 200);
    let before = written(&w.pool).await;
    let both = json!([items[0], items[1]]);
    let good = |qty: i32| cheque(MANAGER_KEY, &provider, qty, &salt());
    let unauthorized = format!("Unauthorized user {{}} for collections {id}");
    let invalid = "Tried to publish TP items with an invalid signed message or signature. Error: ";
    let mut high_s = good(2);
    {
        let sig = hex::decode(&high_s["signature"].as_str().unwrap()[2..]).unwrap();
        let n = alloy_primitives::U256::from_str_radix(
            "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141",
            16,
        )
        .unwrap();
        let mut raw = sig.clone();
        raw[32..64].copy_from_slice(
            &(n - alloy_primitives::U256::from_be_slice(&sig[32..64])).to_be_bytes::<32>(),
        );
        raw[64] = if sig[64] == 27 { 28 } else { 27 };
        high_s["signature"] = json!(format!("0x{}", hex::encode(raw)));
    }
    let cases: Vec<Case> = vec![
        (
            "no itemIds",
            Some(MANAGER_KEY),
            json!({ "cheque": good(2) }),
            (200, "Could not get itemIds from request".into()),
        ),
        (
            "no cheque",
            Some(MANAGER_KEY),
            json!({ "itemIds": both }),
            (200, "Could not get cheque from request".into()),
        ),
        (
            "a stranger",
            Some(STRANGER_KEY),
            json!({ "itemIds": both, "cheque": cheque(STRANGER_KEY, &provider, 2, &salt()) }),
            (401, unauthorized.replace("{}", &address(STRANGER_KEY))),
        ),
        (
            "the committee",
            Some(COMMITTEE_KEY),
            json!({ "itemIds": both, "cheque": cheque(COMMITTEE_KEY, &provider, 2, &salt()) }),
            (401, unauthorized.replace("{}", &address(COMMITTEE_KEY))),
        ),
        (
            "another provider's manager",
            Some(OTHER_KEY),
            json!({ "itemIds": both, "cheque": cheque(OTHER_KEY, &elsewhere, 2, &salt()) }),
            (401, unauthorized.replace("{}", &other)),
        ),
        (
            "more items than slots",
            Some(MANAGER_KEY),
            json!({ "itemIds": (0..10).map(|_| items[0]).collect::<Vec<_>>(), "cheque": good(1) }),
            (
                409,
                "The amount of items to publish exceeds the available slots.".into(),
            ),
        ),
        (
            "no item",
            Some(MANAGER_KEY),
            json!({ "itemIds": [Uuid::new_v4()], "cheque": good(1) }),
            (400, "Tried to publish no TP items".into()),
        ),
        (
            "an empty list",
            Some(MANAGER_KEY),
            json!({ "itemIds": [], "cheque": good(1) }),
            (400, "Tried to publish no TP items".into()),
        ),
        (
            "another quantity",
            Some(MANAGER_KEY),
            json!({ "itemIds": both, "cheque": good(1) }),
            (
                400,
                "The check quantity is different from the amount of published items".into(),
            ),
        ),
        (
            "another quantity, before any file is looked up",
            Some(MANAGER_KEY),
            json!({ "itemIds": [items[0], unreadable], "cheque": good(1) }),
            (
                400,
                "The check quantity is different from the amount of published items".into(),
            ),
        ),
        (
            "a quantity as text",
            Some(MANAGER_KEY),
            json!({ "itemIds": both, "cheque": { "qty": "2", "salt": salt(), "signature": good(2)["signature"] } }),
            (
                400,
                "The check quantity is different from the amount of published items".into(),
            ),
        ),
        (
            "a cheque another wallet signed",
            Some(MANAGER_KEY),
            json!({ "itemIds": both, "cheque": cheque(STRANGER_KEY, &provider, 2, &salt()) }),
            (400, format!("{invalid}Address missmatch")),
        ),
        (
            "a cheque for another provider",
            Some(MANAGER_KEY),
            json!({ "itemIds": both, "cheque": cheque(MANAGER_KEY, &elsewhere, 2, &salt()) }),
            (400, format!("{invalid}Address missmatch")),
        ),
        (
            "a cheque whose quantity was raised",
            Some(MANAGER_KEY),
            json!({ "itemIds": both, "cheque": { "qty": 2, "salt": good(1)["salt"], "signature": good(1)["signature"] } }),
            (400, format!("{invalid}Address missmatch")),
        ),
        (
            "a high-s signature",
            Some(MANAGER_KEY),
            json!({ "itemIds": both, "cheque": high_s }),
            (400, format!("{invalid}invalid signature")),
        ),
        (
            "a short salt",
            Some(MANAGER_KEY),
            json!({ "itemIds": both, "cheque": { "qty": 2, "salt": "0x12", "signature": good(2)["signature"] } }),
            (400, format!("{invalid}invalid salt")),
        ),
        (
            "an item of another collection",
            Some(MANAGER_KEY),
            json!({ "itemIds": [items[0], sibling_items[0]], "cheque": good(2) }),
            (
                400,
                "Cannot publish items that belong to different collections".into(),
            ),
        ),
        (
            "an item without a content hash",
            Some(MANAGER_KEY),
            json!({ "itemIds": [items[0], hashless], "cheque": good(2) }),
            (409, "The item has no content hash to publish".into()),
        ),
        (
            "an item whose content hash is not a keccak digest",
            Some(MANAGER_KEY),
            json!({ "itemIds": [items[0], cid_hashed], "cheque": good(2) }),
            (409, "The item has no content hash to publish".into()),
        ),
        (
            "more item ids than a publish takes",
            Some(MANAGER_KEY),
            json!({ "itemIds": (0..=20_000).map(|_| Uuid::new_v4()).collect::<Vec<_>>(), "cheque": good(1) }),
            (400, "Invalid request body".into()),
        ),
        (
            "an item naming a file its writer never uploaded",
            Some(MANAGER_KEY),
            json!({ "itemIds": [items[0], unuploaded], "cheque": good(2) }),
            (
                400,
                "Unuploaded: upload missing content before publishing".into(),
            ),
        ),
        (
            "an item naming a recorded file the store doesn't hold",
            Some(MANAGER_KEY),
            json!({ "itemIds": [items[0], lost], "cheque": good(2) }),
            (400, "Lost: upload missing content before publishing".into()),
        ),
        (
            "an item whose video the videos route never took",
            Some(MANAGER_KEY),
            json!({ "itemIds": [items[0], unchecked], "cheque": good(2) }),
            (
                400,
                "Unchecked: upload missing content before publishing".into(),
            ),
        ),
        (
            "a used salt",
            Some(MANAGER_KEY),
            json!({ "itemIds": both, "cheque": cheque(MANAGER_KEY, &provider, 2, &used) }),
            (
                400,
                "An error occurred trying to publish: the cheque was already used".into(),
            ),
        ),
        (
            "an id that is not a uuid",
            Some(MANAGER_KEY),
            json!({ "itemIds": ["hat"], "cheque": good(1) }),
            (400, "Invalid request body".into()),
        ),
    ];
    for (case, key, body, expected) in cases {
        assert_eq!(
            error(call(&w.app, key, publish(id), Some(body)).await),
            expected,
            "{case}"
        );
        assert_eq!(written(&w.pool).await, before, "{case}");
    }
    let (status, _) = call(&w.app, None, publish(id), Some(json!({ "itemIds": both }))).await;
    assert_eq!(status, 401);
    assert_eq!(written(&w.pool).await, before);
}

fn provider_of_other(w: &World, other: &str) -> String {
    listed(w, &[other], "10")
}

#[tokio::test]
async fn a_former_manager_and_a_virtual_holder_publish_nothing_and_the_registry_must_answer() {
    let Some(w) = world().await else { return };
    let manager = address(MANAGER_KEY);
    let holder = address(HOLDER_KEY);
    let provider = listed(&w, &[&manager], "10");
    let (id, items) = collection(&w, &provider, 1).await;
    let before = written(&w.pool).await;
    let body = |key: &str, provider: &str| json!({ "itemIds": items, "cheque": cheque(key, provider, 1, &salt()) });
    w.registry.set(&provider, "managers", json!([]));
    assert_eq!(
        call(
            &w.app,
            Some(MANAGER_KEY),
            publish(id),
            Some(body(MANAGER_KEY, &provider))
        )
        .await
        .0,
        401
    );
    let unlisted = format!(
        "urn:decentraland:matic:collections-thirdparty:v{}",
        Uuid::new_v4().simple()
    );
    sqlx::query(
        "INSERT INTO builder.virtual_third_parties (id, managers, raw_metadata) VALUES ($1, $2, 'tp:1:Virtual:')",
    )
    .bind(&unlisted)
    .bind(vec![holder.clone()])
    .execute(&w.pool)
    .await
    .unwrap();
    let (virtual_id, virtual_items) = collection(&w, &unlisted, 1).await;
    assert_eq!(
        error(
            call(
                &w.app,
                Some(HOLDER_KEY),
                publish(virtual_id),
                Some(json!({ "itemIds": virtual_items, "cheque": cheque(HOLDER_KEY, &unlisted, 1, &salt()) })),
            )
            .await
        ),
        (409, "The amount of items to publish exceeds the available slots.".into())
    );
    let listed = listed(&w, &[&manager], "10");
    sqlx::query(
        "INSERT INTO builder.virtual_third_parties (id, managers, raw_metadata) VALUES ($1, $2, 'tp:1:Virtual:')",
    )
    .bind(&listed)
    .bind(vec![holder.clone()])
    .execute(&w.pool)
    .await
    .unwrap();
    let (claimed, claimed_items) = collection(&w, &listed, 1).await;
    assert_eq!(
        call(
            &w.app,
            Some(HOLDER_KEY),
            publish(claimed),
            Some(json!({ "itemIds": claimed_items, "cheque": cheque(HOLDER_KEY, &listed, 1, &salt()) })),
        )
        .await
        .0,
        401
    );
    w.registry.set(&provider, "managers", json!([manager]));
    w.registry.fail(true);
    assert_eq!(
        call(
            &w.app,
            Some(MANAGER_KEY),
            publish(id),
            Some(body(MANAGER_KEY, &provider))
        )
        .await
        .0,
        503
    );
    assert_eq!(written(&w.pool).await, before);
}

#[tokio::test]
async fn concurrent_publishes_spend_each_slot_once() {
    let Some(w) = world().await else { return };
    let manager = address(MANAGER_KEY);
    let provider = listed(&w, &[&manager], "2");
    let (first, first_items) = collection(&w, &provider, 2).await;
    let (second, second_items) = collection(&w, &provider, 2).await;
    let (a, b) = tokio::join!(
        call(
            &w.app,
            Some(MANAGER_KEY),
            publish(first),
            Some(
                json!({ "itemIds": first_items, "cheque": cheque(MANAGER_KEY, &provider, 2, &salt()) })
            ),
        ),
        call(
            &w.app,
            Some(MANAGER_KEY),
            publish(second),
            Some(
                json!({ "itemIds": second_items, "cheque": cheque(MANAGER_KEY, &provider, 2, &salt()) })
            ),
        ),
    );
    let mut statuses = [a.0, b.0];
    statuses.sort();
    assert_eq!(statuses, [200, 409], "{a:?} {b:?}");
    let raised = listed(&w, &[&manager], "5");
    let (fourth, fourth_items) = collection(&w, &raised, 1).await;
    let (e, f) = tokio::join!(
        call(
            &w.app,
            Some(MANAGER_KEY),
            publish(fourth),
            Some(
                json!({ "itemIds": fourth_items, "cheque": cheque(MANAGER_KEY, &raised, 1, &salt()) })
            ),
        ),
        call(
            &w.app,
            Some(MANAGER_KEY),
            publish(fourth),
            Some(
                json!({ "itemIds": fourth_items, "cheque": cheque(MANAGER_KEY, &raised, 1, &salt()) })
            ),
        ),
    );
    let mut statuses = [e.0, f.0];
    statuses.sort();
    assert_eq!(statuses, [200, 409], "{e:?} {f:?}");
    let (fifth, fifth_items) = collection(&w, &raised, 1).await;
    let (sixth, sixth_items) = collection(&w, &raised, 1).await;
    let same = salt();
    let (g, h) = tokio::join!(
        call(
            &w.app,
            Some(MANAGER_KEY),
            publish(fifth),
            Some(
                json!({ "itemIds": fifth_items, "cheque": cheque(MANAGER_KEY, &raised, 1, &same) })
            ),
        ),
        call(
            &w.app,
            Some(MANAGER_KEY),
            publish(sixth),
            Some(
                json!({ "itemIds": sixth_items, "cheque": cheque(MANAGER_KEY, &raised, 1, &same) })
            ),
        ),
    );
    let mut statuses = [g.0, h.0];
    statuses.sort();
    assert_eq!(statuses, [200, 400], "{g:?} {h:?}");
    let curations: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM builder.collection_curations WHERE collection_id = $1",
    )
    .bind(fourth)
    .fetch_one(&w.pool)
    .await
    .unwrap();
    assert_eq!(curations, 1);
    let _ = &w.state;
}

/// A publish locks its collection, then the provider, then only its own
/// items: one naming another collection's item is refused without waiting
/// on that item, so it never holds locks in an order a delete of the other
/// collection reverses.
#[tokio::test]
async fn a_publish_naming_another_collections_item_never_waits_on_it() {
    let Some(w) = world().await else {
        return;
    };
    let manager = address(MANAGER_KEY);
    let provider = listed(&w, &[&manager], "10");
    let (id, items) = collection(&w, &provider, 1).await;
    let (_, others) = collection(&w, &provider, 1).await;
    let mut held = w.pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM builder.items WHERE id = $1 FOR UPDATE")
        .bind(others[0])
        .execute(&mut *held)
        .await
        .unwrap();
    let answer = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        call(
            &w.app,
            Some(MANAGER_KEY),
            publish(id),
            Some(json!({ "itemIds": [items[0], others[0]], "cheque": cheque(MANAGER_KEY, &provider, 2, &salt()) })),
        ),
    )
    .await;
    held.rollback().await.unwrap();
    let answer = answer.expect("the publish waited on another collection's item");
    assert_eq!(
        error(answer),
        (
            400,
            "Cannot publish items that belong to different collections".into()
        )
    );
}

#[derive(Clone, Default)]
struct Slack(Arc<std::sync::Mutex<Vec<String>>>);

impl Slack {
    async fn start(&self) -> String {
        let seen = self.clone();
        let app = Router::new().route(
            "/api/chat.postMessage",
            axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
                let seen = seen.clone();
                async move {
                    seen.0
                        .lock()
                        .unwrap()
                        .push(body["text"].as_str().unwrap_or_default().to_string());
                    axum::Json(json!({ "ok": true }))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}/api")
    }

    fn take(&self) -> Vec<String> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

#[tokio::test]
async fn the_curators_hear_of_each_publish_on_slack_and_in_one_forum_topic() {
    use crate::tests::forum_fake::FakeForum;
    let forum = FakeForum::default();
    let origin = forum.start().await;
    let slack = Slack::default();
    let api = slack.start().await;
    let Some(w) = world_with(crate::Settings {
        forum: Some(FakeForum::forum(&origin)),
        slack: Some(crate::curation::slack::Slack {
            token: "xoxb-example123".into(),
            channel: "curation-example".into(),
            api,
        }),
        links: crate::curation::slack::Links {
            creators_url: Some("https://creators.example.org".into()),
            builder_url: None,
        },
        builder_server_url: "https://builder-api.example.org".into(),
        ..crate::Settings::default()
    })
    .await
    else {
        return;
    };
    let manager = address(MANAGER_KEY);
    let provider = listed(&w, &[&manager], "10");
    let (id, items) = collection(&w, &provider, 3).await;
    let name = format!("Linked {}", id.simple());
    let (status, body) = call(
        &w.app,
        Some(MANAGER_KEY),
        publish(id),
        Some(json!({ "itemIds": [items[0], items[1]], "cheque": cheque(MANAGER_KEY, &provider, 2, &salt()) })),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    w.state.notices.settled().await;
    w.state.topics.settled().await;
    let short = format!("{}...{}", &manager[..6], &manager[manager.len() - 4..]);
    assert_eq!(
        slack.take(),
        [format!(
            "📦 New Third Party items ready for review: <https://creators.example.org/collections/editor?collection={id}&reviewing=true|{name}> by <https://creators.example.org/shop/items/creator/{manager}|{short}> (2 items)."
        )]
    );
    let calls = forum.take();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert_eq!(calls[0]["method"], "POST");
    assert_eq!(
        calls[0]["body"]["title"],
        json!(format!(
            "Third Party collection {name} with URN: {provider}"
        ))
    );
    let raw = calls[0]["body"]["raw"].as_str().unwrap().to_string();
    assert!(raw.starts_with(&format!("# {name}")), "{raw}");
    for item in &items[..2] {
        assert!(raw.contains(&format!("item={item}&")), "{raw}");
    }
    assert!(!raw.contains(&format!("item={}&", items[2])), "{raw}");
    let (link, post): (Option<String>, Option<i32>) =
        sqlx::query_as("SELECT forum_link, forum_id FROM builder.collections WHERE id = $1")
            .bind(id)
            .fetch_one(&w.pool)
            .await
            .unwrap();
    assert!(link.unwrap().starts_with(&format!("{origin}/t/")));
    let post = post.unwrap();

    sqlx::query("UPDATE builder.item_curations SET status = 'approved'")
        .execute(&w.pool)
        .await
        .unwrap();
    let (status, _) = call(
        &w.app,
        Some(MANAGER_KEY),
        publish(id),
        Some(
            json!({ "itemIds": [items[2]], "cheque": cheque(MANAGER_KEY, &provider, 1, &salt()) }),
        ),
    )
    .await;
    assert_eq!(status, 200);
    w.state.notices.settled().await;
    w.state.topics.settled().await;
    assert_eq!(slack.take().len(), 1);
    let calls = forum.take();
    assert_eq!(
        calls
            .iter()
            .map(|c| (c["method"].as_str().unwrap(), c["path"].as_str().unwrap()))
            .collect::<Vec<_>>(),
        [
            ("GET", format!("/posts/{post}.json").as_str()),
            ("PUT", format!("/posts/{post}.json").as_str())
        ]
    );
    let updated = calls[1]["body"]["raw"].as_str().unwrap();
    assert!(
        updated.starts_with(&format!("\n  {raw}\n  \n\n\n## New wearables updated at ")),
        "{updated}"
    );
    assert!(
        updated.contains(&format!("item={}&", items[2])),
        "{updated}"
    );

    let (other, other_items) = collection(&w, &provider, 1).await;
    forum.refuse(1);
    let (status, _) = call(
        &w.app,
        Some(MANAGER_KEY),
        publish(other),
        Some(
            json!({ "itemIds": other_items, "cheque": cheque(MANAGER_KEY, &provider, 1, &salt()) }),
        ),
    )
    .await;
    assert_eq!(status, 200);
    let curated: i64 =
        sqlx::query_scalar("SELECT count(*) FROM builder.item_curations WHERE item_id = $1")
            .bind(other_items[0])
            .fetch_one(&w.pool)
            .await
            .unwrap();
    assert_eq!(curated, 1);
}

async fn get(app: &Router, key: Option<&str>, path: String) -> (u16, Value) {
    let mut request = Request::builder().method("GET").uri(&path);
    if let Some(key) = key {
        let now = Utc::now().timestamp_millis().to_string();
        for (name, value) in signed_headers_by(&wallet(key), "get", &path, "{}", true, &now)
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
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), 64 << 20)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn approval(id: impl std::fmt::Display) -> String {
    format!("/v1/collections/{id}/approvalData")
}

#[tokio::test]
async fn the_committee_reads_what_its_registry_transaction_needs() {
    let Some(w) = world().await else { return };
    let manager = address(MANAGER_KEY);
    let provider = listed(&w, &[&manager], "10");
    let (id, items) = collection(&w, &provider, 3).await;
    let first = cheque(MANAGER_KEY, &provider, 2, &salt());
    let (status, _) = call(
        &w.app,
        Some(MANAGER_KEY),
        publish(id),
        Some(json!({ "itemIds": [items[0], items[1]], "cheque": first })),
    )
    .await;
    assert_eq!(status, 200);
    let (status, body) = get(&w.app, Some(COMMITTEE_KEY), approval(id)).await;
    assert_eq!(status, 200, "{body}");
    let mut hashes = serde_json::Map::new();
    for (n, item) in items[..2].iter().enumerate() {
        hashes.insert(item.to_string(), json!(format!("{n:064x}")));
    }
    assert_eq!(
        body["data"],
        json!({
            "cheque": first,
            "root": format!("0x{}", "ab".repeat(32)),
            "content_hashes": hashes,
            "chequeWasConsumed": false,
        })
    );
    let digest = crate::cheques::digest(
        &crate::network::POLYGON,
        &provider,
        2,
        first["salt"].as_str().unwrap().parse().unwrap(),
    );
    w.registry.consume(&format!("0x{}", hex::encode(digest)));
    let (_, body) = get(
        &w.app,
        Some(COMMITTEE_KEY),
        format!("/builder-api{}", approval(id)),
    )
    .await;
    assert_eq!(body["data"]["chequeWasConsumed"], true);

    sqlx::query("UPDATE builder.item_curations SET status = 'approved'")
        .execute(&w.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE builder.items SET local_content_hash = $2 WHERE id = $1")
        .bind(items[0])
        .bind("f".repeat(64))
        .execute(&w.pool)
        .await
        .unwrap();
    let second = cheque(MANAGER_KEY, &provider, 2, &salt());
    let (status, _) = call(
        &w.app,
        Some(MANAGER_KEY),
        publish(id),
        Some(json!({ "itemIds": [items[0], items[2]], "cheque": second })),
    )
    .await;
    assert_eq!(status, 200);
    let (_, body) = get(&w.app, Some(COMMITTEE_KEY), approval(id)).await;
    assert_eq!(body["data"]["cheque"], second);
    assert_eq!(body["data"]["chequeWasConsumed"], false);
    assert_eq!(
        body["data"]["content_hashes"][items[0].to_string()],
        json!("f".repeat(64))
    );
    assert_eq!(
        body["data"]["content_hashes"][items[2].to_string()],
        json!(format!("{:064x}", 2))
    );
    assert_eq!(body["data"]["content_hashes"].as_object().unwrap().len(), 3);
}

#[tokio::test]
async fn approval_data_is_the_committee_s_and_needs_its_sources() {
    let Some(w) = world().await else { return };
    let manager = address(MANAGER_KEY);
    let provider = listed(&w, &[&manager], "10");
    let (id, items) = collection(&w, &provider, 1).await;
    let (unpublished, _) = collection(&w, &provider, 1).await;
    let standard = Uuid::new_v4();
    sqlx::query("INSERT INTO builder.collections (id, name, eth_address) VALUES ($1, $2, $3)")
        .bind(standard)
        .bind(format!("Standard {}", standard.simple()))
        .bind(&manager)
        .execute(&w.pool)
        .await
        .unwrap();
    let (status, _) = call(
        &w.app,
        Some(MANAGER_KEY),
        publish(id),
        Some(json!({ "itemIds": items, "cheque": cheque(MANAGER_KEY, &provider, 1, &salt()) })),
    )
    .await;
    assert_eq!(status, 200);
    let missing = Uuid::new_v4();
    for (key, path, expected, data) in [
        (
            Some(MANAGER_KEY),
            approval(id),
            (401, "Unauthorized".to_string()),
            json!({ "eth_address": manager }),
        ),
        (
            Some(COMMITTEE_KEY),
            approval(missing),
            (404, format!("Couldn't find \"{missing}\" on collections")),
            json!({ "id": missing, "tableName": "collections" }),
        ),
        (
            Some(COMMITTEE_KEY),
            approval("hat"),
            (400, "Invalid uuid hat".to_string()),
            json!({ "id": "hat" }),
        ),
        (
            Some(COMMITTEE_KEY),
            approval(standard),
            (409, "Collection is not Third Party".to_string()),
            json!({ "id": standard }),
        ),
        (
            Some(COMMITTEE_KEY),
            approval(unpublished),
            (401, "The collection is not published.".to_string()),
            json!({ "id": unpublished }),
        ),
    ] {
        let answer = get(&w.app, key, path.clone()).await;
        assert_eq!(error(answer.clone()), expected, "{path}");
        assert_eq!(answer.1["data"], data, "{path}");
    }
    assert_eq!(get(&w.app, None, approval(id)).await.0, 401);
    w.registry.fail_receipts(true);
    assert_eq!(get(&w.app, Some(COMMITTEE_KEY), approval(id)).await.0, 503);
    w.registry.fail_receipts(false);
    assert_eq!(get(&w.app, Some(COMMITTEE_KEY), approval(id)).await.0, 200);
    w.registry.fail(true);
    assert_eq!(get(&w.app, Some(COMMITTEE_KEY), approval(id)).await.0, 503);
    w.registry.fail(false);
    w.committee.set(None);
    w.state.committee.outdate();
    assert_eq!(get(&w.app, Some(COMMITTEE_KEY), approval(id)).await.0, 503);
}

#[tokio::test]
async fn the_publish_decides_again_inside_its_transaction() {
    let Some(w) = world().await else { return };
    let manager = address(MANAGER_KEY);
    let provider = listed(&w, &[&manager], "10");
    let (id, items) = collection(&w, &provider, 1).await;
    let before = written(&w.pool).await;
    let stranger = address(STRANGER_KEY);
    let grant = crate::access::granted::third_party_publish(&stranger, id, &provider);
    let request = crate::collections::publish_third_party::parse(
        json!({ "itemIds": items, "cheque": cheque(STRANGER_KEY, &provider, 1, &salt()) })
            .to_string()
            .as_bytes(),
    )
    .unwrap();
    let refused = crate::collections::publish_third_party::publish(&w.state, &grant, &request)
        .await
        .unwrap_err();
    assert_eq!(
        (refused.status(), refused.to_string().as_str()),
        (401, "Unauthorized to upsert collection")
    );
    assert_eq!(written(&w.pool).await, before);
}

async fn send(app: &Router, key: &str, method: &str, path: &str, body: Value) -> (u16, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    let now = Utc::now().timestamp_millis().to_string();
    for (name, value) in
        signed_headers_by(&wallet(key), &method.to_lowercase(), path, "{}", true, &now)
            .await
            .iter()
    {
        request = request.header(name, value);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), 64 << 20)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// The manager's save of `item` of `collection` at `urn` with a model of
/// `model`'s bytes; answers the item's new content hash.
async fn edit(w: &World, collection: Uuid, item: Uuid, urn: &str, model: &str) -> String {
    let main = dcl_auth::hash_bytes_v1(model.as_bytes());
    let body = json!({ "item": {
        "id": item, "name": "Hat", "description": null, "type": "wearable",
        "eth_address": address(MANAGER_KEY), "collection_id": collection, "urn": urn,
        "contents": { "model.glb": main },
        "metrics": { "meshes": 1, "bodies": 1, "materials": 1, "textures": 1, "triangles": 1, "entities": 1 },
        "data": {
            "category": "hat", "replaces": [], "hides": [], "tags": [],
            "representations": [{
                "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
                "mainFile": "model.glb", "contents": ["model.glb"], "overrideHides": [], "overrideReplaces": []
            }]
        }
    } });
    let (status, answer) = send(
        &w.app,
        MANAGER_KEY,
        "PUT",
        &format!("/v1/items/{item}"),
        body,
    )
    .await;
    assert_eq!(status, 200, "{answer}");
    sqlx::query_scalar("SELECT local_content_hash FROM builder.items WHERE id = $1")
        .bind(item)
        .fetch_one(&w.pool)
        .await
        .unwrap()
}

async fn submitted(w: &World) -> (Uuid, Uuid, String) {
    let manager = address(MANAGER_KEY);
    let provider = listed(w, &[&manager], "10");
    let (id, items) = collection(w, &provider, 1).await;
    let (status, answer) = call(
        &w.app,
        Some(MANAGER_KEY),
        publish(id),
        Some(json!({ "itemIds": items, "cheque": cheque(MANAGER_KEY, &provider, 1, &salt()) })),
    )
    .await;
    assert_eq!(status, 200, "{answer}");
    let urn: String = sqlx::query_scalar("SELECT urn_suffix FROM builder.items WHERE id = $1")
        .bind(items[0])
        .fetch_one(&w.pool)
        .await
        .unwrap();
    (id, items[0], urn)
}

/// A save through the node moves the pending curation to the saved content,
/// so approval data answers it, as builder-server does; a pending curation
/// whose content the item no longer holds, which only rows written outside
/// the node's saves (an import) can leave, is refused by name.
#[tokio::test]
async fn approval_data_refuses_an_item_whose_content_left_its_pending_curation() {
    let Some(w) = world().await else { return };
    let (id, item, urn) = submitted(&w).await;
    let edited = edit(&w, id, item, &urn, "saved after submitting").await;
    let (status, body) = get(&w.app, Some(COMMITTEE_KEY), approval(id)).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        body["data"]["content_hashes"][item.to_string()],
        json!(edited)
    );
    sqlx::query("UPDATE builder.items SET local_content_hash = $2 WHERE id = $1")
        .bind(item)
        .bind("e".repeat(64))
        .execute(&w.pool)
        .await
        .unwrap();
    let (status, body) = get(&w.app, Some(COMMITTEE_KEY), approval(id)).await;
    assert_eq!(
        (status, &body["data"]),
        (409, &json!({ "id": id, "itemIds": [item] })),
        "{body}"
    );
}

async fn save_hat(w: &World, collection: Uuid, item: Option<Uuid>, urn: &str) -> (u16, Value) {
    let main = dcl_auth::hash_bytes_v1(urn.as_bytes());
    let mut fields = json!({
        "name": "Hat", "description": null, "type": "wearable",
        "eth_address": address(MANAGER_KEY), "collection_id": collection, "urn": urn,
        "contents": { "model.glb": main },
        "metrics": { "meshes": 1, "bodies": 1, "materials": 1, "textures": 1, "triangles": 1, "entities": 1 },
        "data": {
            "category": "hat", "replaces": [], "hides": [], "tags": [],
            "representations": [{
                "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
                "mainFile": "model.glb", "contents": ["model.glb"], "overrideHides": [], "overrideReplaces": []
            }]
        }
    });
    let path = match item {
        Some(item) => {
            fields["id"] = json!(item);
            item.to_string()
        }
        None => urn.to_string(),
    };
    send(
        &w.app,
        MANAGER_KEY,
        "PUT",
        &format!("/v1/items/{path}"),
        json!({ "item": fields }),
    )
    .await
}

/// A linked collection renamed before anything was submitted takes its
/// items' URNs with it: each answers and is found at the new URN, and the
/// old one can't be claimed by an item of another collection while the
/// provider already holds that item.
#[tokio::test]
async fn a_renamed_linked_collection_takes_its_items_urns_with_it() {
    let Some(w) = world().await else { return };
    w.state.content_sync.set_own_content_authority(true);
    let manager = address(MANAGER_KEY);
    let provider = listed(&w, &[&manager], "10");
    let (a, _) = collection(&w, &provider, 0).await;
    let old: String =
        sqlx::query_scalar("SELECT urn_suffix FROM builder.collections WHERE id = $1")
            .bind(a)
            .fetch_one(&w.pool)
            .await
            .unwrap();
    let hat = Uuid::new_v4();
    let (status, answer) = save_hat(&w, a, Some(hat), &format!("{old}:hat")).await;
    assert_eq!(status, 200, "{answer}");
    let renamed = format!("{provider}:renamed");
    let rename = |id: Uuid, name: &str, urn: &str| json!({ "collection": { "id": id, "name": name, "urn": urn, "eth_address": manager, "salt": null, "contract_address": null } });
    let (status, answer) = send(
        &w.app,
        MANAGER_KEY,
        "PUT",
        &format!("/v1/collections/{a}"),
        rename(a, "Renamed", &renamed),
    )
    .await;
    assert_eq!(status, 200, "{answer}");
    let (status, body) = get(&w.app, Some(MANAGER_KEY), format!("/v1/items/{hat}")).await;
    assert_eq!(
        (status, &body["data"]["urn"]),
        (200, &json!(format!("{renamed}:hat"))),
        "{body}"
    );
    let (status, answer) = save_hat(&w, a, None, &format!("{renamed}:hat")).await;
    assert_eq!(
        (status, &answer["data"]["id"]),
        (200, &json!(hat)),
        "{answer}"
    );
    let b = Uuid::new_v4();
    let (status, answer) = send(
        &w.app,
        MANAGER_KEY,
        "PUT",
        &format!("/v1/collections/{b}"),
        rename(b, "Takes the old urn", &old),
    )
    .await;
    assert_eq!(status, 200, "{answer}");
    let (status, answer) = save_hat(&w, b, Some(Uuid::new_v4()), &format!("{old}:hat")).await;
    assert_eq!(status, 409, "{answer}");
    let holders: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM builder.items WHERE lower(urn_suffix) = lower($1)",
    )
    .bind(format!("{old}:hat"))
    .fetch_one(&w.pool)
    .await
    .unwrap();
    assert_eq!(holders, 0);
}

/// A rename that would move an item onto a URN the node's content already
/// holds is refused, and the items keep their URNs.
#[tokio::test]
async fn a_rename_never_moves_an_item_onto_a_urn_the_content_holds() {
    let Some(w) = world().await else { return };
    w.state.content_sync.set_own_content_authority(true);
    let manager = address(MANAGER_KEY);
    let provider = listed(&w, &[&manager], "10");
    let (a, _) = collection(&w, &provider, 0).await;
    let old: String =
        sqlx::query_scalar("SELECT urn_suffix FROM builder.collections WHERE id = $1")
            .bind(a)
            .fetch_one(&w.pool)
            .await
            .unwrap();
    let hat = Uuid::new_v4();
    let (status, answer) = save_hat(&w, a, Some(hat), &format!("{old}:hat")).await;
    assert_eq!(status, 200, "{answer}");
    let renamed = format!("{provider}:taken{}", Uuid::new_v4().simple());
    let entity = format!("bafkentity{}", Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, \
           entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
         VALUES ('0x0', 'v3', 'wearable', $1, '{\"v\":{}}'::json, timezone('UTC', now()), ARRAY[$2], \
           timezone('UTC', now()), '[]'::json)",
    )
    .bind(&entity)
    .bind(format!("{renamed}:hat"))
    .execute(&w.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO content.active_pointers (pointer, entity_id, entity_type) VALUES ($1, $2, 'wearable')",
    )
    .bind(format!("{renamed}:hat"))
    .bind(&entity)
    .execute(&w.pool)
    .await
    .unwrap();
    let (status, answer) = send(
        &w.app,
        MANAGER_KEY,
        "PUT",
        &format!("/v1/collections/{a}"),
        json!({ "collection": { "id": a, "name": "Taken", "urn": renamed, "eth_address": manager, "salt": null, "contract_address": null } }),
    )
    .await;
    let urn: String = sqlx::query_scalar("SELECT urn_suffix FROM builder.items WHERE id = $1")
        .bind(hat)
        .fetch_one(&w.pool)
        .await
        .unwrap();
    assert_eq!((status, urn), (409, format!("{old}:hat")), "{answer}");
    w.state.content_sync.set_own_content_authority(false);
    let free = format!("{provider}:free{}", Uuid::new_v4().simple());
    let (status, answer) = send(
        &w.app,
        MANAGER_KEY,
        "PUT",
        &format!("/v1/collections/{a}"),
        json!({ "collection": { "id": a, "name": "Free", "urn": free, "eth_address": manager, "salt": null, "contract_address": null } }),
    )
    .await;
    let urn: String = sqlx::query_scalar("SELECT urn_suffix FROM builder.items WHERE id = $1")
        .bind(hat)
        .fetch_one(&w.pool)
        .await
        .unwrap();
    assert_eq!(
        (status, urn),
        (503, format!("{old}:hat")),
        "a rename the content can't vouch for moves nothing: {answer}"
    );
}
