use std::collections::HashSet;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

use super::*;
use crate::queries::MaskedColumns;
use crate::tests::support::{TestNode, require_test_db};

fn fields_of(query: &str) -> Option<Vec<EntityField>> {
    requested_fields(&parse_query_string(query))
}

#[test]
fn fields_parse_leniently_and_the_first_occurrence_wins() {
    use EntityField::{Content, Metadata, Pointers};
    assert_eq!(fields_of(""), None);
    assert_eq!(fields_of("other=1"), None);
    assert_eq!(fields_of("fields=pointers"), Some(vec![Pointers]));
    assert_eq!(
        fields_of("fields=POINTERS,%20metadata,bogus"),
        Some(vec![Pointers, Metadata])
    );
    assert_eq!(fields_of("fields="), Some(vec![]));
    assert_eq!(fields_of("fields=bogus"), Some(vec![]));
    assert_eq!(
        fields_of("fields=pointers&fields=content"),
        Some(vec![Pointers])
    );
    assert_eq!(fields_of("fields=content"), Some(vec![Content]));
}

#[test]
fn masked_projection_reads_content_files_and_metadata_only_when_kept() {
    for metadata in [false, true] {
        let sql = MaskedColumns {
            content: false,
            metadata,
        }
        .projection();
        assert!(!sql.contains("content_files"), "{sql}");
        assert!(sql.contains("'[]'::json AS content_json"), "{sql}");
        assert_eq!(sql.contains("dep.entity_metadata"), metadata, "{sql}");
        assert_eq!(
            sql.contains("NULL::json AS entity_metadata"),
            !metadata,
            "{sql}"
        );
    }
    let sql = MaskedColumns {
        content: true,
        metadata: false,
    }
    .projection();
    assert!(sql.contains("content.content_files"), "{sql}");
    assert!(sql.contains("dep.entity_metadata,"), "{sql}");
}

struct Seed {
    id: String,
    pointers: Vec<String>,
}

