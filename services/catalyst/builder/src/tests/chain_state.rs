//! The chain-state reader over the shop's copy of the Polygon collections
//! subgraph, the published and approved state it derives with the node's
//! own publication records, and the 503 every route answers while the copy
//! can't tell.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use chrono::{SubsecRound, TimeDelta, Utc};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};
use tower::ServiceExt;
use uuid::Uuid;

use crate::chain_state::{self, CollectionFacts, STALE_AFTER};
use crate::shared::signed_fetch::test_support::signed_headers_by;

use super::support::{OWNER_KEY, temp_storage, wallet};

fn random_address() -> String {
    format!("0x{}", Uuid::new_v4().simple().to_string()[..20].repeat(2))
}

async fn set_window(
    conn: &mut PgConnection,
    completed: Option<chrono::NaiveDateTime>,
    started: Option<chrono::NaiveDateTime>,
    cursor: Option<&str>,
) {
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at, pass_started_at, cursor_id) \
         VALUES ('collections', 'matic', 'https://subgraph.example', $1, $2, $3) \
         ON CONFLICT (mirror, network) DO UPDATE SET last_complete_pass_at = EXCLUDED.last_complete_pass_at, \
           pass_started_at = EXCLUDED.pass_started_at, cursor_id = EXCLUDED.cursor_id",
    )
    .bind(completed)
    .bind(started)
    .bind(cursor)
    .execute(conn)
    .await
    .unwrap();
}

async fn set_pass(conn: &mut PgConnection, at: Option<chrono::NaiveDateTime>) {
    set_window(conn, at, at, None).await;
}

fn ago(minutes: i64) -> chrono::NaiveDateTime {
    Utc::now().naive_utc() - TimeDelta::minutes(minutes)
}

async fn fresh_pass(conn: &mut PgConnection) {
    set_pass(conn, Some(Utc::now().naive_utc())).await;
}

async fn copy_row(
    conn: &mut PgConnection,
    network: &str,
    contract: &str,
    approved: bool,
    managers: &[&str],
) {
    sqlx::query(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, \
           is_completed, is_approved, is_editable, managers, minters, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ($1, $2, 137, 'Copy', '0x00000000000000000000000000000000000000c0', \
           '0x00000000000000000000000000000000000000c1', 'urn:c', true, $3, true, $4, \
           ARRAY['0x00000000000000000000000000000000000000AB'], 1, 1, 1700000000, timezone('UTC', now()))",
    )
    .bind(network)
    .bind(contract)
    .bind(approved)
    .bind(managers)
    .execute(conn)
    .await
    .unwrap();
}

async fn pool() -> Option<PgPool> {
    crate::db::scratch::pool(4).await
}

