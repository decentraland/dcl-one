//! A collection's lifecycle routes as builder-server answers them, against
//! the router as `serve` mounts it: the lock and what it refuses for a day,
//! the delete, the owner sync, the terms of service and the standard
//! publish, with the chain facts from the copy or the Polygon RPC.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use super::support::{
    FakeCommittee, FakeOracle, OWNER_KEY, address, call, committee_index, contents_of, own_pool,
    request, save_collection, saved_item, unique, upload,
};

const STRANGER_KEY: &str = "0x2222222222222222222222222222222222222222222222222222222222222222";
const COMMITTEE_KEY: &str = "0x3333333333333333333333333333333333333333333333333333333333333333";
const MANAGER_KEY: &str = "0x4444444444444444444444444444444444444444444444444444444444444444";
const CREATOR_KEY: &str = "0x5555555555555555555555555555555555555555555555555555555555555555";
const PROVIDER: &str = "urn:decentraland:matic:collections-thirdparty:lifecycle";

/// A collection name unique to `id`.
fn short(label: &str, id: Uuid) -> String {
    format!("{label} {}", &id.simple().to_string()[..12])
}

fn at(iso: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(iso)
        .unwrap()
        .with_timezone(&Utc)
}

fn locked_at() -> DateTime<Utc> {
    at("2026-03-01T00:00:00.123Z")
}

fn a_day_less_a_millisecond_later() -> DateTime<Utc> {
    at("2026-03-02T00:00:00.122Z")
}

fn a_day_later() -> DateTime<Utc> {
    at("2026-03-02T00:00:00.123Z")
}

struct Node {
    pool: PgPool,
    app: Router,
    state: crate::SharedState,
}

struct Options {
    rpc: Option<String>,
    committee: Option<FakeCommittee>,
    now: fn() -> DateTime<Utc>,
    warehouse: Option<String>,
    writes: bool,
    network: crate::network::Network,
    tune: fn(&mut crate::State),
}

impl Default for Options {
    fn default() -> Self {
        Self {
            rpc: None,
            committee: None,
            now: Utc::now,
            warehouse: None,
            writes: true,
            network: crate::network::POLYGON,
            tune: |_| {},
        }
    }
}

