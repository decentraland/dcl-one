//! The worlds change feed against the real schema: what each local write
//! records in its own transaction, and `GET /changes`' cursor, visibility,
//! paging, refusals and retention. Each test reads its own database
//! (`isolated_pool`), so the feed holds only its rows; transaction ids are
//! cluster-wide, so visibility is awaited, never assumed after a commit.

use super::super::change_log::{self, Cursor};
use super::*;

const BASE: &str = "https://worlds.example/worlds-content-server";
const FEED_TOKEN: &str = "feed-reader-example";

async fn isolated() -> Option<Fixture> {
    isolated_with(test_settings().with_change_feed_token(Some(FEED_TOKEN.into()))).await
}

async fn isolated_with(settings: Settings) -> Option<Fixture> {
    let pool = crate::db::scratch::isolated_pool(8).await?;
    Some(
        fixture_on_pool(
            pool,
            PersonalWorldsPolicy::disabled(),
            None,
            None,
            settings,
            "",
        )
        .await,
    )
}

/// One page of the feed through the route.
async fn page(f: &Fixture, query: &str) -> Value {
    expect(
        &f.app,
        Case::new("GET", format!("/changes{query}")).bearer(FEED_TOKEN),
        200,
    )
    .await
}

/// Every entry readable now, from the start.
async fn feed(f: &Fixture) -> Vec<Value> {
    let (changes, _) = change_log::read(&f.pool, Cursor::START, change_log::MAX_LIMIT)
        .await
        .unwrap();
    changes
        .into_iter()
        .map(|c| json!({ "event": c.event, "attributes": c.attributes }))
        .collect()
}

/// The feed once `count` rows are readable.
async fn feed_of(f: &Fixture, count: usize) -> Vec<Value> {
    let mut seen = Vec::new();
    assert!(
        crate::shared::test_wait::until(async || {
            seen = feed(f).await;
            seen.len() >= count
        })
        .await,
        "expected {count} readable rows, saw {seen:?}"
    );
    seen
}

async fn rows(f: &Fixture) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM worlds.change_log")
        .fetch_one(&f.pool)
        .await
        .unwrap()
}