#[tokio::test]
async fn the_copy_answers_approval_and_roles_of_a_polygon_collection() {
    let Some(pool) = pool().await else {
        return;
    };
    let mut tx = pool.begin().await.unwrap();
    fresh_pass(&mut tx).await;
    let (approved, unapproved, absent, ethereum) = (
        random_address(),
        random_address(),
        random_address(),
        random_address(),
    );
    copy_row(
        &mut tx,
        "matic",
        &approved,
        true,
        &["0x00000000000000000000000000000000000000Aa"],
    )
    .await;
    copy_row(&mut tx, "matic", &unapproved, false, &[]).await;
    copy_row(&mut tx, "ethereum", &ethereum, true, &[]).await;
    let view = chain_state::for_decision(&mut tx).await.unwrap();
    let facts =
        chain_state::collection(&mut tx, &view, &approved.to_uppercase().replace("0X", "0x"))
            .await
            .unwrap()
            .expect("an approved collection on chain, looked up in any case");
    assert_eq!(
        facts,
        CollectionFacts {
            is_approved: true,
            creator: "0x00000000000000000000000000000000000000c0".into(),
            owner: "0x00000000000000000000000000000000000000c1".into(),
            managers: vec!["0x00000000000000000000000000000000000000aa".into()],
            minters: vec!["0x00000000000000000000000000000000000000ab".into()],
            reviewed_at: 1_700_000_000,
            mirrored_at: facts.mirrored_at,
        },
        "role lists come back lowercase whatever case the subgraph sent"
    );
    assert!(Utc::now() - facts.mirrored_at < TimeDelta::minutes(1));
    assert!(Utc::now() - view.as_of() < TimeDelta::minutes(1));
    let facts = chain_state::collection(&mut tx, &view, &unapproved)
        .await
        .unwrap()
        .expect("an unapproved collection is on chain too");
    assert!(!facts.is_approved);
    assert_eq!(
        chain_state::collection(&mut tx, &view, &absent)
            .await
            .unwrap(),
        None,
        "an absent row after a complete pass is not on chain"
    );
    assert_eq!(
        chain_state::collection(&mut tx, &view, &ethereum)
            .await
            .unwrap(),
        None,
        "an Ethereum row is not the Polygon collection"
    );
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn the_copy_cannot_tell_without_a_complete_pass_and_a_decision_needs_a_fresh_one() {
    let Some(pool) = pool().await else {
        return;
    };
    let mut tx = pool.begin().await.unwrap();
    set_pass(&mut tx, None).await;
    assert_eq!(
        chain_state::for_read(&mut tx).await.unwrap_err().status(),
        503,
        "a pass that never completed"
    );
    assert_eq!(
        chain_state::for_decision(&mut tx)
            .await
            .unwrap_err()
            .status(),
        503
    );
    sqlx::query("DELETE FROM shop.mirror_state WHERE mirror = 'collections' AND network = 'matic'")
        .execute(&mut *tx)
        .await
        .unwrap();
    assert_eq!(
        chain_state::for_read(&mut tx).await.unwrap_err().status(),
        503,
        "no mirror state at all"
    );
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at) \
         VALUES ('collections', 'ethereum', 'https://subgraph.example', timezone('UTC', now())), \
                ('items', 'matic', 'https://subgraph.example', timezone('UTC', now())) \
         ON CONFLICT (mirror, network) DO UPDATE SET last_complete_pass_at = EXCLUDED.last_complete_pass_at",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    assert_eq!(
        chain_state::for_read(&mut tx).await.unwrap_err().status(),
        503,
        "another mirror's or network's pass says nothing about Polygon collections"
    );
    let stale = (Utc::now().naive_utc() - STALE_AFTER - TimeDelta::minutes(1)).trunc_subsecs(6);
    set_pass(&mut tx, Some(stale)).await;
    let view = chain_state::for_read(&mut tx)
        .await
        .expect("a stale copy still serves reads");
    assert_eq!(view.as_of().naive_utc(), stale);
    assert_eq!(
        chain_state::for_decision(&mut tx)
            .await
            .unwrap_err()
            .status(),
        503,
        "a stale copy grants nothing"
    );
    set_pass(
        &mut tx,
        Some(Utc::now().naive_utc() - STALE_AFTER + TimeDelta::minutes(1)),
    )
    .await;
    chain_state::for_decision(&mut tx)
        .await
        .expect("a pass inside the bound decides");
    tx.rollback().await.unwrap();
}

struct Collection {
    id: Uuid,
    contract: Option<String>,
}

async fn builder_collection(
    conn: &mut PgConnection,
    third_party: bool,
    contract: Option<&str>,
) -> Collection {
    let id = Uuid::new_v4();
    let (urn, provider) = if third_party {
        (
            Some(format!(
                "urn:decentraland:matic:collections-thirdparty:p{}:c",
                id.simple()
            )),
            Some(format!(
                "urn:decentraland:matic:collections-thirdparty:p{}",
                id.simple()
            )),
        )
    } else {
        (None, None)
    };
    sqlx::query(
        "INSERT INTO builder.collections (id, name, eth_address, contract_address, urn_suffix, third_party_id) \
         VALUES ($1, 'Derived', '0x00000000000000000000000000000000000000d0', $2, $3, $4)",
    )
    .bind(id)
    .bind(contract)
    .bind(urn)
    .bind(provider)
    .execute(conn)
    .await
    .unwrap();
    Collection {
        id,
        contract: contract.map(str::to_string),
    }
}

