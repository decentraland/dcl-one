//! An item's body shows what this node's content holds at the item's URN,
//! read through the router as `serve` mounts it, over a database with the
//! content tables the sync writes.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use crate::shared::signed_fetch::test_support::signed_headers_by;
use crate::tests::support::{FakeCommittee, FakeRegistry, registry_index, temp_storage, wallet};

const MANAGER_KEY: &str = "0x9101010101010101010101010101010101010101010101010101010101010101";
const OWNER_KEY: &str = "0x9202020202020202020202020202020202020202020202020202020202020202";

fn address(key: &str) -> String {
    format!("{:#x}", wallet(key).address())
}

struct Node {
    pool: PgPool,
    app: Router,
    state: crate::SharedState,
    denylist: Arc<crate::content::Denylist>,
    provider: String,
}

async fn node() -> Option<Node> {
    node_with(false).await
}

/// A node that is its own content authority when `own`.
async fn node_with(own: bool) -> Option<Node> {
    let pool = crate::db::scratch::isolated_pool(4).await?;
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at, pass_started_at) \
         VALUES ('collections', 'matic', 'https://subgraph.example', timezone('UTC', now()), timezone('UTC', now()))",
    )
    .execute(&pool)
    .await
    .unwrap();
    let committee = FakeCommittee::with(&[]);
    let registry = FakeRegistry::up();
    let provider = format!(
        "urn:decentraland:matic:collections-thirdparty:held{}",
        &Uuid::new_v4().simple().to_string()[..8]
    );
    registry.put(&provider, &[&address(MANAGER_KEY)], true, false);
    let denylist: Arc<crate::content::Denylist> = Arc::default();
    let state = crate::State::new(
        pool.clone(),
        temp_storage().await,
        denylist.clone(),
        dcl_http::Outbound::for_tests(),
        Some(registry_index(&committee, &registry).await),
        None,
        crate::Settings {
            writes_enabled: true,
            ..crate::Settings::default()
        },
    )
    .unwrap();
    state.content_sync.set_own_content_authority(own);
    let state = Arc::new(state);
    Some(Node {
        pool,
        app: crate::root_router(state.clone()),
        state,
        denylist,
        provider,
    })
}

async fn read(node: &Node, key: &str, path: &str) -> Value {
    let now = Utc::now().timestamp_millis().to_string();
    let mut request = Request::builder().method("GET").uri(path);
    for (name, value) in signed_headers_by(&wallet(key), "get", path, "{}", true, &now)
        .await
        .iter()
    {
        request = request.header(name, value);
    }
    let response = node
        .app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "{path}");
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 22)
        .await
        .unwrap();
    serde_json::from_slice::<Value>(&bytes).unwrap()["data"].clone()
}

/// A third-party collection of the node's provider with one item at `urn`.
async fn third_party_item(node: &Node, item: &str) -> (Uuid, Uuid, String) {
    let collection = Uuid::new_v4();
    let collection_urn = format!("{}:c{}", node.provider, collection.simple());
    sqlx::query(
        "INSERT INTO builder.collections (id, name, eth_address, third_party_id, urn_suffix) VALUES ($1, $2, '', $3, $4)",
    )
    .bind(collection)
    .bind(format!("Held {}", collection.simple()))
    .bind(&node.provider)
    .bind(&collection_urn)
    .execute(&node.pool)
    .await
    .unwrap();
    let id = Uuid::new_v4();
    let urn = format!("{collection_urn}:{item}");
    sqlx::query(
        "INSERT INTO builder.items (id, collection_id, eth_address, name, type, urn_suffix, local_content_hash) \
         VALUES ($1, $2, $3, 'Hat', 'wearable', $4, repeat('cd', 32))",
    )
    .bind(id)
    .bind(collection)
    .bind(address(MANAGER_KEY))
    .bind(&urn)
    .execute(&node.pool)
    .await
    .unwrap();
    (collection, id, urn)
}