async fn node_over(pool: PgPool, options: Options) -> Node {
    sqlx::query(
        "INSERT INTO builder.virtual_third_parties (id, managers, raw_metadata) VALUES ($1, $2, 'tp:1:Lifecycle:') \
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(PROVIDER)
    .bind(vec![address(OWNER_KEY)])
    .execute(&pool)
    .await
    .unwrap();
    let market = match &options.committee {
        Some(committee) => Some(committee_index(committee).await),
        None => Some(super::support::registry_only_index().await),
    };
    let mut state = super::support::state(
        pool.clone(),
        market,
        crate::Settings {
            network: options.network,
            writes_enabled: options.writes,
            polygon_rpc_url: options.rpc,
            warehouse: options.warehouse.map(|url| crate::WarehouseForward {
                url,
                token: "example-warehouse-token".into(),
                context_prefix: None,
            }),
            ..crate::Settings::default()
        },
    )
    .await;
    state.items.clock.now = options.now;
    (options.tune)(&mut state);
    let state = Arc::new(state);
    let app = super::support::mounted(&state);
    Node { pool, app, state }
}

async fn node(options: Options) -> Option<Node> {
    Some(node_over(super::support::scratch_pool(8).await?, options).await)
}

/// A standard draft of the owner's, with `items` items created in order, each
/// naming a file of its own that the owner uploaded.
async fn draft(node: &Node, items: usize) -> (Uuid, Vec<Uuid>, Vec<String>) {
    let id = Uuid::new_v4();
    let owner = address(OWNER_KEY);
    save_collection(
        &node.state.items,
        &owner,
        json!({ "id": id, "name": short("Lifecycle", id) }),
    )
    .await
    .unwrap();
    let mut ids = Vec::new();
    let mut hashes = Vec::new();
    for _ in 0..items {
        let item = Uuid::new_v4();
        let bytes = unique("lifecycle");
        let contents = contents_of(&[("model.glb", &bytes)]);
        hashes.push(contents["model.glb"].as_str().unwrap().to_string());
        saved_item(
            &node.state.items,
            &owner,
            &json!({ "id": item, "name": "Item", "collection_id": id, "rarity": "common", "contents": contents }),
        )
        .await;
        upload(&node.state.items, &owner, item, &[&bytes])
            .await
            .unwrap();
        ids.push(item);
    }
    (id, ids, hashes)
}

async fn contract_of(pool: &PgPool, id: Uuid) -> String {
    sqlx::query_scalar("SELECT contract_address FROM builder.collections WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// The copy shows the collection's stored contract on chain, created by
/// `creator`, with `managers`, and its on-chain items `item_ids`.
async fn copy_holds(
    pool: &PgPool,
    contract: &str,
    creator: &str,
    managers: &[String],
    item_ids: &[u64],
) {
    sqlx::query(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, \
           is_completed, is_approved, is_editable, managers, minters, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ('matic', $1, 137, 'Lifecycle', $2, $2, 'urn:lifecycle', true, false, true, $3, '{}', 1, 1, 1, timezone('UTC', now()))",
    )
    .bind(contract)
    .bind(creator)
    .bind(managers)
    .execute(pool)
    .await
    .unwrap();
    for item in item_ids {
        sqlx::query(
            "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, \
               total_supply, max_supply, available, price, name, category, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('matic', $1, $2::numeric, 137, 'wearable_v2', $3, $4, 'common', 0, 100000, 100000, 0, 'Item', 'hat', \
               1, 1, 1, timezone('UTC', now()))",
        )
        .bind(contract)
        .bind(item.to_string())
        .bind(format!("urn:decentraland:matic:collections-v2:{contract}:{item}"))
        .bind(creator)
        .execute(pool)
        .await
        .unwrap();
    }
}

async fn chain_ids(pool: &PgPool, collection: Uuid) -> Vec<Option<String>> {
    sqlx::query_scalar(
        "SELECT blockchain_item_id FROM builder.items WHERE collection_id = $1 ORDER BY created_at, id",
    )
    .bind(collection)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn stored_lock(pool: &PgPool, id: Uuid) -> Option<chrono::NaiveDateTime> {
    sqlx::query_scalar("SELECT lock FROM builder.collections WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn collection_save(id: Uuid, name: &str) -> Value {
    super::support::collection_body(json!({ "id": id, "name": name }), &address(OWNER_KEY))
}

fn item_save(id: Uuid, collection: Uuid) -> Value {
    json!({ "item": {
        "id": id, "name": "Item", "description": "", "type": "wearable", "eth_address": address(OWNER_KEY),
        "rarity": "common", "collection_id": collection,
        "contents": { "model.glb": "bafkreigh2akiscaildcqabsyg3dfr6chu3fgpregiymsck7e7aqa4s52zy" },
        "metrics": { "meshes": 1, "bodies": 1, "materials": 1, "textures": 1, "triangles": 1, "entities": 1 },
        "data": { "category": "hat", "replaces": [], "hides": [], "tags": [], "representations": [{
            "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"], "mainFile": "model.glb",
            "contents": ["model.glb"], "overrideHides": [], "overrideReplaces": [] }] }
    } })
}

fn locked(error: &str) -> (u16, String) {
    (423, error.to_string())
}

fn answer((status, body): (u16, Value)) -> (u16, String) {
    (
        status,
        body["error"].as_str().unwrap_or_default().to_string(),
    )
}

#[tokio::test]
async fn the_owner_locks_a_standard_collection_and_its_body_answers_the_lock() {
    let Some(node) = node(Options {
        now: locked_at,
        ..Options::default()
    })
    .await
    else {
        return;
    };
    let (id, _, _) = draft(&node, 0).await;
    let path = format!("/v1/collections/{id}/lock");
    let (status, body) = call(
        &node.app,
        Some(OWNER_KEY),
        "POST",
        &path,
        Some(json!({ "collection_address": id })),
    )
    .await;
    assert_eq!(
        (status, body.clone()),
        (
            200,
            json!({ "ok": true, "data": "2026-03-01T00:00:00.123Z" })
        ),
        "{body}"
    );
    assert_eq!(
        stored_lock(&node.pool, id).await,
        Some(locked_at().naive_utc())
    );
    let (_, read) = call(
        &node.app,
        Some(OWNER_KEY),
        "GET",
        &format!("/v1/collections/{id}"),
        None,
    )
    .await;
    assert_eq!(read["data"]["lock"], "2026-03-01T00:00:00.123Z");
    let (status, again) = call(
        &node.app,
        Some(OWNER_KEY),
        "POST",
        &format!("/builder-api{path}"),
        None,
    )
    .await;
    assert_eq!(
        (status, again["data"].clone()),
        (200, json!("2026-03-01T00:00:00.123Z"))
    );
}

#[tokio::test]
async fn only_the_owner_of_a_standard_collection_sets_its_lock() {
    let committee = FakeCommittee::with(&[&address(COMMITTEE_KEY)]);
    let Some(node) = node(Options {
        committee: Some(committee),
        now: locked_at,
        ..Options::default()
    })
    .await
    else {
        return;
    };
    let (published, _, _) = draft(&node, 0).await;
    let contract = contract_of(&node.pool, published).await;
    copy_holds(
        &node.pool,
        &contract,
        &address(OWNER_KEY),
        &[address(MANAGER_KEY)],
        &[],
    )
    .await;
    let third_party = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO builder.collections (id, name, eth_address, third_party_id, urn_suffix) VALUES ($1, 'TP', $2, $3, $4)",
    )
    .bind(third_party)
    .bind(address(OWNER_KEY))
    .bind(PROVIDER)
    .bind(format!("{PROVIDER}:{}", third_party.simple()))
    .execute(&node.pool)
    .await
    .unwrap();
    for (key, id) in [
        (MANAGER_KEY, published),
        (COMMITTEE_KEY, published),
        (STRANGER_KEY, published),
        (OWNER_KEY, third_party),
    ] {
        let refused = call(
            &node.app,
            Some(key),
            "POST",
            &format!("/v1/collections/{id}/lock"),
            None,
        )
        .await;
        assert_eq!(
            answer(refused),
            (
                403,
                format!("Unauthorized user {} for collections {id}", address(key))
            ),
            "{key} on {id}"
        );
        assert_eq!(stored_lock(&node.pool, id).await, None);
    }
    let (status, _) = call(
        &node.app,
        Some(OWNER_KEY),
        "POST",
        &format!("/v1/collections/{published}/lock"),
        None,
    )
    .await;
    assert_eq!(status, 200, "the owner locks a published collection too");
    let (status, body) = call(
        &node.app,
        None,
        "POST",
        &format!("/v1/collections/{published}/lock"),
        None,
    )
    .await;
    assert_eq!(
        (status, body["error"].clone()),
        (401, json!("Unauthenticated"))
    );
    let missing = Uuid::new_v4();
    let (status, _) = call(
        &node.app,
        Some(OWNER_KEY),
        "POST",
        &format!("/v1/collections/{missing}/lock"),
        None,
    )
    .await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn a_lock_refuses_saves_creations_and_deletes_for_a_day_and_never_uploads() {
    let Some(pool) = super::support::scratch_pool(8).await else {
        return;
    };
    let locking = node_over(
        pool.clone(),
        Options {
            now: locked_at,
            ..Options::default()
        },
    )
    .await;
    let (id, items, _) = draft(&locking, 1).await;
    let (status, _) = call(
        &locking.app,
        Some(OWNER_KEY),
        "POST",
        &format!("/v1/collections/{id}/lock"),
        None,
    )
    .await;
    assert_eq!(status, 200);
    let within = node_over(
        pool.clone(),
        Options {
            now: a_day_less_a_millisecond_later,
            ..Options::default()
        },
    )
    .await;
    let item = items[0];
    let created = Uuid::new_v4();
    assert_eq!(
        answer(
            call(
                &within.app,
                Some(OWNER_KEY),
                "PUT",
                &format!("/v1/items/{item}"),
                Some(item_save(item, id))
            )
            .await
        ),
        locked("The collection for the item is locked. The item can't be inserted or updated.")
    );
    assert_eq!(
        answer(
            call(
                &within.app,
                Some(OWNER_KEY),
                "PUT",
                &format!("/v1/items/{created}"),
                Some(item_save(created, id))
            )
            .await
        ),
        locked("The collection for the item is locked. The item can't be inserted or updated.")
    );
    let (_, moved, _) = draft(&locking, 1).await;
    assert_eq!(
        answer(
            call(
                &within.app,
                Some(OWNER_KEY),
                "PUT",
                &format!("/v1/items/{}", moved[0]),
                Some(item_save(moved[0], id))
            )
            .await
        ),
        locked("The collection for the item is locked. The item can't be inserted or updated."),
        "an item of another collection can't move into a locked one"
    );
    assert_eq!(
        answer(
            call(
                &within.app,
                Some(OWNER_KEY),
                "DELETE",
                &format!("/v1/items/{item}"),
                None
            )
            .await
        ),
        locked("The collection for the item is locked. The item can't be deleted.")
    );
    assert_eq!(
        answer(
            call(
                &within.app,
                Some(OWNER_KEY),
                "PUT",
                &format!("/v1/collections/{id}"),
                Some(collection_save(id, &short("Renamed", id)))
            )
            .await
        ),
        locked("The collection is locked. It can't be updated.")
    );
    assert_eq!(
        answer(
            call(
                &within.app,
                Some(OWNER_KEY),
                "DELETE",
                &format!("/v1/collections/{id}"),
                None
            )
            .await
        ),
        locked("The collection is locked. It can't be deleted.")
    );
    let bytes = unique("lifecycle upload");
    let hash = dcl_auth::hash_bytes_v1(&bytes);
    sqlx::query("UPDATE builder.item_contents SET hash = $2 WHERE item_id = $1")
        .bind(item)
        .bind(&hash)
        .execute(&pool)
        .await
        .unwrap();
    let boundary = "lifecycle-boundary";
    let upload = request(Some(OWNER_KEY), "POST", &format!("/v1/items/{item}/files"))
        .await
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{hash}\"; filename=\"blob\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n{}\r\n--{boundary}--\r\n",
            String::from_utf8_lossy(&bytes)
        )))
        .unwrap();
    assert_eq!(
        within.app.clone().oneshot(upload).await.unwrap().status(),
        200,
        "an upload is never locked"
    );
    let after = node_over(
        pool.clone(),
        Options {
            now: a_day_later,
            ..Options::default()
        },
    )
    .await;
    let (status, body) = call(
        &after.app,
        Some(OWNER_KEY),
        "PUT",
        &format!("/v1/items/{created}"),
        Some(item_save(created, id)),
    )
    .await;
    assert_eq!(status, 200, "a lock holds for a day: {body}");
    let (status, body) = call(
        &after.app,
        Some(OWNER_KEY),
        "PUT",
        &format!("/v1/collections/{id}"),
        Some(collection_save(id, &short("Renamed", id))),
    )
    .await;
    assert_eq!(status, 200, "{body}");
}

#[tokio::test]
async fn the_owner_deletes_a_draft_with_its_items_and_the_upload_records_no_other_item_names() {
    let Some(node) = node(Options::default()).await else {
        return;
    };
    let (id, items, hashes) = draft(&node, 2).await;
    let (other, _, _) = draft(&node, 0).await;
    let sharing = Uuid::new_v4();
    saved_item(
        &node.state.items,
        &address(OWNER_KEY),
        &json!({ "id": sharing, "name": "Sharing", "collection_id": other, "rarity": "common",
                 "contents": { "model.glb": hashes[0] } }),
    )
    .await;
    let (status, body) = call(
        &node.app,
        Some(STRANGER_KEY),
        "DELETE",
        &format!("/v1/collections/{id}"),
        None,
    )
    .await;
    assert_eq!(
        (status, body["error"].clone()),
        (
            403,
            json!(format!(
                "Unauthorized user {} for collections {id}",
                address(STRANGER_KEY)
            ))
        )
    );
    let (status, body) = call(
        &node.app,
        Some(OWNER_KEY),
        "DELETE",
        &format!("/v1/collections/{id}"),
        Some(json!({})),
    )
    .await;
    assert_eq!((status, body), (200, json!({ "ok": true, "data": true })));
    let left: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM builder.collections WHERE id = $1), \
                (SELECT count(*) FROM builder.items WHERE id = ANY($2)), \
                (SELECT count(*) FROM builder.item_contents WHERE item_id = ANY($2))",
    )
    .bind(id)
    .bind(&items)
    .fetch_one(&node.pool)
    .await
    .unwrap();
    assert_eq!(left, (0, 0, 0));
    let records: Vec<String> =
        sqlx::query_scalar("SELECT hash FROM builder.uploaded_contents WHERE hash = ANY($1)")
            .bind(&hashes)
            .fetch_all(&node.pool)
            .await
            .unwrap();
    assert_eq!(
        records,
        vec![hashes[0].clone()],
        "the record another collection's item names stays"
    );
    let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM builder.items WHERE id = $1")
        .bind(sharing)
        .fetch_one(&node.pool)
        .await
        .unwrap();
    assert_eq!(kept, 1);
    let (status, _) = call(
        &node.app,
        Some(OWNER_KEY),
        "DELETE",
        &format!("/v1/collections/{id}"),
        None,
    )
    .await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn a_published_or_deployed_collection_is_never_deleted() {
    let oracle = FakeOracle::default();
    let Some(node) = node(Options {
        rpc: Some(oracle.url().await),
        ..Options::default()
    })
    .await
    else {
        return;
    };
    let (on_copy, _, _) = draft(&node, 1).await;
    let contract = contract_of(&node.pool, on_copy).await;
    copy_holds(&node.pool, &contract, &address(OWNER_KEY), &[], &[0]).await;
    let (deployed, _, _) = draft(&node, 1).await;
    oracle.deploy(&contract_of(&node.pool, deployed).await, 1);
    for id in [on_copy, deployed] {
        let refused = call(
            &node.app,
            Some(OWNER_KEY),
            "DELETE",
            &format!("/v1/collections/{id}"),
            None,
        )
        .await;
        assert_eq!(refused.0, 409, "{id}: {}", refused.1);
        assert_eq!(
            refused.1,
            json!({ "ok": false, "data": { "id": id }, "error": "The collection is published. It can't be deleted." })
        );
    }
    let (unknown, _, _) = draft(&node, 1).await;
    oracle.set_down(true);
    let (status, _) = call(
        &node.app,
        Some(OWNER_KEY),
        "DELETE",
        &format!("/v1/collections/{unknown}"),
        None,
    )
    .await;
    assert_eq!(status, 503, "a chain that can't tell keeps the collection");
    let left: i64 =
        sqlx::query_scalar("SELECT count(*) FROM builder.collections WHERE id = ANY($1)")
            .bind(vec![on_copy, deployed, unknown])
            .fetch_one(&node.pool)
            .await
            .unwrap();
    assert_eq!(left, 3);
}