async fn item(conn: &mut PgConnection, collection: Uuid, minted: bool) {
    sqlx::query(
        "INSERT INTO builder.items (id, collection_id, eth_address, name, type, blockchain_item_id) \
         VALUES ($1, $2, '0x00000000000000000000000000000000000000d0', 'Hat', 'wearable', $3)",
    )
    .bind(Uuid::new_v4())
    .bind(collection)
    .bind(minted.then_some("0"))
    .execute(conn)
    .await
    .unwrap();
}

async fn standard_receipt(conn: &mut PgConnection, c: &Collection) {
    sqlx::query(
        "INSERT INTO builder.collection_publications (collection_id, preparation, status, tx_hash, contract_address) \
         VALUES ($1, '{\"revision\":\"r\"}', 'published', $2, $3)",
    )
    .bind(c.id)
    .bind(format!("0x{}", Uuid::new_v4().simple()))
    .bind(c.contract.as_deref().unwrap_or("0x00000000000000000000000000000000000000ee"))
    .execute(conn)
    .await
    .unwrap();
}

async fn submitted(conn: &mut PgConnection, c: &Collection) {
    sqlx::query(
        "INSERT INTO builder.item_curations (id, item_id, status, content_hash) \
         SELECT $1, i.id, 'pending', repeat('ab', 32) FROM builder.items i WHERE i.collection_id = $2 LIMIT 1",
    )
    .bind(Uuid::new_v4())
    .bind(c.id)
    .execute(conn)
    .await
    .unwrap();
}

async fn derived(conn: &mut PgConnection, id: Uuid) -> (bool, bool) {
    let view = chain_state::for_read(&mut *conn).await.unwrap();
    let listed: (bool, bool) = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {}, {} FROM {} WHERE c.id = $1",
        view.published(),
        view.approved(),
        view.collections()
    )))
    .bind(id)
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    let decision = chain_state::for_decision(&mut *conn).await.unwrap();
    assert_eq!(
        chain_state::published(&mut *conn, &decision, id)
            .await
            .unwrap(),
        listed.0,
        "the guard and the listing agree"
    );
    listed
}

#[tokio::test]
async fn a_collection_is_published_by_the_copy_its_own_receipt_or_a_minted_item() {
    let Some(pool) = pool().await else {
        return;
    };
    let mut tx = pool.begin().await.unwrap();
    fresh_pass(&mut tx).await;

    let draft = builder_collection(&mut tx, false, Some(&random_address())).await;
    item(&mut tx, draft.id, false).await;
    assert_eq!(
        derived(&mut tx, draft.id).await,
        (false, false),
        "a draft with a computed contract the copy doesn't hold"
    );

    let contract = random_address();
    let copied = builder_collection(&mut tx, false, Some(&contract)).await;
    copy_row(&mut tx, "matic", &contract, false, &[]).await;
    assert_eq!(
        derived(&mut tx, copied.id).await,
        (true, false),
        "on chain by the copy alone"
    );
    let approved_contract = random_address();
    let approved = builder_collection(&mut tx, false, Some(&approved_contract)).await;
    copy_row(&mut tx, "matic", &approved_contract, true, &[]).await;
    assert_eq!(derived(&mut tx, approved.id).await, (true, true));
    let on_ethereum_contract = random_address();
    let on_ethereum = builder_collection(&mut tx, false, Some(&on_ethereum_contract)).await;
    copy_row(&mut tx, "ethereum", &on_ethereum_contract, true, &[]).await;
    assert_eq!(
        derived(&mut tx, on_ethereum.id).await,
        (false, false),
        "an Ethereum row publishes nothing"
    );

    let received = builder_collection(&mut tx, false, Some(&random_address())).await;
    standard_receipt(&mut tx, &received).await;
    assert_eq!(
        derived(&mut tx, received.id).await,
        (true, false),
        "the node's verified receipt alone, while the copy lags"
    );

    let minted = builder_collection(&mut tx, false, None).await;
    item(&mut tx, minted.id, false).await;
    item(&mut tx, minted.id, true).await;
    assert_eq!(
        derived(&mut tx, minted.id).await,
        (true, false),
        "an item with an on-chain id alone"
    );

    let every_signal_contract = random_address();
    let every_signal = builder_collection(&mut tx, false, Some(&every_signal_contract)).await;
    copy_row(&mut tx, "matic", &every_signal_contract, true, &[]).await;
    standard_receipt(&mut tx, &every_signal).await;
    item(&mut tx, every_signal.id, true).await;
    assert_eq!(derived(&mut tx, every_signal.id).await, (true, true));

    let pending = builder_collection(&mut tx, false, None).await;
    sqlx::query("UPDATE builder.collections SET publication_pending = true WHERE id = $1")
        .bind(pending.id)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO builder.collection_publications (collection_id, preparation, status, tx_hash) \
         VALUES ($1, '{\"revision\":\"r\"}', 'submitted', $2)",
    )
    .bind(pending.id)
    .bind(format!("0x{}", Uuid::new_v4().simple()))
    .execute(&mut *tx)
    .await
    .unwrap();
    assert_eq!(
        derived(&mut tx, pending.id).await,
        (false, false),
        "a submitted, unverified publication is pending, not published"
    );

    let linked = builder_collection(&mut tx, true, None).await;
    item(&mut tx, linked.id, false).await;
    assert_eq!(
        derived(&mut tx, linked.id).await,
        (false, false),
        "a third-party collection with no item submitted is a draft"
    );
    submitted(&mut tx, &linked).await;
    assert_eq!(
        derived(&mut tx, linked.id).await,
        (true, false),
        "a third-party collection with an item submitted is published; it is never approved, as upstream's"
    );
    item(&mut tx, linked.id, false).await;
    assert_eq!(
        derived(&mut tx, linked.id).await,
        (true, false),
        "one item still under review"
    );
    tx.rollback().await.unwrap();
}