async fn seed(
    pool: &sqlx::PgPool,
    tag: &str,
    n: u32,
    entity_type: &str,
    pointers: &[&str],
    metadata: Value,
    files: &[&str],
) -> Seed {
    let id = format!("test-{tag}-{n}");
    let pointers: Vec<String> = pointers.iter().map(|p| p.to_string()).collect();
    let deployment: i32 = sqlx::query_scalar(
        "INSERT INTO content.deployments
            (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp,
             entity_pointers, local_timestamp, auth_chain)
         VALUES ('0xDEPLOYER', 'v3', $1, $2, $3::json, to_timestamp($4 / 1000.0), $5,
                 to_timestamp($4 / 1000.0), '[{\"type\":\"SIGNER\",\"payload\":\"0xdeployer\"}]'::json)
         RETURNING id",
    )
    .bind(entity_type)
    .bind(&id)
    .bind(json!({ "v": metadata }))
    .bind((1_700_000_000_000i64 + i64::from(n) * 1000) as f64)
    .bind(&pointers)
    .fetch_one(pool)
    .await
    .unwrap();
    for (i, key) in files.iter().enumerate() {
        sqlx::query(
            "INSERT INTO content.content_files (deployment, content_hash, key) VALUES ($1, $2, $3)",
        )
        .bind(deployment)
        .bind(format!("test-{tag}-hash-{n}-{i}"))
        .bind(key)
        .execute(pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO content.active_pointers (pointer, entity_id, entity_type)
         SELECT p, $2, $3 FROM unnest($1::text[]) AS p",
    )
    .bind(&pointers)
    .bind(&id)
    .bind(entity_type)
    .execute(pool)
    .await
    .unwrap();
    Seed { id, pointers }
}

async fn unseed(pool: &sqlx::PgPool, tag: &str) {
    let like = format!("test-{tag}-%");
    for sql in [
        "DELETE FROM content.content_files WHERE content_hash LIKE $1",
        "DELETE FROM content.active_pointers WHERE entity_id LIKE $1",
        "DELETE FROM content.deployments WHERE entity_id LIKE $1",
    ] {
        sqlx::query(sql).bind(&like).execute(pool).await.unwrap();
    }
}

struct World {
    matching: Seed,
    differing: Seed,
    profile: Seed,
}

async fn seed_world(pool: &sqlx::PgPool, tag: &str, base: i32) -> World {
    unseed(pool, tag).await;
    let p = |x: i32, y: i32| format!("{},{}", base + x, base + y);
    let (a, b, c, d) = (p(0, 0), p(0, 1), p(1, 0), p(3, 3));
    let matching = seed(
        pool,
        tag,
        1,
        "scene",
        &[&a, &b, &c],
        json!({
            "main": "bin/index.js",
            "scene": {"base": a, "parcels": [c, a, b]},
            "display": {"title": "Pla\u{e7}a \u{2713}", "ratio": 1.0},
        }),
        &[
            "scene.json",
            "models/b.glb",
            "A.png",
            "move_02.glb",
            "move-04.glb",
        ],
    )
    .await;
    let differing = seed(
        pool,
        tag,
        2,
        "scene",
        &[&d],
        json!({"main": "main.js", "scene": {"base": d, "parcels": [d, p(3, 4)]}}),
        &["scene.json"],
    )
    .await;
    let profile = seed(
        pool,
        tag,
        3,
        "profile",
        &[&format!("0x{:040x}", base)],
        json!({"avatars": [{"name": "m"}]}),
        &[],
    )
    .await;
    World {
        matching,
        differing,
        profile,
    }
}

async fn post(node: &TestNode, uri: &str, body: &Value) -> (StatusCode, Option<String>, Vec<u8>) {
    let resp = node
        .router()
        .oneshot(
            Request::post(uri)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let content_type = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, content_type, bytes.to_vec())
}

fn masked_bytes(full: &[EntityDoc], fields: &[EntityField]) -> Vec<u8> {
    let masked: Vec<Value> = full
        .iter()
        .filter_map(EntityDoc::to_value)
        .map(|e| mask_entity(&e, Some(fields)))
        .collect();
    serde_json::to_vec(&Value::Array(masked)).unwrap()
}

const FIELD_QUERIES: &[&str] = &[
    "fields=pointers",
    "fields=pointers,metadata",
    "fields=metadata",
    "fields=content",
    "fields=content,pointers",
    "fields=CONTENT,pointers,metadata",
    "fields=",
    "fields=bogus",
    "fields=pointers&fields=content",
];

#[tokio::test]
async fn post_without_fields_is_unchanged_and_with_fields_equals_the_full_documents_masked() {
    require_test_db!(Some(node) = TestNode::with_db("maskptr").await);
    let pool = node.state.pool.clone();
    let tag = "maskptr";
    let world = seed_world(&pool, tag, 7600).await;
    let mut pointers: Vec<String> = world
        .matching
        .pointers
        .iter()
        .chain(&world.differing.pointers)
        .cloned()
        .collect();
    pointers.push(world.profile.pointers[0].to_uppercase());
    pointers.push("7699,7699".to_string());
    let body = json!({ "pointers": pointers });

    let full = queries::active_entity_docs_by_pointers(&pool, &pointers)
        .await
        .unwrap();
    assert_eq!(full.len(), 3);
    let legacy = EntityDoc::json_array(&full).to_vec();
    for uri in [
        "/content/entities/active",
        "/content/entities/active?",
        "/content/entities/active?unrelated=1&field=pointers",
    ] {
        let (status, content_type, bytes) = post(&node, uri, &body).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert_eq!(content_type.as_deref(), Some("application/json"), "{uri}");
        assert_eq!(bytes, legacy, "{uri}");
    }

    for query in FIELD_QUERIES {
        let fields = fields_of(query).unwrap();
        let (status, content_type, bytes) =
            post(&node, &format!("/content/entities/active?{query}"), &body).await;
        assert_eq!(status, StatusCode::OK, "{query}");
        assert_eq!(content_type.as_deref(), Some("application/json"), "{query}");
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            String::from_utf8(masked_bytes(&full, &fields)).unwrap(),
            "{query}"
        );
    }

    let (_, _, bytes) = post(&node, "/content/entities/active?fields=pointers", &body).await;
    let answer: Value = serde_json::from_slice(&bytes).unwrap();
    let scene = answer
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == world.matching.id.as_str())
        .unwrap();
    assert_eq!(
        scene,
        &json!({
            "content": [],
            "id": world.matching.id,
            "pointers": world.matching.pointers,
            "timestamp": 1_700_000_001_000i64,
            "type": "scene",
            "version": "v3",
        })
    );

    unseed(&pool, tag).await;
}

#[tokio::test]
async fn masked_post_by_ids_honours_the_denylist() {
    require_test_db!(Some(node) = TestNode::with_db("maskids").await);
    let pool = node.state.pool.clone();
    let tag = "maskids";
    let world = seed_world(&pool, tag, 7620).await;
    let ids = vec![
        world.differing.id.clone(),
        world.matching.id.clone(),
        world.profile.id.clone(),
        "test-maskids-404".to_string(),
    ];
    let body = json!({ "ids": ids });
    let full = queries::active_entity_docs_by_ids(&pool, &ids)
        .await
        .unwrap();
    assert_eq!(full.len(), 3);

    let (status, _, bytes) = post(&node, "/content/entities/active", &body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, EntityDoc::json_array(&full).to_vec());
    for query in FIELD_QUERIES {
        let fields = fields_of(query).unwrap();
        let (status, _, bytes) =
            post(&node, &format!("/content/entities/active?{query}"), &body).await;
        assert_eq!(status, StatusCode::OK, "{query}");
        assert_eq!(bytes, masked_bytes(&full, &fields), "{query}");
    }

    node.state.denylist.add(&world.matching.id);
    let (_, _, bytes) = post(&node, "/content/entities/active?fields=pointers", &body).await;
    let answer: Value = serde_json::from_slice(&bytes).unwrap();
    let served: HashSet<&str> = answer
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        served,
        HashSet::from([world.differing.id.as_str(), world.profile.id.as_str()])
    );

    unseed(&pool, tag).await;
}
