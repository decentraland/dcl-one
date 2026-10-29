//! A bounded pass against a real upstream (`WORLDS_LIVE_SOURCE`, the mirror
//! posture's WORLDS_CONTENT_MIRROR_SOURCE for instance: `catalyst posture env
//! mirror WORLDS_CONTENT_MIRROR_SOURCE`): the first three catalogue names,
//! then a second pass that must find them unchanged, and every listed scene
//! and file present in the blob store. Network-bound and opt-in: it runs only
//! with `CATALYST_WORLDS_LIVE=1`, so the ordinary suite never touches the
//! upstream; opted in, a missing `WORLDS_LIVE_SOURCE` or
//! `CATALYST_TEST_DATABASE_URL` fails it.
//!
//! ```sh
//! CATALYST_WORLDS_LIVE=1 WORLDS_LIVE_SOURCE=<source> CATALYST_TEST_DATABASE_URL="$DATABASE_URL" \
//!   cargo test --test worlds_live -- --nocapture
//! ```

use std::sync::Arc;

use catalyst::jobs::Jobs;
use catalyst::storage::Storage;
use catalyst::worlds::mirror::{MirrorSettings, run_pass};

#[path = "support/db.rs"]
mod db;

const LIMIT: usize = 3;

/// The upstream the pass reads once `CATALYST_WORLDS_LIVE=1` opts in; `None`
/// (skip) without the opt-in. Opted in with no source panics: a skip there
/// would report a pass that read nothing.
fn live_source(opt_in: Option<&str>, source: Option<&str>) -> Option<String> {
    if opt_in != Some("1") {
        return None;
    }
    let source = source.map(str::trim).filter(|s| !s.is_empty());
    Some(
        source
            .unwrap_or_else(|| {
                panic!(
                    "CATALYST_WORLDS_LIVE=1 with no WORLDS_LIVE_SOURCE: name the upstream the pass reads"
                )
            })
            .to_string(),
    )
}

#[test]
fn the_opt_in_names_its_source_or_fails() {
    assert_eq!(live_source(None, None), None);
    assert_eq!(
        live_source(Some("0"), Some("https://worlds.example.org")),
        None
    );
    assert_eq!(
        live_source(Some("1"), Some(" https://worlds.example.org ")).as_deref(),
        Some("https://worlds.example.org")
    );
    for blank in [None, Some(""), Some("  ")] {
        let err = std::panic::catch_unwind(|| live_source(Some("1"), blank))
            .expect_err("opted in with no source is a failure, never a pass");
        let message = err
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| err.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default();
        assert!(message.contains("WORLDS_LIVE_SOURCE"), "{message}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_bounded_live_pass_mirrors_the_first_catalogue_worlds_with_their_blobs() {
    let Some(source) = live_source(
        std::env::var("CATALYST_WORLDS_LIVE").ok().as_deref(),
        std::env::var("WORLDS_LIVE_SOURCE").ok().as_deref(),
    ) else {
        eprintln!("skipping: CATALYST_WORLDS_LIVE=1 not set");
        return;
    };
    let scratch = db::scratch_database().await.unwrap_or_else(|| {
        panic!("CATALYST_WORLDS_LIVE=1 with no CATALYST_TEST_DATABASE_URL: the pass needs a scratch database")
    });
    let tmp = std::env::temp_dir().join(format!("catalyst-worlds-live-{}", std::process::id()));
    let _ = tokio::fs::remove_dir_all(&tmp).await;
    let storage = Arc::new(Storage::from_config(&tmp).await.unwrap());
    let client = catalyst::http::Outbound::new(
        catalyst::http::HttpClientCfg::default()
            .with_total_timeout(std::time::Duration::from_secs(120)),
    )
    .unwrap();
    let settings = MirrorSettings::parse(
        &source,
        &["http://127.0.0.1:5210".to_string()],
        std::time::Duration::from_secs(3600),
        2,
        8,
        LIMIT,
        "",
    )
    .unwrap();
    let jobs = Jobs::new();
    let (tx, rx) = tokio::sync::oneshot::channel();
    jobs.spawn("ctx", move |ctx| async move {
        let _ = tx.send(ctx.clone());
        ctx.cancel.cancelled().await;
        Ok(())
    });
    let ctx = rx.await.unwrap();

    let started = std::time::Instant::now();
    let first = run_pass(&client, &scratch.pool, &storage, &settings, &ctx)
        .await
        .expect("first pass");
    eprintln!("first pass in {:?}: {first:?}", started.elapsed());
    assert!(
        first.catalogue.complete && first.catalogue.listed > 1000,
        "{first:?}"
    );
    assert_eq!(first.scenes.processed, LIMIT);
    assert!(first.scenes.complete);
    assert_eq!(first.scenes.failed + first.scenes.refused, 0, "{first:?}");

    let rows: Vec<(String, String, serde_json::Value)> = sqlx::query_as(
        "SELECT world_name, entity_id, entity FROM worlds.world_scenes ORDER BY world_name, entity_id",
    )
    .fetch_all(&scratch.pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), first.scenes.scenes);
    let listed: Vec<(String, Vec<String>)> = sqlx::query_as(
        "SELECT world_name, scene_ids FROM worlds.remote_worlds WHERE mirrored_at IS NOT NULL ORDER BY world_name",
    )
    .fetch_all(&scratch.pool)
    .await
    .unwrap();
    assert_eq!(listed.len(), LIMIT);
    let mut files = 0usize;
    for (world, entity_id, entity) in &rows {
        assert!(
            listed
                .iter()
                .any(|(w, ids)| w == world && ids.contains(entity_id)),
            "{world} {entity_id}"
        );
        assert!(
            storage.exist(entity_id).await.unwrap(),
            "entity {entity_id} of {world}"
        );
        for c in entity["content"].as_array().unwrap() {
            let hash = c["hash"].as_str().unwrap();
            assert!(storage.exist(hash).await.unwrap(), "file {hash} of {world}");
            files += 1;
        }
    }
    eprintln!(
        "{} worlds, {} scenes, {files} files present in {}",
        listed.len(),
        rows.len(),
        tmp.display()
    );

    let second = run_pass(&client, &scratch.pool, &storage, &settings, &ctx)
        .await
        .expect("second pass");
    eprintln!("second pass: {second:?}");
    assert_eq!(second.scenes.unchanged, LIMIT, "{second:?}");
    assert_eq!(second.scenes.new_blobs, 0);
    let (cursor, number): (Option<String>, i64) = sqlx::query_as(
        "SELECT cursor, pass_number FROM worlds.mirror_passes WHERE mirror = 'scenes'",
    )
    .fetch_one(&scratch.pool)
    .await
    .unwrap();
    assert!(cursor.is_none() && number == 2);

    jobs.shutdown(std::time::Duration::from_secs(1)).await;
    let _ = tokio::fs::remove_dir_all(&tmp).await;
    scratch.teardown().await;
}