async fn send(app: &Router, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
    let timestamp = Utc::now().timestamp_millis().to_string();
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    for (name, value) in signed_headers_by(
        &wallet(OWNER_KEY),
        &method.to_lowercase(),
        path,
        "{}",
        true,
        &timestamp,
    )
    .await
    .iter()
    {
        request = request.header(name, value);
    }
    let body = if body.is_null() {
        Body::empty()
    } else {
        Body::from(body.to_string())
    };
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// Every route that reads derived state, on a database whose copy never
/// completed a pass, then on one whose pass is stale: 503 with nothing
/// written while the copy can't tell, reads served from a stale copy, writes
/// refused until the pass is fresh.
#[tokio::test]
async fn no_route_reads_or_writes_derived_state_while_the_copy_cannot_tell() {
    let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
        return;
    };
    let owner = format!("{:#x}", wallet(OWNER_KEY).address());
    let (collection, item, created, created_item) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    sqlx::query("INSERT INTO builder.collections (id, name, eth_address) VALUES ($1, 'Kept', $2)")
        .bind(collection)
        .bind(&owner)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO builder.items (id, collection_id, eth_address, name, type) VALUES ($1, $2, $3, 'Kept', 'wearable')",
    )
    .bind(item)
    .bind(collection)
    .bind(&owner)
    .execute(&pool)
    .await
    .unwrap();
    let state = std::sync::Arc::new(
        crate::State::new(
            pool.clone(),
            temp_storage().await,
            std::sync::Arc::default(),
            dcl_http::Outbound::for_tests(),
            Some(super::support::registry_only_index().await),
            None,
            crate::Settings {
                writes_enabled: true,
                ..crate::Settings::default()
            },
        )
        .unwrap(),
    );
    let app = crate::router(state);
    let reads = [
        "/v1/drafts/collections".to_string(),
        "/v1/drafts/items".to_string(),
    ];
    let decision_reads = [
        format!("/v1/collections/{collection}"),
        format!("/v1/collections/{collection}/publication"),
    ];
    let owner = format!("{:#x}", wallet(OWNER_KEY).address());
    let new_collection =
        super::support::collection_body(json!({ "id": created, "name": "Created" }), &owner);
    let item_body = |id: Uuid, name: &str| {
        json!({ "item": {
            "id": id, "name": name, "description": "", "type": "wearable", "eth_address": owner,
            "collection_id": collection,
            "contents": { "model.glb": dcl_auth::hash_bytes_v1(b"model") },
            "metrics": { "meshes": 1, "bodies": 1, "materials": 1, "textures": 1, "triangles": 1, "entities": 1 },
            "data": {
                "category": "hat", "replaces": [], "hides": [], "tags": [],
                "representations": [{
                    "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
                    "mainFile": "model.glb", "contents": ["model.glb"],
                    "overrideHides": [], "overrideReplaces": []
                }]
            }
        } })
    };
    let new_item = item_body(created_item, "Created");
    let kept_item = item_body(item, "Renamed");
    let writes = [
        (
            "PUT",
            format!("/v1/collections/{created}"),
            new_collection.clone(),
        ),
        (
            "PUT",
            format!("/v1/collections/{collection}"),
            super::support::collection_body(json!({ "id": collection, "name": "Renamed" }), &owner),
        ),
        ("PUT", format!("/v1/items/{created_item}"), new_item.clone()),
        ("PUT", format!("/v1/items/{item}"), kept_item.clone()),
        ("DELETE", format!("/v1/items/{item}"), Value::Null),
    ];
    let unchanged = async || {
        let rows: (i64, i64, String, String) = sqlx::query_as(
            "SELECT (SELECT count(*) FROM builder.collections), (SELECT count(*) FROM builder.items), \
                    (SELECT name FROM builder.collections WHERE id = $1), (SELECT name FROM builder.items WHERE id = $2)",
        )
        .bind(collection)
        .bind(item)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(rows, (1, 1, "Kept".to_string(), "Kept".to_string()));
    };
    for path in reads.iter().chain(&decision_reads) {
        let (status, body) = send(&app, "GET", path, Value::Null).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{path}: {body}");
        assert_eq!(body["ok"], false, "{path}: the upstream envelope");
    }
    for (method, path, body) in &writes {
        let (status, answer) = send(&app, method, path, body.clone()).await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "{method} {path}: {answer}"
        );
    }
    unchanged().await;

    let mut conn = pool.acquire().await.unwrap();
    set_pass(
        &mut conn,
        Some(Utc::now().naive_utc() - STALE_AFTER - TimeDelta::minutes(1)),
    )
    .await;
    for path in &reads {
        let (status, body) = send(&app, "GET", path, Value::Null).await;
        assert_ne!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "{path}: a stale copy serves reads: {body}"
        );
    }
    let (status, body) = send(&app, "GET", &decision_reads[0], Value::Null).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the owner reads its own draft from a stale copy, as its own listing does: {body}"
    );
    for path in &decision_reads[1..] {
        let (status, body) = send(&app, "GET", path, Value::Null).await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "{path}: a preparation guards against a published collection, so a stale copy refuses it: {body}"
        );
    }
    for (method, path, body) in &writes {
        let (status, answer) = send(&app, method, path, body.clone()).await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "{method} {path}: {answer}"
        );
    }
    unchanged().await;

    fresh_pass(&mut conn).await;
    let (_, body) = send(
        &app,
        "GET",
        &format!("/v1/collections/{collection}"),
        Value::Null,
    )
    .await;
    assert_eq!(body["data"]["is_published"], false, "{body}");
    for (method, path, body) in &writes {
        let (status, answer) = send(&app, method, path, body.clone()).await;
        assert_eq!(status, StatusCode::OK, "{method} {path}: {answer}");
    }
    drop(conn);
    pool.close().await;
}

