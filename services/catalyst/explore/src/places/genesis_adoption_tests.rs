use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt as _;
use uuid::Uuid;

use super::{PlaceListFilters, derive, genesis_identity, interactions, mirror, parcel_place_id};
use crate::db::scratch::{OwnedDatabase, Schema};

const SOURCE: &str = "https://places.decentraland.org";
const THUMBNAIL: &str = "https://places.decentraland.org/images/genesis.png";
const CREATOR: &str = "0x0000000000000000000000000000000000000001";
const VIEWER: &str = "0x0000000000000000000000000000000000000002";

async fn publish(pool: &PgPool, base: &str, cid: &str) {
    let mut tx = pool.begin().await.unwrap();
    let meta = json!({
        "v": {
            "scene": {"base": base, "parcels": [base]},
            "display": {"title": "Local Genesis scene"},
        }
    });
    sqlx::query(
        "INSERT INTO content.deployments
             (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp,
              entity_pointers, local_timestamp, auth_chain)
         VALUES ($1, 'v3', 'scene', $2, $3::json, timezone('UTC', now()), ARRAY[$4::text],
                 timezone('UTC', now()), '[]')",
    )
    .bind(CREATOR)
    .bind(cid)
    .bind(meta.to_string())
    .bind(base)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query("INSERT INTO content.local_entities (entity_id, signer) VALUES ($1, $2)")
        .bind(cid)
        .bind(CREATOR)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO content.active_pointers (pointer, entity_id, entity_type)
         VALUES ($1, $2, 'scene')
         ON CONFLICT (pointer) DO UPDATE SET entity_id = EXCLUDED.entity_id",
    )
    .bind(base)
    .bind(cid)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
}

async fn derive_held(pool: &PgPool) {
    derive::run_once(
        pool,
        derive::DeriveScope::Local,
        "https://node.example/content",
    )
    .await
    .unwrap();
}

fn upstream(public: &str, base: &str, cid: &str) -> Value {
    json!({
        "id": public,
        "title": "Upstream copy",
        "image": THUMBNAIL,
        "categories": ["art"],
        "base_position": base,
        "positions": [base],
        "deployment_id": cid,
        "disabled": false,
        "world": false,
    })
}

async fn places_pass(pool: &PgPool, source: &str, places: &[Value]) {
    places_pass_with_total(pool, source, places, Some(places.len() as i64)).await;
}

async fn places_pass_with_total(
    pool: &PgPool,
    source: &str,
    places: &[Value],
    reported_total: Option<i64>,
) {
    let start = crate::mirror_state::begin_pass(pool, mirror::PLACES_MIRROR, source)
        .await
        .unwrap();
    let rows: Vec<_> = places
        .iter()
        .map(|place| mirror::extract_place(place).unwrap())
        .collect();
    genesis_identity::record_page(
        pool,
        start.started_at.naive_utc(),
        start.offset,
        places,
        reported_total,
    )
    .await
    .unwrap();
    mirror::upsert_rows(pool, &rows).await.unwrap();
    genesis_identity::finish_pass(pool, start.started_at.naive_utc(), source)
        .await
        .unwrap();
    crate::mirror_state::finish_pass(
        pool,
        mirror::PLACES_MIRROR,
        &crate::mirror_state::PassEnd {
            complete: true,
            rows: places.len() as i64,
            ..Default::default()
        },
    )
    .await
    .unwrap();
}