fn kinds(entries: &[Value]) -> Vec<&str> {
    entries
        .iter()
        .map(|e| e["event"]["subType"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn a_publish_records_its_deployment_and_the_settings_it_seeded() {
    let Some(f) = isolated().await else { return };
    let world = world_name();
    let owner = Address::parse(&format!("0x{}", "a1".repeat(20))).unwrap();
    let mut entity = scene_entity(&world, "First", "bafythumb", "0,0", &["0,0"]);
    entity["metadata"]["multiplayerId"] = json!("room-1");
    deploy(
        &f,
        &world,
        &owner,
        "scene-1",
        &entity,
        &["0,0"],
        &Deploy::Owner,
    )
    .await
    .unwrap();

    let entries = feed_of(&f, 2).await;
    assert_eq!(kinds(&entries), ["deployment", "world_settings_changed"]);
    let ts = entries[0]["event"]["timestamp"].as_i64().unwrap();
    assert_eq!(
        entries[0],
        json!({
            "event": {
                "entity": {
                    "entityId": "scene-1",
                    "authChain": [{ "type": "SIGNER", "payload": owner.as_str() }],
                },
                "contentServerUrls": [BASE],
                "type": "world",
                "subType": "deployment",
                "key": "scene-1",
                "timestamp": ts,
            },
            "attributes": { "isMultiplayer": "true", "priority": "1" },
        })
    );
    assert_eq!(
        entries[1]["event"],
        json!({
            "type": "world",
            "subType": "world_settings_changed",
            "key": format!("{}-{ts}", world.display()),
            "timestamp": ts,
            "metadata": {
                "worldName": world.display(),
                "title": "First",
                "description": "A test world",
                "contentRating": "E",
                "skyboxTime": 36000,
                "categories": ["art", "game"],
                "singlePlayer": true,
                "showInPlaces": true,
            },
        })
    );
    assert_eq!(entries[1]["attributes"], json!({}));

    // The same settings again: a deployment, no settings event.
    let same = scene_entity(&world, "First", "bafythumb", "0,0", &["0,0"]);
    deploy(
        &f,
        &world,
        &owner,
        "scene-2",
        &same,
        &["0,0"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    let entries = feed_of(&f, 3).await;
    assert_eq!(kinds(&entries)[2], "deployment");
    assert_eq!(entries[2]["attributes"]["isMultiplayer"], "false");

    // A sole occupant with a new title refreshes them; a refused publish records nothing.
    let renamed = scene_entity(&world, "Second", "bafythumb", "0,0", &["0,0"]);
    deploy(
        &f,
        &world,
        &owner,
        "scene-3",
        &renamed,
        &["0,0"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    let older = scene_at(500, &["0,0"]);
    assert!(
        deploy(
            &f,
            &world,
            &owner,
            "scene-4",
            &older,
            &["0,0"],
            &Deploy::Owner
        )
        .await
        .is_err()
    );
    let entries = feed_of(&f, 5).await;
    assert_eq!(
        kinds(&entries)[3..],
        ["deployment", "world_settings_changed"]
    );
    assert_eq!(entries[4]["event"]["metadata"]["title"], "Second");
    assert_eq!(rows(&f).await, 5);
}

#[tokio::test]
async fn undeploys_record_the_world_or_the_scenes_removed() {
    let Some(f) = isolated().await else { return };
    let world = world_name();
    let owner = Address::parse(&format!("0x{}", "a2".repeat(20))).unwrap();
    for (id, base, parcels) in [
        ("scene-a", "0,0", ["0,0", "0,1"]),
        ("scene-b", "5,5", ["5,5", "5,6"]),
    ] {
        let entity = scene_entity(&world, "World", "bafythumb", base, &parcels);
        deploy(&f, &world, &owner, id, &entity, &parcels, &Deploy::Owner)
            .await
            .unwrap();
    }
    let before = rows(&f).await;

    store::undeploy_scene(&f.pool, &world, Parcel::parse("0,1").unwrap(), None)
        .await
        .unwrap();
    // Nothing stands there any more: nothing to announce.
    store::undeploy_scene(&f.pool, &world, Parcel::parse("0,1").unwrap(), None)
        .await
        .unwrap();
    store::undeploy_world(&f.pool, &world).await.unwrap();
    // A whole-world undeploy is announced even when nothing was left.
    store::undeploy_world(&f.pool, &world).await.unwrap();

    let entries = feed_of(&f, before as usize + 3).await;
    let tail = &entries[before as usize..];
    assert_eq!(
        kinds(tail),
        [
            "world_scenes_undeployment",
            "world_undeployment",
            "world_undeployment"
        ]
    );
    let ts = tail[0]["event"]["timestamp"].as_i64().unwrap();
    assert_eq!(
        tail[0]["event"],
        json!({
            "type": "world",
            "subType": "world_scenes_undeployment",
            "key": world.display(),
            "timestamp": ts,
            "metadata": {
                "worldName": world.display(),
                "scenes": [{ "entityId": "scene-a", "baseParcel": "0,0", "parcels": ["0,0", "0,1"] }],
            },
        })
    );
    let ts = tail[1]["event"]["timestamp"].as_i64().unwrap();
    assert_eq!(
        tail[1]["event"],
        json!({
            "type": "world",
            "subType": "world_undeployment",
            "key": world.display(),
            "timestamp": ts,
            "metadata": { "worldName": world.display() },
        })
    );
}

#[tokio::test]
async fn settings_and_access_edits_record_their_events() {
    let Some(f) = isolated().await else { return };
    let world = world_name();
    let owner = Address::parse(&format!("0x{}", "a3".repeat(20))).unwrap();
    let entity = scene_entity(&world, "World", "bafythumb", "0,0", &["0,0", "1,0"]);
    deploy(
        &f,
        &world,
        &owner,
        "scene-s",
        &entity,
        &["0,0", "1,0"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    let before = rows(&f).await as usize;

    let update = WorldSettingsUpdate {
        title: Some("Edited".into()),
        spawn_coordinates: Some("1,0".into()),
        thumbnail_hash: Some("bafyedited".into()),
        ..Default::default()
    };
    store::update_world_settings(&f.pool, &world, &owner, Some(&owner), &update, None, BASE)
        .await
        .unwrap();
    // The spawn it already has: settings only.
    store::update_world_settings(&f.pool, &world, &owner, Some(&owner), &update, None, BASE)
        .await
        .unwrap();
    store::store_access_for_owner(
        &f.pool,
        &world,
        &owner,
        &AccessSetting::SharedSecret {
            secret_hash: "$2b$10$example".into(),
        },
    )
    .await
    .unwrap();

    let entries = feed_of(&f, before + 4).await;
    let tail = &entries[before..];
    assert_eq!(
        kinds(tail),
        [
            "world_settings_changed",
            "world_spawn_coordinate_set",
            "world_settings_changed",
            "world_settings_changed"
        ]
    );
    let ts = tail[0]["event"]["timestamp"].as_i64().unwrap();
    assert_eq!(tail[0]["event"]["metadata"]["title"], "Edited");
    assert_eq!(
        tail[0]["event"]["metadata"]["thumbnailUrl"],
        format!("{BASE}/contents/bafyedited")
    );
    assert_eq!(
        tail[1]["event"],
        json!({
            "type": "world",
            "subType": "world_spawn_coordinate_set",
            "key": format!("{}-{ts}", world.display()),
            "timestamp": ts,
            "metadata": {
                "name": world.display(),
                "oldCoordinate": { "x": 0, "y": 0 },
                "newCoordinate": { "x": 1, "y": 0 },
            },
        })
    );
    assert_eq!(
        tail[3]["event"]["metadata"],
        json!({ "worldName": world.display(), "accessType": "shared-secret" })
    );
}

/// The emission clock is read once the world's row lock is held, so an
/// undeploy that waited for a concurrent writer is stamped after it.
#[tokio::test]
async fn the_timestamp_is_read_after_the_world_lock() {
    let Some(f) = isolated().await else { return };
    let world = world_name();
    let owner = Address::parse(&format!("0x{}", "a4".repeat(20))).unwrap();
    let entity = scene_entity(&world, "World", "bafythumb", "0,0", &["0,0"]);
    deploy(
        &f,
        &world,
        &owner,
        "scene-l",
        &entity,
        &["0,0"],
        &Deploy::Owner,
    )
    .await
    .unwrap();
    let before = rows(&f).await as usize;

    let mut holder = f.pool.begin().await.unwrap();
    sqlx::query("SELECT name FROM worlds.worlds WHERE name = $1 FOR UPDATE")
        .bind(world.key())
        .execute(&mut *holder)
        .await
        .unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *holder)
        .await
        .unwrap();
    let (pool, waiting) = (f.pool.clone(), world.clone());
    let undeploy = tokio::spawn(async move { store::undeploy_world(&pool, &waiting).await });
    wait_for_lock(&f.pool, pid, 1).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let released: i64 =
        sqlx::query_scalar("SELECT (extract(epoch FROM clock_timestamp()) * 1000)::bigint")
            .fetch_one(&mut *holder)
            .await
            .unwrap();
    holder.commit().await.unwrap();
    undeploy.await.unwrap().unwrap();

    let entries = feed_of(&f, before + 1).await;
    assert!(entries[before]["event"]["timestamp"].as_i64().unwrap() >= released);
}

/// A transaction that took its id first and commits last is not skipped:
/// nothing after it is readable until it commits, then both are, in id order.
#[tokio::test]
async fn a_lower_txid_committed_later_is_never_skipped() {
    let Some(f) = isolated().await else { return };
    let early_world = world_name();
    let mut early = f.pool.begin().await.unwrap();
    let event = change_log::world_undeployment(&early_world, 1);
    change_log::record(&mut early, &early_world, &event, &json!({}))
        .await
        .unwrap();

    let world = world_name();
    store::undeploy_world(&f.pool, &world).await.unwrap();
    assert_eq!(rows(&f).await, 1, "only the committed row is in the table");
    // The open transaction holds xmin at or below its own id, which is below the later one's.
    assert!(feed(&f).await.is_empty());

    early.commit().await.unwrap();
    let entries = feed_of(&f, 2).await;
    assert_eq!(
        entries[0]["event"]["metadata"]["worldName"],
        early_world.display()
    );
    assert_eq!(
        entries[1]["event"]["metadata"]["worldName"],
        world.display()
    );
}

#[tokio::test]
async fn the_route_pages_by_cursor_until_caught_up() {
    let Some(f) = isolated().await else { return };
    for _ in 0..5 {
        store::undeploy_world(&f.pool, &world_name()).await.unwrap();
    }
    feed_of(&f, 5).await;

    let first = page(&f, "?limit=2").await;
    assert_eq!(first["changes"].as_array().unwrap().len(), 2);
    let next = first["next"].as_str().unwrap().to_string();
    assert_eq!(first["changes"][1]["cursor"], next.as_str());
    assert_eq!(first["changes"][0]["attributes"], json!({}));
    assert_eq!(
        first["changes"][0]["event"]["subType"],
        "world_undeployment"
    );

    let second = page(&f, &format!("?after={next}&limit=2")).await;
    let next = second["next"].as_str().unwrap().to_string();
    let third = page(&f, &format!("?after={next}&limit=2")).await;
    assert_eq!(third["changes"].as_array().unwrap().len(), 1);
    assert_eq!(third["next"], Value::Null);
    let last = third["changes"][0]["cursor"].as_str().unwrap().to_string();
    let caught_up = page(&f, &format!("?after={last}")).await;
    assert_eq!(caught_up, json!({ "changes": [], "next": null }));

    // Without `after`, from the oldest row; past the maximum, a page holds 1000.
    let all = page(&f, "?limit=100000").await;
    assert_eq!(all["changes"].as_array().unwrap().len(), 5);
    let cursors: Vec<&str> = all["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["cursor"].as_str().unwrap())
        .collect();
    let parsed: Vec<Cursor> = cursors.iter().map(|c| Cursor::parse(c).unwrap()).collect();
    assert!(
        parsed
            .windows(2)
            .all(|w| (w[0].txid, w[0].id) < (w[1].txid, w[1].id))
    );
}

/// The feed takes its own bearer only: the admin token does not open it, and
/// it opens no admin route.
#[tokio::test]
async fn the_route_takes_only_the_feed_token_and_refuses_bad_cursors_and_limits() {
    let Some(f) = isolated().await else { return };
    expect(&f.app, Case::new("GET", "/changes"), 401).await;
    expect(&f.app, Case::new("GET", "/changes").bearer("wrong"), 401).await;
    expect(
        &f.app,
        Case::new("GET", "/changes").bearer(ADMIN_TOKEN),
        401,
    )
    .await;
    expect(&f.app, Case::new("GET", "/changes").bearer(FEED_TOKEN), 200).await;
    expect(
        &f.app,
        Case::new("GET", "/admin/access-log").bearer(FEED_TOKEN),
        401,
    )
    .await;
    for query in [
        "?after=12",
        "?after=a:1",
        "?after=1:2:3",
        "?after=-1:2",
        "?limit=0",
        "?limit=x",
    ] {
        expect(
            &f.app,
            Case::new("GET", format!("/changes{query}")).bearer(FEED_TOKEN),
            400,
        )
        .await;
    }
}

#[tokio::test]
async fn retention_deletes_only_rows_past_it() {
    let Some(f) = isolated().await else { return };
    let (old, fresh) = (world_name(), world_name());
    store::undeploy_world(&f.pool, &old).await.unwrap();
    store::undeploy_world(&f.pool, &fresh).await.unwrap();
    sqlx::query(
        "UPDATE worlds.change_log SET created_at = timezone('UTC', now()) - interval '31 days' \
         WHERE world_name = $1",
    )
    .bind(old.key())
    .execute(&f.pool)
    .await
    .unwrap();

    let cutoff = chrono::Utc::now().naive_utc() - change_log::RETENTION;
    assert_eq!(
        change_log::delete_expired(&f.pool, cutoff).await.unwrap(),
        1
    );
    let left: Vec<String> = sqlx::query_scalar("SELECT world_name FROM worlds.change_log")
        .fetch_all(&f.pool)
        .await
        .unwrap();
    assert_eq!(left, [fresh.key().to_string()]);
}

/// Removing a mirrored copy is not a change of this node's: no scenes event.
#[tokio::test]
async fn removing_a_mirrored_scene_records_no_scenes_event() {
    let Some(f) = isolated().await else { return };
    let world = world_name();
    sqlx::query(
        "INSERT INTO worlds.worlds (name, mirror_source) VALUES ($1, 'https://upstream.example')",
    )
    .bind(world.key())
    .execute(&f.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size, mirror_source) \
         VALUES ($1, 'mirrored', '[]', $2, '0xabc', ARRAY['0,0'], 1, 'https://upstream.example')",
    )
    .bind(world.key())
    .bind(scene_entity(&world, "Mirrored", "bafythumb", "0,0", &["0,0"]))
    .execute(&f.pool)
    .await
    .unwrap();

    let removed = store::undeploy_scene(&f.pool, &world, Parcel::parse("0,0").unwrap(), None)
        .await
        .unwrap();
    assert_eq!(removed, 1);
    assert_eq!(rows(&f).await, 0);
}

/// Without `WORLDS_CHANGE_FEED_TOKEN` the feed refuses every request.
#[tokio::test]
async fn the_route_is_refused_while_no_feed_token_is_set() {
    let Some(f) = isolated_with(test_settings()).await else {
        return;
    };
    for case in [
        Case::new("GET", "/changes"),
        Case::new("GET", "/changes").bearer(FEED_TOKEN),
        Case::new("GET", "/changes").bearer(ADMIN_TOKEN),
        Case::new("GET", "/changes").bearer(""),
    ] {
        expect(&f.app, case, 401).await;
    }
}