async fn age_row(conn: &mut PgConnection, contract: &str, minutes: i64) {
    sqlx::query("UPDATE shop.collections SET mirrored_at = $2 WHERE network = 'matic' AND contract_address = $1")
        .bind(contract)
        .bind(ago(minutes))
        .execute(conn)
        .await
        .unwrap();
}

/// A decision dates what it read: a row by its own `mirrored_at`, an absence
/// by the start of the pass that walked past it.
#[tokio::test]
async fn a_decision_needs_the_row_or_the_absence_it_read_to_be_recent() {
    let Some(pool) = pool().await else {
        return;
    };
    let mut tx = pool.begin().await.unwrap();
    let (old_row, absent) = (random_address(), random_address());
    copy_row(
        &mut tx,
        "matic",
        &old_row,
        true,
        &["0x00000000000000000000000000000000000000aa"],
    )
    .await;
    age_row(&mut tx, &old_row, 31).await;
    let draft = builder_collection(&mut tx, false, Some(&absent)).await;
    let published_by_old_row = builder_collection(&mut tx, false, Some(&old_row)).await;

    set_window(&mut tx, Some(ago(0)), Some(ago(1)), None).await;
    let view = chain_state::for_decision(&mut tx).await.unwrap();
    assert_eq!(
        chain_state::collection(&mut tx, &view, &old_row)
            .await
            .unwrap_err()
            .status(),
        503,
        "a fresh pass whose walk left this row 31 minutes old grants nothing from it"
    );
    assert!(
        chain_state::published(&mut tx, &view, published_by_old_row.id)
            .await
            .unwrap(),
        "an old row still proves publication: published only grows"
    );
    assert_eq!(
        chain_state::collection(&mut tx, &view, &absent)
            .await
            .unwrap(),
        None
    );
    assert!(
        !chain_state::published(&mut tx, &view, draft.id)
            .await
            .unwrap()
    );

    set_window(&mut tx, Some(ago(0)), Some(ago(31)), None).await;
    let view = chain_state::for_decision(&mut tx).await.unwrap();
    assert_eq!(
        chain_state::collection(&mut tx, &view, &absent)
            .await
            .unwrap_err()
            .status(),
        503,
        "an absence is as old as the start of the pass that saw it"
    );
    assert_eq!(
        chain_state::published(&mut tx, &view, draft.id)
            .await
            .unwrap_err()
            .status(),
        503,
        "a write guard on a draft with a contract dates its absence too"
    );
    let no_contract = builder_collection(&mut tx, false, None).await;
    assert!(
        !chain_state::published(&mut tx, &view, no_contract.id)
            .await
            .unwrap(),
        "a draft without a contract consults no copy row"
    );

    let (below, above) = (
        format!("0x0{}", &absent[3..]),
        format!("0xf{}", &absent[3..]),
    );
    let cursor = format!("0x8{}", &absent[3..]);
    set_window(&mut tx, Some(ago(20)), Some(ago(1)), Some(&cursor)).await;
    let view = chain_state::for_decision(&mut tx).await.unwrap();
    assert_eq!(
        chain_state::collection(&mut tx, &view, &below)
            .await
            .unwrap(),
        None,
        "the pass in progress walked past this contract a minute ago"
    );
    assert_eq!(
        chain_state::collection(&mut tx, &view, &above)
            .await
            .unwrap_err()
            .status(),
        503,
        "beyond the cursor, the absence dates from a pass whose start is gone"
    );

    set_window(&mut tx, Some(ago(0)), None, None).await;
    let view = chain_state::for_decision(&mut tx).await.unwrap();
    assert_eq!(
        chain_state::collection(&mut tx, &view, &absent)
            .await
            .unwrap_err()
            .status(),
        503,
        "a pass with no recorded start dates no absence"
    );
    tx.rollback().await.unwrap();
}

