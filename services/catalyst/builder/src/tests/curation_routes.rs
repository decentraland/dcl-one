//! The curation routes beyond the authorization matrix: every cell of the
//! transition table over HTTP, what a write leaves (its row, its timeline
//! event, its `ops.admin_audit` row, the videos an approval pins), the Slack
//! notices and when they are sent, the callers the committee no longer or
//! never admits, the registry outage, concurrent curators, hostile input and
//! the item listing's curation filters.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use super::support::{
    FakeCommittee, FakeRegistry, OWNER_KEY, registry_index, signed_map, temp_storage, wallet,
};
const COMMITTEE_KEY: &str = "0x3333333333333333333333333333333333333333333333333333333333333333";
const ADMIN_KEY: &str = "0x8888888888888888888888888888888888888888888888888888888888888888";
const PROVIDER: &str = "urn:decentraland:matic:collections-thirdparty:curation";
const TOKEN: &str = "xoxb-example123";

fn address(key: &str) -> String {
    format!("{:#x}", wallet(key).address())
}

fn hash(n: u8) -> String {
    format!("{n:02x}").repeat(32)
}

struct Node {
    pool: PgPool,
    app: Router,
}

struct Options {
    committee: FakeCommittee,
    registry: FakeRegistry,
    slack: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            committee: FakeCommittee::with(&[&address(COMMITTEE_KEY)]),
            registry: FakeRegistry::up(),
            slack: None,
        }
    }
}

async fn node_over(pool: PgPool, options: Options) -> Node {
    sqlx::query(
        "INSERT INTO builder.virtual_third_parties (id, managers, raw_metadata) VALUES ($1, $2, 'tp:1:Curation:') \
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(PROVIDER)
    .bind(vec![address(OWNER_KEY)])
    .execute(&pool)
    .await
    .unwrap();
    let mut state = crate::State::new(
        pool.clone(),
        temp_storage().await,
        Arc::default(),
        dcl_http::Outbound::for_tests(),
        Some(registry_index(&options.committee, &options.registry).await),
        None,
        crate::Settings {
            writes_enabled: true,
            admin_addresses: vec![address(ADMIN_KEY)],
            slack: options.slack.map(|api| crate::curation::slack::Slack {
                token: TOKEN.into(),
                channel: "curation-example".into(),
                api,
            }),
            links: crate::curation::slack::Links {
                creators_url: Some("https://creators.example.org".into()),
                builder_url: None,
            },
            ..crate::Settings::default()
        },
    )
    .unwrap();
    state.guarded = dcl_http::Outbound::for_tests();
    let state = Arc::new(state);
    let app = Router::new()
        .merge(crate::root_router(state.clone()))
        .nest("/builder-api", crate::router(state));
    Node { pool, app }
}

async fn node(options: Options) -> Option<Node> {
    Some(node_over(super::support::scratch_pool(8).await?, options).await)
}

async fn call(
    app: &Router,
    key: Option<&str>,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (u16, Value) {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(key) = key {
        let signed = path.split('?').next().unwrap();
        for (name, value) in signed_map(key, &method.to_lowercase(), signed, "{}", false).await {
            request = request.header(name, value);
        }
    }
    let body = match body {
        Some(body) => {
            request = request.header("content-type", "application/json");
            Body::from(body.to_string())
        }
        None => Body::empty(),
    };
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), 16 << 20)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// A third-party collection of the provider the owner manages, with one item
/// of the owner's whose content hash is `hash(1)`; `anchored` adds a second
/// item with an approved curation, so the collection is published whatever
/// the first one's curations.
async fn third_party(pool: &PgPool, anchored: bool) -> (Uuid, Uuid) {
    let id = Uuid::new_v4();
    let urn = format!("{PROVIDER}:{}", id.simple());
    sqlx::query(
        "INSERT INTO builder.collections (id, name, eth_address, third_party_id, urn_suffix) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(format!("Curated {}", &id.simple().to_string()[..8]))
    .bind(address(OWNER_KEY))
    .bind(PROVIDER)
    .bind(&urn)
    .execute(pool)
    .await
    .unwrap();
    let item = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO builder.items (id, collection_id, eth_address, name, type, urn_suffix, local_content_hash) \
         VALUES ($1, $2, $3, 'Curated item', 'wearable', $4, $5)",
    )
    .bind(item)
    .bind(id)
    .bind(address(OWNER_KEY))
    .bind(format!("{urn}:item"))
    .bind(hash(1))
    .execute(pool)
    .await
    .unwrap();
    if anchored {
        let anchor = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO builder.items (id, collection_id, eth_address, name, type, urn_suffix, local_content_hash) \
             VALUES ($1, $2, $3, 'Anchor', 'wearable', $4, $5)",
        )
        .bind(anchor)
        .bind(id)
        .bind(address(OWNER_KEY))
        .bind(format!("{urn}:anchor"))
        .bind(hash(8))
        .execute(pool)
        .await
        .unwrap();
        item_curation(pool, anchor, "approved").await;
    }
    (id, item)
}

