// from: catalyrst/crates/catalyrst-archipelago/tests/hot_scenes_content_pg.rs, catalyrst/crates/catalyrst-archipelago/tests/batched_thumbnails.rs
//! The per-tile scene cache and the single joined scene query behind /hot-scenes, over the
//! real `content.*` tables of a scratch database (skips without `CATALYST_TEST_DATABASE_URL`).
//! The old statement counter (`sql_capture`) is gone with the test-gate crate; the thumbnail
//! fallback is asserted through its results.

use std::collections::HashMap;

use serde_json::json;
use sqlx::PgPool;

use catalyst::comms::archipelago::content::ContentResolver;

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
    .bind(sqlx::types::Json(metadata))
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
async fn hot_scenes_resolve_thumbnails_in_the_scene_query_and_cache_per_tile() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let pool = scratch.pool.clone();

    let s1 = deploy(
        &pool,
        "bafy-s1",
        "scene",
        scene_meta("0,0", &["0,0", "0,1"], "thumb.png"),
        &["0,0", "0,1"],
        false,
    )
    .await;
    sqlx::query("INSERT INTO content.content_files (deployment, key, content_hash) VALUES ($1, 'thumb.png', 'bafy-thumb')")
        .bind(s1)
        .execute(&pool)
        .await
        .unwrap();
    deploy(
        &pool,
        "bafy-s2",
        "scene",
        json!({ "v": scene_meta("5,5", &["5,5"], "https://cdn/t.png") }),
        &["5,5"],
        false,
    )
    .await;
    deploy(
        &pool,
        "bafy-s3",
        "scene",
        scene_meta("9,9", &["9,9"], "gone.png"),
        &["9,9"],
        true,
    )
    .await;
    deploy(
        &pool,
        "bafy-p1",
        "profile",
        json!({}),
        &["0xprofile"],
        false,
    )
    .await;

    let resolver = ContentResolver::new(pool.clone(), "https://content/".into(), 60);
    let tiles: Vec<String> = ["0,0", "5,5", "9,9", "7,7"]
        .iter()
        .map(|t| t.to_string())
        .collect();
    let scenes = resolver.fetch_scenes(&tiles).await.expect("scenes");
    let ids: Vec<&str> = scenes.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["bafy-s1", "bafy-s2"],
        "deleted and non-scene entities are skipped"
    );
    assert_eq!(
        scenes[0].thumbnail.as_deref(),
        Some("https://content/contents/bafy-thumb"),
        "a file-key thumbnail resolves to its content hash in the same query"
    );
    assert_eq!(scenes[0].parcels, vec!["0,0", "0,1"]);
    assert_eq!(scenes[0].creator.as_deref(), Some("creator"));
    assert_eq!(
        scenes[1].thumbnail.as_deref(),
        Some("https://cdn/t.png"),
        "an http thumbnail is kept verbatim; v-wrapped metadata is unwrapped"
    );
    assert_eq!(scenes[1].base, [5, 5]);

    deploy(
        &pool,
        "bafy-s4",
        "scene",
        scene_meta("7,7", &["7,7"], "x.png"),
        &["7,7"],
        false,
    )
    .await;
    let again = resolver
        .fetch_scenes(&["0,1".to_string(), "7,7".to_string()])
        .await
        .expect("scenes");
    let ids: Vec<&str> = again.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["bafy-s1"],
        "an unseen tile is queried, a tile seen inside the TTL is served from its entry"
    );

    scratch.teardown().await;
}

#[tokio::test]
async fn thumbnails_keep_entity_scope_external_urls_and_optional_failure() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let pool = scratch.pool.clone();
    for (id, thumb, wrapped) in [
        ("a", Some("preview.png"), true),
        ("b", Some("preview.png"), false),
        ("c", Some("https://example.org/image.png"), true),
        ("d", Some("missing.png"), true),
        ("e", None, true),
    ] {
        let metadata = json!({"display":{"title":id,"navmapThumbnail":thumb},"scene":{"base":"0,0","parcels":["0,0"]}});
        let metadata = if wrapped {
            json!({"v":metadata})
        } else {
            metadata
        };
        let deployment = deploy(&pool, id, "scene", metadata, &[id], false).await;
        sqlx::query("INSERT INTO content.content_files (deployment, key, content_hash) VALUES ($1, 'preview.png', $2)")
            .bind(deployment)
            .bind(format!("hash-{id}"))
            .execute(&pool)
            .await
            .unwrap();
    }
    let resolver = ContentResolver::new(pool.clone(), "http://content".into(), 30);
    let pointers: Vec<_> = ["a", "b", "c", "d", "e"]
        .into_iter()
        .map(str::to_string)
        .collect();
    let scenes = resolver.fetch_scenes(&pointers).await.unwrap();
    assert_eq!(scenes.len(), 5);
    let thumbs: HashMap<_, _> = scenes.into_iter().map(|s| (s.id, s.thumbnail)).collect();
    assert_eq!(
        thumbs["a"].as_deref(),
        Some("http://content/contents/hash-a")
    );
    assert_eq!(
        thumbs["b"].as_deref(),
        Some("http://content/contents/hash-b")
    );
    assert_eq!(
        thumbs["c"].as_deref(),
        Some("https://example.org/image.png")
    );
    assert_eq!(thumbs["d"], None);
    assert_eq!(thumbs["e"], None);
    sqlx::query("DROP TABLE content.content_files")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        resolver.fetch_scenes(&pointers).await.unwrap().len(),
        5,
        "tiles inside the TTL are served from the cache"
    );
    let uncached = ContentResolver::new(pool.clone(), "http://content".into(), 30);
    let scenes = uncached.fetch_scenes(&pointers).await.unwrap();
    assert_eq!(
        scenes.len(),
        5,
        "a failed joined statement is retried without thumbnail hashes: optional thumbnail failures preserve scenes"
    );
    assert!(
        scenes
            .iter()
            .filter(|s| s.id != "c")
            .all(|s| s.thumbnail.is_none())
    );
    scratch.teardown().await;
}
