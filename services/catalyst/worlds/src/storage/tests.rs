//! The storage contracts against the lib's scratch database
//! (`CATALYST_TEST_DATABASE_URL`; skipped without it): the store's cache,
//! listing, byte-identity and quota behaviour; the routes' authorization
//! matrix (authoritative signer, scoped delegation, owner, covering and
//! partial deployers, stranger, Genesis LAND authority); the DATABASE.md
//! invariants (cross-scene and cross-player isolation, redeploy retention,
//! the Genesis scope); and the signed-fetch posture. Rows are scoped by
//! fresh world names, wallets and Genesis parcels, so the tests share the
//! database with the other lib tests.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use tower::ServiceExt;
use uuid::Uuid;

use crate::comms::world_identity::{Parcel, RealmId, SceneNamespace, WorldName};
use crate::policy::{LandOperatorsReader, Permission, PersonalWorldsPolicy, Policy};
use crate::shared::identity::Address;
use crate::shared::test_keys::{
    AUTHORITATIVE_KEY, KEY, OUTSIDER_KEY, Shape, address_of, random_key, signed_as, signed_at,
    wallet,
};

use super::auth::{AuthChainError, verify_request};
use super::delegation::test_support::{claim_payload, scope_header};
use super::export::ExportSettings;
use super::store::{NamespaceLimits, StorageCacheConfig, Store, value_size_bytes};
use super::{NOT_AUTHORIZED_MSG, Settings, SharedState, StorageState, router};

const LIMITS: NamespaceLimits = NamespaceLimits {
    max_value_size_bytes: 1_048_576,
    max_total_size_bytes: 10_485_760,
};

fn cache_cfg(enabled: bool) -> StorageCacheConfig {
    StorageCacheConfig {
        enabled,
        ttl_seconds: 300,
        max_entries: 1000,
        max_value_bytes: 32_768,
    }
}

async fn scratch() -> Option<PgPool> {
    crate::db::scratch::pool(8).await
}

fn address() -> Address {
    Address::parse(&format!("0x00000000{}", Uuid::new_v4().simple())).unwrap()
}

fn world_name() -> WorldName {
    WorldName::parse(&format!(
        "S{}.dcl.eth",
        &Uuid::new_v4().simple().to_string()[..10]
    ))
    .unwrap()
}

/// A world realm's scope stands for the scene based at `base` (an identity
/// derived from it); a shared realm's is `base` itself.
fn ns(realm: &str, base: &str) -> SceneNamespace {
    let identity = Uuid::new_v5(&Uuid::NAMESPACE_OID, base.as_bytes());
    SceneNamespace::new(
        &RealmId::parse(realm).unwrap(),
        Parcel::parse(base).unwrap(),
        Some(identity),
    )
    .unwrap()
}

fn ser(v: &Value) -> String {
    serde_json::to_string(v).unwrap()
}

async fn seed_world(pool: &PgPool, world: &WorldName, owner: Option<&Address>) {
    sqlx::query("INSERT INTO worlds.worlds (name, owner) VALUES ($1, $2) ON CONFLICT (name) DO UPDATE SET owner = EXCLUDED.owner")
        .bind(world.key())
        .bind(owner)
        .execute(pool)
        .await
        .unwrap();
}

async fn seed_scene(
    pool: &PgPool,
    world: &WorldName,
    id: &str,
    parcels: &[&str],
    base: Option<&str>,
    mirrored: bool,
) {
    let entity = match base {
        Some(b) => json!({ "metadata": { "scene": { "base": b } } }),
        None => json!({}),
    };
    sqlx::query(
        "INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size, mirror_source) \
         VALUES ($1, $2, '[]'::json, $3, $4, $5, 1, $6)",
    )
    .bind(world.key())
    .bind(id)
    .bind(entity)
    .bind(crate::ZERO_ADDRESS)
    .bind(parcels.iter().map(|p| p.to_string()).collect::<Vec<_>>())
    .bind(mirrored.then_some("https://upstream.example"))
    .execute(pool)
    .await
    .unwrap();
    let parcels: Vec<String> = parcels.iter().map(|p| p.to_string()).collect();
    let mut conn = pool.acquire().await.unwrap();
    crate::explore::places::scene_identity::assign(&mut conn, world.key(), id, &parcels)
        .await
        .unwrap();
}

async fn delete_scene(pool: &PgPool, world: &WorldName, id: &str) {
    sqlx::query("DELETE FROM worlds.world_scenes WHERE world_name = $1 AND entity_id = $2")
        .bind(world.key())
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    let mut conn = pool.acquire().await.unwrap();
    crate::explore::places::scene_identity::retire_orphans(&mut conn, &[world.key().to_string()])
        .await
        .unwrap();
}

/// The namespace the routes resolve for the world scene standing on `parcel`.
async fn scene_ns(pool: &PgPool, world: &WorldName, parcel: &str) -> SceneNamespace {
    let identity = active_identity(pool, world, parcel).await;
    SceneNamespace::new(
        &RealmId::World(world.clone()),
        Parcel::parse(parcel).unwrap(),
        identity,
    )
    .expect("the scene has a storage identity")
}

/// Replaces scenes as a publish does (`core::store::deploy_scene`): the old
/// rows go, the new one lands, then the identity rule and the orphan sweep.
async fn redeploy(
    pool: &PgPool,
    world: &WorldName,
    replaced: &[&str],
    id: &str,
    parcels: &[&str],
    base: Option<&str>,
) {
    sqlx::query("DELETE FROM worlds.world_scenes WHERE world_name = $1 AND entity_id = ANY($2)")
        .bind(world.key())
        .bind(replaced)
        .execute(pool)
        .await
        .unwrap();
    seed_scene(pool, world, id, parcels, base, false).await;
    let mut conn = pool.acquire().await.unwrap();
    crate::explore::places::scene_identity::retire_orphans(&mut conn, &[world.key().to_string()])
        .await
        .unwrap();
}

/// The active storage identity of the world scene standing on `parcel`.
async fn active_identity(pool: &PgPool, world: &WorldName, parcel: &str) -> Option<Uuid> {
    sqlx::query_scalar(
        "SELECT identity FROM explore.world_scene_place WHERE world_name = $1 AND active AND $2 = ANY(parcels)",
    )
    .bind(world.key())
    .bind(parcel)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn grant(
    pool: &PgPool,
    world: &WorldName,
    permission: Permission,
    who: &Address,
    parcels: &[&str],
) {
    sqlx::query("INSERT INTO worlds.world_permissions (world_name, permission, address) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
        .bind(world.key())
        .bind(permission.as_str())
        .bind(who)
        .execute(pool)
        .await
        .unwrap();
    for parcel in parcels {
        sqlx::query("INSERT INTO worlds.world_permission_parcels (world_name, permission, address, parcel) VALUES ($1, $2, $3, $4)")
            .bind(world.key())
            .bind(permission.as_str())
            .bind(who)
            .bind(parcel)
            .execute(pool)
            .await
            .unwrap();
    }
}

fn state_with(pool: &PgPool, settings: Settings) -> SharedState {
    let policy = Arc::new(Policy::new(
        pool.clone(),
        PersonalWorldsPolicy::disabled(),
        None,
    ));
    Arc::new(StorageState::new(pool.clone(), policy, settings, None))
}

/// A loopback lambdas answering `/parcels/{x}/{y}/operators` for one parcel
/// with the given manager document, 404 elsewhere.
async fn fake_land_operators(parcel: &str, document: Value) -> String {
    use axum::extract::{Path, State};
    use axum::response::IntoResponse;
    async fn answer(
        State((parcel, document)): State<(String, Value)>,
        Path((x, y)): Path<(i32, i32)>,
    ) -> axum::response::Response {
        if format!("{x},{y}") == parcel {
            axum::Json(document).into_response()
        } else {
            StatusCode::NOT_FOUND.into_response()
        }
    }
    let app = Router::new()
        .route("/parcels/{x}/{y}/operators", axum::routing::get(answer))
        .with_state((parcel.to_string(), document));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}

fn state_with_land_operators(pool: &PgPool, lambdas_url: &str) -> SharedState {
    let reader = LandOperatorsReader::new(
        crate::http::Outbound::for_tests().allowing_private_hosts(),
        lambdas_url,
    );
    let policy = Arc::new(
        Policy::new(pool.clone(), PersonalWorldsPolicy::disabled(), None)
            .with_land_operators(Some(reader)),
    );
    Arc::new(StorageState::new(
        pool.clone(),
        policy,
        Settings::for_tests(None),
        None,
    ))
}

fn state(pool: &PgPool, authoritative: Option<Address>) -> SharedState {
    state_with(pool, Settings::for_tests(authoritative))
}

async fn raw_world_value_text(pool: &PgPool, ns: &SceneNamespace, key: &str) -> Option<String> {
    sqlx::query("SELECT value::text AS value FROM worlds.world_storage WHERE realm = $1 AND scene_namespace = $2 AND key = $3")
        .bind(ns.realm())
        .bind(ns.scene_namespace())
        .bind(key)
        .fetch_optional(pool)
        .await
        .unwrap()
        .map(|r| r.get("value"))
}

async fn sneaky_world_update(pool: &PgPool, ns: &SceneNamespace, key: &str, value: &Value) {
    sqlx::query("UPDATE worlds.world_storage SET value = $4::jsonb WHERE realm = $1 AND scene_namespace = $2 AND key = $3")
        .bind(ns.realm())
        .bind(ns.scene_namespace())
        .bind(key)
        .bind(ser(value))
        .execute(pool)
        .await
        .unwrap();
}

async fn sneaky_player_update(
    pool: &PgPool,
    ns: &SceneNamespace,
    player: &Address,
    key: &str,
    value: &Value,
) {
    sqlx::query(
        "UPDATE worlds.player_storage SET value = $5::jsonb \
         WHERE realm = $1 AND scene_namespace = $2 AND player_address = $3 AND key = $4",
    )
    .bind(ns.realm())
    .bind(ns.scene_namespace())
    .bind(player)
    .bind(key)
    .bind(ser(value))
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn world_single_key_cache_hit_miss_and_write_invalidation() {
    let Some(pool) = scratch().await else { return };
    let scope = ns(world_name().key(), "0,0");
    let store = Store::new(pool.clone(), cache_cfg(true));

    assert!(store.world_get(&scope, "k").await.unwrap().is_none());

    store
        .world_upsert_with_quota(&scope, "k", &ser(&json!({"a": 1})), LIMITS)
        .await
        .unwrap();
    let first = store.world_get(&scope, "k").await.unwrap().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&first).unwrap(),
        json!({"a": 1})
    );

    sneaky_world_update(&pool, &scope, "k", &json!({"sneaky": true})).await;
    let cached = store.world_get(&scope, "k").await.unwrap().unwrap();
    assert_eq!(&*cached, &*first, "second read must be served by the cache");

    store
        .world_upsert_with_quota(&scope, "k", &ser(&json!({"a": 2})), LIMITS)
        .await
        .unwrap();
    let after_write = store.world_get(&scope, "k").await.unwrap().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&after_write).unwrap(),
        json!({"a": 2}),
        "a write must invalidate the cached value"
    );

    store.world_delete(&scope, "k").await.unwrap();
    assert!(
        store.world_get(&scope, "k").await.unwrap().is_none(),
        "a delete must invalidate the cached value"
    );
}

#[tokio::test]
async fn player_single_key_cache_hit_miss_and_write_invalidation() {
    let Some(pool) = scratch().await else { return };
    let scope = ns(world_name().key(), "0,0");
    let player = address();
    let store = Store::new(pool.clone(), cache_cfg(true));

    store
        .player_upsert_with_quota(&scope, &player, "k", &ser(&json!("v1")), LIMITS)
        .await
        .unwrap();
    let first = store
        .player_get(&scope, &player, "k")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(serde_json::from_str::<Value>(&first).unwrap(), json!("v1"));

    sneaky_player_update(&pool, &scope, &player, "k", &json!("sneaky")).await;
    let cached = store
        .player_get(&scope, &player, "k")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&*cached, &*first, "second read must be served by the cache");

    store
        .player_upsert_with_quota(&scope, &player, "k", &ser(&json!("v2")), LIMITS)
        .await
        .unwrap();
    let after_write = store
        .player_get(&scope, &player, "k")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&after_write).unwrap(),
        json!("v2")
    );

    store.player_delete(&scope, &player, "k").await.unwrap();
    assert!(
        store
            .player_get(&scope, &player, "k")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn delete_all_paths_invalidate_their_scope() {
    let Some(pool) = scratch().await else { return };
    let scope = ns(world_name().key(), "0,0");
    let (a, b) = (address(), address());
    let store = Store::new(pool.clone(), cache_cfg(true));

    for key in ["k1", "k2"] {
        store
            .world_upsert_with_quota(&scope, key, "1", LIMITS)
            .await
            .unwrap();
        store.world_get(&scope, key).await.unwrap();
    }
    store.world_delete_all(&scope).await.unwrap();
    for key in ["k1", "k2"] {
        assert!(
            store.world_get(&scope, key).await.unwrap().is_none(),
            "world delete_all must clear cached values for the scene"
        );
    }

    for player in [&a, &b] {
        store
            .player_upsert_with_quota(&scope, player, "k", &ser(&json!(player.as_str())), LIMITS)
            .await
            .unwrap();
        store.player_get(&scope, player, "k").await.unwrap();
    }
    store
        .player_delete_all_for_player(&scope, &a)
        .await
        .unwrap();
    assert!(store.player_get(&scope, &a, "k").await.unwrap().is_none());
    sqlx::query("DELETE FROM worlds.player_storage WHERE realm = $1 AND scene_namespace = $2 AND player_address = $3")
        .bind(scope.realm())
        .bind(scope.scene_namespace())
        .bind(&b)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        store.player_get(&scope, &b, "k").await.unwrap().is_some(),
        "per-player clear must not touch other players' cached values"
    );

    store.player_delete_all(&scope).await.unwrap();
    assert!(
        store.player_get(&scope, &b, "k").await.unwrap().is_none(),
        "scene-wide player clear must drop every cached player value"
    );
}

#[tokio::test]
async fn disabled_cache_reads_always_hit_the_database() {
    let Some(pool) = scratch().await else { return };
    let scope = ns(world_name().key(), "0,0");
    let store = Store::new(pool.clone(), cache_cfg(false));

    store
        .world_upsert_with_quota(&scope, "k", "1", LIMITS)
        .await
        .unwrap();
    store.world_get(&scope, "k").await.unwrap();
    sneaky_world_update(&pool, &scope, "k", &json!(2)).await;
    let read = store.world_get(&scope, "k").await.unwrap().unwrap();
    assert_eq!(&*read, "2", "with the cache disabled every read is fresh");
}

#[tokio::test]
async fn values_pass_through_byte_identical_to_postgres_text() {
    let Some(pool) = scratch().await else { return };
    let scope = ns(world_name().key(), "0,0");
    let store = Store::new(pool.clone(), cache_cfg(true));

    let values = [
        json!(0),
        json!(false),
        json!(""),
        json!(null),
        json!({"quotes":"a\"b","backslash":"c\\d","nl":"e\nf","unicode":"h\u{E9}llo \u{1F30D}"}),
        json!([1, 2.5, {"nested": [true, null, "\\u0000 literal"]}]),
    ];
    for (i, v) in values.iter().enumerate() {
        let key = format!("k{i}");
        store
            .world_upsert_with_quota(&scope, &key, &ser(v), LIMITS)
            .await
            .unwrap();
        let got = store
            .world_get(&scope, &key)
            .await
            .unwrap()
            .expect("stored falsy values must read back as Some");
        let pg_text = raw_world_value_text(&pool, &scope, &key).await.unwrap();
        assert_eq!(&*got, &*pg_text, "get must return the jsonb text verbatim");
        assert_eq!(&serde_json::from_str::<Value>(&got).unwrap(), v);
    }
}

#[tokio::test]
async fn list_page_splices_row_text_verbatim() {
    let Some(pool) = scratch().await else { return };
    let scope = ns(world_name().key(), "0,0");
    let player = address();
    let store = Store::new(pool.clone(), cache_cfg(true));

    let rows = [
        ("plain", json!({"a": 1})),
        ("with\"quote", json!("line1\nline2")),
        ("uni\u{1F511}", json!(["h\u{E9}llo", 2.5, null])),
    ];
    for (key, v) in &rows {
        store
            .world_upsert_with_quota(&scope, key, &ser(v), LIMITS)
            .await
            .unwrap();
        store
            .player_upsert_with_quota(&scope, &player, key, &ser(v), LIMITS)
            .await
            .unwrap();
    }

    let (entries, total) = store.world_list_page(&scope, 100, 0, None).await.unwrap();
    assert_eq!((entries.len(), total), (rows.len(), 3));
    for e in &entries {
        let pg_text = raw_world_value_text(&pool, &scope, &e.key).await.unwrap();
        assert_eq!(
            e.value, pg_text,
            "list rows must carry the jsonb text verbatim"
        );
    }

    let body: Value =
        serde_json::from_str(&super::handlers::raw_paginated_response(&entries, 100, 0, 3).0)
            .unwrap();
    let mut expected: Vec<(&str, &Value)> = rows.iter().map(|(k, v)| (*k, v)).collect();
    expected.sort_by_key(|(k, _)| k.to_string());
    let expected_data: Vec<Value> = expected
        .iter()
        .map(|(k, v)| json!({"key": k, "value": v}))
        .collect();
    assert_eq!(
        body,
        json!({"data": expected_data, "pagination": {"limit": 100, "offset": 0, "total": 3}})
    );

    let (player_entries, _) = store
        .player_list_page(&scope, &player, 100, 0, None)
        .await
        .unwrap();
    let player_body: Value = serde_json::from_str(
        &super::handlers::raw_paginated_response(&player_entries, 100, 0, 3).0,
    )
    .unwrap();
    assert_eq!(player_body["data"], json!(expected_data));
}