async fn item_curation(pool: &PgPool, item: Uuid, status: &str) {
    sqlx::query(
        "INSERT INTO builder.item_curations (id, item_id, status, content_hash, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, timezone('UTC', now()) - interval '1 hour', timezone('UTC', now()) - interval '1 hour')",
    )
    .bind(Uuid::new_v4())
    .bind(item)
    .bind(status)
    .bind(hash(9))
    .execute(pool)
    .await
    .unwrap();
}

/// A collection deleted while an operator's bulk write waits for it is
/// decided again: the operator is told it is gone.
#[tokio::test]
async fn an_operator_bulk_write_on_a_collection_deleted_meanwhile_answers_not_found() {
    let Some(n) = node(Options::default()).await else {
        return;
    };
    let (collection, item) = third_party(&n.pool, true).await;
    item_curation(&n.pool, item, "pending").await;
    let mut holder = n.pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM builder.collections WHERE id = $1 FOR UPDATE")
        .bind(collection)
        .execute(&mut *holder)
        .await
        .unwrap();
    let app = n.app.clone();
    let path = format!("/v1/collections/{collection}/items/status");
    let operator = tokio::spawn(async move {
        call(
            &app,
            Some(ADMIN_KEY),
            "PATCH",
            &path,
            Some(json!({ "status": "approved", "itemIds": [item] })),
        )
        .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    sqlx::query("DELETE FROM builder.collections WHERE id = $1")
        .bind(collection)
        .execute(&mut *holder)
        .await
        .unwrap();
    holder.commit().await.unwrap();
    let (status, answer) = operator.await.unwrap();
    assert_eq!(status, 404, "{answer}");
}

/// `GET /v1/collections/:id/items`' curation filters read each item's newest
/// item curation, as builder-server's do.
#[tokio::test]
async fn the_item_list_filters_by_each_items_newest_curation() {
    let Some(n) = node(Options::default()).await else {
        return;
    };
    let (collection, synced) = third_party(&n.pool, false).await;
    let mut ids = vec![synced];
    for (label, hash_of_item) in [("changed", 2), ("pending", 3), ("mapped", 4), ("never", 5)] {
        let item = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO builder.items (id, collection_id, eth_address, name, type, urn_suffix, local_content_hash, mappings) \
             SELECT $1, collection_id, eth_address, $2, type, urn_suffix || $2, $3, \
               CASE WHEN $2 IN ('mapped', 'never') THEN '{}'::jsonb END FROM builder.items WHERE id = $4",
        )
        .bind(item)
        .bind(label)
        .bind(hash(hash_of_item))
        .bind(synced)
        .execute(&n.pool)
        .await
        .unwrap();
        ids.push(item);
    }
    let curate = |item: Uuid, status: &'static str, content: String, complete: Option<bool>| {
        let pool = n.pool.clone();
        async move {
            sqlx::query(
                "INSERT INTO builder.item_curations (id, item_id, status, content_hash, is_mapping_complete) VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(Uuid::new_v4())
            .bind(item)
            .bind(status)
            .bind(content)
            .bind(complete)
            .execute(&pool)
            .await
            .unwrap();
        }
    };
    curate(ids[0], "approved", hash(1), None).await;
    curate(ids[1], "approved", hash(7), None).await;
    curate(ids[2], "pending", hash(3), None).await;
    curate(ids[3], "approved", hash(4), Some(true)).await;
    let listed = |query: &'static str| {
        let app = n.app.clone();
        async move {
            let (status, answer) = call(
                &app,
                Some(OWNER_KEY),
                "GET",
                &format!("/v1/collections/{collection}/items?{query}"),
                None,
            )
            .await;
            assert_eq!(status, 200, "{query}: {answer}");
            let mut names: Vec<String> = answer["data"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i["name"].as_str().unwrap().to_string())
                .collect();
            names.sort();
            names
        }
    };
    assert_eq!(listed("synced=true").await, ["Curated item", "mapped"]);
    assert_eq!(listed("synced=false").await, ["changed"]);
    assert_eq!(listed("status=approved").await, ["pending"]);
    assert_eq!(listed("mappingStatus=unpublished_mapping").await, ["never"]);
    assert_eq!(listed("mappingStatus=missing_mapping").await.len(), 3);
}

/// The signer `key` as a signed `PATCH` of `path` proves it.
pub(super) async fn signer_of(key: &str, path: &str) -> crate::shared::signed_fetch::Signer {
    let mut headers = axum::http::HeaderMap::new();
    for (name, value) in signed_map(key, "patch", path, "{}", false).await {
        headers.insert(
            axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            value.parse().unwrap(),
        );
    }
    crate::auth::require_signer(
        &headers,
        "patch",
        &crate::auth::SignedPath::local(path.to_string()),
        None,
    )
    .await
    .unwrap()
}