async fn request(
    app: &axum::Router,
    method: &str,
    path: &str,
    input: Option<Value>,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(input.map_or_else(Body::empty, |json| Body::from(json.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn state(pool: &PgPool) -> Arc<crate::ExploreState> {
    Arc::new(crate::ExploreState::new(
        pool.clone(),
        crate::Settings::for_tests(),
        None,
    ))
}

#[tokio::test]
async fn genesis_adoption_converges_in_both_writer_orders() {
    for mirror_first in [false, true] {
        let Some(db) = OwnedDatabase::new("genesis_orders", Schema::Full, 4).await else {
            return;
        };
        let base = "1,2";
        let cid = "bafylocalgenesis";
        let fallback = parcel_place_id(base);
        let public = Uuid::new_v4().to_string();
        let places = [upstream(&public, base, cid)];
        let state = state(&db.pool);
        let app = crate::router(state.clone());
        if mirror_first {
            places_pass(&db.pool, SOURCE, &places).await;
            interactions::set_favorite(&db.pool, &public, VIEWER, true, 150.0)
                .await
                .unwrap();
        }
        publish(&db.pool, base, cid).await;
        if !mirror_first {
            derive_held(&db.pool).await;
            let (code, detail) =
                request(&app, "GET", &format!("/api/places/{fallback}"), None).await;
            assert_eq!(code, StatusCode::OK, "{detail}");
            assert_eq!(detail["data"]["id"], fallback);
            interactions::set_favorite(&db.pool, &fallback, VIEWER, true, 150.0)
                .await
                .unwrap();
            places_pass(&db.pool, SOURCE, &places).await;
        } else {
            derive_held(&db.pool).await;
        }

        for id in [&fallback, &public] {
            let collision = sqlx::query(
                "INSERT INTO explore.world_scene_place (world_name, identity, parcels, active)
                 VALUES ('collision.dcl.eth', $1::uuid, ARRAY['1,2'], false)",
            )
            .bind(id)
            .execute(&db.pool)
            .await
            .unwrap_err();
            assert_eq!(
                collision.as_database_error().unwrap().code().as_deref(),
                Some("23505")
            );
        }

        let mut event_ids = Vec::new();
        for id in [&fallback, &public] {
            let event = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO explore.event
                     (id, name, start_at, finish_at, next_start_at, next_finish_at, duration_ms, approved, raw)
                 VALUES ($1, 'Genesis scene event', now(), now() + interval '1 hour',
                         now(), now() + interval '1 hour', 3600000, true, jsonb_build_object('place_id', $2::text))",
            )
            .bind(&event)
            .bind(id)
            .execute(&db.pool)
            .await
            .unwrap();
            event_ids.push(event);
        }

        for round in 0..2 {
            for id in [&fallback, &public] {
                let (code, detail) = request(&app, "GET", &format!("/api/places/{id}"), None).await;
                assert_eq!(code, StatusCode::OK, "{detail}");
                assert_eq!(detail["data"]["id"], public);
                assert_eq!(detail["data"]["deployment_id"], cid);
                assert_eq!(detail["data"]["title"], "Local Genesis scene");
                assert_eq!(detail["data"]["favorites"], 1);
                assert_eq!(detail["data"]["image"], THUMBNAIL);
                let destination = format!("/v1/destinations/{id}");
                let (code, detail) = request(&app, "GET", &destination, None).await;
                assert_eq!(code, StatusCode::OK, "{detail}");
                assert_eq!(detail["data"]["id"], public);
                let (code, events) =
                    request(&app, "GET", &format!("{destination}/events"), None).await;
                assert_eq!(code, StatusCode::OK, "{events}");
                let events = events["data"].as_array().unwrap();
                assert_eq!(events.len(), 2);
                assert!(
                    event_ids
                        .iter()
                        .all(|id| events.iter().any(|event| event["id"] == *id))
                );
            }
            let ids = [public.clone(), fallback.clone()];
            let batch = state
                .places
                .find_by_ids_for(&ids, Some(VIEWER))
                .await
                .unwrap();
            assert_eq!(batch.len(), 1);
            assert_eq!(batch[0].id, public);
            assert!(batch[0].user_favorite);
            let (code, status) =
                request(&app, "POST", "/api/places/status", Some(json!(ids))).await;
            assert!(code.is_success(), "{status}");
            assert_eq!(status["total"], 1);
            assert_eq!(status["data"][0]["id"], public);
            assert_eq!(status["data"][0]["disabled"], false);
            assert_eq!(state.places.count_by_ids(&ids).await.unwrap(), 1);
            let filters = PlaceListFilters {
                positions: vec![base.to_string()],
                limit: 20,
                ..Default::default()
            };
            let (rows, total) = state.places.list_page(&filters).await.unwrap();
            assert_eq!((rows.len(), total), (1, 1));
            assert_eq!(rows[0].id, public);
            assert_eq!(state.places.count_list(&filters).await.unwrap(), 1);
            let (code, page) =
                request(&app, "GET", &format!("/api/places?positions={base}"), None).await;
            assert_eq!(code, StatusCode::OK, "{page}");
            assert_eq!(page["total"], 1);
            assert_eq!(page["data"][0]["id"], public);
            for route in [
                "/api/places?search=genesis",
                "/api/places?categories=art",
                "/api/destinations?search=genesis",
            ] {
                let (code, page) = request(&app, "GET", route, None).await;
                assert_eq!(code, StatusCode::OK, "{page}");
                assert_eq!(page["total"], 1, "{page}");
                assert_eq!(page["data"][0]["id"], public);
                assert_eq!(page["data"][0]["image"], THUMBNAIL);
                assert_eq!(page["data"][0]["categories"], json!(["art"]));
            }
            let (code, map) =
                request(&app, "GET", &format!("/api/map?positions={base}"), None).await;
            assert_eq!(code, StatusCode::OK, "{map}");
            assert_eq!(map["total"], 1, "{map}");
            assert_eq!(map["data"][base]["id"], public);

            if round == 0 {
                places_pass(&db.pool, SOURCE, &places).await;
                derive_held(&db.pool).await;
                interactions::set_favorite(&db.pool, &public, VIEWER, true, 150.0)
                    .await
                    .unwrap();
            }
        }
        db.drop().await;
    }
}

#[tokio::test]
async fn genesis_adoption_requires_a_unique_exact_live_production_candidate() {
    let Some(db) = OwnedDatabase::new("genesis_proof", Schema::Full, 4).await else {
        return;
    };
    let base = "3,4";
    let cid = "bafylivegenesis";
    let fallback = parcel_place_id(base);
    let public = Uuid::new_v4().to_string();
    let other = Uuid::new_v4().to_string();
    let state = state(&db.pool);
    publish(&db.pool, base, cid).await;
    derive_held(&db.pool).await;
    for (source, places) in [
        ("https://places.example", vec![upstream(&public, base, cid)]),
        (SOURCE, vec![upstream(&public, base, "bafywrongcid")]),
        (
            SOURCE,
            vec![upstream(&public, base, cid), upstream(&other, base, cid)],
        ),
    ] {
        places_pass(&db.pool, source, &places).await;
        derive_held(&db.pool).await;
        let row = state
            .places
            .find_by_id_for(&fallback, None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.id, fallback);
    }

    // A partial walk sees an exact candidate but still may omit a competing row.
    let candidate = [upstream(&public, base, cid)];
    for reported_total in [None, Some(2)] {
        places_pass_with_total(&db.pool, SOURCE, &candidate, reported_total).await;
        derive_held(&db.pool).await;
        let row = state
            .places
            .find_by_id_for(&fallback, None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.id, fallback);
    }

    // A complete source pass repairs ambiguity and can adopt the same deployment.
    places_pass(&db.pool, SOURCE, &candidate).await;
    derive_held(&db.pool).await;
    let row = state
        .places
        .find_by_id_for(&fallback, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.id, public);

    // A new CID at the same parcel cannot inherit the previous public proof.
    let replacement = "bafynewgenesis";
    publish(&db.pool, base, replacement).await;
    sqlx::query("UPDATE content.local_entities SET tombstoned_at = timezone('UTC', now()) WHERE entity_id = $1")
        .bind(cid)
        .execute(&db.pool)
        .await
        .unwrap();
    derive_held(&db.pool).await;
    let row = state
        .places
        .find_by_id_for(&fallback, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.id, fallback);
    assert_eq!(row.deployment_id.as_deref(), Some(replacement));
    let app = crate::router(state.clone());
    let (code, mirrored) = request(&app, "GET", &format!("/api/places/{public}"), None).await;
    assert_eq!(code, StatusCode::OK, "{mirrored}");
    assert_eq!(mirrored["data"]["id"], public);
    assert_eq!(mirrored["data"]["deployment_id"], cid);

    interactions::set_favorite(&db.pool, &public, VIEWER, true, 150.0)
        .await
        .unwrap();
    interactions::set_like(&db.pool, &public, VIEWER, Some(true), 150.0)
        .await
        .unwrap();
    let local = state
        .places
        .find_by_id_for(&fallback, Some(VIEWER))
        .await
        .unwrap()
        .unwrap();
    assert!(!local.user_favorite);
    assert!(!local.user_like);

    sqlx::query(
        "UPDATE content.local_entities SET tombstoned_at = CASE
                 WHEN entity_id = $1 THEN NULL ELSE timezone('UTC', now()) END
                 WHERE entity_id = ANY(ARRAY[$1::text, $2::text])",
    )
    .bind(cid)
    .bind(replacement)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query("UPDATE content.active_pointers SET entity_id = $1 WHERE pointer = $2")
        .bind(cid)
        .bind(base)
        .execute(&db.pool)
        .await
        .unwrap();
    derive_held(&db.pool).await;
    for id in [&fallback, &public] {
        let row = state
            .places
            .find_by_id_for(id, Some(VIEWER))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.id, public);
        assert_eq!(row.deployment_id.as_deref(), Some(cid));
        assert!(row.user_favorite);
        assert!(row.user_like);
        assert_eq!(row.favorites, 1);
        assert_eq!(row.likes, 1);
    }
    db.drop().await;
}