/// What each reader requires: the facts and the guard answer only through a
/// decision view, which a stale copy never yields; the listing SQL through a
/// read view, which it does. That the facts and the guard can't take a read
/// view, and that no view can be built outside the module, is checked by the
/// `compile_fail` examples in `chain_state`.
#[tokio::test]
async fn facts_and_guards_need_a_decision_view_and_listings_a_read_view() {
    let Some(pool) = pool().await else {
        return;
    };
    let mut tx = pool.begin().await.unwrap();
    let contract = random_address();
    copy_row(&mut tx, "matic", &contract, true, &[]).await;
    let collection = builder_collection(&mut tx, false, Some(&contract)).await;

    set_pass(&mut tx, Some(ago(0))).await;
    let decision = chain_state::for_decision(&mut tx).await.unwrap();
    assert!(
        chain_state::collection(&mut tx, &decision, &contract)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        chain_state::published(&mut tx, &decision, collection.id)
            .await
            .unwrap()
    );

    set_pass(&mut tx, Some(ago(31))).await;
    assert_eq!(
        chain_state::for_decision(&mut tx)
            .await
            .unwrap_err()
            .status(),
        503,
        "no decision view, so neither the facts nor the guard answer"
    );
    let read = chain_state::for_read(&mut tx).await.unwrap();
    let listed: bool = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT {} FROM {} WHERE c.id = $1",
        read.published(),
        read.collections()
    )))
    .bind(collection.id)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert!(listed, "a listing still reads the stale copy");
    tx.rollback().await.unwrap();
}

