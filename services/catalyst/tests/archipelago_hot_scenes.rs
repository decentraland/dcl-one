// from: catalyrst/crates/catalyrst-archipelago/tests/hot_scenes_content_pg.rs, catalyrst/crates/catalyrst-archipelago/tests/batched_thumbnails.rs
//! Uncached active content enrichment behind /hot-scenes, over the
//! real `content.*` tables of a scratch database (skips without `CATALYST_TEST_DATABASE_URL`).
//! The old statement counter (`sql_capture`) is gone with the test-gate crate; the thumbnail
//! fallback is asserted through its results.

use serde_json::json;
use sqlx::PgPool;

#[path = "support/db.rs"]
mod db;

async fn deploy(
    pool: &PgPool,
    entity_id: &str,
    entity_type: &str,
    metadata: serde_json::Value,
    pointers: &[&str],
    deleted: bool,
) -> i32 {
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, \
         entity_timestamp, entity_pointers, local_timestamp, auth_chain, deleter_deployment) \
         VALUES ('0x0000000000000000000000000000000000000001', 'v3', $1, $2, $3, timezone('UTC', now()), $4, \
         timezone('UTC', now()), '[]'::json, CASE WHEN $5 THEN 1 ELSE NULL END) RETURNING id",
    )
    .bind(entity_type)
    .bind(entity_id)
    .bind(sqlx::types::Json(json!({"v": metadata})))
    .bind(pointers.iter().map(|p| p.to_string()).collect::<Vec<_>>())
    .bind(deleted)
    .fetch_one(pool)
    .await
    .unwrap();
    for p in pointers {
        sqlx::query(
            "INSERT INTO content.active_pointers (pointer, entity_id, entity_type) VALUES ($1, $2, $3) \
             ON CONFLICT (pointer) DO UPDATE SET entity_id = EXCLUDED.entity_id",
        )
        .bind(p)
        .bind(entity_id)
        .bind(entity_type)
        .execute(pool)
        .await
        .unwrap();
    }
    id
}

fn scene_meta(base: &str, parcels: &[&str], thumbnail: &str) -> serde_json::Value {
    json!({
        "display": { "title": format!("scene at {base}"), "navmapThumbnail": thumbnail },
        "scene": { "base": base, "parcels": parcels },
        "contact": { "name": "creator" },
    })
}

#[tokio::test]
async fn active_content_updates_are_visible_without_a_tile_cache() {
    use catalyst::comms::stats::content::{Content, Source, project_hot_scenes};
    let scratch = db::scratch_database()
        .await
        .expect("CATALYST_TEST_DATABASE_URL is required");
    let pool = &scratch.pool;
    let id = deploy(
        pool,
        "scene",
        "scene",
        scene_meta("0,0", &["0,0"], "thumb.png"),
        &["0,0"],
        false,
    )
    .await;
    sqlx::query("INSERT INTO content.content_files (deployment,key,content_hash) VALUES ($1,'thumb.png','hash')").bind(id).execute(pool).await.unwrap();
    let source = Content {
        pool: pool.clone(),
        denylist: std::sync::Arc::new(catalyst::content::Denylist::default()),
    };
    let docs = source.fetch(&["0,0".into()]).await.unwrap();
    let scenes =
        project_hot_scenes(&docs, &[("0,0".into(), 2)], "https://content.invalid/").unwrap();
    assert_eq!(scenes[0].users_total_count, 2);
    assert_eq!(
        scenes[0]
            .thumbnail
            .as_ref()
            .and_then(catalyst::content::OrderedJson::as_str),
        Some("https://content.invalid//contents/hash")
    );
    sqlx::query("UPDATE content.deployments SET entity_metadata=$1 WHERE id=$2")
        .bind(sqlx::types::Json(
            json!({"v": scene_meta("0,0", &["0,0"], "https://new.invalid/image")}),
        ))
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    let docs = source.fetch(&["0,0".into()]).await.unwrap();
    let scenes =
        project_hot_scenes(&docs, &[("0,0".into(), 2)], "https://content.invalid/").unwrap();
    assert_eq!(
        scenes[0]
            .thumbnail
            .as_ref()
            .and_then(catalyst::content::OrderedJson::as_str),
        Some("https://new.invalid/image")
    );
    scratch.teardown().await;
}