#[tokio::test]
async fn paged_lists_carry_an_exact_total_in_one_statement() {
    let Some(pool) = scratch().await else { return };
    let scope = ns(world_name().key(), "0,0");
    let (a, b) = (address(), address());
    let store = Store::new(pool.clone(), cache_cfg(false));

    for key in ["a", "b", "c"] {
        store
            .world_upsert_with_quota(&scope, key, "1", LIMITS)
            .await
            .unwrap();
        store
            .player_upsert_with_quota(&scope, &a, key, "1", LIMITS)
            .await
            .unwrap();
        store
            .env_upsert_with_quota(&scope, key, b"x", 1, LIMITS)
            .await
            .unwrap();
    }
    store
        .player_upsert_with_quota(&scope, &b, "z", "1", LIMITS)
        .await
        .unwrap();

    let (page, total) = store.world_list_page(&scope, 2, 0, None).await.unwrap();
    assert_eq!(
        page.iter().map(|e| e.key.as_str()).collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert_eq!(total, 3);
    let (past_end, total) = store.world_list_page(&scope, 2, 10, None).await.unwrap();
    assert!(past_end.is_empty());
    assert_eq!(
        total, 3,
        "an empty page past the start still reports the count"
    );
    let (none, total) = store
        .world_list_page(&scope, 2, 0, Some("zz"))
        .await
        .unwrap();
    assert!(none.is_empty());
    assert_eq!(total, 0);

    let (page, total) = store
        .player_list_page(&scope, &a, 10, 1, Some("b"))
        .await
        .unwrap();
    assert!(page.is_empty());
    assert_eq!(total, 1);
    let (page, total) = store
        .player_list_page(&scope, &a, 10, 1, None)
        .await
        .unwrap();
    assert_eq!(
        page.iter().map(|e| e.key.as_str()).collect::<Vec<_>>(),
        ["b", "c"]
    );
    assert_eq!(total, 3);

    let mut both = [a.as_str().to_string(), b.as_str().to_string()];
    both.sort();
    let (players, total) = store.player_list_players_page(&scope, 1, 0).await.unwrap();
    assert_eq!((players, total), (vec![both[0].clone()], 2));
    let (players, total) = store.player_list_players_page(&scope, 1, 1).await.unwrap();
    assert_eq!((players, total), (vec![both[1].clone()], 2));

    let (keys, total) = store.env_list_keys_page(&scope, 5, 0, None).await.unwrap();
    assert_eq!(
        (keys, total),
        (vec!["a".to_string(), "b".to_string(), "c".to_string()], 3)
    );
    let (keys, total) = store.env_list_keys_page(&scope, 5, 3, None).await.unwrap();
    assert!(keys.is_empty());
    assert_eq!(total, 3);
}

#[tokio::test]
async fn quota_upsert_refuses_over_limit_writes_in_place() {
    let Some(pool) = scratch().await else { return };
    let scope = ns(world_name().key(), "0,0");
    let (a, b) = (address(), address());
    let store = Store::new(pool.clone(), cache_cfg(false));
    let tight = NamespaceLimits {
        max_value_size_bytes: 8,
        max_total_size_bytes: 12,
    };

    store
        .world_upsert_with_quota(&scope, "a", "12345678", tight)
        .await
        .unwrap();
    let too_big = store
        .world_upsert_with_quota(&scope, "b", "123456789", tight)
        .await
        .unwrap_err();
    assert!(
        too_big
            .to_string()
            .contains("exceeds the maximum allowed size (8 bytes)"),
        "{too_big}"
    );
    let over_total = store
        .world_upsert_with_quota(&scope, "b", "12345", tight)
        .await
        .unwrap_err();
    assert!(
        over_total
            .to_string()
            .contains("would exceed the maximum allowed (12 bytes)"),
        "{over_total}"
    );
    assert!(store.world_get(&scope, "b").await.unwrap().is_none());
    // Replacing the existing value counts its old size against the total.
    store
        .world_upsert_with_quota(&scope, "a", "1234", tight)
        .await
        .unwrap();
    store
        .world_upsert_with_quota(&scope, "b", "12345678", tight)
        .await
        .unwrap();
    assert_eq!(store.world_size_info(&scope).await.unwrap().total_size, 12);

    store
        .player_upsert_with_quota(&scope, &a, "k", "12345678", tight)
        .await
        .unwrap();
    let err = store
        .player_upsert_with_quota(&scope, &a, "k2", "12345", tight)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("would exceed"), "{err}");
    store
        .player_upsert_with_quota(&scope, &b, "k", "12345678", tight)
        .await
        .unwrap();
    assert_eq!(
        store.player_size_info(&scope, &a).await.unwrap().total_size,
        8,
        "a player's quota is their own"
    );

    store
        .env_upsert_with_quota(&scope, "e", b"xxxxxxxx", 8, tight)
        .await
        .unwrap();
    let err = store
        .env_upsert_with_quota(&scope, "e2", b"xxxxx", 5, tight)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("would exceed"), "{err}");
    assert!(store.env_get_enc(&scope, "e2").await.unwrap().is_none());
    assert_eq!(store.env_size_info(&scope).await.unwrap().total_size, 8);
}

/// DATABASE.md: a scene reaches no other scene's rows and a player no other
/// player's; the namespace is the world scene's identity (or the Genesis
/// base parcel), never the hash, and two scenes of one world never share it.
#[tokio::test]
async fn scenes_and_players_are_isolated_by_namespace() {
    let Some(pool) = scratch().await else { return };
    let (w1, w2) = (world_name(), world_name());
    let same_base_other_world = ns(w2.key(), "0,0");
    let scene_a = ns(w1.key(), "0,0");
    let same_world_other_scene = ns(w1.key(), "5,5");
    let x = 50_000 + (Uuid::new_v4().as_u128() % 10_000) as i32;
    let scene_b = ns("main", &format!("{x},5"));
    let genesis = ns("main", &format!("{x},0"));
    let (p1, p2) = (address(), address());
    let store = Store::new(pool.clone(), cache_cfg(true));

    store
        .world_upsert_with_quota(&scene_a, "k", "\"a\"", LIMITS)
        .await
        .unwrap();
    store
        .env_upsert_with_quota(&scene_a, "secret", b"enc-a", 5, LIMITS)
        .await
        .unwrap();
    store
        .player_upsert_with_quota(&scene_a, &p1, "k", "\"p1\"", LIMITS)
        .await
        .unwrap();

    for other in [
        &scene_b,
        &same_base_other_world,
        &genesis,
        &same_world_other_scene,
    ] {
        assert!(
            store.world_get(other, "k").await.unwrap().is_none(),
            "{other}"
        );
        assert!(
            store.env_get_enc(other, "secret").await.unwrap().is_none(),
            "{other}"
        );
        assert!(
            store.player_get(other, &p1, "k").await.unwrap().is_none(),
            "{other}"
        );
        assert_eq!(
            store.world_list_page(other, 10, 0, None).await.unwrap().1,
            0
        );
        assert_eq!(
            store
                .env_list_keys_page(other, 10, 0, None)
                .await
                .unwrap()
                .1,
            0
        );
        assert_eq!(
            store
                .player_list_players_page(other, 10, 0)
                .await
                .unwrap()
                .1,
            0
        );
    }
    assert!(
        store
            .player_get(&scene_a, &p2, "k")
            .await
            .unwrap()
            .is_none(),
        "another player never reads p1's value"
    );
    assert_eq!(
        store
            .player_size_info(&scene_a, &p2)
            .await
            .unwrap()
            .total_size,
        0
    );

    // A clear of one scope leaves every other untouched.
    store
        .world_upsert_with_quota(&scene_b, "k", "\"b\"", LIMITS)
        .await
        .unwrap();
    store
        .player_upsert_with_quota(&scene_b, &p1, "k", "\"p1b\"", LIMITS)
        .await
        .unwrap();
    store.world_delete_all(&scene_a).await.unwrap();
    store.player_delete_all(&scene_a).await.unwrap();
    store.env_delete_all(&scene_a).await.unwrap();
    assert_eq!(
        store.world_get(&scene_b, "k").await.unwrap().as_deref(),
        Some("\"b\"")
    );
    assert_eq!(
        store
            .player_get(&scene_b, &p1, "k")
            .await
            .unwrap()
            .as_deref(),
        Some("\"p1b\"")
    );
}

/// A world's quota spans every scene of the world (each its own scope); a
/// shared realm's is per scene namespace (the old Places-id scope, the base
/// parcel).
#[tokio::test]
async fn a_world_quota_spans_its_scenes_and_a_shared_realm_quota_is_per_scene() {
    let Some(pool) = scratch().await else { return };
    let w = world_name();
    let store = Store::new(pool.clone(), cache_cfg(false));
    let tight = NamespaceLimits {
        max_value_size_bytes: 8,
        max_total_size_bytes: 12,
    };
    let (a, b) = (ns(w.key(), "0,0"), ns(w.key(), "9,9"));
    store
        .world_upsert_with_quota(&a, "k", "12345678", tight)
        .await
        .unwrap();
    let err = store
        .world_upsert_with_quota(&b, "k2", "12345", tight)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("would exceed"),
        "the second scene shares the world's 12 bytes: {err}"
    );
    assert_eq!(
        store.world_size_info(&b).await.unwrap().total_size,
        8,
        "usage is the world's total"
    );

    // Genesis parcels unique to this test.
    let x = 30_000 + (Uuid::new_v4().as_u128() % 10_000) as i32;
    let (g1, g2) = (ns("main", &format!("{x},1")), ns("main", &format!("{x},2")));
    store
        .world_upsert_with_quota(&g1, "k", "12345678", tight)
        .await
        .unwrap();
    store
        .world_upsert_with_quota(&g2, "k", "12345678", tight)
        .await
        .unwrap();
    assert_eq!(
        store.world_size_info(&g2).await.unwrap().total_size,
        8,
        "each Genesis scene has its own budget"
    );
}

/// A legacy world undeployed before migration keeps its parked rows, but
/// they cannot exhaust the first new scene's quotas. Other unknown scopes
/// remain counted, and a different world's active scene grants no exemption.
#[tokio::test]
async fn parked_legacy_storage_survives_migration_without_blocking_the_first_redeployment() {
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    let Ok(url) = std::env::var("CATALYST_TEST_DATABASE_URL") else {
        return;
    };
    let opts: PgConnectOptions = url.parse().unwrap();
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(opts.clone().database("postgres"))
        .await
        .unwrap();
    let name = format!("dcl_storage_migration_{}", Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
        .execute(&admin)
        .await
        .unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect_with(opts.database(&name))
        .await
        .unwrap();
    crate::db::apply_schema(&pool).await.unwrap();
    let world = world_name();
    seed_world(&pool, &world, Some(&address())).await;
    let player = address();
    for (scope, value, size) in [("world", "123456", 6i32), ("unrecognized", "12", 2)] {
        sqlx::query("INSERT INTO worlds.world_storage (realm, scene_namespace, key, value, value_size) VALUES ($1, $2, 'k', $3::jsonb, $4)")
            .bind(world.key()).bind(scope).bind(value).bind(size).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO worlds.player_storage (realm, scene_namespace, player_address, key, value, value_size) VALUES ($1, $2, $3, 'k', $4::jsonb, $5)")
            .bind(world.key()).bind(scope).bind(&player).bind(value).bind(size).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO worlds.env_variables (realm, scene_namespace, key, value_enc, value_size) VALUES ($1, $2, 'secret', $3, $4)")
            .bind(world.key()).bind(scope).bind(value.as_bytes()).bind(size).execute(&pool).await.unwrap();
    }
    // Re-run the actual pending upgrade step against the legacy rows. The
    // fixture database is private: removing this marker cannot affect peers.
    sqlx::query("DELETE FROM meta.schema_steps WHERE step = '0010_world_scene_storage_identity'")
        .execute(&pool)
        .await
        .unwrap();
    let migration =
        crate::db::schema_migrate::migrate(&pool, crate::db::SCHEMA_SQL, crate::db::MIGRATIONS_SQL)
            .await
            .unwrap();
    assert_eq!(migration.steps, vec!["0010_world_scene_storage_identity"]);
    let store = Store::new(pool.clone(), cache_cfg(false));
    let unresolved = ns(world.key(), "0,0");
    seed_owned_world(&pool).await;
    assert_eq!(
        store.world_size_info(&unresolved).await.unwrap().total_size,
        8
    );
    assert_eq!(
        store
            .player_size_info(&unresolved, &player)
            .await
            .unwrap()
            .total_size,
        8
    );
    assert_eq!(
        store.env_size_info(&unresolved).await.unwrap().total_size,
        8
    );
    seed_scene(
        &pool,
        &world,
        "bafkrei-after-migration",
        &["0,0"],
        Some("0,0"),
        false,
    )
    .await;
    let scene = scene_ns(&pool, &world, "0,0").await;
    assert!(store.world_get(&scene, "k").await.unwrap().is_none());
    assert!(
        store
            .player_get(&scene, &player, "k")
            .await
            .unwrap()
            .is_none()
    );
    assert!(store.env_get_enc(&scene, "secret").await.unwrap().is_none());
    assert_eq!(store.world_size_info(&scene).await.unwrap().total_size, 2);
    assert_eq!(
        store
            .player_size_info(&scene, &player)
            .await
            .unwrap()
            .total_size,
        2
    );
    assert_eq!(store.env_size_info(&scene).await.unwrap().total_size, 2);
    let limits = NamespaceLimits {
        max_value_size_bytes: 8,
        max_total_size_bytes: 8,
    };
    store
        .world_upsert_with_quota(&scene, "k", "654321", limits)
        .await
        .unwrap();
    store
        .player_upsert_with_quota(&scene, &player, "k", "654321", limits)
        .await
        .unwrap();
    store
        .env_upsert_with_quota(&scene, "secret", b"654321", 6, limits)
        .await
        .unwrap();
    assert_eq!(store.world_size_info(&scene).await.unwrap().total_size, 8);
    assert_eq!(
        store
            .player_size_info(&scene, &player)
            .await
            .unwrap()
            .total_size,
        8
    );
    assert_eq!(store.env_size_info(&scene).await.unwrap().total_size, 8);
    for table in ["world_storage", "player_storage", "env_variables"] {
        let rows: (i64, i64) = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT count(*)::bigint, COALESCE(sum(value_size), 0)::bigint FROM worlds.{table} WHERE realm = $1 AND scene_namespace = 'world'"
        ))).bind(world.key()).fetch_one(&pool).await.unwrap();
        assert_eq!(rows, (1, 6), "{table}: parked rows retained");
    }
    drop(store);
    pool.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE \"{name}\" WITH (FORCE)"
    )))
    .execute(&admin)
    .await
    .unwrap();
}

/// Retiring a scope preserves its bytes without exhausting the next scene's
/// world/player/env quotas; a replacement never inherits another scope.
#[tokio::test]
async fn undeployed_storage_is_retained_but_does_not_consume_the_redeployment_quota() {
    let Some(pool) = scratch().await else { return };
    let (world, _) = seed_owned_world(&pool).await;
    let store = Store::new(pool.clone(), cache_cfg(false));
    let old = scene_ns(&pool, &world, "0,0").await;
    let player = address();
    let limits = NamespaceLimits {
        max_value_size_bytes: 8,
        max_total_size_bytes: 8,
    };
    store
        .world_upsert_with_quota(&old, "k", "12345678", limits)
        .await
        .unwrap();
    store
        .player_upsert_with_quota(&old, &player, "k", "12345678", limits)
        .await
        .unwrap();
    store
        .env_upsert_with_quota(&old, "secret", b"encrypted", 8, limits)
        .await
        .unwrap();
    delete_scene(&pool, &world, SCENE_ID).await;
    seed_scene(
        &pool,
        &world,
        "bafkrei-redeployed",
        &["0,0"],
        Some("0,0"),
        false,
    )
    .await;
    let new = scene_ns(&pool, &world, "0,0").await;
    assert_ne!(old, new);
    assert!(store.world_get(&new, "k").await.unwrap().is_none());
    assert!(
        store
            .player_get(&new, &player, "k")
            .await
            .unwrap()
            .is_none()
    );
    assert!(store.env_get_enc(&new, "secret").await.unwrap().is_none());
    assert_eq!(store.world_size_info(&new).await.unwrap().total_size, 0);
    assert_eq!(
        store
            .player_size_info(&new, &player)
            .await
            .unwrap()
            .total_size,
        0
    );
    assert_eq!(store.env_size_info(&new).await.unwrap().total_size, 0);
    store
        .world_upsert_with_quota(&new, "k", "12345678", limits)
        .await
        .unwrap();
    store
        .player_upsert_with_quota(&new, &player, "k", "12345678", limits)
        .await
        .unwrap();
    store
        .env_upsert_with_quota(&new, "secret", b"encrypted-new", 8, limits)
        .await
        .unwrap();
    assert_eq!(store.world_size_info(&new).await.unwrap().total_size, 8);
    assert_eq!(
        store
            .player_size_info(&new, &player)
            .await
            .unwrap()
            .total_size,
        8
    );
    assert_eq!(store.env_size_info(&new).await.unwrap().total_size, 8);
    assert_eq!(
        store.world_get(&old, "k").await.unwrap().as_deref(),
        Some("12345678")
    );
    assert_eq!(
        store
            .player_get(&old, &player, "k")
            .await
            .unwrap()
            .as_deref(),
        Some("12345678")
    );
    assert_eq!(
        store.env_get_enc(&old, "secret").await.unwrap().unwrap(),
        b"encrypted"
    );
}

const SCENE_ID: &str = "bafkrei-delegated-scene";
const PARCEL: &str = "0,0";
const ENV_KEY: &str = "api-key";
const SECRET: &str = "secret-api-key-12345";

fn scene_metadata(realm: &str, parcel: &str, scene_id: &str) -> String {
    json!({
        "realm": { "serverName": realm },
        "parcel": parcel,
        "sceneId": scene_id,
        "signer": "dcl:authoritative-server"
    })
    .to_string()
}

async fn signed_request(
    method: &str,
    path: &str,
    key: &str,
    metadata: &str,
    scope: Option<&str>,
    body: Option<&str>,
) -> Request<Body> {
    let headers = signed_as(key, Shape::Legacy, method, path, metadata, metadata).await;
    with_headers(method, path, headers, scope, body)
}