/// Deploys an entity of `entity_type` whose metadata names `id` and the
/// merkle proof hash `proof`, active at `pointer` unless `active` is false;
/// answers its entity id.
async fn deploy(
    pool: &PgPool,
    pointer: &str,
    entity_type: &str,
    id: &str,
    proof: &str,
    active: bool,
) -> String {
    let entity = format!("bafkentity{}", Uuid::new_v4().simple());
    let deleter: Option<i32> = if active {
        None
    } else {
        Some(
            sqlx::query_scalar(
                "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, \
                   entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
                 VALUES ('0x0', 'v3', 'wearable', $1, '{\"v\":{}}'::json, timezone('UTC', now()), ARRAY[$2], \
                   timezone('UTC', now()), '[]'::json) RETURNING id",
            )
            .bind(format!("bafknewer{}", Uuid::new_v4().simple()))
            .bind(pointer)
            .fetch_one(pool)
            .await
            .unwrap(),
        )
    };
    sqlx::query(
        "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, \
           entity_timestamp, entity_pointers, local_timestamp, auth_chain, deleter_deployment) \
         VALUES ('0x0', 'v3', $1, $2, $3::json, timezone('UTC', now()), ARRAY[$4], timezone('UTC', now()), '[]'::json, $5)",
    )
    .bind(entity_type)
    .bind(&entity)
    .bind(json!({ "v": { "id": id, "merkleProof": { "entityHash": proof } } }))
    .bind(pointer)
    .bind(deleter)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO content.active_pointers (pointer, entity_id, entity_type) VALUES ($1, $2, $3) \
         ON CONFLICT (pointer) DO UPDATE SET entity_id = EXCLUDED.entity_id, entity_type = EXCLUDED.entity_type",
    )
    .bind(pointer)
    .bind(&entity)
    .bind(entity_type)
    .execute(pool)
    .await
    .unwrap();
    entity
}

fn held(body: &Value) -> (bool, bool, bool, Value) {
    (
        body["in_catalyst"] == true,
        body["is_approved"] == true,
        body["is_published"] == true,
        body["catalyst_content_hash"].clone(),
    )
}

#[tokio::test]
async fn a_manager_sees_its_deployed_third_party_item_approved_with_its_proof_hash() {
    let Some(node) = node().await else { return };
    let (collection, id, urn) = third_party_item(&node, "Hat").await;
    let proof = "ef".repeat(32);
    deploy(
        &node.pool,
        &urn.to_lowercase(),
        "wearable",
        &urn,
        &proof,
        true,
    )
    .await;
    let by_id = read(&node, MANAGER_KEY, &format!("/v1/items/{id}")).await;
    assert_eq!(held(&by_id), (true, true, true, json!(proof)));
    assert_eq!(by_id["content_hash"], Value::Null);
    let listed = read(
        &node,
        MANAGER_KEY,
        &format!("/v1/collections/{collection}/items"),
    )
    .await;
    assert_eq!(held(&listed[0]), (true, true, true, json!(proof)));
}

#[tokio::test]
async fn an_entity_of_another_urn_case_type_or_state_or_one_the_node_withholds_is_not_the_item() {
    let Some(node) = node().await else { return };
    let proof = "ef".repeat(32);
    let none = (false, false, false, Value::Null);
    let (_, recased, urn) = third_party_item(&node, "Cap").await;
    deploy(
        &node.pool,
        &urn.to_lowercase(),
        "wearable",
        &urn.to_lowercase(),
        &proof,
        true,
    )
    .await;
    assert_eq!(
        held(&read(&node, MANAGER_KEY, &format!("/v1/items/{recased}")).await),
        none
    );
    let (_, scene, urn) = third_party_item(&node, "scene").await;
    deploy(&node.pool, &urn, "scene", &urn, &proof, true).await;
    assert_eq!(
        held(&read(&node, MANAGER_KEY, &format!("/v1/items/{scene}")).await),
        none
    );
    let (_, superseded, urn) = third_party_item(&node, "old").await;
    deploy(&node.pool, &urn, "wearable", &urn, &proof, false).await;
    assert_eq!(
        held(&read(&node, MANAGER_KEY, &format!("/v1/items/{superseded}")).await),
        none
    );
    let (_, withheld, urn) = third_party_item(&node, "denied").await;
    let entity = deploy(&node.pool, &urn, "emote", &urn, &proof, true).await;
    assert_eq!(
        held(&read(&node, MANAGER_KEY, &format!("/v1/items/{withheld}")).await),
        (true, true, true, json!(proof))
    );
    node.denylist.operator_insert(&entity);
    assert_eq!(
        held(&read(&node, MANAGER_KEY, &format!("/v1/items/{withheld}")).await),
        none
    );
}