#[test]
fn the_unavailable_condition_is_logged_once_per_change() {
    let log = chain_state::ConditionLog::default();
    let seen: Vec<bool> = [
        chain_state::Condition::NoPass,
        chain_state::Condition::NoPass,
        chain_state::Condition::Stale,
        chain_state::Condition::Stale,
        chain_state::Condition::Fresh,
        chain_state::Condition::Fresh,
        chain_state::Condition::NoPass,
    ]
    .into_iter()
    .map(|condition| log.changed(condition))
    .collect();
    assert_eq!(seen, [true, false, true, false, true, false, true]);
}

#[tokio::test]
async fn a_standard_collection_this_node_published_waits_in_the_committees_queue() {
    let Some(pool) = super::validator::own_pool().await else {
        return;
    };
    let member = super::validator::address(OWNER_KEY);
    let mut queued = Vec::new();
    for urn_suffix in [false, true] {
        let mut conn = pool.acquire().await.unwrap();
        let c = builder_collection(&mut conn, false, None).await;
        let contract = super::support::on_chain(&pool, c.id, false).await;
        if urn_suffix {
            sqlx::query("UPDATE builder.collections SET urn_suffix = $2 WHERE id = $1")
                .bind(c.id)
                .bind(format!("urn:decentraland:matic:collections-v2:{contract}"))
                .execute(&pool)
                .await
                .unwrap();
        }
        sqlx::query(
            "INSERT INTO builder.collection_curations (id, collection_id, status) VALUES ($1, $2, 'pending')",
        )
        .bind(Uuid::new_v4())
        .bind(c.id)
        .execute(&pool)
        .await
        .unwrap();
        queued.push(c.id.to_string());
    }
    let committee =
        super::support::committee_index(&super::support::FakeCommittee::with(&[&member])).await;
    let app = super::limits::app_over(pool, |state| {
        state.market = Some(committee);
        state.limits = crate::limits::Limits::unbounded();
    })
    .await;
    for status in ["to_review", "pending"] {
        let (code, body, _) = super::limits::send(
            &app,
            &super::limits::Call {
                key: Some(OWNER_KEY),
                ..super::limits::Call::get(format!(
                    "/v1/collections?status={status}&page=1&limit=50"
                ))
            },
        )
        .await;
        assert_eq!(code, 200, "{status}: {body}");
        let listed: Value = serde_json::from_str(&body).unwrap();
        let mut ids: Vec<String> = listed["data"]["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["id"].as_str().unwrap().to_string())
            .collect();
        ids.sort();
        let mut expected = queued.clone();
        expected.sort();
        assert_eq!(ids, expected, "{status}");
    }
}

#[tokio::test]
async fn the_committee_cannot_filter_by_an_automated_review_the_node_does_not_run() {
    let Some(pool) = super::validator::own_pool().await else {
        return;
    };
    let member = super::validator::address(OWNER_KEY);
    let committee =
        super::support::committee_index(&super::support::FakeCommittee::with(&[&member])).await;
    let app = super::limits::app_over(pool, |state| {
        state.market = Some(committee);
        state.limits = crate::limits::Limits::unbounded();
    })
    .await;
    let (code, body, _) = super::limits::send(
        &app,
        &super::limits::Call {
            key: Some(OWNER_KEY),
            ..super::limits::Call::get("/v1/collections?status=auto_reviewing&page=1&limit=50")
        },
    )
    .await;
    let answer: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        (code, answer["error"].as_str(), &answer["data"]["status"]),
        (400, Some("Invalid status"), &json!("auto_reviewing")),
        "{body}"
    );
}