fn with_headers(
    method: &str,
    path: &str,
    headers: HeaderMap,
    scope: Option<&str>,
    body: Option<&str>,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method.to_uppercase().as_str())
        .uri(path);
    for (name, value) in headers.iter() {
        builder = builder.header(name, value);
    }
    if let Some(scope) = scope {
        builder = builder.header("x-authoritative-scope", scope);
    }
    match body {
        Some(b) => builder
            .header("content-type", "application/json")
            .header("content-length", b.len().to_string())
            .body(Body::from(b.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, body)
}

/// A world owned by a random wallet with one local scene at 0,0 (+0,1).
async fn seed_owned_world(pool: &PgPool) -> (WorldName, Address) {
    let (world, owner) = (world_name(), address());
    seed_world(pool, &world, Some(&owner)).await;
    seed_scene(pool, &world, SCENE_ID, &["0,0", "0,1"], Some("0,0"), false).await;
    (world, owner)
}

#[tokio::test]
async fn scoped_delegation_reads_env_values_and_nothing_else() {
    let Some(pool) = scratch().await else { return };
    let (world, _owner) = seed_owned_world(&pool).await;
    let authoritative = Address::parse(&address_of(AUTHORITATIVE_KEY)).unwrap();
    let state = state(&pool, Some(authoritative));
    let app = router(state.clone());
    let scope = scene_ns(&pool, &world, PARCEL).await;
    let enc = state.encryptor().unwrap().encrypt(SECRET).unwrap();
    state
        .store
        .env_upsert_with_quota(
            &scope,
            ENV_KEY,
            &enc,
            value_size_bytes(SECRET),
            state.settings.env_limits,
        )
        .await
        .unwrap();

    let worker_key = random_key();
    let worker = address_of(&worker_key);
    let claim = claim_payload(
        &worker,
        world.key(),
        SCENE_ID,
        PARCEL,
        chrono::Utc::now() + chrono::Duration::hours(1),
    );
    let scope_hdr = scope_header(&wallet(AUTHORITATIVE_KEY), &claim).await;
    let meta = scene_metadata(world.display(), PARCEL, SCENE_ID);
    let value_path = format!("/env/{ENV_KEY}");

    let (status, body) = send(
        &app,
        signed_request(
            "get",
            &value_path,
            &worker_key,
            &meta,
            Some(&scope_hdr),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "delegated env read: {body}");
    assert_eq!(body, json!({ "value": SECRET }));

    let (status, body) = send(
        &app,
        signed_request("get", "/env", &worker_key, &meta, Some(&scope_hdr), None).await,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a delegation must not list env keys: {body}"
    );
    assert_eq!(body["message"], NOT_AUTHORIZED_MSG);

    let (status, body) = send(
        &app,
        signed_request(
            "put",
            &value_path,
            &worker_key,
            &meta,
            Some(&scope_hdr),
            Some(r#"{"value":"hijacked"}"#),
        )
        .await,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a delegation must not upsert env values: {body}"
    );
    assert_eq!(body["message"], NOT_AUTHORIZED_MSG);

    let (status, body) = send(
        &app,
        signed_request(
            "delete",
            &value_path,
            &worker_key,
            &meta,
            Some(&scope_hdr),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a delegation must not delete env values: {body}"
    );
    assert_eq!(body["message"], NOT_AUTHORIZED_MSG);

    let (status, body) = send(
        &app,
        signed_request("get", &value_path, OUTSIDER_KEY, &meta, None, None).await,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "an unrelated signer must stay denied: {body}"
    );
    assert_eq!(body["message"], NOT_AUTHORIZED_MSG);

    // A delegation for another scene id of the same world opens nothing.
    let other_claim = claim_payload(
        &worker,
        world.key(),
        "bafkrei-other",
        PARCEL,
        chrono::Utc::now() + chrono::Duration::hours(1),
    );
    let other_scope = scope_header(&wallet(AUTHORITATIVE_KEY), &other_claim).await;
    let (status, _) = send(
        &app,
        signed_request(
            "get",
            &value_path,
            &worker_key,
            &meta,
            Some(&other_scope),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = send(
        &app,
        signed_request("get", &value_path, AUTHORITATIVE_KEY, &meta, None, None).await,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "authorized-address reads must keep working: {body}"
    );
    assert_eq!(
        body,
        json!({ "value": SECRET }),
        "the denied write and delete must not have touched the value"
    );
}

#[tokio::test]
async fn parcel_mismatched_claim_is_rejected() {
    let Some(pool) = scratch().await else { return };
    let (world, _owner) = seed_owned_world(&pool).await;
    let authoritative = Address::parse(&address_of(AUTHORITATIVE_KEY)).unwrap();
    let state = state(&pool, Some(authoritative));
    let app = router(state.clone());

    let worker_key = random_key();
    let claim = claim_payload(
        &address_of(&worker_key),
        world.key(),
        SCENE_ID,
        "10,-25",
        chrono::Utc::now() + chrono::Duration::hours(1),
    );
    let scope_hdr = scope_header(&wallet(AUTHORITATIVE_KEY), &claim).await;

    let (status, body) = send(
        &app,
        signed_request(
            "get",
            &format!("/env/{ENV_KEY}"),
            &worker_key,
            &scene_metadata(world.display(), PARCEL, SCENE_ID),
            Some(&scope_hdr),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        body,
        json!({ "ok": false, "error": "Not Authorized", "message": NOT_AUTHORIZED_MSG })
    );
}

/// The permission core decides: the owner and a deployer whose grant covers
/// the scene's footprint write; a partial deployer, a streamer and a stranger
/// do not; the clears need an owner or deployer and the confirm header.
#[tokio::test]
async fn owners_and_covering_deployers_write_values_and_clear_with_the_confirm_header() {
    let Some(pool) = scratch().await else { return };
    let (world, owner) = seed_owned_world(&pool).await;
    let (owner_key, deployer_key, partial_key, streamer_key, stranger_key) = (
        random_key(),
        random_key(),
        random_key(),
        random_key(),
        random_key(),
    );
    let owner_addr = Address::parse(&address_of(&owner_key)).unwrap();
    seed_world(&pool, &world, Some(&owner_addr)).await;
    let _ = owner;
    grant(
        &pool,
        &world,
        Permission::Deployment,
        &Address::parse(&address_of(&deployer_key)).unwrap(),
        &["0,0", "0,1"],
    )
    .await;
    grant(
        &pool,
        &world,
        Permission::Deployment,
        &Address::parse(&address_of(&partial_key)).unwrap(),
        &["0,0"],
    )
    .await;
    grant(
        &pool,
        &world,
        Permission::Streaming,
        &Address::parse(&address_of(&streamer_key)).unwrap(),
        &[],
    )
    .await;
    let authoritative = Address::parse(&address_of(AUTHORITATIVE_KEY)).unwrap();
    let state = state(&pool, Some(authoritative));
    let app = router(state.clone());
    let meta = scene_metadata(world.display(), "0,1", SCENE_ID);

    for (who, key, expected) in [
        ("owner", owner_key.as_str(), StatusCode::OK),
        ("covering deployer", deployer_key.as_str(), StatusCode::OK),
        ("authoritative", AUTHORITATIVE_KEY, StatusCode::OK),
        (
            "partial deployer",
            partial_key.as_str(),
            StatusCode::UNAUTHORIZED,
        ),
        ("streamer", streamer_key.as_str(), StatusCode::UNAUTHORIZED),
        ("stranger", stranger_key.as_str(), StatusCode::UNAUTHORIZED),
    ] {
        let body = format!(r#"{{"value":{{"by":"{who}"}}}}"#);
        let (status, resp) = send(
            &app,
            signed_request("put", "/values/k", key, &meta, None, Some(&body)).await,
        )
        .await;
        assert_eq!(status, expected, "{who}: {resp}");
        if expected == StatusCode::OK {
            assert_eq!(resp, json!({ "value": { "by": who } }));
        }
    }
    let (status, body) = send(
        &app,
        signed_request("get", "/values/k", &deployer_key, &meta, None, None).await,
    )
    .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::OK,
            json!({ "value": { "by": "authoritative" } })
        )
    );
    let (status, body) = send(
        &app,
        signed_request("get", "/values?limit=10", &owner_key, &meta, None, None).await,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["pagination"],
        json!({ "limit": 10, "offset": 0, "total": 1 })
    );
    // parseInt semantics and the 100000 offset cap, echoed as clamped.
    let (status, body) = send(
        &app,
        signed_request(
            "get",
            "/values?limit=10abc&offset=999999999999",
            &owner_key,
            &meta,
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "data": [], "pagination": { "limit": 10, "offset": 100_000, "total": 1 } })
    );
    let (status, body) = send(
        &app,
        signed_request("get", "/usage/world", &owner_key, &meta, None, None).await,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["usedBytes"], json!(r#"{"by":"authoritative"}"#.len()));

    // The clears: not for the authoritative address, not without the header.
    let (status, _) = send(
        &app,
        signed_request("delete", "/values", AUTHORITATIVE_KEY, &meta, None, None).await,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "OWNERS_DEPLOYERS_ONLY excludes the direct addresses"
    );
    let (status, body) = send(
        &app,
        signed_request("delete", "/values", &owner_key, &meta, None, None).await,
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (
            StatusCode::BAD_REQUEST,
            Some("Missing required header: X-Confirm-Delete-All")
        )
    );
    let mut req = signed_request("delete", "/values", &deployer_key, &meta, None, None).await;
    req.headers_mut()
        .insert("x-confirm-delete-all", HeaderValue::from_static("1"));
    let (status, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = send(
        &app,
        signed_request("get", "/values/k", &owner_key, &meta, None, None).await,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Player values: the scene writes for a player, another player's scope stays empty.
    let (p1, p2) = (address(), address());
    let path = format!(
        "/players/{}/values/score",
        p1.as_str().to_uppercase().replace("0X", "0x")
    );
    let (status, _) = send(
        &app,
        signed_request(
            "put",
            &path,
            &owner_key,
            &meta,
            None,
            Some(r#"{"value":42}"#),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = send(
        &app,
        signed_request(
            "get",
            &format!("/players/{}/values/score", p1.as_str()),
            &owner_key,
            &meta,
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!((status, body), (StatusCode::OK, json!({ "value": 42 })));
    let (status, _) = send(
        &app,
        signed_request(
            "get",
            &format!("/players/{}/values/score", p2.as_str()),
            &owner_key,
            &meta,
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = send(
        &app,
        signed_request("get", "/players", &owner_key, &meta, None, None).await,
    )
    .await;
    assert_eq!(
        (status, body["data"].clone()),
        (StatusCode::OK, json!([p1.as_str()]))
    );
    let (status, body) = send(
        &app,
        signed_request(
            "get",
            "/players/not-an-address/values/score",
            &owner_key,
            &meta,
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::BAD_REQUEST, Some("Invalid player address"))
    );
}

/// An upsert answers 413/411 between the scene resolution and the
/// authorization, as upstream's body-size middleware: an unauthorized signer
/// sees the body refusal, a parcel without a scene still the 400.
#[tokio::test]
async fn an_unauthorized_upsert_gets_the_body_refusal_before_the_401() {
    let Some(pool) = scratch().await else {
        assert!(
            std::env::var("CI").as_deref() != Ok("true"),
            "CATALYST_TEST_DATABASE_URL is required in CI for the upsert refusal-order regression"
        );
        eprintln!("Skipping upsert refusal-order regression: CATALYST_TEST_DATABASE_URL is unset");
        return;
    };
    let (world, _) = seed_owned_world(&pool).await;
    let app = router(state(&pool, None));
    let meta = scene_metadata(world.display(), PARCEL, SCENE_ID);
    let no_scene = scene_metadata(world.display(), "7,7", SCENE_ID);
    let oversized = json!({ "value": "x".repeat(600_000) }).to_string();
    let small = json!({ "value": 1 }).to_string();

    for path in [
        "/values/k".to_string(),
        format!("/players/{}/values/k", address()),
        "/env/k".to_string(),
    ] {
        let put = async |meta: &str, body: Option<&str>| {
            signed_request("put", &path, OUTSIDER_KEY, meta, None, body).await
        };
        let (status, body) = send(&app, put(&meta, Some(&oversized)).await).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "{path}: {body}");

        let (parts, _) = put(&meta, None).await.into_parts();
        let no_length = Request::from_parts(parts, Body::from(small.clone()));
        let (status, body) = send(&app, no_length).await;
        assert_eq!(status, StatusCode::LENGTH_REQUIRED, "{path}: {body}");

        let (status, body) = send(&app, put(&meta, Some(&small)).await).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}: {body}");

        let (status, body) = send(&app, put(&no_scene, Some(&oversized)).await).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}: {body}");
    }
}

/// A request whose parcel holds no scene is the old Places-miss 400, ahead
/// of any authorization answer; a mirrored world grants nobody anything.
#[tokio::test]
async fn a_missing_scene_is_a_400_and_a_mirrored_world_refuses_everyone() {
    let Some(pool) = scratch().await else { return };
    let (world, _) = seed_owned_world(&pool).await;
    let owner_key = random_key();
    seed_world(
        &pool,
        &world,
        Some(&Address::parse(&address_of(&owner_key)).unwrap()),
    )
    .await;
    let app = router(state(&pool, None));

    for key in [&owner_key, OUTSIDER_KEY] {
        let (status, body) = send(
            &app,
            signed_request(
                "get",
                "/values/k",
                key,
                &scene_metadata(world.display(), "7,7", SCENE_ID),
                None,
                None,
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(
            body["message"]
                .as_str()
                .unwrap()
                .contains("Scene not found"),
            "{body}"
        );
    }
    let empty = world_name();
    let (status, _) = send(
        &app,
        signed_request(
            "get",
            "/values/k",
            &owner_key,
            &scene_metadata(empty.display(), "0,0", SCENE_ID),
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "an unknown world has no scene"
    );

    let mirrored = world_name();
    seed_world(&pool, &mirrored, None).await;
    seed_scene(&pool, &mirrored, "bafyMirrored", &["0,0"], None, true).await;
    for key in [&owner_key, OUTSIDER_KEY] {
        let (status, body) = send(
            &app,
            signed_request(
                "put",
                "/values/k",
                key,
                &scene_metadata(mirrored.display(), "0,0", "bafyMirrored"),
                None,
                Some(r#"{"value":1}"#),
            )
            .await,
        )
        .await;
        assert_eq!(
            (status, body["message"].as_str()),
            (StatusCode::UNAUTHORIZED, Some(NOT_AUTHORIZED_MSG)),
            "{key}"
        );
    }
}

/// Two scenes of one world (A at 0,0 +0,1, B at 5,5) hold the same key
/// independently; a delegation minted for A, or a deployer granted only A's
/// parcels, reaches none of B's values or env (the issue-81 reproduction).
#[tokio::test]
async fn two_scenes_of_a_world_keep_their_own_rows_and_a_delegation_reaches_one() {
    let Some(pool) = scratch().await else { return };
    let (world, _owner) = seed_owned_world(&pool).await;
    seed_scene(
        &pool,
        &world,
        "bafkrei-scene-b",
        &["5,5"],
        Some("5,5"),
        false,
    )
    .await;
    let authoritative = Address::parse(&address_of(AUTHORITATIVE_KEY)).unwrap();
    let state = state(&pool, Some(authoritative));
    let app = router(state.clone());
    let meta_a = scene_metadata(world.display(), PARCEL, SCENE_ID);
    let meta_b = scene_metadata(world.display(), "5,5", "bafkrei-scene-b");
    for (meta, value) in [(&meta_a, "a"), (&meta_b, "b")] {
        let body = format!(r#"{{"value":"{value}"}}"#);
        let (status, answer) = send(
            &app,
            signed_request(
                "put",
                "/values/score",
                AUTHORITATIVE_KEY,
                meta,
                None,
                Some(&body),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{answer}");
    }
    let enc = state.encryptor().unwrap().encrypt(SECRET).unwrap();
    state
        .store
        .env_upsert_with_quota(
            &scene_ns(&pool, &world, "5,5").await,
            ENV_KEY,
            &enc,
            value_size_bytes(SECRET),
            state.settings.env_limits,
        )
        .await
        .unwrap();
    for (meta, value) in [(&meta_a, "a"), (&meta_b, "b")] {
        let (status, body) = send(
            &app,
            signed_request("get", "/values/score", AUTHORITATIVE_KEY, meta, None, None).await,
        )
        .await;
        assert_eq!((status, body), (StatusCode::OK, json!({ "value": value })));
    }

    let worker_key = random_key();
    let claim = claim_payload(
        &address_of(&worker_key),
        world.key(),
        SCENE_ID,
        PARCEL,
        chrono::Utc::now() + chrono::Duration::hours(1),
    );
    let scope_hdr = scope_header(&wallet(AUTHORITATIVE_KEY), &claim).await;
    let env_path = format!("/env/{ENV_KEY}");
    let as_worker = |method: &'static str, path: String, meta: String, body: Option<String>| {
        let (key, scope_hdr) = (worker_key.clone(), scope_hdr.clone());
        async move {
            signed_request(
                method,
                &path,
                &key,
                &meta,
                Some(&scope_hdr),
                body.as_deref(),
            )
            .await
        }
    };
    let (status, body) = send(
        &app,
        as_worker("get", "/values/score".into(), meta_a.clone(), None).await,
    )
    .await;
    assert_eq!((status, body), (StatusCode::OK, json!({ "value": "a" })));
    let (status, _) = send(
        &app,
        as_worker("get", env_path.clone(), meta_a.clone(), None).await,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "B's env is not in A's scope");
    let (status, _) = send(
        &app,
        as_worker(
            "put",
            "/values/score".into(),
            meta_a.clone(),
            Some(r#"{"value":"a2"}"#.into()),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    for path in ["/values/score".to_string(), env_path.clone()] {
        let (status, _) = send(&app, as_worker("get", path, meta_b.clone(), None).await).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "A's delegation names A's scene, not B's"
        );
    }
    let (status, body) = send(
        &app,
        signed_request(
            "get",
            "/values/score",
            AUTHORITATIVE_KEY,
            &meta_b,
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(
        (status, body),
        (StatusCode::OK, json!({ "value": "b" })),
        "A's write left B's value alone"
    );

    // A deployer granted only A's parcels manages A's rows, not B's.
    let partial_key = random_key();
    let partial = Address::parse(&address_of(&partial_key)).unwrap();
    grant(
        &pool,
        &world,
        Permission::Deployment,
        &partial,
        &["0,0", "0,1"],
    )
    .await;
    let (status, body) = send(
        &app,
        signed_request("get", "/values/score", &partial_key, &meta_a, None, None).await,
    )
    .await;
    assert_eq!((status, body), (StatusCode::OK, json!({ "value": "a2" })));
    for path in ["/values/score", "/env", "/values"] {
        let (status, _) = send(
            &app,
            signed_request("get", path, &partial_key, &meta_b, None, None).await,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}");
    }
}

/// DATABASE.md: the namespace is the world scene's identity, not the
/// deployment hash or the base parcel, following Places' overlap rule
/// (`scene_identity`). A redeploy overlapping exactly one previous scene
/// keeps every row, whether it keeps its base, moves it or drops the
/// declaration; a deploy overlapping none or several starts empty and
/// retires the identities it overlapped; an undeploy deletes nothing.
#[tokio::test]
async fn a_world_redeploy_overlapping_one_scene_keeps_the_rows_and_an_undeploy_deletes_nothing() {
    let Some(pool) = scratch().await else { return };
    let (world, _) = seed_owned_world(&pool).await;
    let owner_key = random_key();
    seed_world(
        &pool,
        &world,
        Some(&Address::parse(&address_of(&owner_key)).unwrap()),
    )
    .await;
    let state = state(&pool, None);
    let app = router(state.clone());
    let meta_v1 = scene_metadata(world.display(), "0,1", SCENE_ID);
    let player = address();
    let get = |path: String, meta: String| {
        let key = owner_key.clone();
        async move { signed_request("get", &path, &key, &meta, None, None).await }
    };

    let (status, _) = send(
        &app,
        signed_request(
            "put",
            "/values/k",
            &owner_key,
            &meta_v1,
            None,
            Some(r#"{"value":"v1"}"#),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let ppath = format!("/players/{}/values/k", player.as_str());
    let (status, _) = send(
        &app,
        signed_request(
            "put",
            &ppath,
            &owner_key,
            &meta_v1,
            None,
            Some(r#"{"value":"p"}"#),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let kept = active_identity(&pool, &world, "0,0").await.unwrap();
    sqlx::query("UPDATE worlds.worlds SET show_in_places = true WHERE name = $1")
        .bind(world.key())
        .execute(&pool)
        .await
        .unwrap();
    let places =
        crate::explore::places::PlacesComponent::new(pool.clone(), "https://content.example");
    assert_eq!(
        places
            .find_by_id_for(&kept.to_string(), None)
            .await
            .unwrap()
            .unwrap()
            .id,
        kept.to_string(),
        "discovery exposes the UUID used by signed storage requests"
    );

    // v2: a new hash, a wider footprint, the same base.
    redeploy(
        &pool,
        &world,
        &[SCENE_ID],
        "bafkrei-v2",
        &["0,0", "0,1", "0,2"],
        Some("0,0"),
    )
    .await;
    let meta_v2 = scene_metadata(world.display(), "0,2", "bafkrei-v2");
    let (status, body) = send(&app, get("/values/k".into(), meta_v2.clone()).await).await;
    assert_eq!(
        (status, body),
        (StatusCode::OK, json!({ "value": "v1" })),
        "the redeploy kept the world value"
    );
    let (status, body) = send(&app, get(ppath.clone(), meta_v2).await).await;
    assert_eq!(
        (status, body),
        (StatusCode::OK, json!({ "value": "p" })),
        "the redeploy kept the player value"
    );

    // v3: the base moves to 0,3, overlapping v2 alone at 0,2.
    redeploy(
        &pool,
        &world,
        &["bafkrei-v2"],
        "bafkrei-v3",
        &["0,2", "0,3"],
        Some("0,3"),
    )
    .await;
    let meta_v3 = scene_metadata(world.display(), "0,3", "bafkrei-v3");
    let (status, body) = send(&app, get("/values/k".into(), meta_v3.clone()).await).await;
    assert_eq!(
        (status, body),
        (StatusCode::OK, json!({ "value": "v1" })),
        "a moved base overlapping one scene keeps the world value"
    );
    let (status, body) = send(&app, get(ppath.clone(), meta_v3).await).await;
    assert_eq!(
        (status, body),
        (StatusCode::OK, json!({ "value": "p" })),
        "a moved base overlapping one scene keeps the player value"
    );
    // v4: no declared base and a footprint whose first parcel differs, still over v3 alone.
    redeploy(
        &pool,
        &world,
        &["bafkrei-v3"],
        "bafkrei-v4",
        &["0,3", "7,7"],
        None,
    )
    .await;
    let meta_v4 = scene_metadata(world.display(), "7,7", "bafkrei-v4");
    let (status, body) = send(&app, get("/values/k".into(), meta_v4).await).await;
    assert_eq!((status, body), (StatusCode::OK, json!({ "value": "v1" })));
    assert_eq!(active_identity(&pool, &world, "7,7").await, Some(kept));
    assert_eq!(
        places
            .find_by_id_for(&kept.to_string(), None)
            .await
            .unwrap()
            .unwrap()
            .id,
        kept.to_string(),
        "discovery and retained storage keep the same UUID after moved-base redeployment"
    );

    // A scene beside it, overlapping none, starts empty.
    seed_scene(&pool, &world, "bafkrei-b", &["5,5"], Some("5,5"), false).await;
    let beside = active_identity(&pool, &world, "5,5").await.unwrap();
    assert_ne!(beside, kept);
    let meta_b = scene_metadata(world.display(), "5,5", "bafkrei-b");
    let (status, _) = send(&app, get("/values/k".into(), meta_b).await).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // One overlapping both starts empty and retires both identities.
    redeploy(
        &pool,
        &world,
        &["bafkrei-v4", "bafkrei-b"],
        "bafkrei-merged",
        &["7,7", "5,5"],
        Some("5,5"),
    )
    .await;
    let merged = active_identity(&pool, &world, "7,7").await.unwrap();
    assert!(merged != kept && merged != beside);
    assert!(
        places
            .find_by_id_for(&kept.to_string(), None)
            .await
            .unwrap()
            .is_none(),
        "retired storage remains retained but its place is no longer discoverable"
    );
    assert_eq!(
        places
            .find_by_id_for(&merged.to_string(), None)
            .await
            .unwrap()
            .unwrap()
            .id,
        merged.to_string()
    );
    let meta_m = scene_metadata(world.display(), "7,7", "bafkrei-merged");
    let (status, _) = send(&app, get("/values/k".into(), meta_m).await).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let active: Vec<Uuid> = sqlx::query_scalar(
        "SELECT identity FROM explore.world_scene_place WHERE world_name = $1 AND active",
    )
    .bind(world.key())
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(active, vec![merged]);
    let scope = SceneNamespace::new(
        &RealmId::World(world.clone()),
        Parcel::parse("0,0").unwrap(),
        Some(kept),
    )
    .unwrap();
    assert_eq!(
        state.store.world_get(&scope, "k").await.unwrap().as_deref(),
        Some("\"v1\""),
        "a retired identity keeps its rows"
    );
    assert_eq!(
        state
            .store
            .player_get(&scope, &player, "k")
            .await
            .unwrap()
            .as_deref(),
        Some("\"p\"")
    );

    // Undeployed: the identity retires, the routes answer 400, the rows survive.
    delete_scene(&pool, &world, "bafkrei-merged").await;
    assert_eq!(active_identity(&pool, &world, "7,7").await, None);
    let (status, _) = send(&app, get("/values/k".into(), meta_v1).await).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM worlds.world_storage WHERE realm = $1")
            .bind(world.key())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rows, 1);
    // Even the world row's deletion cascades to scenes and grants, never to storage.
    sqlx::query("DELETE FROM worlds.worlds WHERE name = $1")
        .bind(world.key())
        .execute(&pool)
        .await
        .unwrap();
    let rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM worlds.player_storage WHERE realm = $1")
            .bind(world.key())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rows, 1);
}

async fn seed_genesis_scene(pool: &PgPool) -> (String, String, String, i32) {
    let id = format!("bafyWS{}", Uuid::new_v4().simple());
    let x = 40_000 + (Uuid::new_v4().as_u128() % 10_000) as i32;
    let (p1, p2) = (format!("{x},7"), format!("{x},8"));
    let dep_id: i32 = sqlx::query_scalar(
        "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
         VALUES ('0x0', 'v3', 'scene', $1, $2::json, timezone('UTC', now()), $3, timezone('UTC', now()), '[]'::json) RETURNING id",
    )
    .bind(&id)
    .bind(json!({ "scene": { "base": p2 } }))
    .bind(vec![p1.clone(), p2.clone()])
    .fetch_one(pool)
    .await
    .unwrap();
    for p in [&p1, &p2] {
        sqlx::query("INSERT INTO content.active_pointers (pointer, entity_id) VALUES ($1, $2)")
            .bind(p)
            .bind(&id)
            .execute(pool)
            .await
            .unwrap();
    }
    (id, p1, p2, dep_id)
}

async fn remove_genesis_scene(pool: &PgPool, id: &str, dep_id: i32) {
    sqlx::query("DELETE FROM social.scene_admins WHERE place_id = $1")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM content.active_pointers WHERE entity_id = $1")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM content.deployments WHERE id = $1")
        .bind(dep_id)
        .execute(pool)
        .await
        .unwrap();
}

/// Genesis scenes have a valid scope too: the active pointer names the
/// deployment, every shared realm folds into `main`, and the LAND authority
/// of the parcel the request names is admitted (owner, operator,
/// updateOperator, updateManager, approvedForAll: the old lambdas matrix),
/// never a scene admin (comms' row) nor an outsider.
#[tokio::test]
async fn genesis_scenes_scope_by_the_active_pointer_and_admit_the_land_authority_only() {
    let Some(pool) = scratch().await else { return };
    let (id, p1, p2, dep_id) = seed_genesis_scene(&pool).await;
    let admin_key = random_key();
    sqlx::query(
        "INSERT INTO social.scene_admins (place_id, admin_address, added_by) VALUES ($1, $2, $3)",
    )
    .bind(&id)
    .bind(Address::parse(&address_of(&admin_key)).unwrap())
    .bind(address())
    .execute(&pool)
    .await
    .unwrap();
    // The manager document of p1 only; p2 has none.
    let [
        owner_key,
        operator_key,
        update_operator_key,
        manager_key,
        approved_key,
    ] = std::array::from_fn(|_| random_key());
    let lambdas = fake_land_operators(
        &p1,
        json!({
            "owner": address_of(&owner_key),
            "operator": address_of(&operator_key).to_uppercase(),
            "updateOperator": address_of(&update_operator_key),
            "updateManagers": [address_of(&manager_key)],
            "approvedForAll": [address_of(&approved_key)],
        }),
    )
    .await;
    let state = state_with_land_operators(&pool, &lambdas);
    let app = router(state.clone());
    let meta_p1 = scene_metadata("main", &p1, &id);
    let meta_p2 = scene_metadata("main", &p2, &id);

    for (who, key) in [
        ("owner", &owner_key),
        ("operator", &operator_key),
        ("updateOperator", &update_operator_key),
        ("updateManager", &manager_key),
        ("approvedForAll", &approved_key),
    ] {
        let body = format!(r#"{{"value":"{who}"}}"#);
        let (status, resp) = send(
            &app,
            signed_request("put", "/values/k", key, &meta_p1, None, Some(&body)).await,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{who}: {resp}");
        let (status, _) = send(
            &app,
            signed_request("get", "/values/k", key, &meta_p2, None, None).await,
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "{who}: LAND at another footprint parcel grants nothing at the one named"
        );
    }

    // What the refused matrix must leave untouched: a world, player and env value.
    let player = address();
    let player_values = format!("/players/{}/values", player.as_str());
    let player_value = format!("{player_values}/score");
    for (path, body) in [
        ("/values/k", r#"{"value":"g"}"#),
        (player_value.as_str(), r#"{"value":1}"#),
        ("/env/API_URL", r#"{"value":"https://api.example.com"}"#),
    ] {
        let (status, resp) = send(
            &app,
            signed_request("put", path, &owner_key, &meta_p1, None, Some(body)).await,
        )
        .await;
        assert!(status.is_success(), "owner PUT {path}: {status} {resp}");
    }

    // The scene admin (no LAND right) and an outsider: 401 on every route.
    let routes = [
        ("get", "/values".to_string(), None),
        ("delete", "/values".to_string(), None),
        ("get", "/values/k".to_string(), None),
        ("put", "/values/k".to_string(), Some(r#"{"value":"x"}"#)),
        ("delete", "/values/k".to_string(), None),
        ("get", "/usage/world".to_string(), None),
        ("get", "/players".to_string(), None),
        ("delete", "/players".to_string(), None),
        ("get", player_values.clone(), None),
        ("delete", player_values.clone(), None),
        ("get", player_value.clone(), None),
        ("put", player_value.clone(), Some(r#"{"value":2}"#)),
        ("delete", player_value.clone(), None),
        ("get", format!("/usage/players/{}", player.as_str()), None),
        ("get", "/env".to_string(), None),
        ("delete", "/env".to_string(), None),
        ("get", "/env/API_URL".to_string(), None),
        (
            "put",
            "/env/API_URL".to_string(),
            Some(r#"{"value":"https://evil.example.com"}"#),
        ),
        ("delete", "/env/API_URL".to_string(), None),
        ("get", "/usage/env".to_string(), None),
    ];
    for (who, key) in [
        ("scene admin", admin_key.as_str()),
        ("outsider", OUTSIDER_KEY),
    ] {
        for (method, path, body) in &routes {
            let mut req = signed_request(method, path, key, &meta_p1, None, *body).await;
            req.headers_mut()
                .insert("x-confirm-delete-all", HeaderValue::from_static("1"));
            let (status, resp) = send(&app, req).await;
            assert_eq!(
                (status, resp["message"].as_str()),
                (StatusCode::UNAUTHORIZED, Some(NOT_AUTHORIZED_MSG)),
                "{who}: {method} {path}"
            );
        }
    }
    let scope = ns("main", &p2);
    assert_eq!(
        state.store.world_get(&scope, "k").await.unwrap().as_deref(),
        Some("\"g\""),
        "the scope is (main, base)"
    );
    assert_eq!(
        state
            .store
            .player_get(&scope, &player, "score")
            .await
            .unwrap()
            .as_deref(),
        Some("1")
    );
    assert!(
        state
            .store
            .env_get_enc(&scope, "API_URL")
            .await
            .unwrap()
            .is_some()
    );

    // Another shared realm: the same scope.
    let (status, body) = send(
        &app,
        signed_request(
            "get",
            "/values/k",
            &owner_key,
            &scene_metadata("artemis", &p1, &id),
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!((status, body), (StatusCode::OK, json!({ "value": "g" })));

    // Without a LAND source no LAND authority (and no admin) is admitted.
    let bare = router(self::state(&pool, None));
    for key in [&owner_key, &operator_key, &admin_key] {
        let (status, _) = send(
            &bare,
            signed_request("get", "/values/k", key, &meta_p1, None, None).await,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    remove_genesis_scene(&pool, &id, dep_id).await;
}

#[tokio::test]
async fn genesis_land_source_failure_returns_503_without_granting_storage() {
    let Some(pool) = scratch().await else { return };
    let (id, p1, _, dep_id) = seed_genesis_scene(&pool).await;
    let service = Router::new().fallback(|| async { StatusCode::SERVICE_UNAVAILABLE });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, service).await.unwrap() });
    let app = router(state_with_land_operators(&pool, &format!("http://{addr}")));
    let key = random_key();
    let meta = scene_metadata("main", &p1, &id);
    let (status, _) = send(
        &app,
        signed_request(
            "put",
            "/values/k",
            &key,
            &meta,
            None,
            Some(r#"{"value":1}"#),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    server.abort();
    remove_genesis_scene(&pool, &id, dep_id).await;
}

/// The published LAND index supplies estate ownership and every grant kind
/// to the same storage authorization path as the lambdas document.
#[tokio::test]
async fn genesis_storage_admits_estate_owner_and_grants_from_the_land_index() {
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    let Ok(url) = std::env::var("CATALYST_TEST_DATABASE_URL") else {
        return;
    };
    // Readiness is global per network. Use an independent database so this
    // fixture cannot replace another concurrently running test's LAND source.
    let opts: PgConnectOptions = url.parse().unwrap();
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(opts.clone().database("postgres"))
        .await
        .unwrap();
    let name = format!("catalyst_test_storage_land_{}", Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
        .execute(&admin)
        .await
        .unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect_with(opts.database(&name))
        .await
        .unwrap();
    crate::db::apply_schema(&pool).await.unwrap();
    let (id, p1, _, _) = seed_genesis_scene(&pool).await;
    let parcel = Parcel::parse(&p1).unwrap();
    let keys = [
        random_key(),
        random_key(),
        random_key(),
        random_key(),
        random_key(),
        random_key(),
    ];
    let addresses: Vec<_> = keys.iter().map(|key| address_of(key)).collect();
    let settings = crate::land_authz::Settings::for_tests();
    sqlx::query("INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block, folded_block, folded_at, full_replay_started) VALUES ($1, $2, $3, 1, 1, timezone('UTC', now()), true)")
        .bind(&settings.network).bind(&settings.land_registry).bind(&settings.estate_registry)
        .execute(&pool).await.unwrap();
    // The parcel belongs to estate 500; the estate supplies its owner,
    // operator and updateOperator, with account-level grants below.
    sqlx::query("INSERT INTO land_authz.token_right (token_address, token_id, x, y, owner, updated_block, updated_log) VALUES ($1, $2::int::numeric * 340282366920938463463374607431768211456 + $3::int::numeric, $2, $3, $4, 1, 0)")
        .bind(&settings.land_registry).bind(parcel.x).bind(parcel.y).bind(&settings.estate_registry)
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO land_authz.token_right (token_address, token_id, owner, operator, update_operator, updated_block, updated_log) VALUES ($1, 500, $2, $3, $4, 1, 0)")
        .bind(&settings.estate_registry).bind(&addresses[0]).bind(&addresses[1]).bind(&addresses[2])
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO land_authz.estate_land (land_token_id, estate_token_id, updated_block, updated_log) VALUES ($1::int::numeric * 340282366920938463463374607431768211456 + $2::int::numeric, 500, 1, 0)")
        .bind(parcel.x).bind(parcel.y).execute(&pool).await.unwrap();
    for (operator, kind) in [
        (&addresses[3], "update_manager"),
        (&addresses[4], "approved_for_all"),
    ] {
        sqlx::query("INSERT INTO land_authz.account_right (token_address, account, operator, kind, is_approved, updated_block, updated_log) VALUES ($1, $2, $3, $4, true, 1, 0)")
            .bind(&settings.estate_registry).bind(&addresses[0]).bind(operator).bind(kind)
            .execute(&pool).await.unwrap();
    }
    let land = Arc::new(crate::land_authz::LandAuthzState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        settings,
        None,
    ));
    let policy = Arc::new(
        Policy::new(pool.clone(), PersonalWorldsPolicy::disabled(), None)
            .with_land_authz(Some(land)),
    );
    let app = router(Arc::new(StorageState::new(
        pool.clone(),
        policy,
        Settings::for_tests(None),
        None,
    )));
    let meta = scene_metadata("main", &p1, &id);
    for (i, key) in keys.iter().enumerate() {
        let (status, _) = send(
            &app,
            signed_request(
                "put",
                "/values/indexed",
                key,
                &meta,
                None,
                Some(r#"{"value":1}"#),
            )
            .await,
        )
        .await;
        assert_eq!(
            status,
            if i < 5 {
                StatusCode::OK
            } else {
                StatusCode::UNAUTHORIZED
            },
            "grant {i}"
        );
    }
    sqlx::query("UPDATE land_authz.account_right SET is_approved = false WHERE operator = $1")
        .bind(&addresses[4])
        .execute(&pool)
        .await
        .unwrap();
    let (status, _) = send(
        &app,
        signed_request(
            "put",
            "/values/indexed",
            &keys[4],
            &meta,
            None,
            Some(r#"{"value":2}"#),
        )
        .await,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a revoked index grant must stop writes"
    );
    drop(app);
    pool.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE \"{name}\" WITH (FORCE)"
    )))
    .execute(&admin)
    .await
    .unwrap();
}

/// With only the marketplace LAND reader, a Genesis request is the LAND
/// owner's of the parcel it names: owning another footprint parcel grants
/// nothing, nor does a scene admin row.
#[tokio::test]
async fn genesis_land_ownership_counts_only_at_the_request_parcel() {
    use axum::extract::State;
    let Some(pool) = scratch().await else { return };
    let (id, p1, p2, dep_id) = seed_genesis_scene(&pool).await;
    let (owner_key, admin_key) = (random_key(), random_key());
    sqlx::query(
        "INSERT INTO social.scene_admins (place_id, admin_address, added_by) VALUES ($1, $2, $3)",
    )
    .bind(&id)
    .bind(Address::parse(&address_of(&admin_key)).unwrap())
    .bind(address())
    .execute(&pool)
    .await
    .unwrap();

    // A subgraph where the owner holds p1 only.
    let x = p1.split_once(',').unwrap().0;
    let nfts = json!({ "data": { "nfts": [
        { "searchParcelX": x.to_string(), "searchParcelY": "7", "owner": { "id": address_of(&owner_key) } },
        { "searchParcelX": x.to_string(), "searchParcelY": "8", "owner": { "id": "0x1111111111111111111111111111111111111111" } },
    ] } });
    let subgraph = Router::new()
        .route(
            "/subgraph",
            axum::routing::post(|State(nfts): State<Value>| async move { axum::Json(nfts) }),
        )
        .with_state(nfts);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, subgraph).await.unwrap() });
    let market = crate::chain_index::ChainIndex::new(
        crate::http::Outbound::for_tests(),
        crate::chain_index::IndexSettings {
            marketplace_subgraph_url: Some(format!("http://{addr}/subgraph")),
            ..crate::chain_index::IndexSettings::for_tests()
        },
    );
    market.allow_loopback_for_tests();
    let policy = Arc::new(Policy::new(
        pool.clone(),
        PersonalWorldsPolicy::disabled(),
        Some(Arc::new(market)),
    ));
    let app = router(Arc::new(StorageState::new(
        pool.clone(),
        policy,
        Settings::for_tests(None),
        None,
    )));

    for (who, key, parcel, expected) in [
        ("owner at p1", &owner_key, &p1, StatusCode::OK),
        (
            "owner of p1 naming p2",
            &owner_key,
            &p2,
            StatusCode::UNAUTHORIZED,
        ),
        ("scene admin", &admin_key, &p1, StatusCode::UNAUTHORIZED),
    ] {
        let meta = scene_metadata("main", parcel, &id);
        let (status, _) = send(
            &app,
            signed_request("put", "/values/k", key, &meta, None, Some(r#"{"value":1}"#)).await,
        )
        .await;
        assert_eq!(status, expected, "{who}");
    }

    remove_genesis_scene(&pool, &id, dep_id).await;
}

/// Nested under `/world-storage`, a signature over the public path verifies
/// without a proxy header; the env routes answer 503 without a key.
#[tokio::test]
async fn the_nested_public_path_verifies_and_env_needs_a_key() {
    let Some(pool) = scratch().await else { return };
    let (world, _) = seed_owned_world(&pool).await;
    let owner_key = random_key();
    seed_world(
        &pool,
        &world,
        Some(&Address::parse(&address_of(&owner_key)).unwrap()),
    )
    .await;
    let mut settings = Settings::for_tests(None);
    settings.encryption_key = None;
    let app = Router::new().nest("/world-storage", router(state_with(&pool, settings)));
    let meta = scene_metadata(world.display(), "0,0", SCENE_ID);

    let (status, body) = send(
        &app,
        signed_request(
            "put",
            "/world-storage/values/k",
            &owner_key,
            &meta,
            None,
            Some(r#"{"value":1}"#),
        )
        .await,
    )
    .await;
    assert_eq!((status, body), (StatusCode::OK, json!({ "value": 1 })));
    // Signed over the stripped route, delivered through a proxy header: the old deployment's shape.
    let headers = signed_as(&owner_key, Shape::V6, "get", "/values/k", &meta, &meta).await;
    let mut req = with_headers("get", "/world-storage/values/k", headers, None, None);
    req.headers_mut().insert(
        "x-original-path",
        HeaderValue::from_static("/world-storage/values/k"),
    );
    let (status, body) = send(&app, req).await;
    assert_eq!((status, body), (StatusCode::OK, json!({ "value": 1 })));
    // Signed over an unrelated path: refused.
    let headers = signed_as(
        &owner_key,
        Shape::V6,
        "get",
        "/elsewhere/values/k",
        &meta,
        &meta,
    )
    .await;
    let (status, body) = send(
        &app,
        with_headers("get", "/world-storage/values/k", headers, None, None),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .starts_with("Invalid signature"),
        "{body}"
    );

    let (status, body) = send(
        &app,
        signed_request(
            "put",
            "/world-storage/env/k",
            &owner_key,
            &meta,
            None,
            Some(r#"{"value":"s3cret"}"#),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert!(
        body["message"].as_str().unwrap().contains("ENCRYPTION_KEY"),
        "{body}"
    );
    let (status, body) = send(
        &app,
        signed_request("get", "/world-storage/env/k", &owner_key, &meta, None, None).await,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "an owner never reads env values, key or no key: {body}"
    );
    let (status, body) = send(
        &app,
        signed_request(
            "get",
            "/world-storage/usage/env",
            &owner_key,
            &meta,
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(
        (status, body["usedBytes"].as_i64()),
        (StatusCode::OK, Some(0)),
        "usage and the key listing need no key"
    );
    let (status, body) = send(
        &app,
        signed_request("get", "/world-storage/env", &owner_key, &meta, None, None).await,
    )
    .await;
    assert_eq!(
        (status, body["pagination"]["total"].as_i64()),
        (StatusCode::OK, Some(0))
    );
}

/// A GET to `target` signed by `key` over `method signed`, optionally with an `x-original-path`.
async fn signed_get(
    app: &Router,
    key: &str,
    shape: Shape,
    (method, signed): (&str, &str),
    target: &str,
    original_path: Option<&'static str>,
    meta: &str,
) -> (StatusCode, Value) {
    let headers = signed_as(key, shape, method, signed, meta, meta).await;
    let mut req = with_headers("get", target, headers, None, None);
    if let Some(p) = original_path {
        req.headers_mut()
            .insert("x-original-path", HeaderValue::from_static(p));
    }
    send(app, req).await
}

#[tokio::test]
async fn a_pathname_signature_verifies_a_listing_with_a_query_and_opens_nothing_else() {
    let Some(pool) = scratch().await else { return };
    let world = world_name();
    let owner_key = random_key();
    seed_world(
        &pool,
        &world,
        Some(&Address::parse(&address_of(&owner_key)).unwrap()),
    )
    .await;
    seed_scene(&pool, &world, SCENE_ID, &["0,0", "0,1"], Some("0,0"), false).await;
    let state = state_with(&pool, Settings::for_tests(None));
    let root = router(state.clone());
    let nested = Router::new().nest("/world-storage", router(state));
    let meta = scene_metadata(world.display(), "0,0", SCENE_ID);
    let (status, body) = send(
        &root,
        signed_request(
            "put",
            "/values/alpha",
            &owner_key,
            &meta,
            None,
            Some(r#"{"value":1}"#),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let key = owner_key.as_str();
    for (case, app, shape, signed, target, original_path) in [
        (
            "path and query, nested",
            &nested,
            Shape::V6,
            "/world-storage/values?limit=10",
            "/world-storage/values?limit=10",
            None,
        ),
        (
            "pathname, nested",
            &nested,
            Shape::V6,
            "/world-storage/values",
            "/world-storage/values?limit=10",
            None,
        ),
        (
            "pathname, direct",
            &root,
            Shape::V6,
            "/values",
            "/values?limit=10",
            None,
        ),
        (
            "pathname, subdomain proxy",
            &nested,
            Shape::V6,
            "/values",
            "/world-storage/values?limit=10",
            Some("/values?limit=10"),
        ),
        (
            "public pathname, prefix proxy",
            &nested,
            Shape::V6,
            "/world-storage/values",
            "/world-storage/values?limit=10",
            Some("/world-storage/values?limit=10"),
        ),
        (
            "pathname, root mount",
            &root,
            Shape::V6,
            "/values",
            "/values?prefix=a&limit=1",
            None,
        ),
        (
            "legacy payload over the pathname",
            &nested,
            Shape::Legacy,
            "/world-storage/values",
            "/world-storage/values?limit=10",
            None,
        ),
    ] {
        let (status, body) = signed_get(
            app,
            key,
            shape,
            ("get", signed),
            target,
            original_path,
            &meta,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{case}: {body}");
        assert_eq!(body["pagination"]["total"], json!(1), "{case}: {body}");
    }

    for (case, method, signed, target, original_path) in [
        ("another route", "get", "/values", "/env?limit=1", None),
        (
            "another key",
            "get",
            "/values/alpha",
            "/values?limit=1",
            None,
        ),
        ("players", "get", "/players", "/values?limit=1", None),
        (
            "player route suffix",
            "get",
            "/players/0xabc/values",
            "/values",
            Some("/players/0xabc/values"),
        ),
        (
            "another method",
            "delete",
            "/values",
            "/values?limit=1",
            None,
        ),
        (
            "forged x-original-path",
            "get",
            "/values",
            "/env",
            Some("/values?limit=1"),
        ),
    ] {
        let (status, body) = signed_get(
            &root,
            key,
            Shape::V6,
            (method, signed),
            target,
            original_path,
            &meta,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{case}: {body}");
        assert!(
            body["error"]
                .as_str()
                .unwrap()
                .starts_with("Invalid signature"),
            "{case}: {body}"
        );
    }
}

const METHOD: &str = "put";
const PATH: &str = "/values/k";

fn posture_metadata(signer: &str) -> String {
    json!({
        "signer": signer,
        "realmName": "some.dcl.eth",
        "realm": { "serverName": "some.dcl.eth" },
        "sceneId": "bafkreiabc",
        "parcel": "0,0",
    })
    .to_string()
}

async fn verify(headers: &HeaderMap) -> Result<Address, AuthChainError> {
    verify_request(headers, METHOD, &[PATH.to_string()], None)
        .await
        .map(|v| v.signer)
}

/// The crypto-middleware 6 wire format: method and path folded, metadata joined verbatim.
#[tokio::test]
async fn accepts_a_v6_signed_request_with_cased_metadata() {
    let metadata = json!({ "realmName": "Some.DCL.eth", "sceneId": "bafkreiABC" }).to_string();
    let signer = verify(&signed_as(KEY, Shape::V6, METHOD, PATH, &metadata, &metadata).await)
        .await
        .expect("v6-signed request must verify");
    assert_eq!(signer.as_str(), address_of(KEY));
}

/// Our own in-tree signers (the scene-state storage proxy) still mint the
/// folded payload, so it stays accepted behind the declared-key guard.
#[tokio::test]
async fn still_accepts_a_legacy_signed_request() {
    let metadata = posture_metadata("dcl:authoritative-server");
    let signer = verify(&signed_as(KEY, Shape::Legacy, METHOD, PATH, &metadata, &metadata).await)
        .await
        .expect("legacy-signed request must still verify");
    assert_eq!(signer.as_str(), address_of(KEY));
}

/// A key folding to `signer` is refused by the scene gate ahead of any
/// signature check; the other three are refused by the declared-key guard.
#[tokio::test]
async fn refuses_a_legacy_request_that_respells_an_authorized_key() {
    for (declared, respelled, from_the_gate) in [
        ("signer", "Signer", true),
        ("realmName", "RealmName", false),
        ("parcel", "Parcel", false),
        ("sceneId", "SceneId", false),
    ] {
        let mut object: serde_json::Map<String, Value> =
            serde_json::from_str(&posture_metadata("dcl:authoritative-server")).unwrap();
        let value = object.remove(declared).unwrap();
        object.insert(respelled.to_string(), value);
        let metadata = Value::Object(object).to_string();
        let err = verify(&signed_as(KEY, Shape::Legacy, METHOD, PATH, &metadata, &metadata).await)
            .await
            .expect_err("a re-spelled authorized key must not be served");
        let refused_by_the_expected_guard = if from_the_gate {
            matches!(err, AuthChainError::SceneSignerRejected)
        } else {
            matches!(err, AuthChainError::MalformedChain { .. })
        };
        assert!(
            refused_by_the_expected_guard,
            "{respelled:?} produced {err:?}"
        );
    }
}

#[tokio::test]
async fn refuses_a_scene_signer_in_every_spelling() {
    for signer in [
        "decentraland-kernel-scene",
        " decentraland-kernel-scene",
        "decentraland-kernel-scene ",
        "\tdecentraland-kernel-scene",
        "Decentraland-Kernel-Scene",
    ] {
        let metadata = posture_metadata(signer);
        for shape in [Shape::Legacy, Shape::V6] {
            let err = verify(&signed_as(KEY, shape, METHOD, PATH, &metadata, &metadata).await)
                .await
                .expect_err("a scene-signed request must never reach this surface");
            assert!(
                matches!(err, AuthChainError::SceneSignerRejected),
                "{signer:?} {shape:?} produced {err:?}"
            );
        }
    }
}

/// The gate answers before signature verification, so a refused request pays
/// no catalyst round-trip for an EIP-1654 chain.
#[tokio::test]
async fn the_scene_gate_answers_before_signature_verification() {
    let signed = posture_metadata("dcl:authoritative-server");
    let delivered = posture_metadata("decentraland-kernel-scene");
    let err = verify(&signed_as(KEY, Shape::V6, METHOD, PATH, &signed, &delivered).await)
        .await
        .expect_err("a scene-signed request must be refused");
    assert!(
        matches!(err, AuthChainError::SceneSignerRejected),
        "the gate must win over the signature check, got {err:?}"
    );
}

/// Freshness answers before the gate: a replayed scene-signed request is
/// refused as expired, not as a scene.
#[tokio::test]
async fn a_stale_scene_signed_request_is_expired_before_the_gate_runs() {
    let ts = (chrono::Utc::now().timestamp_millis() - 10 * 60 * 1000).to_string();
    let metadata = posture_metadata("decentraland-kernel-scene");
    let err = verify(&signed_at(KEY, Shape::V6, METHOD, PATH, &metadata, &metadata, &ts).await)
        .await
        .expect_err("a ten-minute-old request must be refused");
    assert!(
        matches!(err, AuthChainError::Expired { .. }),
        "freshness must answer before the gate, got {err:?}"
    );
    // The window is 60 s: 90 s old is refused.
    let ts = (chrono::Utc::now().timestamp_millis() - 90 * 1000).to_string();
    let metadata = posture_metadata("dcl:authoritative-server");
    assert!(matches!(
        verify(&signed_at(KEY, Shape::V6, METHOD, PATH, &metadata, &metadata, &ts).await).await,
        Err(AuthChainError::Expired { .. })
    ));
}

#[tokio::test]
async fn refuses_a_timestamp_that_is_not_plain_integer_milliseconds() {
    for ts in ["1.7e12", "1700000000000.0", "Infinity"] {
        let metadata = posture_metadata("dcl:authoritative-server");
        let err = verify(&signed_at(KEY, Shape::V6, METHOD, PATH, &metadata, &metadata, ts).await)
            .await
            .expect_err("a timestamp with no computable window must not be served");
        assert!(
            matches!(&err, AuthChainError::InvalidTimestamp(value) if value == ts),
            "timestamp {ts:?} produced {err:?}"
        );
    }
}

/// Upstream reads the header as `Number(raw || '0')`: absent or empty is
/// timestamp zero and answers the expiration window's 401.
#[tokio::test]
async fn an_absent_or_empty_timestamp_header_expires() {
    let metadata = posture_metadata("dcl:authoritative-server");
    let mut headers = signed_at(KEY, Shape::V6, METHOD, PATH, &metadata, &metadata, "0").await;
    headers.remove("x-identity-timestamp");
    let err = verify(&headers)
        .await
        .expect_err("a request with no timestamp must not be served");
    assert!(
        matches!(err, AuthChainError::Expired { signed_at: 0, .. }),
        "an absent timestamp produced {err:?}"
    );
    assert_eq!(err.status_code(), 401);
    assert!(
        err.raw_message().starts_with("Expired signature: "),
        "{}",
        err.raw_message()
    );

    let mut headers = signed_at(KEY, Shape::V6, METHOD, PATH, &metadata, &metadata, "0").await;
    headers.insert("x-identity-timestamp", HeaderValue::from_static(""));
    let err = verify(&headers)
        .await
        .expect_err("an empty timestamp must not be served");
    assert!(
        matches!(err, AuthChainError::Expired { signed_at: 0, .. }),
        "an empty timestamp produced {err:?}"
    );
}

#[tokio::test]
async fn a_non_object_metadata_header_is_refused_before_the_gate() {
    for metadata in ["not json", "[1,2]", "\"decentraland-kernel-scene\"", "5"] {
        let err = verify(&signed_as(KEY, Shape::V6, METHOD, PATH, metadata, metadata).await)
            .await
            .expect_err("a non-object metadata header must not be served");
        assert_eq!(
            err.status_code(),
            400,
            "metadata {metadata:?} produced {err:?}"
        );
        assert_eq!(
            err.raw_message(),
            format!("Invalid chain format: invalid chain metadata: \"{metadata}\"")
        );
    }
    let err = verify(&HeaderMap::new()).await.unwrap_err();
    assert_eq!(
        (err.status_code(), err.raw_message().as_str()),
        (400, "Invalid Auth Chain")
    );
}

#[tokio::test]
async fn a_canonical_non_scene_signer_is_served_and_a_tampered_one_is_not() {
    let metadata = posture_metadata("dcl:authoritative-server");
    assert!(
        verify(&signed_as(KEY, Shape::V6, METHOD, PATH, &metadata, &metadata).await)
            .await
            .is_ok()
    );
    let tampered = metadata.replace("bafkreiabc", "bafkreixyz");
    let err = verify(&signed_as(KEY, Shape::V6, METHOD, PATH, &metadata, &tampered).await)
        .await
        .unwrap_err();
    assert!(
        matches!(err, AuthChainError::InvalidSignature(_)),
        "{err:?}"
    );
    assert_eq!(err.status_code(), 401);
    // Signed over another route: refused even though the chain is authentic.
    let err = verify(&signed_as(KEY, Shape::V6, METHOD, "/env/k", &metadata, &metadata).await)
        .await
        .unwrap_err();
    assert!(
        matches!(err, AuthChainError::InvalidSignature(_)),
        "{err:?}"
    );
}

/// The nested public path is one of the candidates; an unrelated one never is.
#[tokio::test]
async fn the_signature_may_be_over_the_nested_original_path() {
    let metadata = posture_metadata("dcl:authoritative-server");
    let paths = vec!["/world-storage/values/k".to_string(), PATH.to_string()];
    let headers = signed_as(
        KEY,
        Shape::V6,
        METHOD,
        "/world-storage/values/k",
        &metadata,
        &metadata,
    )
    .await;
    assert!(verify_request(&headers, METHOD, &paths, None).await.is_ok());
    let headers = signed_as(
        KEY,
        Shape::Legacy,
        METHOD,
        "/world-storage/values/k",
        &metadata,
        &metadata,
    )
    .await;
    assert!(
        verify_request(&headers, METHOD, &paths, None).await.is_ok(),
        "the legacy shape over the public path"
    );
    let headers = signed_as(
        KEY,
        Shape::V6,
        METHOD,
        "/other/values/k",
        &metadata,
        &metadata,
    )
    .await;
    assert!(matches!(
        verify_request(&headers, METHOD, &paths, None)
            .await
            .map(|_| ()),
        Err(AuthChainError::InvalidSignature(_))
    ));
}

/// A local world scene whose entity carries `logsPermissions` and a title.
async fn seed_logs_scene(
    pool: &PgPool,
    world: &WorldName,
    id: &str,
    parcels: &[&str],
    logs: Value,
    mirrored: bool,
) {
    seed_scene(pool, world, id, parcels, Some(parcels[0]), mirrored).await;
    sqlx::query(
        "UPDATE worlds.world_scenes SET entity = jsonb_set(jsonb_set(entity, '{metadata,logsPermissions}', $3), '{metadata,display}', jsonb_build_object('title', 'Scene ' || $2)) \
         WHERE world_name = $1 AND entity_id = $2",
    )
    .bind(world.key())
    .bind(id)
    .bind(logs)
    .execute(pool)
    .await
    .unwrap();
}

/// A Genesis scene deployment, with its active pointers when `active`.
async fn seed_genesis(
    pool: &PgPool,
    id: &str,
    pointers: &[String],
    logs: Value,
    active: bool,
) -> i32 {
    let dep_id: i32 = sqlx::query_scalar(
        "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
         VALUES ('0x0', 'v3', 'scene', $1, $2::json, timezone('UTC', now()), $3, timezone('UTC', now()), '[]'::json) RETURNING id",
    )
    .bind(id)
    .bind(json!({ "scene": { "base": pointers[0] }, "display": { "title": "Genesis" }, "logsPermissions": logs }))
    .bind(pointers)
    .fetch_one(pool)
    .await
    .unwrap();
    if active {
        for p in pointers {
            sqlx::query(
                "INSERT INTO content.active_pointers (pointer, entity_id) VALUES ($1, $2) \
                 ON CONFLICT (pointer) DO UPDATE SET entity_id = EXCLUDED.entity_id",
            )
            .bind(p)
            .bind(id)
            .execute(pool)
            .await
            .unwrap();
        }
    }
    dep_id
}

// LAND readiness is global per network, so these fixtures use private databases.
struct CollaboratorLandFixture {
    pool: PgPool,
    admin: PgPool,
    name: String,
}

impl CollaboratorLandFixture {
    async fn new() -> Option<Self> {
        use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
        let url = std::env::var("CATALYST_TEST_DATABASE_URL").ok()?;
        let opts: PgConnectOptions = url.parse().unwrap();
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(opts.clone().database("postgres"))
            .await
            .unwrap();
        let name = format!("storage_collab_{}", Uuid::new_v4().simple());
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&admin)
            .await
            .unwrap();
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect_with(opts.database(&name))
            .await
            .unwrap();
        crate::db::apply_schema(&pool).await.unwrap();
        let settings = crate::land_authz::Settings::for_tests();
        sqlx::query("INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block, folded_block, folded_at, full_replay_started, history_timestamps_ready) VALUES ($1, $2, $3, 100, 100, timezone('UTC', now()), true, true)")
            .bind(&settings.network).bind(&settings.land_registry).bind(&settings.estate_registry)
            .execute(&pool).await.unwrap();
        Some(Self { pool, admin, name })
    }

    fn state(&self) -> SharedState {
        let land = Arc::new(crate::land_authz::LandAuthzState::new(
            self.pool.clone(),
            crate::http::Outbound::for_tests(),
            crate::land_authz::Settings::for_tests(),
            None,
        ));
        let policy = Policy::new(self.pool.clone(), PersonalWorldsPolicy::disabled(), None)
            .with_land_authz(Some(land));
        Arc::new(StorageState::new(
            self.pool.clone(),
            Arc::new(policy),
            Settings::for_tests(None),
            None,
        ))
    }

    async fn parcel(&self, pointer: &str, owner: &str, block: i64) {
        let parcel = Parcel::parse(pointer).unwrap();
        let settings = crate::land_authz::Settings::for_tests();
        sqlx::query("INSERT INTO land_authz.token_right (token_address, token_id, x, y, owner, updated_block, updated_log) VALUES ($1, $2::int::numeric * 340282366920938463463374607431768211456 + $3::int::numeric, $2, $3, $4, $5, 0)")
            .bind(&settings.land_registry).bind(parcel.x).bind(parcel.y).bind(owner).bind(block)
            .execute(&self.pool).await.unwrap();
        self.transfer(pointer, owner, block, 3600).await;
    }

    async fn transfer(&self, pointer: &str, owner: &str, block: i64, seconds_ago: i64) {
        let parcel = Parcel::parse(pointer).unwrap();
        let settings = crate::land_authz::Settings::for_tests();
        sqlx::query("INSERT INTO land_authz.authz_event (block_number, log_index, block_time, token_address, kind, token_id, account) VALUES ($1, 0, extract(epoch FROM now())::bigint - $2, $3, 'transfer', $4::int::numeric * 340282366920938463463374607431768211456 + $5::int::numeric, $6)")
            .bind(block).bind(seconds_ago).bind(&settings.land_registry).bind(parcel.x).bind(parcel.y).bind(owner)
            .execute(&self.pool).await.unwrap();
        sqlx::query("UPDATE land_authz.token_right SET owner = $3 WHERE token_address = $1 AND x = $2 AND y = $4")
            .bind(&settings.land_registry).bind(parcel.x).bind(owner).bind(parcel.y)
            .execute(&self.pool).await.unwrap();
    }

    async fn close(self) {
        self.pool.close().await;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{}\" WITH (FORCE)",
            self.name
        )))
        .execute(&self.admin)
        .await
        .unwrap();
        self.admin.close().await;
    }
}

fn genesis_parcel() -> String {
    format!("{},9", 60_000 + (Uuid::new_v4().as_u128() % 10_000) as i32)
}

fn shouting(address: &str) -> String {
    format!("0x{}", address[2..].to_uppercase())
}

#[tokio::test]
async fn logs_permissions_collaborators_reach_values_and_players_but_not_clears_env_or_usage() {
    let Some(pool) = scratch().await else { return };
    let (world, _owner) = seed_owned_world(&pool).await;
    let (collab_key, nested_key) = (random_key(), random_key());
    delete_scene(&pool, &world, SCENE_ID).await;
    seed_logs_scene(
        &pool,
        &world,
        SCENE_ID,
        &["0,0", "0,1"],
        json!([shouting(&address_of(&collab_key)), 42, { "a": address_of(&nested_key) }, "not-an-address"]),
        false,
    )
    .await;
    let app = router(state(&pool, None));
    let meta = scene_metadata(world.display(), "0,1", SCENE_ID);
    let player = address();
    let player_path = format!("/players/{}/values", player.as_str());

    for (method, path, body, expected) in [
        (
            "put",
            "/values/k".to_string(),
            Some(r#"{"value":1}"#),
            StatusCode::OK,
        ),
        ("get", "/values/k".to_string(), None, StatusCode::OK),
        ("get", "/values".to_string(), None, StatusCode::OK),
        (
            "delete",
            "/values/k".to_string(),
            None,
            StatusCode::NO_CONTENT,
        ),
        (
            "put",
            format!("{player_path}/s"),
            Some(r#"{"value":2}"#),
            StatusCode::OK,
        ),
        ("get", format!("{player_path}/s"), None, StatusCode::OK),
        ("get", player_path.clone(), None, StatusCode::OK),
        ("get", "/players".to_string(), None, StatusCode::OK),
        (
            "delete",
            format!("{player_path}/s"),
            None,
            StatusCode::NO_CONTENT,
        ),
    ] {
        let (status, resp) = send(
            &app,
            signed_request(method, &path, &collab_key, &meta, None, body).await,
        )
        .await;
        assert_eq!(status, expected, "{method} {path}: {resp}");
    }

    for (method, path, body) in [
        ("delete", "/values".to_string(), None),
        ("delete", "/players".to_string(), None),
        ("delete", player_path.clone(), None),
        ("get", "/env".to_string(), None),
        ("delete", "/env".to_string(), None),
        ("get", "/env/k".to_string(), None),
        ("put", "/env/k".to_string(), Some(r#"{"value":"v"}"#)),
        ("delete", "/env/k".to_string(), None),
        ("get", "/usage/world".to_string(), None),
        ("get", "/usage/env".to_string(), None),
        ("get", format!("/usage/players/{}", player.as_str()), None),
    ] {
        let mut req = signed_request(method, &path, &collab_key, &meta, None, body).await;
        req.headers_mut()
            .insert("x-confirm-delete-all", HeaderValue::from_static("1"));
        let (status, resp) = send(&app, req).await;
        assert_eq!(
            (status, resp["message"].as_str()),
            (StatusCode::UNAUTHORIZED, Some(NOT_AUTHORIZED_MSG)),
            "{method} {path}"
        );
    }

    // A nested or non-string entry grants nothing.
    let (status, _) = send(
        &app,
        signed_request("get", "/values", &nested_key, &meta, None, None).await,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // A redeploy without the wallet revokes it on the next request.
    delete_scene(&pool, &world, SCENE_ID).await;
    seed_logs_scene(
        &pool,
        &world,
        "bafkrei-redeployed",
        &["0,0", "0,1"],
        json!([]),
        false,
    )
    .await;
    let (status, _) = send(
        &app,
        signed_request("get", "/values", &collab_key, &meta, None, None).await,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "revoked by the redeploy");

    // A mirrored scene grants its collaborators nothing.
    let mirrored = world_name();
    seed_world(&pool, &mirrored, None).await;
    seed_logs_scene(
        &pool,
        &mirrored,
        "bafyMirroredLogs",
        &["0,0"],
        json!([address_of(&collab_key)]),
        true,
    )
    .await;
    let (status, _) = send(
        &app,
        signed_request(
            "get",
            "/values",
            &collab_key,
            &scene_metadata(mirrored.display(), "0,0", "bafyMirroredLogs"),
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn genesis_logs_permissions_follow_the_active_pointer() {
    let Some(fixture) = CollaboratorLandFixture::new().await else {
        return;
    };
    let pool = fixture.pool.clone();
    let collab_key = random_key();
    let pointer = genesis_parcel();
    fixture.parcel(&pointer, address().as_str(), 1).await;
    let first = format!("bafyLogsA{}", Uuid::new_v4().simple());
    let first_dep = seed_genesis(
        &pool,
        &first,
        std::slice::from_ref(&pointer),
        json!([shouting(&address_of(&collab_key))]),
        true,
    )
    .await;
    let app = router(fixture.state());
    let meta = scene_metadata("main", &pointer, &first);

    let (status, body) = send(
        &app,
        signed_request(
            "put",
            "/values/g",
            &collab_key,
            &meta,
            None,
            Some(r#"{"value":"g"}"#),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // Collaborator grants never authorize destructive clears, secrets or usage.
    for (method, path, payload) in [
        ("delete", "/values", None),
        ("delete", "/players", None),
        ("get", "/env", None),
        ("put", "/env/k", Some(r#"{"value":1}"#)),
        ("delete", "/env", None),
        ("get", "/usage/world", None),
        ("get", "/usage/env", None),
    ] {
        let mut request = signed_request(method, path, &collab_key, &meta, None, payload).await;
        request
            .headers_mut()
            .insert("x-confirm-delete-all", HeaderValue::from_static("1"));
        let (status, response) = send(&app, request).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "{method} {path}: {response}"
        );
    }

    let player = address();
    for (method, path) in [
        ("delete", format!("/players/{}/values", player.as_str())),
        ("get", format!("/usage/players/{}", player.as_str())),
    ] {
        let mut request = signed_request(method, &path, &collab_key, &meta, None, None).await;
        request
            .headers_mut()
            .insert("x-confirm-delete-all", HeaderValue::from_static("1"));
        let (status, response) = send(&app, request).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "{method} {path}: {response}"
        );
    }

    // Published history still proves the collaborator grant while ordinary
    // LAND readiness falls back during cursor advancement. A broken fallback
    // must not suppress that independent grant, but an outsider gets its 503.
    sqlx::query("UPDATE land_authz.index_cursor SET last_block = 101")
        .execute(&pool)
        .await
        .unwrap();
    let broken_source = fake_land_operators(&pointer, json!(null)).await;
    let land = Arc::new(crate::land_authz::LandAuthzState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        crate::land_authz::Settings::for_tests(),
        None,
    ));
    let policy = Policy::new(pool.clone(), PersonalWorldsPolicy::disabled(), None)
        .with_land_authz(Some(land))
        .with_land_operators(Some(LandOperatorsReader::new(
            crate::http::Outbound::for_tests().allowing_private_hosts(),
            &broken_source,
        )));
    let broken_app = router(Arc::new(StorageState::new(
        pool.clone(),
        Arc::new(policy),
        Settings::for_tests(None),
        None,
    )));
    let (status, body) = send(
        &broken_app,
        signed_request("get", "/values/g", &collab_key, &meta, None, None).await,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = send(
        &broken_app,
        signed_request("get", "/values/g", OUTSIDER_KEY, &meta, None, None).await,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    sqlx::query("UPDATE land_authz.index_cursor SET last_block = 100")
        .execute(&pool)
        .await
        .unwrap();

    // Overwritten by a deployment that drops the wallet: refused.
    let second = format!("bafyLogsB{}", Uuid::new_v4().simple());
    let second_dep = seed_genesis(
        &pool,
        &second,
        std::slice::from_ref(&pointer),
        json!([]),
        true,
    )
    .await;
    sqlx::query("UPDATE content.deployments SET deleter_deployment = $2 WHERE id = $1")
        .bind(first_dep)
        .bind(second_dep)
        .execute(&pool)
        .await
        .unwrap();
    let (status, _) = send(
        &app,
        signed_request("get", "/values/g", &collab_key, &meta, None, None).await,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    sqlx::query("DELETE FROM content.active_pointers WHERE pointer = $1")
        .bind(&pointer)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM content.deployments WHERE id = ANY($1)")
        .bind(vec![first_dep, second_dep])
        .execute(&pool)
        .await
        .unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn collaborator_lists_exactly_the_signers_active_scenes() {
    let Some(fixture) = CollaboratorLandFixture::new().await else {
        return;
    };
    let pool = fixture.pool.clone();
    let (collab_key, other_key) = (random_key(), random_key());
    let collab = address_of(&collab_key);
    let (older, newer, mirrored, unrelated) =
        (world_name(), world_name(), world_name(), world_name());
    for w in [&older, &newer, &unrelated] {
        seed_world(&pool, w, Some(&address())).await;
    }
    seed_world(&pool, &mirrored, None).await;
    let ids: Vec<String> = (0..4)
        .map(|i| format!("bafkrei-collab-{i}-{}", Uuid::new_v4().simple()))
        .collect();
    seed_logs_scene(
        &pool,
        &older,
        &ids[0],
        &["3,3"],
        json!([shouting(&collab)]),
        false,
    )
    .await;
    seed_logs_scene(&pool, &newer, &ids[1], &["0,0"], json!([collab]), false).await;
    seed_logs_scene(&pool, &mirrored, &ids[2], &["0,0"], json!([collab]), true).await;
    let other = address_of(&other_key);
    seed_logs_scene(&pool, &unrelated, &ids[3], &["0,0"], json!([other]), false).await;
    for (id, hours_ago) in [(&ids[0], 3), (&ids[1], 2)] {
        sqlx::query(
            "UPDATE worlds.world_scenes SET entity = jsonb_set(entity, '{timestamp}', to_jsonb((extract(epoch FROM timezone('UTC', now()) - make_interval(hours => $2)) * 1000)::bigint)) WHERE entity_id = $1",
        )
        .bind(id)
        .bind(hours_ago)
        .execute(&pool)
        .await
        .unwrap();
    }

    // A declared base outside the footprint must use the resolver's fallback.
    sqlx::query("UPDATE worlds.world_scenes SET entity = jsonb_set(entity, '{metadata,scene,base}', '\"99,99\"'::jsonb) WHERE entity_id = $1")
        .bind(&ids[0]).execute(&pool).await.unwrap();

    // Genesis: one live (the newest), one overwritten, one without an active pointer.
    let (live, overwritten, inactive) = (
        format!("bafyCollabLive{}", Uuid::new_v4().simple()),
        format!("bafyCollabOld{}", Uuid::new_v4().simple()),
        format!("bafyCollabGone{}", Uuid::new_v4().simple()),
    );
    let live_parcel = genesis_parcel();
    fixture.parcel(&live_parcel, address().as_str(), 1).await;
    let deps = vec![
        seed_genesis(
            &pool,
            &live,
            std::slice::from_ref(&live_parcel),
            json!([collab]),
            true,
        )
        .await,
        seed_genesis(
            &pool,
            &overwritten,
            &[genesis_parcel()],
            json!([collab]),
            false,
        )
        .await,
        seed_genesis(
            &pool,
            &inactive,
            &[genesis_parcel()],
            json!([collab]),
            false,
        )
        .await,
    ];
    sqlx::query("UPDATE content.deployments SET deleter_deployment = $2 WHERE id = $1")
        .bind(deps[1])
        .bind(deps[0])
        .execute(&pool)
        .await
        .unwrap();

    let app = router(fixture.state());
    let (status, body) = send(
        &app,
        signed_request("get", "/collaborator", &collab_key, "{}", None, None).await,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "data": [
                { "sceneId": live, "worldName": "main", "baseParcel": live_parcel, "title": "Genesis", "realmKind": "genesis" },
                { "sceneId": ids[1], "worldName": newer.key(), "baseParcel": "0,0", "title": format!("Scene {}", ids[1]), "realmKind": "world" },
                { "sceneId": ids[0], "worldName": older.key(), "baseParcel": "3,3", "title": format!("Scene {}", ids[0]), "realmKind": "world" },
            ],
            "pagination": { "limit": 100, "offset": 0, "total": 3 }
        })
    );

    // Signed over the path with its query; the pathname-only form needs #80.
    let (status, body) = send(
        &app,
        signed_request(
            "get",
            "/collaborator?limit=1&limit=2&offset=1&offset=0",
            &collab_key,
            "{}",
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"][0]["sceneId"], json!(ids[1]));
    assert_eq!(
        body["pagination"],
        json!({ "limit": 1, "offset": 1, "total": 3 })
    );
    let (_, body) = send(
        &app,
        signed_request(
            "get",
            "/collaborator?offset=10",
            &collab_key,
            "{}",
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(
        (body["data"].clone(), body["pagination"]["total"].clone()),
        (json!([]), json!(3))
    );

    let (_, body) = send(
        &app,
        signed_request("get", "/collaborator", OUTSIDER_KEY, "{}", None, None).await,
    )
    .await;
    assert_eq!(body["pagination"]["total"], json!(0));

    let (status, body) = send(
        &app,
        Request::builder()
            .uri("/collaborator")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (
            StatusCode::BAD_REQUEST,
            Some(super::errors::SIGNED_FETCH_MESSAGE)
        )
    );

    sqlx::query("DELETE FROM content.active_pointers WHERE entity_id = $1")
        .bind(&live)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM content.deployments WHERE id = ANY($1)")
        .bind(deps)
        .execute(&pool)
        .await
        .unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn genesis_nonmembers_never_evaluate_the_ownership_history_function() {
    let Some(fixture) = CollaboratorLandFixture::new().await else {
        return;
    };
    let key = random_key();
    let pointer = genesis_parcel();
    fixture.parcel(&pointer, address().as_str(), 1).await;
    let id = format!("bafyHistoryGuard{}", Uuid::new_v4().simple());
    seed_genesis(
        &fixture.pool,
        &id,
        std::slice::from_ref(&pointer),
        json!([address_of(&key)]),
        true,
    )
    .await;
    // Only this private database is changed. A listed collaborator proves the
    // sentinel is live; an outsider must not execute it in either query.
    sqlx::query("CREATE OR REPLACE FUNCTION land_authz.scene_collaborators_current(configured_network text, configured_land text, configured_estate text, pointers text[], entity_timestamp timestamp without time zone) RETURNS boolean LANGUAGE plpgsql STABLE AS $$ BEGIN RAISE EXCEPTION 'history probe executed'; END $$")
        .execute(&fixture.pool).await.unwrap();
    let app = router(fixture.state());
    let meta = scene_metadata("main", &pointer, &id);
    for (signer, expected) in [
        (&key[..], StatusCode::INTERNAL_SERVER_ERROR),
        (OUTSIDER_KEY, StatusCode::UNAUTHORIZED),
    ] {
        let (status, body) = send(
            &app,
            signed_request("get", "/values/guard", signer, &meta, None, None).await,
        )
        .await;
        assert_eq!(status, expected, "{body}");
    }
    for (signer, expected) in [
        (&key[..], StatusCode::INTERNAL_SERVER_ERROR),
        (OUTSIDER_KEY, StatusCode::OK),
    ] {
        let (status, body) = send(
            &app,
            signed_request("get", "/collaborator", signer, "{}", None, None).await,
        )
        .await;
        assert_eq!(status, expected, "{body}");
        if status == StatusCode::OK {
            assert_eq!(body["pagination"]["total"], json!(0));
        }
    }
    fixture.close().await;
}

#[tokio::test]
async fn collaborator_pagination_excludes_only_scenes_without_a_parseable_parcel() {
    let Some(pool) = scratch().await else { return };
    let signer = address();
    let world = world_name();
    seed_world(&pool, &world, Some(&address())).await;
    let ids: Vec<_> = (0..3)
        .map(|i| format!("bafyMalformed{i}{}", Uuid::new_v4().simple()))
        .collect();
    for (i, id) in ids.iter().enumerate() {
        seed_logs_scene(
            &pool,
            &world,
            id,
            &[&format!("{i},0")],
            json!([signer.as_str()]),
            false,
        )
        .await;
        sqlx::query("UPDATE worlds.world_scenes SET entity = jsonb_set(entity, '{timestamp}', to_jsonb($2::bigint)) WHERE entity_id = $1")
            .bind(id).bind(i as i64).execute(&pool).await.unwrap();
    }
    // Newest invalid scene must not occupy the first slot or contribute to total.
    sqlx::query("UPDATE worlds.world_scenes SET parcels = $2 WHERE entity_id = $1")
        .bind(&ids[2])
        .bind(vec!["bad", "2147483648,0", "0,0,0", "+1,0"])
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE worlds.world_scenes SET parcels = $2 WHERE entity_id = $1")
        .bind(&ids[1])
        .bind(vec![None, Some("bad"), Some("\u{2003}-002 , 01\u{00a0}")])
        .execute(&pool)
        .await
        .unwrap();
    for (offset, expected) in [(0, &ids[1]), (1, &ids[0])] {
        let (page, total) = super::collaborator::list(&pool, None, &signer, 1, offset)
            .await
            .unwrap();
        assert_eq!(total, 2);
        assert_eq!(page.len(), 1);
        assert_eq!(&page[0].scene_id, expected);
    }
    let (page, total) = super::collaborator::list(&pool, None, &signer, 1, 2)
        .await
        .unwrap();
    assert_eq!(total, 2);
    assert!(page.is_empty());

    // Keep the SQL gate aligned with Parcel::parse, including range, Unicode
    // trim, leading zeros, signs, extra commas, and non-ASCII digits.
    sqlx::query("UPDATE worlds.world_scenes SET parcels = '{}' WHERE entity_id = ANY($1)")
        .bind(&ids[..2])
        .execute(&pool)
        .await
        .unwrap();
    for raw in [
        "0,0",
        "-2147483648,2147483647",
        "2147483648,0",
        "-2147483649,0",
        "\u{0085}001\u{3000},\t-002\n",
        "0000000000000000000000000000001,0",
        "+1,0",
        "1,+0",
        "1,2,3",
        "1,",
        ",1",
        "١,0",
        "1.0,0",
        "1 2,0",
        "\u{200b}1,0",
        "bad",
    ] {
        sqlx::query("UPDATE worlds.world_scenes SET parcels = $2 WHERE entity_id = $1")
            .bind(&ids[2])
            .bind(vec![raw])
            .execute(&pool)
            .await
            .unwrap();
        let (page, total) = super::collaborator::list(&pool, None, &signer, 1, 0)
            .await
            .unwrap();
        let expected = usize::from(Parcel::parse(raw).is_ok());
        assert_eq!(page.len(), expected, "{raw:?}");
        assert_eq!(total, expected as i64, "{raw:?}");
    }
}

async fn assert_genesis_collaboration(
    app: &Router,
    key: &str,
    pointer: &str,
    scene: &str,
    allowed: bool,
) {
    let meta = scene_metadata("main", pointer, scene);
    let (status, body) = send(
        app,
        signed_request(
            "put",
            "/values/epoch",
            key,
            &meta,
            None,
            Some(r#"{"value":1}"#),
        )
        .await,
    )
    .await;
    assert_eq!(
        status,
        if allowed {
            StatusCode::OK
        } else {
            StatusCode::UNAUTHORIZED
        },
        "{body}"
    );
    let (status, body) = send(
        app,
        signed_request("get", "/collaborator?limit=1", key, "{}", None, None).await,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["pagination"]["total"],
        json!(if allowed { 1 } else { 0 }),
        "{body}"
    );
    assert_eq!(body["data"].as_array().unwrap().len(), usize::from(allowed));
}

#[tokio::test]
async fn genesis_collaboration_requires_redeployment_after_any_parcel_ownership_change() {
    let Some(fixture) = CollaboratorLandFixture::new().await else {
        return;
    };
    let pool = &fixture.pool;
    let key = random_key();
    let owner_key = random_key();
    let owner = address_of(&owner_key);
    let collab = address_of(&key);
    let pointers = vec![genesis_parcel(), genesis_parcel()];
    fixture.parcel(&pointers[0], &owner, 1).await;
    fixture.parcel(&pointers[1], &owner, 2).await;
    let old = format!("bafyCollabEpoch{}", Uuid::new_v4().simple());
    seed_genesis(pool, &old, &pointers, json!([collab]), true).await;
    sqlx::query("UPDATE content.deployments SET entity_timestamp = timezone('UTC', now()) - interval '30 minutes' WHERE entity_id = $1")
        .bind(&old).execute(pool).await.unwrap();
    let app = router(fixture.state());
    assert_genesis_collaboration(&app, &key, &pointers[0], &old, true).await;

    // A transfer of the OTHER parcel invalidates the whole scene grant.
    fixture
        .transfer(&pointers[1], address().as_str(), 3, 1200)
        .await;
    assert_genesis_collaboration(&app, &key, &pointers[0], &old, false).await;
    fixture.transfer(&pointers[1], &owner, 4, 1140).await;
    assert_genesis_collaboration(&app, &key, &pointers[0], &old, false).await;
    // Re-ingesting this hash changes receipt time, never its immutable timestamp.
    sqlx::query("UPDATE content.deployments SET local_timestamp = timezone('UTC', now()) WHERE entity_id = $1")
        .bind(&old).execute(pool).await.unwrap();
    assert_genesis_collaboration(&app, &key, &pointers[0], &old, false).await;

    // A real new entity too close to transfer remains conservatively denied.
    let near = format!("bafyCollabNear{}", Uuid::new_v4().simple());
    seed_genesis(pool, &near, &pointers, json!([collab]), true).await;
    sqlx::query("UPDATE content.deployments SET entity_timestamp = timezone('UTC', now()) - interval '18 minutes' WHERE entity_id = $1")
        .bind(&near).execute(pool).await.unwrap();
    assert_genesis_collaboration(&app, &key, &pointers[0], &near, false).await;

    // This fixture has no local_entities row, just like a synced deployment.
    let new = format!("bafyCollabNew{}", Uuid::new_v4().simple());
    seed_genesis(pool, &new, &pointers, json!([collab]), true).await;
    assert_genesis_collaboration(&app, &key, &pointers[0], &new, true).await;
    // Future entity time and missing history never establish collaborator rights.
    sqlx::query("UPDATE content.deployments SET entity_timestamp = timezone('UTC', now()) + interval '1 minute' WHERE entity_id = $1")
        .bind(&new).execute(pool).await.unwrap();
    assert_genesis_collaboration(&app, &key, &pointers[0], &new, false).await;
    sqlx::query("UPDATE content.deployments SET entity_timestamp = timezone('UTC', now()) WHERE entity_id = $1")
        .bind(&new).execute(pool).await.unwrap();
    sqlx::query("UPDATE land_authz.authz_event SET block_time = 0 WHERE block_number = 4")
        .execute(pool)
        .await
        .unwrap();
    assert_genesis_collaboration(&app, &key, &pointers[0], &new, false).await;
    // Owner remains independent, even when historical timestamps cannot prove grants.
    let (status, body) = send(
        &app,
        signed_request(
            "put",
            "/values/owner",
            &owner_key,
            &scene_metadata("main", &pointers[0], &new),
            None,
            Some(r#"{"value":1}"#),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    sqlx::query("UPDATE land_authz.authz_event SET block_time = extract(epoch FROM now())::bigint - 1140 WHERE block_number = 4")
        .execute(pool).await.unwrap();
    sqlx::query("UPDATE land_authz.index_cursor SET folded_at = timezone('UTC', now()) - interval '31 minutes'")
        .execute(pool).await.unwrap();
    assert_genesis_collaboration(&app, &key, &pointers[0], &new, false).await;
    // A stale history index does not prevent the independently configured
    // LAND document source from establishing the current owner's authority.
    let source = fake_land_operators(
        &pointers[0],
        json!({
            "owner": owner, "operator": null, "updateOperator": null,
            "updateManagers": [], "approvedForAll": []
        }),
    )
    .await;
    let fallback = router(state_with_land_operators(pool, &source));
    let (status, body) = send(
        &fallback,
        signed_request(
            "put",
            "/values/fallback",
            &owner_key,
            &scene_metadata("main", &pointers[0], &new),
            None,
            Some(r#"{"value":1}"#),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_genesis_collaboration(&router(state(pool, None)), &key, &pointers[0], &new, false).await;
    let direct = router(state(pool, Some(Address::parse(&collab).unwrap())));
    let (status, body) = send(
        &direct,
        signed_request(
            "put",
            "/values/direct",
            &key,
            &scene_metadata("main", &pointers[0], &new),
            None,
            Some(r#"{"value":1}"#),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    fixture.close().await;
}

#[tokio::test]
async fn genesis_collaboration_tracks_estate_transfers_and_membership() {
    let Some(fixture) = CollaboratorLandFixture::new().await else {
        return;
    };
    let pool = &fixture.pool;
    let key = random_key();
    let owner = address();
    let pointer = genesis_parcel();
    let parcel = Parcel::parse(&pointer).unwrap();
    let settings = crate::land_authz::Settings::for_tests();
    fixture.parcel(&pointer, &settings.estate_registry, 1).await;
    sqlx::query("INSERT INTO land_authz.token_right (token_address, token_id, owner, updated_block, updated_log) VALUES ($1, 500, $2, 2, 0)")
        .bind(&settings.estate_registry).bind(&owner).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO land_authz.estate_land (land_token_id, estate_token_id, updated_block, updated_log) VALUES ($1::int::numeric * 340282366920938463463374607431768211456 + $2::int::numeric, 500, 3, 0)")
        .bind(parcel.x).bind(parcel.y).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO land_authz.authz_event (block_number, log_index, block_time, token_address, kind, token_id, account) VALUES (2, 0, extract(epoch FROM now())::bigint - 3600, $1, 'transfer', 500, $2)")
        .bind(&settings.estate_registry).bind(&owner).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO land_authz.authz_event (block_number, log_index, block_time, token_address, kind, token_id, estate_id) VALUES (3, 0, extract(epoch FROM now())::bigint - 3500, $1, 'add_land', $2::int::numeric * 340282366920938463463374607431768211456 + $3::int::numeric, 500)")
        .bind(&settings.estate_registry).bind(parcel.x).bind(parcel.y).execute(pool).await.unwrap();
    let old = format!("bafyEstateOld{}", Uuid::new_v4().simple());
    seed_genesis(
        pool,
        &old,
        std::slice::from_ref(&pointer),
        json!([address_of(&key)]),
        true,
    )
    .await;
    sqlx::query("UPDATE content.deployments SET entity_timestamp = timezone('UTC', now()) - interval '30 minutes' WHERE entity_id = $1")
        .bind(&old).execute(pool).await.unwrap();
    let app = router(fixture.state());
    assert_genesis_collaboration(&app, &key, &pointer, &old, true).await;
    for (block, seconds_ago, new_owner) in [(4i64, 1200i64, address()), (5, 1140, owner.clone())] {
        sqlx::query("INSERT INTO land_authz.authz_event (block_number, log_index, block_time, token_address, kind, token_id, account) VALUES ($1, 0, extract(epoch FROM now())::bigint - $2, $3, 'transfer', 500, $4)")
            .bind(block).bind(seconds_ago).bind(&settings.estate_registry).bind(&new_owner).execute(pool).await.unwrap();
        sqlx::query("UPDATE land_authz.token_right SET owner = $2 WHERE token_address = $1 AND token_id = 500")
            .bind(&settings.estate_registry).bind(&new_owner).execute(pool).await.unwrap();
        assert_genesis_collaboration(&app, &key, &pointer, &old, false).await;
    }
    let new = format!("bafyEstateNew{}", Uuid::new_v4().simple());
    seed_genesis(
        pool,
        &new,
        std::slice::from_ref(&pointer),
        json!([address_of(&key)]),
        true,
    )
    .await;
    assert_genesis_collaboration(&app, &key, &pointer, &new, true).await;
    sqlx::query("DELETE FROM land_authz.estate_land")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO land_authz.authz_event (block_number, log_index, block_time, token_address, kind, token_id, estate_id) VALUES (6, 0, extract(epoch FROM now())::bigint - 600, $1, 'remove_land', $2::int::numeric * 340282366920938463463374607431768211456 + $3::int::numeric, 500)")
        .bind(&settings.estate_registry).bind(parcel.x).bind(parcel.y).execute(pool).await.unwrap();
    fixture.transfer(&pointer, owner.as_str(), 7, 600).await;
    assert_genesis_collaboration(&app, &key, &pointer, &new, false).await;
    // Returning to the same estate must not restore its previous grant.
    fixture
        .transfer(&pointer, &settings.estate_registry, 9, 500)
        .await;
    sqlx::query("INSERT INTO land_authz.estate_land (land_token_id, estate_token_id, updated_block, updated_log) VALUES ($1::int::numeric * 340282366920938463463374607431768211456 + $2::int::numeric, 500, 8, 0)")
        .bind(parcel.x).bind(parcel.y).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO land_authz.authz_event (block_number, log_index, block_time, token_address, kind, token_id, estate_id) VALUES (8, 0, extract(epoch FROM now())::bigint - 500, $1, 'add_land', $2::int::numeric * 340282366920938463463374607431768211456 + $3::int::numeric, 500)")
        .bind(&settings.estate_registry).bind(parcel.x).bind(parcel.y).execute(pool).await.unwrap();
    assert_genesis_collaboration(&app, &key, &pointer, &new, false).await;
    fixture.close().await;
}

#[tokio::test]
async fn malformed_logs_permissions_grant_nothing() {
    let Some(pool) = scratch().await else { return };
    for value in [
        json!(null),
        json!(42),
        json!("0xabc"),
        json!({"address":"0xabc"}),
        json!([42, null, {"address":"0xabc"}]),
    ] {
        let grants: Vec<String> = sqlx::query_scalar("SELECT worlds.logs_permissions($1)")
            .bind(value.to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(grants.is_empty(), "{value}: {grants:?}");
    }
}

/// Resolve export fixtures through the same deployment as the request. The
/// optional conversion also supports the per-scene identity migration (#90).
async fn export_namespace(pool: &PgPool, world: &WorldName) -> SceneNamespace {
    let deployment = crate::rooms::world_scene_at(pool, world, Parcel::parse(PARCEL).unwrap())
        .await
        .unwrap()
        .unwrap();
    Option::<SceneNamespace>::from(deployment.namespace).expect("fixture has a storage identity")
}

fn export_state(
    pool: &PgPool,
    authoritative: Option<Address>,
    authorized: Vec<Address>,
    export: ExportSettings,
) -> SharedState {
    let mut settings = Settings::for_tests(authoritative);
    settings.authorized_addresses = authorized;
    settings.export = export;
    state_with(pool, settings)
}

fn no_cooldown() -> ExportSettings {
    ExportSettings {
        scene_cooldown: Duration::ZERO,
        ..Default::default()
    }
}

async fn export_request(key: &str, meta: &str, query: &str, body: Option<&str>) -> Request<Body> {
    signed_request("post", &format!("/exports{query}"), key, meta, None, body).await
}

async fn open_export(app: &Router, req: Request<Body>) -> axum::response::Response {
    app.clone().oneshot(req).await.unwrap()
}

/// The gunzipped download, or the error that cut the stream.
async fn download(resp: axum::response::Response) -> Result<String, String> {
    use std::io::Read;
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 26)
        .await
        .map_err(|e| e.to_string())?;
    let mut text = String::new();
    flate2::read::GzDecoder::new(&bytes[..])
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    Ok(text)
}

#[tokio::test]
async fn exports_admit_owners_deployers_and_collaborators_only() {
    let Some(pool) = scratch().await else { return };
    let (world, _) = seed_owned_world(&pool).await;
    let (owner_key, deployer_key, collab_key, authorized_key, worker_key, stranger_key) = (
        random_key(),
        random_key(),
        random_key(),
        random_key(),
        random_key(),
        random_key(),
    );
    let addr = |key: &str| Address::parse(&address_of(key)).unwrap();
    seed_world(&pool, &world, Some(&addr(&owner_key))).await;
    grant(
        &pool,
        &world,
        Permission::Deployment,
        &addr(&deployer_key),
        &["0,0", "0,1"],
    )
    .await;
    delete_scene(&pool, &world, SCENE_ID).await;
    seed_logs_scene(
        &pool,
        &world,
        SCENE_ID,
        &["0,0", "0,1"],
        json!([address_of(&collab_key)]),
        false,
    )
    .await;
    let authoritative = addr(AUTHORITATIVE_KEY);
    let app = router(export_state(
        &pool,
        Some(authoritative.clone()),
        vec![addr(&authorized_key)],
        no_cooldown(),
    ));
    let meta = scene_metadata(world.display(), PARCEL, SCENE_ID);

    for key in [&owner_key, &deployer_key, &collab_key] {
        let resp = open_export(&app, export_request(key, &meta, "?format=json", None).await).await;
        assert_eq!(resp.status(), StatusCode::OK);
        download(resp).await.unwrap();
    }
    for (key, message) in [
        (AUTHORITATIVE_KEY, super::AUTHORITATIVE_EXPORT_MSG),
        (authorized_key.as_str(), NOT_AUTHORIZED_MSG),
        (stranger_key.as_str(), NOT_AUTHORIZED_MSG),
    ] {
        let (status, body) =
            send(&app, export_request(key, &meta, "?format=json", None).await).await;
        assert_eq!(
            (status, body["message"].as_str()),
            (StatusCode::UNAUTHORIZED, Some(message)),
            "{key}"
        );
    }
    let claim = claim_payload(
        &address_of(&worker_key),
        world.key(),
        SCENE_ID,
        PARCEL,
        chrono::Utc::now() + chrono::Duration::hours(1),
    );
    let scope_hdr = scope_header(&wallet(AUTHORITATIVE_KEY), &claim).await;
    let (status, body) = send(
        &app,
        signed_request(
            "post",
            "/exports?format=json",
            &worker_key,
            &meta,
            Some(&scope_hdr),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::UNAUTHORIZED, Some(NOT_AUTHORIZED_MSG)),
        "a delegation never exports"
    );

    // The authoritative server stays refused as the world's owner.
    seed_world(&pool, &world, Some(&authoritative)).await;
    let (status, body) = send(
        &app,
        export_request(AUTHORITATIVE_KEY, &meta, "?format=json", None).await,
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (
            StatusCode::UNAUTHORIZED,
            Some(super::AUTHORITATIVE_EXPORT_MSG)
        )
    );

    let resp = open_export(
        &app,
        signed_request("get", "/exports", &owner_key, &meta, None, None).await,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert!(resp.headers()["allow"].to_str().unwrap().contains("POST"));
}

#[tokio::test]
async fn exports_refuse_bad_formats_selections_and_bodies() {
    let Some(pool) = scratch().await else { return };
    let (world, _) = seed_owned_world(&pool).await;
    let owner_key = random_key();
    seed_world(
        &pool,
        &world,
        Some(&Address::parse(&address_of(&owner_key)).unwrap()),
    )
    .await;
    let app = router(export_state(&pool, None, vec![], no_cooldown()));
    let meta = scene_metadata(world.display(), PARCEL, SCENE_ID);
    let many = json!({ "world": { "keys": (0..101).map(|i| i.to_string()).collect::<Vec<_>>() } })
        .to_string();
    let formats = "format must be one of: json, csv";
    for (query, body, message) in [
        ("", None, formats),
        ("?format=xml", None, formats),
        ("?format=json&format=csv", None, formats),
        (
            "?format=json",
            Some("not json"),
            "Request body must be valid JSON",
        ),
        (
            "?format=json",
            Some("[]"),
            "Request body must be a JSON object",
        ),
        (
            "?format=json",
            Some(r#"{"extra":1}"#),
            "Request body may only contain world and players",
        ),
        (
            "?format=json",
            Some(r#"{"world":null}"#),
            "world must be an object",
        ),
        (
            "?format=json",
            Some(r#"{"world":{"keys":[1]}}"#),
            "world.keys must be an array of strings",
        ),
        (
            "?format=json",
            Some(r#"{"players":{"names":[]}}"#),
            "players may only contain addresses and keys",
        ),
        (
            "?format=json",
            Some(many.as_str()),
            "world.keys must have at most 100 entries",
        ),
        (
            "?format=json",
            Some(r#"{"world":{"keys":[""]}}"#),
            "Key must be between 1 and 255 characters",
        ),
        (
            "?format=json",
            Some(r#"{"players":{"keys":["a\u0000"]}}"#),
            "Key must not contain NUL characters",
        ),
        (
            "?format=json",
            Some(r#"{"players":{"addresses":["nope"]}}"#),
            "Invalid player address",
        ),
        (
            "?format=json",
            Some(r#"{"world":{"keys":[]},"players":{"addresses":[]}}"#),
            "The selection is empty: nothing to export",
        ),
    ] {
        let (status, resp) = send(&app, export_request(&owner_key, &meta, query, body).await).await;
        assert_eq!(
            (status, resp["message"].as_str()),
            (StatusCode::BAD_REQUEST, Some(message)),
            "{query} {body:?}"
        );
    }

    let huge = format!(r#"{{"world":{{"keys":["{}"]}}}}"#, "k".repeat(300_000));
    let (status, _) = send(
        &app,
        export_request(&owner_key, &meta, "?format=json", Some(&huge)).await,
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    let mut req = export_request(&owner_key, &meta, "?format=json", None).await;
    req.headers_mut()
        .insert("transfer-encoding", HeaderValue::from_static("chunked"));
    let (status, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::LENGTH_REQUIRED);
}

#[tokio::test]
async fn exports_stream_world_then_player_rows_gzipped_and_never_env() {
    let Some(pool) = scratch().await else { return };
    let (world, _) = seed_owned_world(&pool).await;
    let owner_key = random_key();
    seed_world(
        &pool,
        &world,
        Some(&Address::parse(&address_of(&owner_key)).unwrap()),
    )
    .await;
    let state = export_state(&pool, None, vec![], no_cooldown());
    let app = router(state.clone());
    let meta = scene_metadata(world.display(), PARCEL, SCENE_ID);
    let scope = export_namespace(&pool, &world).await;
    for (key, value) in [
        ("a", json!({ "n": 1, "x": [1.50, "\u{e9}"] })),
        ("b", json!("text, with \"quotes\"")),
        ("=SUM(A1)", json!(1)),
        ("k,1", json!([1, 2])),
    ] {
        state
            .store
            .world_upsert_with_quota(&scope, key, &ser(&value), LIMITS)
            .await
            .unwrap();
    }
    let mut players = [address(), address()];
    players.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    for p in &players {
        for key in ["k1", "k2"] {
            state
                .store
                .player_upsert_with_quota(&scope, p, key, &ser(&json!({ "p": key })), LIMITS)
                .await
                .unwrap();
        }
    }
    let enc = state.encryptor().unwrap().encrypt(SECRET).unwrap();
    state
        .store
        .env_upsert_with_quota(
            &scope,
            ENV_KEY,
            &enc,
            value_size_bytes(SECRET),
            state.settings.env_limits,
        )
        .await
        .unwrap();

    let resp = open_export(
        &app,
        export_request(&owner_key, &meta, "?format=json", None).await,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let header = |name: &str| resp.headers()[name].to_str().unwrap().to_string();
    assert_eq!(
        [
            header("content-type"),
            header("content-encoding"),
            header("content-disposition"),
            header("cache-control")
        ],
        [
            "application/json; charset=utf-8",
            "gzip",
            "attachment; filename=\"storage-export.json\"",
            "no-store"
        ]
    );
    let text = download(resp).await.unwrap();
    // Byte for byte: the stored text in the database's key order.
    let world_rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT key, value::text FROM worlds.world_storage WHERE realm = $1 AND scene_namespace = $2 ORDER BY key",
    )
    .bind(scope.realm())
    .bind(scope.scene_namespace())
    .fetch_all(&pool)
    .await
    .unwrap();
    let player_rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT player_address, key, value::text FROM worlds.player_storage WHERE realm = $1 AND scene_namespace = $2 ORDER BY player_address, key",
    )
    .bind(scope.realm())
    .bind(scope.scene_namespace())
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!((world_rows.len(), player_rows.len()), (4, 4));
    let expected = format!(
        r#"{{"world":[{}],"players":[{}]}}"#,
        world_rows
            .iter()
            .map(|(k, v)| format!(r#"{{"key":{},"value":{v}}}"#, ser(&json!(k))))
            .collect::<Vec<_>>()
            .join(","),
        player_rows
            .iter()
            .map(|(a, k, v)| format!(r#"{{"address":"{a}","key":"{k}","value":{v}}}"#))
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(text, expected);
    assert!(!text.contains(ENV_KEY));

    let resp = open_export(
        &app,
        export_request(&owner_key, &meta, "?format=csv", None).await,
    )
    .await;
    assert_eq!(
        (
            resp.headers()["content-type"].to_str().unwrap(),
            resp.headers()["content-disposition"].to_str().unwrap()
        ),
        (
            "text/csv; charset=utf-8",
            "attachment; filename=\"storage-export.csv\""
        )
    );
    let csv = download(resp).await.unwrap();
    assert!(
        csv.starts_with("scope,player_address,key,value\r\n"),
        "{csv}"
    );
    for row in [
        "world,,'=SUM(A1),1\r\n".to_string(),
        "world,,b,\"\"\"text, with \\\"\"quotes\\\"\"\"\"\"\r\n".to_string(),
        "world,,\"k,1\",\"[1, 2]\"\r\n".to_string(),
        format!(
            "player,{},k1,\"{{\"\"p\"\": \"\"k1\"\"}}\"\r\n",
            players[0].as_str()
        ),
    ] {
        assert!(csv.contains(&row), "{row:?} in {csv}");
    }
    assert!(!csv.contains(ENV_KEY));
    assert_eq!(csv.matches("\r\n").count(), 1 + 4 + 4);

    let selected = |body: Value| {
        let app = app.clone();
        let (key, meta) = (owner_key.clone(), meta.clone());
        async move {
            let resp = open_export(
                &app,
                export_request(&key, &meta, "?format=json", Some(&body.to_string())).await,
            )
            .await;
            serde_json::from_str::<Value>(&download(resp).await.unwrap()).unwrap()
        }
    };
    assert_eq!(
        selected(json!({ "world": { "keys": ["b"] }, "players": { "addresses": [] } })).await,
        json!({ "world": [{ "key": "b", "value": "text, with \"quotes\"" }], "players": [] })
    );
    let one = selected(json!({ "world": { "keys": [] }, "players": { "addresses": [players[1].as_str().to_uppercase().replace("0X", "0x")], "keys": ["k2"] } })).await;
    assert_eq!(
        one,
        json!({ "world": [], "players": [{ "address": players[1].as_str(), "key": "k2", "value": { "p": "k2" } }] })
    );
}

#[tokio::test]
async fn exports_cap_the_scene_and_the_process_and_cool_down() {
    let Some(pool) = scratch().await else { return };
    let owner_key = random_key();
    let owner = Address::parse(&address_of(&owner_key)).unwrap();
    let (a, _) = seed_owned_world(&pool).await;
    let (b, _) = seed_owned_world(&pool).await;
    for w in [&a, &b] {
        seed_world(&pool, w, Some(&owner)).await;
    }
    let (meta_a, meta_b) = (
        scene_metadata(a.display(), PARCEL, SCENE_ID),
        scene_metadata(b.display(), PARCEL, SCENE_ID),
    );
    let state = export_state(
        &pool,
        None,
        vec![],
        ExportSettings {
            max_concurrent_per_pod: 1,
            ..no_cooldown()
        },
    );
    let namespace = export_namespace(&pool, &a).await;
    let first = state
        .exports
        .open(&format!(
            "{}:{}",
            namespace.realm(),
            namespace.scene_namespace()
        ))
        .unwrap();
    let app = router(state);
    for (meta, message) in [
        (
            &meta_a,
            "An export of this scene is already in progress; try again shortly",
        ),
        (
            &meta_b,
            "Too many exports are in progress; try again shortly",
        ),
    ] {
        let resp = open_export(
            &app,
            export_request(&owner_key, meta, "?format=json", None).await,
        )
        .await;
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(resp.headers().get("retry-after").is_none());
        let body: Value = serde_json::from_slice(
            &axum::body::to_bytes(resp.into_body(), 1 << 16)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["message"], message);
    }
    // Completion/cancellation releases the producer's slot.
    drop(first);
    let again = open_export(
        &app,
        export_request(&owner_key, &meta_a, "?format=json", None).await,
    )
    .await;
    assert_eq!(again.status(), StatusCode::OK);
    download(again).await.unwrap();

    let cooling = router(export_state(&pool, None, vec![], ExportSettings::default()));
    let resp = open_export(
        &cooling,
        export_request(&owner_key, &meta_b, "?format=json", None).await,
    )
    .await;
    download(resp).await.unwrap();
    let resp = open_export(
        &cooling,
        export_request(&owner_key, &meta_b, "?format=json", None).await,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    let wait: u64 = resp.headers()["retry-after"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((1..=10).contains(&wait), "{wait}");
    let body: Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), 1 << 16)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        body["message"],
        format!("This scene was exported moments ago; try again in {wait} seconds")
    );
}

#[tokio::test]
async fn exports_abort_past_the_size_cap_and_the_timeout() {
    let Some(pool) = scratch().await else { return };
    let (world, _) = seed_owned_world(&pool).await;
    let owner_key = random_key();
    seed_world(
        &pool,
        &world,
        Some(&Address::parse(&address_of(&owner_key)).unwrap()),
    )
    .await;
    let meta = scene_metadata(world.display(), PARCEL, SCENE_ID);
    let scope = export_namespace(&pool, &world).await;
    Store::new(pool.clone(), cache_cfg(false))
        .world_upsert_with_quota(&scope, "big", &ser(&json!("x".repeat(100))), LIMITS)
        .await
        .unwrap();

    for export in [
        ExportSettings {
            max_size_bytes: 32,
            ..no_cooldown()
        },
        ExportSettings {
            timeout: Duration::ZERO,
            ..no_cooldown()
        },
    ] {
        let app = router(export_state(&pool, None, vec![], export));
        for _ in 0..2 {
            // The second attempt starts: the aborted one released its slot.
            let resp = open_export(
                &app,
                export_request(&owner_key, &meta, "?format=csv", None).await,
            )
            .await;
            if export.timeout.is_zero() {
                assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
                assert_eq!(resp.headers()["retry-after"], "1");
            } else {
                assert_eq!(resp.status(), StatusCode::OK);
                // Check the HTTP body error itself, not only the gzip decoder.
                assert!(
                    axum::body::to_bytes(resp.into_body(), 1 << 20)
                        .await
                        .is_err(),
                    "{export:?}"
                );
            }
        }
    }
}

#[tokio::test]
async fn exports_bound_database_resources_and_keep_one_snapshot() {
    use super::export::{ExportLimits, Format, Selection};
    use axum::response::IntoResponse;
    use futures::StreamExt;
    let Some(pool) = scratch().await else { return };
    let (world, _) = seed_owned_world(&pool).await;
    let scope = export_namespace(&pool, &world).await;
    let dedicated = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    let settings = ExportSettings {
        timeout: Duration::from_millis(200),
        ..no_cooldown()
    };
    let limits = ExportLimits::new(settings);
    let original_timeout: String = sqlx::query_scalar("SHOW statement_timeout")
        .fetch_one(&dedicated)
        .await
        .unwrap();
    let cooling = ExportLimits::new(ExportSettings::default());
    let held = dedicated.acquire().await.unwrap();
    let result = super::export::body(
        dedicated.clone(),
        scope.clone(),
        Selection::default(),
        Format::Json,
        cooling.open("scene").unwrap(),
        settings,
    )
    .await;
    let error = result
        .err()
        .expect("exhausted pool must refuse export setup");
    assert!(matches!(error, super::errors::ApiError::Unavailable(_)));
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()["retry-after"], "1");
    drop(cooling.open("scene").unwrap());
    drop(held);
    let retry = super::export::body(
        dedicated.clone(),
        scope.clone(),
        Selection::default(),
        Format::Json,
        cooling.open("scene").unwrap(),
        settings,
    )
    .await
    .unwrap();
    axum::body::to_bytes(retry, 1 << 20).await.unwrap();
    assert!(matches!(
        cooling.open("scene"),
        Err(super::export::Refusal::Cooldown { .. })
    ));

    // More incompressible data than the four-chunk buffer can hold: an unread
    // body keeps the producer blocked until cancellation or its deadline.
    sqlx::query("INSERT INTO worlds.world_storage (realm, scene_namespace, key, value, value_size)
        SELECT $1, $2, g::text, to_jsonb((SELECT string_agg(md5(g::text || ':' || i::text), '') FROM generate_series(1, 2048) i)), 65538
        FROM generate_series(1, 32) g")
        .bind(scope.realm()).bind(scope.scene_namespace()).execute(&pool).await.unwrap();
    for disconnect in [false, true] {
        let body = super::export::body(
            dedicated.clone(),
            scope.clone(),
            Selection::default(),
            Format::Json,
            limits.open("scene").unwrap(),
            settings,
        )
        .await
        .unwrap();
        let body = if disconnect {
            drop(body);
            None
        } else {
            Some(body)
        };
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Ok(slot) = limits.open("scene") {
                    drop(slot);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("producer must release its slot while the body stays unread");
        // Pool capacity must recover too, including when SQL was cancelled.
        drop(
            tokio::time::timeout(Duration::from_secs(1), dedicated.acquire())
                .await
                .unwrap()
                .unwrap(),
        );
        if let Some(body) = body {
            assert!(axum::body::to_bytes(body, 4 << 20).await.is_err());
        }
    }

    sqlx::query("SET idle_in_transaction_session_timeout = '100ms'")
        .execute(&dedicated)
        .await
        .unwrap();
    let backend: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&dedicated)
        .await
        .unwrap();
    let player = address();
    sqlx::query("INSERT INTO worlds.player_storage (realm, scene_namespace, player_address, key, value, value_size) VALUES ($1, $2, $3, 'snapshot', '1', 1)")
        .bind(scope.realm()).bind(scope.scene_namespace()).bind(&player).execute(&pool).await.unwrap();
    let settings = ExportSettings {
        timeout: Duration::from_secs(10),
        ..no_cooldown()
    };
    let limits = ExportLimits::new(settings);
    let (reached, world_rows_done) = tokio::sync::oneshot::channel();
    let (resume, resumed) = tokio::sync::oneshot::channel();
    let mut slot = limits.open("scene").unwrap();
    slot.test_hooks.after_world_rows = Some(super::export::TestPause {
        reached,
        resume: resumed,
    });
    let body = super::export::body(
        dedicated.clone(),
        scope.clone(),
        Selection::default(),
        Format::Json,
        slot,
        settings,
    )
    .await
    .unwrap();
    // Drain the channel while the producer consumes the whole world query.
    // The per-export pause then holds a genuinely idle transaction independent
    // of PostgreSQL or operating-system socket buffer sizes.
    let reader = tokio::spawn(axum::body::to_bytes(body, 4 << 20));
    tokio::time::timeout(Duration::from_secs(3), world_rows_done)
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let idle: bool = sqlx::query_scalar(
                "SELECT state = 'idle in transaction' FROM pg_stat_activity WHERE pid = $1",
            )
            .bind(backend)
            .fetch_one(&pool)
            .await
            .unwrap();
            if idle {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("completed world query must leave the snapshot idle");
    tokio::time::sleep(Duration::from_millis(300)).await;
    sqlx::query(
        "UPDATE worlds.player_storage SET value = '2' WHERE realm = $1 AND scene_namespace = $2",
    )
    .bind(scope.realm())
    .bind(scope.scene_namespace())
    .execute(&pool)
    .await
    .unwrap();
    resume.send(()).unwrap();
    let compressed = reader.await.unwrap().unwrap();
    let mut text = String::new();
    std::io::Read::read_to_string(
        &mut flate2::read::GzDecoder::new(compressed.as_ref()),
        &mut text,
    )
    .unwrap();
    let snapshot: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(snapshot["players"][0]["value"], json!(1));
    let restored_timeout: String = sqlx::query_scalar("SHOW statement_timeout")
        .fetch_one(&dedicated)
        .await
        .unwrap();
    assert_eq!(restored_timeout, original_timeout);
    let restored_idle: String = sqlx::query_scalar("SHOW idle_in_transaction_session_timeout")
        .fetch_one(&dedicated)
        .await
        .unwrap();
    assert_eq!(restored_idle, "100ms");
    let restored_backend: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&dedicated)
        .await
        .unwrap();
    assert_eq!(
        restored_backend, backend,
        "the same pooled connection survived the slow client"
    );

    // Kill only this test's dedicated export connection after bytes have
    // reached the client. Buffered rows must not turn a database failure into
    // successful EOF or a valid gzip trailer.
    let body = super::export::body(
        dedicated.clone(),
        scope,
        Selection::default(),
        Format::Json,
        limits.open("scene").unwrap(),
        settings,
    )
    .await
    .unwrap();
    let mut stream = body.into_data_stream();
    let mut compressed = stream.next().await.unwrap().unwrap().to_vec();
    let terminated: bool = sqlx::query_scalar("SELECT pg_terminate_backend($1)")
        .bind(backend)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(terminated);
    let mut failed = false;
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => compressed.extend_from_slice(&bytes),
            Err(_) => {
                failed = true;
                break;
            }
        }
    }
    assert!(failed, "database termination must be an HTTP body error");
    let mut decoded = Vec::new();
    assert!(
        std::io::Read::read_to_end(
            &mut flate2::read::GzDecoder::new(compressed.as_slice()),
            &mut decoded
        )
        .is_err(),
        "database failure must precede the gzip trailer"
    );
    drop(stream);
    drop(limits.open("scene").unwrap());
    tokio::time::timeout(Duration::from_secs(2), dedicated.acquire())
        .await
        .unwrap()
        .unwrap();
    dedicated.close().await;
}

#[tokio::test]
async fn genesis_exports_require_a_current_collaborator_grant_after_transfer() {
    let Some(fixture) = CollaboratorLandFixture::new().await else {
        return;
    };
    let key = random_key();
    let pointer = genesis_parcel();
    fixture.parcel(&pointer, address().as_str(), 1).await;
    let old = format!("bafyExportOld{}", Uuid::new_v4().simple());
    seed_genesis(
        &fixture.pool,
        &old,
        std::slice::from_ref(&pointer),
        json!([address_of(&key)]),
        true,
    )
    .await;
    sqlx::query("UPDATE content.deployments SET entity_timestamp = timezone('UTC', now()) - interval '30 minutes' WHERE entity_id = $1")
        .bind(&old).execute(&fixture.pool).await.unwrap();
    let mut state = fixture.state();
    Arc::get_mut(&mut state).unwrap().exports = super::export::ExportLimits::new(no_cooldown());
    let app = router(state);
    let meta = scene_metadata("main", &pointer, &old);
    let response = open_export(
        &app,
        export_request(&key, &meta, "?format=json", None).await,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    download(response).await.unwrap();
    fixture
        .transfer(&pointer, address().as_str(), 2, 1200)
        .await;
    let response = open_export(
        &app,
        export_request(&key, &meta, "?format=json", None).await,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let new = format!("bafyExportNew{}", Uuid::new_v4().simple());
    seed_genesis(
        &fixture.pool,
        &new,
        std::slice::from_ref(&pointer),
        json!([address_of(&key)]),
        true,
    )
    .await;
    let meta = scene_metadata("main", &pointer, &new);
    let response = open_export(
        &app,
        export_request(&key, &meta, "?format=json", None).await,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    download(response).await.unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn exports_report_rollback_failure_before_writing_a_valid_gzip_trailer() {
    use super::export::{ExportLimits, Format, Selection, TestPause};
    use futures::StreamExt;
    let Some(pool) = scratch().await else {
        return;
    };
    let (world, _) = seed_owned_world(&pool).await;
    let scope = export_namespace(&pool, &world).await;
    let dedicated = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    let backend: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&dedicated)
        .await
        .unwrap();
    let settings = ExportSettings {
        timeout: Duration::from_secs(10),
        ..no_cooldown()
    };
    let limits = ExportLimits::new(settings);
    let (reached, produced) = tokio::sync::oneshot::channel();
    let (resume, resumed) = tokio::sync::oneshot::channel();
    let mut slot = limits.open("scene").unwrap();
    slot.test_hooks.before_rollback = Some(TestPause {
        reached,
        resume: resumed,
    });
    let body = super::export::body(
        dedicated.clone(),
        scope,
        Selection::default(),
        Format::Json,
        slot,
        settings,
    )
    .await
    .unwrap();
    let reader = tokio::spawn(async move {
        let mut stream = body.into_data_stream();
        let mut compressed = Vec::new();
        let mut failed = false;
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => compressed.extend_from_slice(&bytes),
                Err(_) => {
                    failed = true;
                    break;
                }
            }
        }
        (compressed, failed)
    });
    tokio::time::timeout(Duration::from_secs(3), produced)
        .await
        .unwrap()
        .unwrap();
    // All rows and serialization have succeeded. Wait for termination of only
    // this export's backend, so the next database operation (rollback) fails.
    let terminated: bool = sqlx::query_scalar("SELECT pg_terminate_backend($1, 5000)")
        .bind(backend)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(terminated);
    resume.send(()).unwrap();
    let (compressed, failed) = reader.await.unwrap();
    assert!(failed, "rollback failure must be an HTTP body error");
    let mut decoded = Vec::new();
    assert!(
        std::io::Read::read_to_end(
            &mut flate2::read::GzDecoder::new(compressed.as_slice()),
            &mut decoded
        )
        .is_err(),
        "rollback failure must precede a valid gzip trailer"
    );
    drop(limits.open("scene").unwrap());
    drop(
        tokio::time::timeout(Duration::from_secs(2), dedicated.acquire())
            .await
            .unwrap()
            .unwrap(),
    );
    dedicated.close().await;
}