#[tokio::test]
async fn a_submitted_third_party_collection_is_never_deleted() {
    let Some(node) = node(Options::default()).await else {
        return;
    };
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO builder.collections (id, name, eth_address, third_party_id, urn_suffix) VALUES ($1, 'TP', $2, $3, $4)",
    )
    .bind(id)
    .bind(address(OWNER_KEY))
    .bind(PROVIDER)
    .bind(format!("{PROVIDER}:{}", id.simple()))
    .execute(&node.pool)
    .await
    .unwrap();
    let item = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO builder.items (id, collection_id, eth_address, name, type) VALUES ($1, $2, $3, 'Hat', 'wearable')",
    )
    .bind(item)
    .bind(id)
    .bind(address(OWNER_KEY))
    .execute(&node.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO builder.item_curations (id, item_id, status, content_hash) VALUES ($1, $2, 'pending', repeat('ab', 32))",
    )
    .bind(Uuid::new_v4())
    .bind(item)
    .execute(&node.pool)
    .await
    .unwrap();
    let (status, body) = call(
        &node.app,
        Some(OWNER_KEY),
        "DELETE",
        &format!("/v1/collections/{id}"),
        None,
    )
    .await;
    assert_eq!(
        (status, body),
        (
            409,
            json!({ "ok": false, "data": { "id": id }, "error": "The third party collection already has published items. It can't be deleted." })
        )
    );
}

/// A third-party collection that holds a slot cheque but no submitted item
/// (a cheque carried over from before the node) is deleted as builder-server
/// deletes it; the cheque stays, so its salt is still spent.
#[tokio::test]
async fn a_third_party_collection_with_a_cheque_but_no_submission_is_deleted_and_its_cheque_kept() {
    let Some(node) = node(Options::default()).await else {
        return;
    };
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO builder.collections (id, name, eth_address, third_party_id, urn_suffix) VALUES ($1, 'TP', $2, $3, $4)",
    )
    .bind(id)
    .bind(address(OWNER_KEY))
    .bind(PROVIDER)
    .bind(format!("{PROVIDER}:{}", id.simple()))
    .execute(&node.pool)
    .await
    .unwrap();
    let salt = format!("0x{}", "5a".repeat(32));
    let cheque = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO builder.slot_usage_cheques (id, collection_id, third_party_id, signature, salt, qty) \
         VALUES ($1, $2, $3, $4, $5, 1)",
    )
    .bind(cheque)
    .bind(id)
    .bind(PROVIDER)
    .bind(format!("0x{}1b", "cd".repeat(64)))
    .bind(&salt)
    .execute(&node.pool)
    .await
    .unwrap();
    let (status, body) = call(
        &node.app,
        Some(OWNER_KEY),
        "DELETE",
        &format!("/v1/collections/{id}"),
        None,
    )
    .await;
    assert_eq!((status, body), (200, json!({ "ok": true, "data": true })));
    let kept: Vec<(Option<Uuid>, String)> =
        sqlx::query_as("SELECT collection_id, salt FROM builder.slot_usage_cheques WHERE id = $1")
            .bind(cheque)
            .fetch_all(&node.pool)
            .await
            .unwrap();
    assert_eq!(kept, vec![(None, salt)]);
}

#[tokio::test]
async fn the_owner_sync_answers_the_chain_creator_and_stores_nothing() {
    let committee = FakeCommittee::with(&[&address(COMMITTEE_KEY)]);
    let Some(node) = node(Options {
        committee: Some(committee),
        ..Options::default()
    })
    .await
    else {
        return;
    };
    let (id, items, _) = draft(&node, 1).await;
    let contract = contract_of(&node.pool, id).await;
    copy_holds(&node.pool, &contract, &address(CREATOR_KEY), &[], &[0]).await;
    let path = format!("/v1/collections/{id}/owner/sync");
    for key in [CREATOR_KEY, COMMITTEE_KEY] {
        let (status, body) = call(&node.app, Some(key), "POST", &path, None).await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(body["data"]["eth_address"], json!(address(CREATOR_KEY)));
        assert_eq!(body["data"]["id"], json!(id));
    }
    let (status, _) = call(&node.app, Some(STRANGER_KEY), "POST", &path, None).await;
    assert_eq!(status, 403);
    let stored: (String, String) = sqlx::query_as(
        "SELECT c.eth_address, i.eth_address FROM builder.collections c JOIN builder.items i ON i.collection_id = c.id \
         WHERE c.id = $1 AND i.id = $2",
    )
    .bind(id)
    .bind(items[0])
    .fetch_one(&node.pool)
    .await
    .unwrap();
    assert_eq!(
        stored,
        (address(OWNER_KEY), address(OWNER_KEY)),
        "the sync stores nothing"
    );
    let third_party = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO builder.collections (id, name, eth_address, third_party_id, urn_suffix) VALUES ($1, 'TP', $2, $3, $4)",
    )
    .bind(third_party)
    .bind(address(OWNER_KEY))
    .bind(PROVIDER)
    .bind(format!("{PROVIDER}:{}", third_party.simple()))
    .execute(&node.pool)
    .await
    .unwrap();
    let (status, body) = call(
        &node.app,
        Some(OWNER_KEY),
        "POST",
        &format!("/v1/collections/{third_party}/owner/sync"),
        None,
    )
    .await;
    assert_eq!(
        (status, body),
        (
            400,
            json!({ "ok": false, "data": { "id": third_party }, "error": "Third party collections have no on-chain owner" })
        )
    );
}