#[tokio::test]
async fn an_owner_sees_its_deployed_minted_item_in_catalyst_under_the_entity_urn_without_a_proof_hash()
 {
    let Some(node) = node().await else { return };
    let owner = address(OWNER_KEY);
    let collection = Uuid::new_v4();
    let contract = format!("0x{}", Uuid::new_v4().simple().to_string()[..20].repeat(2));
    sqlx::query(
        "INSERT INTO builder.collections (id, name, eth_address, contract_address) VALUES ($1, $2, $3, $4)",
    )
    .bind(collection)
    .bind(format!("Minted {}", collection.simple()))
    .bind(&owner)
    .bind(&contract)
    .execute(&node.pool)
    .await
    .unwrap();
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO builder.items (id, collection_id, eth_address, name, type, blockchain_item_id) \
         VALUES ($1, $2, $3, 'Hat', 'wearable', '0')",
    )
    .bind(id)
    .bind(collection)
    .bind(&owner)
    .execute(&node.pool)
    .await
    .unwrap();
    let urn = format!("urn:decentraland:matic:collections-v2:{contract}:0");
    sqlx::query(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, \
           is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ('matic', $1, 137, 'Minted', $2, $2, 'urn:c', true, true, true, 1, 1, 1, timezone('UTC', now()))",
    )
    .bind(&contract)
    .bind(&owner)
    .execute(&node.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, \
           total_supply, max_supply, available, price, name, category, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ('matic', $1, 0, 137, 'wearable_v2', $2, $3, 'common', 0, 100, 100, 0, 'Hat', 'hat', 1, 1, 1, timezone('UTC', now()))",
    )
    .bind(&contract)
    .bind(&urn)
    .bind(&owner)
    .execute(&node.pool)
    .await
    .unwrap();
    let before = read(&node, OWNER_KEY, &format!("/v1/items/{id}")).await;
    assert_eq!(before["in_catalyst"], false);
    let entity_urn = urn.replace("collections-v2", "COLLECTIONS-V2");
    deploy(
        &node.pool,
        &urn,
        "wearable",
        &entity_urn,
        &"ef".repeat(32),
        true,
    )
    .await;
    let after = read(&node, OWNER_KEY, &format!("/v1/items/{id}")).await;
    assert_eq!(after["in_catalyst"], true);
    assert_eq!(after["urn"], json!(entity_urn));
    assert_eq!(after["catalyst_content_hash"], Value::Null);
    assert_eq!(after["is_approved"], true);
}