/// A delivery the fake warehouse received: its `x-token` and its body.
type Delivery = (Option<String>, Value);

/// A loopback warehouse that records each delivery and answers the status
/// [`FakeWarehouse::answer`] set, 200 until one is set.
#[derive(Clone, Default)]
struct FakeWarehouse {
    received: Arc<std::sync::Mutex<Vec<Delivery>>>,
    status: Arc<std::sync::atomic::AtomicU16>,
}

impl FakeWarehouse {
    async fn url(&self) -> String {
        let fake = self.clone();
        let app = Router::new().route(
            "/warehouse",
            axum::routing::post(
                move |headers: axum::http::HeaderMap, axum::Json(body): axum::Json<Value>| {
                    let fake = fake.clone();
                    async move {
                        let token = headers
                            .get("x-token")
                            .and_then(|v| v.to_str().ok())
                            .map(str::to_string);
                        fake.received.lock().unwrap().push((token, body));
                        match fake.status.load(std::sync::atomic::Ordering::SeqCst) {
                            0 => axum::http::StatusCode::OK,
                            status => axum::http::StatusCode::from_u16(status).unwrap(),
                        }
                    }
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}/warehouse")
    }

    fn received(&self) -> Vec<Delivery> {
        self.received.lock().unwrap().clone()
    }

    fn answer(&self, status: u16) {
        self.status
            .store(status, std::sync::atomic::Ordering::SeqCst);
    }
}

async fn pending_terms(pool: &PgPool, collection: Uuid) -> Vec<(bool, i32)> {
    sqlx::query_as(
        "SELECT forwarded_at IS NOT NULL, attempts FROM builder.terms_acceptances WHERE collection_id = $1 ORDER BY created_at",
    )
    .bind(collection)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn accepted_terms_are_stored_and_forwarded_to_the_warehouse() {
    let warehouse = FakeWarehouse::default();
    let Some(pool) = own_pool(6).await else {
        return;
    };
    let node = node_over(
        pool,
        Options {
            warehouse: Some(warehouse.url().await),
            ..Options::default()
        },
    )
    .await;
    let (id, _, _) = draft(&node, 0).await;
    let contract = contract_of(&node.pool, id).await;
    let (status, body) = call(
        &node.app,
        Some(OWNER_KEY),
        "POST",
        &format!("/v1/collections/{id}/tos"),
        Some(json!({ "email": "jane.doe@example.com", "event": "publish_collection_tos", "collection_address": "0xignored", "extra": 1 })),
    )
    .await;
    assert_eq!((status, body), (200, json!({ "ok": true })));
    assert_eq!(
        warehouse.received(),
        vec![(
            Some("example-warehouse-token".to_string()),
            json!({ "context": "builder", "event": "publish_collection_tos",
                    "body": { "email": "jane.doe@example.com", "eth_address": address(OWNER_KEY), "collection_address": contract } })
        )]
    );
    assert_eq!(pending_terms(&node.pool, id).await, vec![(true, 1)]);
    assert_eq!(
        crate::terms::forward_due(&node.state).await.unwrap(),
        0,
        "a delivered acceptance is never sent again"
    );
    assert_eq!(warehouse.received().len(), 1);
}

#[tokio::test]
async fn a_failed_forward_keeps_the_acceptance_and_a_later_pass_delivers_it_once() {
    let warehouse = FakeWarehouse::default();
    warehouse.answer(500);
    let Some(pool) = own_pool(6).await else {
        return;
    };
    let node = node_over(
        pool,
        Options {
            warehouse: Some(warehouse.url().await),
            ..Options::default()
        },
    )
    .await;
    let (id, _, _) = draft(&node, 0).await;
    let (status, body) = call(
        &node.app,
        Some(OWNER_KEY),
        "POST",
        &format!("/v1/collections/{id}/tos"),
        Some(json!({ "email": "jane.doe@example.com" })),
    )
    .await;
    assert_eq!(
        (status, body),
        (200, json!({ "ok": true })),
        "a stored acceptance is never reported as failed"
    );
    assert_eq!(pending_terms(&node.pool, id).await, vec![(false, 1)]);
    assert_eq!(
        crate::terms::forward_due(&node.state).await.unwrap(),
        0,
        "not due yet"
    );
    warehouse.answer(200);
    sqlx::query("UPDATE builder.terms_acceptances SET next_attempt_at = next_attempt_at - interval '1 hour' WHERE collection_id = $1")
        .bind(id)
        .execute(&node.pool)
        .await
        .unwrap();
    assert_eq!(crate::terms::forward_due(&node.state).await.unwrap(), 1);
    assert_eq!(pending_terms(&node.pool, id).await, vec![(true, 2)]);
    assert_eq!(crate::terms::forward_due(&node.state).await.unwrap(), 0);
    assert_eq!(
        warehouse.received().len(),
        2,
        "one failed attempt, one delivery"
    );
}

#[tokio::test]
async fn an_acceptance_is_stored_without_a_warehouse_and_attempts_are_bounded() {
    let Some(pool) = own_pool(6).await else {
        return;
    };
    let unconfigured = node_over(pool.clone(), Options::default()).await;
    let (id, _, _) = draft(&unconfigured, 0).await;
    let (status, body) = call(
        &unconfigured.app,
        Some(OWNER_KEY),
        "POST",
        &format!("/v1/collections/{id}/tos"),
        Some(json!({ "email": "jane.doe@example.com" })),
    )
    .await;
    assert_eq!((status, body), (200, json!({ "ok": true })));
    assert_eq!(pending_terms(&pool, id).await, vec![(false, 0)]);
    assert_eq!(
        crate::terms::forward_due(&unconfigured.state)
            .await
            .unwrap(),
        0
    );
    let warehouse = FakeWarehouse::default();
    warehouse.answer(500);
    let configured = node_over(
        pool.clone(),
        Options {
            warehouse: Some(warehouse.url().await),
            ..Options::default()
        },
    )
    .await;
    for _ in 0..10 {
        sqlx::query("UPDATE builder.terms_acceptances SET next_attempt_at = timezone('UTC', now()) - interval '1 second' WHERE collection_id = $1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        crate::terms::forward_due(&configured.state).await.unwrap();
    }
    assert_eq!(pending_terms(&pool, id).await, vec![(false, 6)]);
    assert_eq!(warehouse.received().len(), 6, "at most six attempts");
}

#[tokio::test]
async fn an_acceptance_the_warehouse_refuses_is_never_sent_again_and_a_throttled_one_is() {
    let warehouse = FakeWarehouse::default();
    let Some(pool) = own_pool(6).await else {
        return;
    };
    let node = node_over(
        pool,
        Options {
            warehouse: Some(warehouse.url().await),
            ..Options::default()
        },
    )
    .await;
    for (status, retried) in [
        (400, false),
        (401, false),
        (403, false),
        (404, false),
        (408, true),
        (429, true),
        (502, true),
        (503, true),
    ] {
        warehouse.answer(status);
        let (id, _, _) = draft(&node, 0).await;
        let (answered, _) = call(
            &node.app,
            Some(OWNER_KEY),
            "POST",
            &format!("/v1/collections/{id}/tos"),
            Some(json!({ "email": "jane.doe@example.com" })),
        )
        .await;
        assert_eq!(answered, 200, "{status}");
        sqlx::query("UPDATE builder.terms_acceptances SET next_attempt_at = timezone('UTC', now()) - interval '1 second' WHERE collection_id = $1")
            .bind(id)
            .execute(&node.pool)
            .await
            .unwrap();
        let sent = warehouse.received().len();
        crate::terms::forward_due(&node.state).await.unwrap();
        assert_eq!(
            (
                warehouse.received().len() - sent,
                pending_terms(&node.pool, id).await
            ),
            if retried {
                (1, vec![(false, 2)])
            } else {
                (0, vec![(false, crate::terms::MAX_ATTEMPTS)])
            },
            "{status}"
        );
    }
}

#[tokio::test]
async fn a_creators_acceptances_keep_one_row_no_larger_than_its_collections_items() {
    let Some(node) = node(Options::default()).await else {
        return;
    };
    let (id, _, _) = draft(&node, 2).await;
    let path = format!("/v1/collections/{id}/tos");
    let hash = |c: char| c.to_string().repeat(64);
    let accept = |hashes: Vec<String>| json!({ "email": "jane.doe@example.com", "event": "publish_third_party_items_tos", "hashes": hashes });
    for _ in 0..3 {
        let (status, body) = call(
            &node.app,
            Some(OWNER_KEY),
            "POST",
            &path,
            Some(accept(vec![hash('a'), hash('b')])),
        )
        .await;
        assert_eq!((status, body), (200, json!({ "ok": true })));
    }
    let (status, _) = call(
        &node.app,
        Some(OWNER_KEY),
        "POST",
        &path,
        Some(json!({ "email": "jane.doe@example.com" })),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(
        pending_terms(&node.pool, id).await.len(),
        2,
        "one row per event, the newest acceptance"
    );
    for (what, hashes) in [
        (
            "more hashes than items",
            vec![hash('a'), hash('b'), hash('c')],
        ),
        ("a hash longer than any content hash", vec!["a".repeat(65)]),
        ("a hash that is not one", vec!["../../etc".to_string()]),
    ] {
        let (status, body) = call(
            &node.app,
            Some(OWNER_KEY),
            "POST",
            &path,
            Some(accept(hashes)),
        )
        .await;
        assert_eq!(
            (status, body["error"].clone()),
            (400, json!("Invalid request body")),
            "{what}"
        );
    }
    assert_eq!(pending_terms(&node.pool, id).await.len(), 2);
}

#[tokio::test]
async fn the_terms_route_takes_upstreams_schema_after_the_role_check() {
    let Some(node) = node(Options::default()).await else {
        return;
    };
    let (id, _, _) = draft(&node, 0).await;
    let path = format!("/v1/collections/{id}/tos");
    let (status, _) = call(
        &node.app,
        Some(STRANGER_KEY),
        "POST",
        &path,
        Some(json!({})),
    )
    .await;
    assert_eq!(status, 403, "the role is checked before the body");
    for refused in [
        json!({}),
        json!({ "email": "not an email" }),
        json!({ "email": 1 }),
        json!({ "email": "jane.doe@example.com", "event": "other" }),
        json!({ "email": "jane.doe@example.com", "hashes": [] }),
        json!({ "email": "jane.doe@example.com", "hashes": [1] }),
        json!({ "email": "jane.doe@example.com", "collection_address": 1 }),
    ] {
        let (status, body) = call(
            &node.app,
            Some(OWNER_KEY),
            "POST",
            &path,
            Some(refused.clone()),
        )
        .await;
        assert_eq!(
            (status, body["error"].clone()),
            (400, json!("Invalid request body")),
            "{refused}"
        );
    }
    assert!(pending_terms(&node.pool, id).await.is_empty());
    let (_, read) = call(
        &node.app,
        Some(OWNER_KEY),
        "GET",
        &format!("/v1/collections/{id}"),
        None,
    )
    .await;
    assert!(!read.to_string().contains("example.com"));
}

#[tokio::test]
async fn publish_answers_not_published_until_the_chain_holds_the_collection() {
    let oracle = FakeOracle::default();
    let Some(pool) = super::support::scratch_pool(8).await else {
        return;
    };
    let committee = FakeCommittee::with(&[&address(COMMITTEE_KEY)]);
    let with_rpc = node_over(
        pool.clone(),
        Options {
            rpc: Some(oracle.url().await),
            committee: Some(committee),
            ..Options::default()
        },
    )
    .await;
    let without_rpc = node_over(pool.clone(), Options::default()).await;
    let (id, _, _) = draft(&with_rpc, 2).await;
    let path = format!("/v1/collections/{id}/publish");
    let not_published =
        json!({ "ok": false, "data": { "id": id }, "error": "The collection is not published." });
    for node in [&with_rpc, &without_rpc] {
        assert_eq!(
            call(&node.app, Some(OWNER_KEY), "POST", &path, None).await,
            (409, not_published.clone())
        );
    }
    let (unknown, _, _) = draft(&with_rpc, 1).await;
    oracle.deploy(&contract_of(&pool, unknown).await, 1);
    oracle.set_down(true);
    assert_eq!(
        call(
            &with_rpc.app,
            Some(OWNER_KEY),
            "POST",
            &format!("/v1/collections/{unknown}/publish"),
            None
        )
        .await,
        (
            503,
            json!({ "ok": false, "data": {}, "error": "The collection's chain state is not available. Try again later." })
        ),
        "an RPC that can't tell keeps the client retrying"
    );
    assert_eq!(chain_ids(&pool, unknown).await, vec![None]);
    assert_eq!(chain_ids(&pool, id).await, vec![None, None]);
    let (status, _) = call(&with_rpc.app, Some(STRANGER_KEY), "POST", &path, None).await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn publish_refuses_while_the_chain_lacks_an_item_and_writes_nothing() {
    let oracle = FakeOracle::default();
    let Some(node) = node(Options {
        rpc: Some(oracle.url().await),
        ..Options::default()
    })
    .await
    else {
        return;
    };
    let (id, items, _) = draft(&node, 2).await;
    oracle.deploy(&contract_of(&node.pool, id).await, 1);
    assert_eq!(
        call(
            &node.app,
            Some(OWNER_KEY),
            "POST",
            &format!("/v1/collections/{id}/publish"),
            None
        )
        .await,
        (
            409,
            json!({ "ok": false, "data": { "id": items[1] }, "error": "The item is not published." })
        )
    );
    assert_eq!(chain_ids(&node.pool, id).await, vec![None, None]);
}

/// The collections copy can hold a deployment before the items copy holds
/// its items: then the copy can't tell, and the publish answers from the
/// chain itself, or keeps the client retrying while the chain can't tell
/// either; only the chain's own count refuses an item for good.
#[tokio::test]
async fn publish_waits_for_the_items_copy_to_catch_up_with_the_collections_copy() {
    let oracle = FakeOracle::default();
    let Some(pool) = super::support::scratch_pool(8).await else {
        return;
    };
    let with_rpc = node_over(
        pool.clone(),
        Options {
            rpc: Some(oracle.url().await),
            ..Options::default()
        },
    )
    .await;
    let without_rpc = node_over(pool.clone(), Options::default()).await;
    let owner = address(OWNER_KEY);
    let (waiting, _, _) = draft(&with_rpc, 2).await;
    let contract = contract_of(&pool, waiting).await;
    copy_holds(&pool, &contract, &owner, &[], &[0]).await;
    assert_eq!(
        call(
            &without_rpc.app,
            Some(OWNER_KEY),
            "POST",
            &format!("/v1/collections/{waiting}/publish"),
            None
        )
        .await,
        (
            409,
            json!({ "ok": false, "data": { "id": waiting }, "error": "The collection is not published." })
        ),
        "neither the copy nor a chain read can tell yet"
    );
    assert_eq!(chain_ids(&pool, waiting).await, vec![None, None]);
    oracle.deploy(&contract, 2);
    let (status, body) = call(
        &with_rpc.app,
        Some(OWNER_KEY),
        "POST",
        &format!("/v1/collections/{waiting}/publish"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        chain_ids(&pool, waiting).await,
        vec![Some("0".into()), Some("1".into())]
    );
    let (short, items, _) = draft(&with_rpc, 2).await;
    let contract = contract_of(&pool, short).await;
    copy_holds(&pool, &contract, &owner, &[], &[0]).await;
    oracle.deploy(&contract, 1);
    assert_eq!(
        call(
            &with_rpc.app,
            Some(OWNER_KEY),
            "POST",
            &format!("/v1/collections/{short}/publish"),
            None
        )
        .await,
        (
            409,
            json!({ "ok": false, "data": { "id": items[1] }, "error": "The item is not published." })
        ),
        "the chain itself holds one item"
    );
    assert_eq!(chain_ids(&pool, short).await, vec![None, None]);
}

/// An RPC that answers an absurd item count costs the publish no more than
/// its own items.
#[tokio::test(flavor = "multi_thread")]
async fn publish_reads_the_chains_item_count_as_a_bound() {
    let oracle = FakeOracle::default();
    let Some(node) = node(Options {
        rpc: Some(oracle.url().await),
        ..Options::default()
    })
    .await
    else {
        return;
    };
    let (id, _, _) = draft(&node, 2).await;
    oracle.deploy(&contract_of(&node.pool, id).await, u64::MAX);
    let app = node.app.clone();
    let path = format!("/v1/collections/{id}/publish");
    let answer = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        tokio::spawn(async move { call(&app, Some(OWNER_KEY), "POST", &path, None).await }),
    )
    .await
    .expect("answered promptly")
    .unwrap();
    assert_eq!(answer.0, 200, "{}", answer.1);
    assert_eq!(
        chain_ids(&node.pool, id).await,
        vec![Some("0".into()), Some("1".into())]
    );
}

#[tokio::test]
async fn publish_assigns_on_chain_ids_in_creation_order_once_the_rpc_shows_the_deployment() {
    let oracle = FakeOracle::default();
    let Some(node) = node(Options {
        rpc: Some(oracle.url().await),
        ..Options::default()
    })
    .await
    else {
        return;
    };
    let (id, _, _) = draft(&node, 2).await;
    oracle.deploy(&contract_of(&node.pool, id).await, 2);
    let path = format!("/v1/collections/{id}/publish");
    let (status, body) = call(&node.app, Some(OWNER_KEY), "POST", &path, None).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"]["collection"]["id"], json!(id));
    assert_eq!(body["data"]["collection"]["is_published"], json!(true));
    let ids: Vec<Value> = body["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["blockchain_item_id"].clone())
        .collect();
    assert_eq!(ids, vec![json!("0"), json!("1")]);
    assert_eq!(
        chain_ids(&node.pool, id).await,
        vec![Some("0".into()), Some("1".into())]
    );
    let reads = oracle.code_reads();
    let (status, again) = call(&node.app, Some(OWNER_KEY), "POST", &path, None).await;
    assert_eq!(
        (status, again["data"]["items"].clone()),
        (200, body["data"]["items"].clone()),
        "a repeated publish answers the same"
    );
    assert_eq!(
        oracle.code_reads(),
        reads,
        "a published collection asks the chain nothing"
    );
    let (_, read) = call(
        &node.app,
        Some(OWNER_KEY),
        "GET",
        &format!("/v1/collections/{id}"),
        None,
    )
    .await;
    assert_eq!(
        read["data"]["is_published"],
        json!(true),
        "published on the node before the copy sees it"
    );
}

#[tokio::test]
async fn publish_reads_the_copy_when_it_holds_the_collection() {
    let committee = FakeCommittee::with(&[&address(COMMITTEE_KEY)]);
    let Some(node) = node(Options {
        committee: Some(committee),
        ..Options::default()
    })
    .await
    else {
        return;
    };
    let (id, _, _) = draft(&node, 2).await;
    let contract = contract_of(&node.pool, id).await;
    copy_holds(&node.pool, &contract, &address(OWNER_KEY), &[], &[1, 0]).await;
    let (status, body) = call(
        &node.app,
        Some(COMMITTEE_KEY),
        "POST",
        &format!("/v1/collections/{id}/publish"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        chain_ids(&node.pool, id).await,
        vec![Some("0".into()), Some("1".into())]
    );
}

/// The standard publish answers the collection's items the way its items
/// listing does, measured before their documents load: where the listing is
/// too large the publish is refused too, after the ids it assigned are
/// kept, and a later publish answers them.
#[tokio::test]
async fn publish_answers_its_items_within_the_bounds_of_the_items_listing() {
    let Some(pool) = own_pool(6).await else {
        return;
    };
    let bounded = node_over(
        pool.clone(),
        Options {
            tune: |state| {
                state.limits = crate::limits::Limits::unbounded();
                state.limits.listing_bytes = 1;
            },
            ..Options::default()
        },
    )
    .await;
    let (id, _, _) = draft(&bounded, 3).await;
    let contract = contract_of(&pool, id).await;
    copy_holds(&pool, &contract, &address(OWNER_KEY), &[], &[0, 1, 2]).await;
    let mut answers = Vec::new();
    for (method, path) in [
        ("GET", format!("/v1/collections/{id}/items")),
        ("POST", format!("/v1/collections/{id}/publish")),
    ] {
        let (status, body) = call(&bounded.app, Some(OWNER_KEY), method, &path, None).await;
        answers.push((status, body["error"].clone()));
    }
    let too_large = (413, json!("The listing is too large, ask for it in pages"));
    assert_eq!(answers, [too_large.clone(), too_large]);
    assert_eq!(
        chain_ids(&pool, id).await,
        vec![Some("0".into()), Some("1".into()), Some("2".into())]
    );
    let open = node_over(pool.clone(), Options::default()).await;
    let (status, body) = call(
        &open.app,
        Some(OWNER_KEY),
        "POST",
        &format!("/v1/collections/{id}/publish"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"]["items"].as_array().map(Vec::len), Some(3));
}

#[tokio::test]
async fn concurrent_publish_calls_ask_the_rpc_once() {
    let oracle = FakeOracle::default();
    let Some(node) = node(Options {
        rpc: Some(oracle.url().await),
        ..Options::default()
    })
    .await
    else {
        return;
    };
    let (id, _, _) = draft(&node, 1).await;
    let path = format!("/v1/collections/{id}/publish");
    let calls: Vec<_> = (0..8)
        .map(|_| {
            let (app, path) = (node.app.clone(), path.clone());
            tokio::spawn(async move { call(&app, Some(OWNER_KEY), "POST", &path, None).await })
        })
        .collect();
    for answered in calls {
        assert_eq!(answered.await.unwrap().0, 409);
    }
    assert_eq!(oracle.code_reads(), 1);
}

#[tokio::test]
async fn a_rename_of_a_collection_deployed_before_the_copy_sees_it_is_refused() {
    let oracle = FakeOracle::default();
    let Some(node) = node(Options {
        rpc: Some(oracle.url().await),
        ..Options::default()
    })
    .await
    else {
        return;
    };
    let (id, _, _) = draft(&node, 0).await;
    let contract = contract_of(&node.pool, id).await;
    oracle.deploy(&contract, 0);
    let mut renamed = collection_save(id, &short("Renamed", id));
    renamed["data"] =
        crate::collections::contract::initialize_calldata_named(&address(OWNER_KEY), "Renamed")
            .into();
    let (status, body) = call(
        &node.app,
        Some(OWNER_KEY),
        "PUT",
        &format!("/v1/collections/{id}"),
        Some(renamed.clone()),
    )
    .await;
    assert_eq!(
        (status, body),
        (
            409,
            json!({ "ok": false, "data": { "id": id }, "error": "The collection is published. It can't be updated." })
        )
    );
    assert_eq!(
        contract_of(&node.pool, id).await,
        contract,
        "the row keeps its deployed address"
    );
    let same_address = collection_save(id, &short("Same", id));
    let (status, body) = call(
        &node.app,
        Some(OWNER_KEY),
        "PUT",
        &format!("/v1/collections/{id}"),
        Some(same_address),
    )
    .await;
    assert_eq!(
        status, 200,
        "a save that keeps the address asks nothing: {body}"
    );
    let (unknown, _, _) = draft(&node, 0).await;
    let before = contract_of(&node.pool, unknown).await;
    let mut renamed = collection_save(unknown, &short("Renamed", unknown));
    renamed["data"] =
        crate::collections::contract::initialize_calldata_named(&address(OWNER_KEY), "Renamed")
            .into();
    oracle.set_down(true);
    let (status, _) = call(
        &node.app,
        Some(OWNER_KEY),
        "PUT",
        &format!("/v1/collections/{unknown}"),
        Some(renamed),
    )
    .await;
    assert_eq!(status, 503, "a chain that can't tell changes no address");
    assert_eq!(contract_of(&node.pool, unknown).await, before);
}

#[tokio::test]
async fn an_rpc_of_another_network_shows_no_deployment() {
    let oracle = FakeOracle::default();
    oracle.set_other_chain(true);
    let Some(node) = node(Options {
        rpc: Some(oracle.url().await),
        ..Options::default()
    })
    .await
    else {
        return;
    };
    let (id, _, _) = draft(&node, 1).await;
    oracle.deploy(&contract_of(&node.pool, id).await, 1);
    let (status, _) = call(
        &node.app,
        Some(OWNER_KEY),
        "POST",
        &format!("/v1/collections/{id}/publish"),
        None,
    )
    .await;
    assert_eq!(status, 503, "only Polygon's facts publish a collection");
    let (status, _) = call(
        &node.app,
        Some(OWNER_KEY),
        "DELETE",
        &format!("/v1/collections/{id}"),
        None,
    )
    .await;
    assert_eq!(
        status, 503,
        "another network can't tell whether the collection is deployed"
    );
    assert_eq!(chain_ids(&node.pool, id).await, vec![None]);
}

/// builder-server with `CHAIN_NAME=Sepolia`: a draft is saved at the Amoy
/// factory's address, publishes once an Amoy RPC shows it there, and its
/// collection and items answer `amoy` URNs; a Polygon mainnet RPC can't
/// tell an Amoy node anything.
#[tokio::test]
async fn an_amoy_node_saves_publishes_and_names_its_collections_on_amoy() {
    use crate::collections::contract;
    use crate::network::{AMOY, POLYGON};
    let amoy = FakeOracle::default().on(AMOY);
    let Some(node) = node(Options {
        rpc: Some(amoy.url().await),
        network: AMOY,
        ..Options::default()
    })
    .await
    else {
        return;
    };
    let saved = Uuid::new_v4();
    let data = contract::initialize_calldata_based(&address(OWNER_KEY), "Test", AMOY.base_uri);
    let mut save = collection_save(saved, &short("Amoy", saved));
    save["data"] = json!(data);
    let (status, body) = call(
        &node.app,
        Some(OWNER_KEY),
        "PUT",
        &format!("/v1/collections/{saved}"),
        Some(save),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let calldata = alloy_primitives::hex::decode(&data[2..]).unwrap();
    let salt = contract::salt(&saved.to_string());
    let at = contract_of(&node.pool, saved).await;
    assert_eq!(at, contract::address(&AMOY, salt, &calldata));
    assert_ne!(at, contract::address(&POLYGON, salt, &calldata));
    assert_eq!(
        body["data"]["urn"],
        json!(format!("urn:decentraland:amoy:collections-v2:{at}"))
    );
    let (status, listed) = call(
        &node.app,
        Some(OWNER_KEY),
        "GET",
        &format!("/v1/{}/collections", address(OWNER_KEY)),
        None,
    )
    .await;
    let rows = listed["data"]
        .as_array()
        .or(listed["data"]["results"].as_array())
        .cloned()
        .unwrap_or_default();
    let row = rows.iter().find(|r| r["id"] == json!(saved));
    assert_eq!(
        (status, row.map(|r| r["urn"].clone())),
        (
            200,
            Some(json!(format!("urn:decentraland:amoy:collections-v2:{at}")))
        ),
        "{listed}"
    );

    let mainnet = FakeOracle::default();
    let on_mainnet_rpc = node_over(
        node.pool.clone(),
        Options {
            rpc: Some(mainnet.url().await),
            network: AMOY,
            ..Options::default()
        },
    )
    .await;
    let (id, items, _) = draft(&node, 1).await;
    let contract = contract_of(&node.pool, id).await;
    mainnet.deploy(&contract, 1);
    let (status, _) = call(
        &on_mainnet_rpc.app,
        Some(OWNER_KEY),
        "DELETE",
        &format!("/v1/collections/{id}"),
        None,
    )
    .await;
    assert_eq!(status, 503, "a Polygon mainnet RPC can't tell an Amoy node");

    amoy.deploy(&contract, 1);
    let (status, body) = call(
        &node.app,
        Some(OWNER_KEY),
        "POST",
        &format!("/v1/collections/{id}/publish"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let item_urn = format!("urn:decentraland:amoy:collections-v2:{contract}:0");
    let stored: Option<String> =
        sqlx::query_scalar("SELECT urn_suffix FROM builder.items WHERE id = $1")
            .bind(items[0])
            .fetch_one(&node.pool)
            .await
            .unwrap();
    assert_eq!(stored.as_deref(), Some(item_urn.as_str()));
    let (status, item) = call(
        &node.app,
        Some(OWNER_KEY),
        "GET",
        &format!("/v1/items/{}", items[0]),
        None,
    )
    .await;
    assert_eq!(
        (status, item["data"]["urn"].clone()),
        (200, json!(item_urn))
    );
}

/// A standard collection's tokens read their metadata from the base its
/// `initialize` data names, so a save is kept only when that base is the
/// network's or the one this node serves at its own address.
#[tokio::test]
async fn a_save_names_the_networks_or_the_nodes_token_metadata_base() {
    use crate::collections::{contract, save};
    use crate::network::{AMOY, POLYGON};
    const NODE: &str = "https://node.example.org";
    const PATH: &str = "/lambdas/collections/standard/erc721/";
    let Some(node) = node(Options {
        tune: |state| state.settings.public_url = NODE.into(),
        ..Options::default()
    })
    .await
    else {
        return;
    };
    let put = |base_uri: String| {
        let id = Uuid::new_v4();
        let data = contract::initialize_calldata_based(&address(OWNER_KEY), "Test", &base_uri);
        let mut body = collection_save(id, &short("Base", id));
        body["data"] = json!(data);
        let app = node.app.clone();
        async move {
            let path = format!("/v1/collections/{id}");
            let (status, body) = call(&app, Some(OWNER_KEY), "PUT", &path, Some(body)).await;
            (id, data, status, body)
        }
    };
    for base_uri in [POLYGON.base_uri.to_string(), format!("{NODE}{PATH}")] {
        let (id, data, status, body) = put(base_uri).await;
        assert_eq!(status, 200, "{body}");
        let calldata = alloy_primitives::hex::decode(&data[2..]).unwrap();
        assert_eq!(
            contract_of(&node.pool, id).await,
            contract::address(&POLYGON, contract::salt(&id.to_string()), &calldata)
        );
    }
    for base_uri in [
        format!("https://other.example.org{PATH}"),
        AMOY.base_uri.to_string(),
        format!("{NODE}{PATH}137/"),
        String::new(),
    ] {
        let (id, _, status, body) = put(base_uri.clone()).await;
        assert_eq!(
            (status, body["ok"].clone(), body["error"].clone()),
            (400, json!(false), json!(save::FOREIGN_BASE_URI)),
            "{base_uri}: {body}"
        );
        let kept: Option<String> =
            sqlx::query_scalar("SELECT name FROM builder.collections WHERE id = $1")
                .bind(id)
                .fetch_optional(&node.pool)
                .await
                .unwrap();
        assert_eq!(kept, None, "{base_uri}");
    }
}

#[tokio::test]
async fn publish_waits_while_the_node_publishes_the_collection_itself() {
    let oracle = FakeOracle::default();
    let Some(node) = node(Options {
        rpc: Some(oracle.url().await),
        ..Options::default()
    })
    .await
    else {
        return;
    };
    let (id, _, _) = draft(&node, 1).await;
    oracle.deploy(&contract_of(&node.pool, id).await, 1);
    sqlx::query("UPDATE builder.collections SET publication_pending = true WHERE id = $1")
        .bind(id)
        .execute(&node.pool)
        .await
        .unwrap();
    let (status, body) = call(
        &node.app,
        Some(OWNER_KEY),
        "POST",
        &format!("/v1/collections/{id}/publish"),
        None,
    )
    .await;
    assert_eq!(
        (status, body["error"].clone()),
        (409, json!("The collection is not published."))
    );
    assert_eq!(
        chain_ids(&node.pool, id).await,
        vec![None],
        "the node's own publication assigns the ids"
    );
}

/// A wallet that loses its role between the decision and the write stores
/// and forwards no acceptance in its name.
#[tokio::test]
async fn a_terms_acceptance_rechecks_the_signers_role_when_it_writes() {
    let Some(node) = node(Options::default()).await else {
        return;
    };
    let (id, _, _) = draft(&node, 0).await;
    let signer = crate::shared::signed_fetch::test_support::signer(&address(OWNER_KEY));
    let grant = crate::access::manage_collection(&node.state, &signer, &id.to_string())
        .await
        .unwrap();
    sqlx::query("UPDATE builder.collections SET eth_address = $2 WHERE id = $1")
        .bind(id)
        .bind(address(STRANGER_KEY))
        .execute(&node.pool)
        .await
        .unwrap();
    let acceptance = crate::terms::parse(&json!({ "email": "jane.doe@example.com" })).unwrap();
    let error = crate::terms::accept(&node.state, &grant, acceptance)
        .await
        .unwrap_err();
    assert_eq!(error.status(), 403);
    assert_eq!(pending_terms(&node.pool, id).await, Vec::new());
}

/// While creator writes are off the terms forward neither calls the
/// warehouse nor touches a row; once they are on again it delivers.
#[tokio::test]
async fn the_terms_forward_waits_while_creator_writes_are_off() {
    let Some(pool) = own_pool(6).await else {
        return;
    };
    let warehouse = FakeWarehouse::default();
    warehouse.answer(500);
    let on = |writes: bool| {
        let (pool, url) = (pool.clone(), warehouse.url());
        async move {
            node_over(
                pool,
                Options {
                    warehouse: Some(url.await),
                    writes,
                    ..Options::default()
                },
            )
            .await
        }
    };
    let open = on(true).await;
    let (id, _, _) = draft(&open, 0).await;
    let (status, _) = call(
        &open.app,
        Some(OWNER_KEY),
        "POST",
        &format!("/v1/collections/{id}/tos"),
        Some(json!({ "email": "jane.doe@example.com" })),
    )
    .await;
    assert_eq!(status, 200);
    warehouse.answer(200);
    let sent = warehouse.received().len();
    sqlx::query("UPDATE builder.terms_acceptances SET next_attempt_at = timezone('UTC', now()) - interval '1 second' WHERE collection_id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let closed = on(false).await;
    assert_eq!(crate::terms::forward_due(&closed.state).await.unwrap(), 0);
    assert_eq!(
        warehouse.received().len(),
        sent,
        "no call while writes are off"
    );
    assert_eq!(pending_terms(&pool, id).await, vec![(false, 1)]);
    assert_eq!(crate::terms::forward_due(&open.state).await.unwrap(), 1);
    assert_eq!(pending_terms(&pool, id).await, vec![(true, 2)]);
}

/// Acceptances the warehouse hasn't taken yet are never lost to a newer one:
/// a manager who accepts for two batches of items while the warehouse
/// refuses has both batches' hashes delivered once it answers.
#[tokio::test]
async fn an_acceptance_waiting_for_the_warehouse_keeps_every_batch_it_covers() {
    let warehouse = FakeWarehouse::default();
    warehouse.answer(500);
    let Some(pool) = own_pool(6).await else {
        return;
    };
    let node = node_over(
        pool,
        Options {
            warehouse: Some(warehouse.url().await),
            ..Options::default()
        },
    )
    .await;
    let (id, _, _) = draft(&node, 3).await;
    let path = format!("/v1/collections/{id}/tos");
    let hash = |c: char| c.to_string().repeat(64);
    for batch in [vec![hash('a'), hash('b')], vec![hash('b'), hash('c')]] {
        let (status, _) = call(
            &node.app,
            Some(OWNER_KEY),
            "POST",
            &path,
            Some(json!({ "email": "jane.doe@example.com", "event": "publish_third_party_items_tos", "hashes": batch })),
        )
        .await;
        assert_eq!(status, 200);
    }
    warehouse.answer(200);
    sqlx::query("UPDATE builder.terms_acceptances SET next_attempt_at = timezone('UTC', now()) - interval '1 second' WHERE collection_id = $1")
        .bind(id)
        .execute(&node.pool)
        .await
        .unwrap();
    crate::terms::forward_due(&node.state).await.unwrap();
    let delivered: std::collections::BTreeSet<String> = warehouse
        .received()
        .iter()
        .skip(2)
        .flat_map(|(_, body)| {
            body["body"]["hashes"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|h| h.as_str().map(str::to_string))
        .collect();
    assert_eq!(
        delivered,
        [hash('a'), hash('b'), hash('c')].into_iter().collect()
    );
    assert!(
        pending_terms(&node.pool, id)
            .await
            .iter()
            .all(|(forwarded, _)| *forwarded)
    );
}

/// A delivery of an acceptance a newer one merged into after it was sent
/// leaves the merged acceptance waiting for its own delivery.
#[tokio::test]
async fn a_delivery_that_predates_a_merge_leaves_the_merged_acceptance_waiting() {
    let Some(node) = node(Options::default()).await else {
        return;
    };
    let (id, _, _) = draft(&node, 2).await;
    let path = format!("/v1/collections/{id}/tos");
    let accept = |hash: char| json!({ "email": "jane.doe@example.com", "event": "publish_third_party_items_tos", "hashes": [hash.to_string().repeat(64)] });
    assert_eq!(
        call(&node.app, Some(OWNER_KEY), "POST", &path, Some(accept('a')))
            .await
            .0,
        200
    );
    let (row, sent): (Uuid, Value) =
        sqlx::query_as("SELECT id, body FROM builder.terms_acceptances WHERE collection_id = $1")
            .bind(id)
            .fetch_one(&node.pool)
            .await
            .unwrap();
    assert_eq!(
        call(&node.app, Some(OWNER_KEY), "POST", &path, Some(accept('b')))
            .await
            .0,
        200
    );
    crate::terms::delivered(&node.state, row, &sent)
        .await
        .unwrap();
    assert_eq!(pending_terms(&node.pool, id).await, vec![(false, 0)]);
}

/// A committee member dropped from the committee between the publication's
/// decision and its write publishes nothing; one still on it publishes.
#[tokio::test]
async fn a_committee_member_dropped_before_the_publish_writes_nothing() {
    let committee = FakeCommittee::with(&[&address(COMMITTEE_KEY)]);
    let Some(node) = node(Options {
        committee: Some(committee.clone()),
        ..Options::default()
    })
    .await
    else {
        return;
    };
    let mut outcomes = Vec::new();
    for dropped in [false, true] {
        let (id, _, _) = draft(&node, 1).await;
        let contract = contract_of(&node.pool, id).await;
        copy_holds(&node.pool, &contract, &address(OWNER_KEY), &[], &[0]).await;
        let sync = crate::access::sync_collection(
            &node.state,
            &crate::shared::signed_fetch::test_support::signer(&address(COMMITTEE_KEY)),
            &id.to_string(),
        )
        .await
        .unwrap();
        if dropped {
            committee.set(Some(&[]));
        }
        let headers = axum::http::HeaderMap::new();
        let listing = crate::limits::Listing {
            state: &node.state,
            headers: &headers,
            signer: &address(COMMITTEE_KEY),
        };
        let published = crate::collections::lifecycle::publish(&node.state, &sync, &listing).await;
        outcomes.push((
            published.err().map(|e| e.status()),
            chain_ids(&node.pool, id).await,
        ));
    }
    assert_eq!(
        outcomes,
        [(None, vec![Some("0".to_string())]), (Some(403), vec![None])]
    );
}