async fn save(node: &Node, collection: Uuid, id: Uuid, urn: &str) -> (u16, Value) {
    let path = format!("/v1/items/{id}");
    let body = crate::tests::support::item_body(
        &node.state.items,
        &address(MANAGER_KEY),
        &json!({ "id": id, "name": "Held helmet", "collection_id": collection, "rarity": null, "urn": urn,
            "contents": { "model.glb": dcl_auth::hash_bytes_v1(b"held helmet") } }),
    )
    .await;
    let now = Utc::now().timestamp_millis().to_string();
    let mut request = Request::builder()
        .method("PUT")
        .uri(&path)
        .header("content-type", "application/json");
    for (name, value) in signed_headers_by(&wallet(MANAGER_KEY), "put", &path, "{}", true, &now)
        .await
        .iter()
    {
        request = request.header(name, value);
    }
    let response = node
        .app
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 22)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn stored(pool: &PgPool, id: Uuid) -> bool {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM builder.items WHERE id = $1)")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Proves item coverage as the sync leader would: `holder` takes the
/// `sync` lease and a completed pass is recorded under its session.
async fn prove_coverage(node: &Node) -> sqlx::pool::PoolConnection<sqlx::Postgres> {
    let mut holder = node.pool.acquire().await.unwrap();
    let held: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_lock(hashtext('catalyst'), hashtext('sync'))")
            .fetch_one(&mut *holder)
            .await
            .unwrap();
    assert!(held);
    sqlx::query(
        "INSERT INTO content.item_history_coverage (algorithm, source_set, entity_types, history_floor_ms, \
           pass_started_at, pass_completed_at, observed_frontier_ms, debt, lease_namespace, lease_pid, lease_backend_start) \
         SELECT 1, ARRAY['https://peer.example'], ARRAY['emote', 'wearable'], 0, timezone('UTC', now()), \
           timezone('UTC', now()), 0, 0, 'catalyst', pg_backend_pid(), \
           (SELECT timezone('UTC', backend_start) FROM pg_stat_activity WHERE pid = pg_backend_pid())",
    )
    .execute(&mut *holder)
    .await
    .unwrap();
    holder
}

#[tokio::test]
async fn a_manager_cannot_create_an_item_under_a_urn_this_node_s_content_holds() {
    let Some(node) = node_with(true).await else {
        return;
    };
    let (collection, _, _) = third_party_item(&node, "kept").await;
    let collection_urn = format!("{}:c{}", node.provider, collection.simple());
    let held = format!("{collection_urn}:held");
    deploy(&node.pool, &held, "wearable", &held, &"ef".repeat(32), true).await;
    let id = Uuid::new_v4();
    let (status, body) = save(&node, collection, id, &held).await;
    assert_eq!(
        (status, body["error"].as_str().unwrap_or_default()),
        (
            409,
            "The URN provided already belong to another item. The item can't be inserted."
        )
    );
    assert_eq!(body["data"], json!({ "id": id, "urn": held }));
    assert!(!stored(&node.pool, id).await);
    let shouted = format!("{collection_urn}:Shouted");
    deploy(
        &node.pool,
        &shouted.to_lowercase(),
        "wearable",
        &shouted,
        &"ef".repeat(32),
        true,
    )
    .await;
    assert_eq!(save(&node, collection, id, &shouted).await.0, 409);
    assert!(!stored(&node.pool, id).await);
    let free = format!("{collection_urn}:free");
    assert_eq!(save(&node, collection, id, &free).await.0, 200);
    let (status, body) = save(&node, collection, id, &held).await;
    assert_eq!(
        (status, body["error"].as_str().unwrap_or_default()),
        (
            409,
            "The URN provided already belong to another item. The item can't be inserted or updated."
        )
    );
}

#[tokio::test]
async fn a_new_third_party_urn_waits_until_the_node_can_tell_it_is_unused() {
    let Some(node) = node().await else { return };
    let (collection, existing, existing_urn) = third_party_item(&node, "kept").await;
    let collection_urn = format!("{}:c{}", node.provider, collection.simple());
    let id = Uuid::new_v4();
    let (status, body) = save(&node, collection, id, &format!("{collection_urn}:new")).await;
    assert_eq!(
        (status, body["error"].as_str().unwrap_or_default()),
        (
            503,
            "The catalyst's items are not available. Try again later."
        )
    );
    assert!(!stored(&node.pool, id).await);
    assert_eq!(
        save(&node, collection, existing, &existing_urn).await.0,
        200
    );
    let holder = prove_coverage(&node).await;
    assert_eq!(
        save(&node, collection, id, &format!("{collection_urn}:new"))
            .await
            .0,
        200
    );
    sqlx::Connection::close(holder.detach()).await.unwrap();
    let other = Uuid::new_v4();
    assert_eq!(
        save(&node, collection, other, &format!("{collection_urn}:other"))
            .await
            .0,
        503
    );
    assert!(!stored(&node.pool, other).await);
}
