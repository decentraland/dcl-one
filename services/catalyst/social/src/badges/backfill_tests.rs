//! `POST /badges/{id}/backfill` over the router as `serve` mounts it, on the
//! scratch database (`CATALYST_TEST_DATABASE_URL`; skipped without it).
//! Rows are scoped by fresh addresses; the test that archives a catalog
//! badge has its own database.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use chrono::{DateTime, NaiveDateTime};
use serde_json::{Value, json};
use sqlx::{AssertSqlSafe, PgPool};
use tower::ServiceExt;

use super::{Settings, SharedState, State, router};
use crate::identity::Address;
use crate::notifications::store::test_support::fresh_address;

const TOKEN: &str = "s3cret";

fn settings() -> Settings {
    Settings {
        admin_token: Some(TOKEN.into()),
        ..Settings::default()
    }
}

async fn state() -> Option<SharedState> {
    let pool = crate::db::scratch::pool(4).await?;
    Some(Arc::new(State::new(pool, settings())))
}

fn naive_ms(ms: i64) -> NaiveDateTime {
    DateTime::from_timestamp_millis(ms).unwrap().naive_utc()
}

async fn backfill(
    state: &SharedState,
    badge: &str,
    authorization: Option<&str>,
    body: &str,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method("POST")
        .uri(format!("/badges/badges/{badge}/backfill"))
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(a) = authorization {
        req = req.header(header::AUTHORIZATION, a);
    }
    let app = Router::new().nest("/badges", router(state.clone()));
    let resp = app
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 24)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn post(state: &SharedState, badge: &str, registries: Value) -> (StatusCode, Value) {
    let bearer = format!("Bearer {TOKEN}");
    backfill(
        state,
        badge,
        Some(&bearer),
        &json!({ "registries": registries }).to_string(),
    )
    .await
}

fn registry(addr: &Address, progress: Value) -> Value {
    json!({ "userAddress": addr.checksummed(), "data": { "progress": progress } })
}

async fn row(
    pool: &PgPool,
    addr: &Address,
    badge: &str,
) -> Option<(i32, Value, Option<NaiveDateTime>)> {
    sqlx::query_as(
        "SELECT steps_done, progress, completed_at FROM social.user_badge_progress \
         WHERE address = $1 AND badge_id = $2",
    )
    .bind(addr)
    .bind(badge)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn tiers(pool: &PgPool, addr: &Address, badge: &str) -> Vec<(String, NaiveDateTime)> {
    sqlx::query_as(
        "SELECT a.tier_id, a.completed_at FROM social.user_achieved_tiers a \
         JOIN social.badge_tiers t ON t.badge_id = a.badge_id AND t.tier_id = a.tier_id \
         WHERE a.address = $1 AND a.badge_id = $2 ORDER BY t.ordinal",
    )
    .bind(addr)
    .bind(badge)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn notifications(pool: &PgPool, addr: &Address) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM social.notifications WHERE address = $1")
        .bind(addr)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn the_route_is_bearer_gated() {
    let Some(unset) = crate::db::scratch::pool(2).await else {
        return;
    };
    let unset: SharedState = Arc::new(State::new(unset, Settings::default()));
    let body = json!({ "registries": [] }).to_string();
    let (status, body_json) = backfill(&unset, "halloweeks_2026", Some("Bearer x"), &body).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body_json["error"], json!("admin token not configured"));

    let state = state().await.unwrap();
    for presented in [None, Some("Bearer wrong")] {
        let (status, _) = backfill(&state, "halloweeks_2026", presented, &body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{presented:?}");
    }
}

#[tokio::test]
async fn badge_and_body_checks_follow_upstream() {
    let Some(state) = state().await else {
        return;
    };
    let addr = fresh_address();
    let (status, body) = post(&state, "own_badge", json!([])).await;
    assert_eq!(
        (status, body),
        (
            StatusCode::NOT_FOUND,
            json!({ "message": "Badge does not exists" })
        )
    );
    let bearer = format!("Bearer {TOKEN}");
    for bad in ["not json", "{}", r#"{"registries": [{"data": {}}]}"#] {
        let (status, body) = backfill(&state, "halloweeks_2026", Some(&bearer), bad).await;
        assert_eq!(
            (status, body),
            (
                StatusCode::BAD_REQUEST,
                json!({ "message": "Invalid body" })
            ),
            "{bad}"
        );
    }
    // one refused registry fails the request before anything is written
    let other = fresh_address();
    let (status, body) = post(
        &state,
        "halloweeks_2026",
        json!([
            registry(&other, json!({ "completedAt": 1_780_000_000_000_i64 })),
            registry(&addr, json!({ "completedAt": "yesterday" })),
        ]),
    )
    .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::BAD_REQUEST,
            json!({ "message": "Could not backfill this badge" })
        )
    );
    assert!(
        row(state.badges.pool(), &other, "halloweeks_2026")
            .await
            .is_none()
    );
    // in upstream's enum without a merger
    let (status, _) = post(&state, "traveler", json!([registry(&addr, json!({}))])).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, body) = post(&state, "traveler", json!([])).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["userProgressesMerged"], json!(0));
}

#[tokio::test]
async fn a_one_time_badge_keeps_its_first_completion_and_never_notifies() {
    let Some(state) = state().await else {
        return;
    };
    let pool = state.badges.pool();
    let (addr, other) = (fresh_address(), fresh_address());
    let first = 1_780_000_000_000_i64;
    let (status, body) = post(
        &state,
        "halloweeks_2026",
        json!([
            registry(&addr, json!({ "completedAt": first + 5_000 })),
            { "userAddress": "not-an-address", "data": { "progress": { "completedAt": first } } },
        ]),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["userProgressesMerged"], json!(1));
    assert_eq!(
        body["data"]["failures"],
        json!([{ "address": "not-an-address", "errors": ["invalid address"] }])
    );
    assert_eq!(body["data"]["badge"]["id"], json!("halloweeks_2026"));
    assert_eq!(body["data"]["badge"]["criteria"], json!({ "steps": 1 }));
    assert!(body["data"]["badge"].get("archived").is_none());

    // earlier moves it back, later does not; a re-run changes nothing
    for (sent, kept) in [(first, first), (first + 9_000, first), (first, first)] {
        let (status, body) = post(
            &state,
            "halloweeks_2026",
            json!([registry(&addr, json!({ "completedAt": sent }))]),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["data"]["userProgressesMerged"], json!(1));
        let (steps, doc, completed) = row(pool, &addr, "halloweeks_2026").await.unwrap();
        assert_eq!((steps, completed), (1, Some(naive_ms(kept))), "{sent}");
        assert_eq!(doc, json!({ "steps": 1, "completed_at": kept }));
    }
    assert_eq!(notifications(pool, &addr).await, 0);
    assert!(row(pool, &other, "halloweeks_2026").await.is_none());

    let app = Router::new().nest("/badges", router(state.clone()));
    let req = Request::builder()
        .uri(format!("/badges/users/{addr}/badges"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 24)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let earned = body["data"]["achieved"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] == json!("halloweeks_2026"))
        .cloned()
        .unwrap();
    assert_eq!(earned["completedAt"], json!(first.to_string()));
}

#[tokio::test]
async fn a_tiered_backfill_stamps_each_tier_with_the_item_that_reached_it() {
    let Some(state) = state().await else {
        return;
    };
    let pool = state.badges.pool();
    let addr = fresh_address();
    let t = 1_780_000_000_000_i64;
    let bought: Vec<Value> = (0..10)
        .map(|i| json!({ "nftId": format!("n{i}"), "transactionHash": format!("0xtx{i}"), "saleAt": t + i * 1_000 }))
        .collect();
    let (status, body) = post(
        &state,
        "emotionista",
        json!([registry(&addr, json!({ "emotesBought": bought }))]),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["userProgressesMerged"], json!(1));
    assert_eq!(body["data"]["badge"]["tiers"].as_array().unwrap().len(), 6);
    let expected = vec![
        ("emotionista-starter".to_string(), naive_ms(t)),
        ("emotionista-bronze".to_string(), naive_ms(t + 9_000)),
    ];
    assert_eq!(tiers(pool, &addr, "emotionista").await, expected);
    let (steps, doc, completed) = row(pool, &addr, "emotionista").await.unwrap();
    assert_eq!((steps, completed), (10, None));
    assert_eq!(
        doc["transactions_emotes_purchase"][0],
        json!({ "transactionHash": "0xtx0", "saleAt": t })
    );
    // the same batch again is a no-op
    let (_, body) = post(
        &state,
        "emotionista",
        json!([registry(&addr, json!({ "emotesBought": bought }))]),
    )
    .await;
    assert_eq!(body["data"]["userProgressesMerged"], json!(1));
    assert_eq!(tiers(pool, &addr, "emotionista").await, expected);
    assert_eq!(row(pool, &addr, "emotionista").await.unwrap().0, 10);
    assert_eq!(notifications(pool, &addr).await, 0);
}

#[tokio::test]
async fn event_enthusiast_and_moves_master_merge_as_upstream() {
    let Some(state) = state().await else {
        return;
    };
    let pool = state.badges.pool();
    let addr = fresh_address();
    let t = 1_780_000_000_000_i64;
    let (status, body) = post(
        &state,
        "event_enthusiast",
        json!([registry(
            &addr,
            json!({ "eventsAttended": [
            { "eventId": "e1", "eventName": "One", "firstVisitAt": t, "timeSpentInSeconds": 600 }
        ] })
        )]),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        tiers(pool, &addr, "event_enthusiast").await,
        vec![("event-enthusiast-starter".to_string(), naive_ms(t))]
    );
    assert_eq!(
        row(pool, &addr, "event_enthusiast").await.unwrap().1,
        json!({ "steps": 1, "events_ids_visited": ["e1"] })
    );

    let moves = json!({ "usedEmotesCount": 1_200, "lastEmoteTriggeredAt": t,
        "achievedTiers": [{ "steps": 100, "completedAt": t - 2_000 }, { "steps": 1_000, "completedAt": t - 1_000 }] });
    let (status, body) = post(&state, "moves_master", json!([registry(&addr, moves)])).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        tiers(pool, &addr, "moves_master").await,
        vec![
            ("moves-master-starter".to_string(), naive_ms(t - 2_000)),
            ("moves-master-bronze".to_string(), naive_ms(t - 1_000)),
        ]
    );
    // an unknown tier target refuses the request
    let (status, _) = post(
        &state,
        "moves_master",
        json!([registry(
            &addr,
            json!({ "usedEmotesCount": 1, "lastEmoteTriggeredAt": t,
            "achievedTiers": [{ "steps": 7, "completedAt": t }] })
        )]),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(notifications(pool, &addr).await, 0);
}

#[tokio::test]
async fn single_badges_report_validation_failures_per_wallet() {
    let Some(state) = state().await else {
        return;
    };
    let pool = state.badges.pool();
    let (partial, claimed, done) = (fresh_address(), fresh_address(), fresh_address());
    let t = 1_780_000_000_000_i64;
    let (status, body) = post(
        &state,
        "open_for_business",
        json!([
            registry(&partial, json!({ "collectionCreatedAt": t })),
            registry(
                &claimed,
                json!({ "collectionCreatedAt": t, "completedAt": t })
            ),
            registry(
                &done,
                json!({ "collectionCreatedAt": t, "storeCreatedAt": t, "completedAt": t + 1 })
            ),
        ]),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["userProgressesMerged"], json!(2));
    assert_eq!(
        body["data"]["failures"],
        json!([{ "address": claimed.as_str(), "errors": ["mismatch between badge criteria and progress steps"] }])
    );
    assert!(row(pool, &claimed, "open_for_business").await.is_none());
    let (steps, doc, completed) = row(pool, &partial, "open_for_business").await.unwrap();
    assert_eq!((steps, completed), (1, None));
    assert_eq!(
        doc,
        json!({ "steps": 1, "store_completed": false, "collection_submitted": true })
    );
    let (steps, _, completed) = row(pool, &done, "open_for_business").await.unwrap();
    assert_eq!((steps, completed), (2, Some(naive_ms(t + 1))));

    for (badge, progress) in [
        ("land_architect", json!({ "firstSceneDeployedAt": t })),
        (
            "profile_pro",
            json!({ "completedAt": t, "description": "hello" }),
        ),
        (
            "mythic_model",
            json!({ "completedAt": t, "completedWith": ["urn:a", "urn:b", "urn:c"] }),
        ),
    ] {
        let (status, body) = post(&state, badge, json!([registry(&done, progress)])).await;
        assert_eq!(status, StatusCode::OK, "{badge} {body}");
        assert_eq!(body["data"]["userProgressesMerged"], json!(1), "{badge}");
        let (steps, _, completed) = row(pool, &done, badge).await.unwrap();
        assert_eq!((steps, completed), (1, Some(naive_ms(t))), "{badge}");
    }
    assert_eq!(notifications(pool, &done).await, 0);
}

#[tokio::test]
async fn an_archived_or_unpublished_badge_is_refused() {
    let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
        return;
    };
    let state: SharedState = Arc::new(State::new(pool.clone(), settings()));
    let addr = fresh_address();
    let progress = json!([registry(
        &addr,
        json!({ "completedAt": 1_780_000_000_000_i64 })
    )]);
    sqlx::query("UPDATE social.badge_definitions SET archived_at = timezone('UTC', now()) WHERE id = 'halloweeks_2026'")
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = post(&state, "halloweeks_2026", progress.clone()).await;
    assert_eq!(
        (status, body),
        (
            StatusCode::BAD_REQUEST,
            json!({ "message": "Badge is archived and cannot be granted" })
        )
    );
    sqlx::query("UPDATE social.badge_definitions SET archived_at = NULL, published_at = NULL WHERE id = 'halloweeks_2026'")
        .execute(&pool)
        .await
        .unwrap();
    let (status, _) = post(&state, "halloweeks_2026", progress).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(row(&pool, &addr, "halloweeks_2026").await.is_none());
}

#[tokio::test]
async fn a_database_error_fails_only_that_wallet() {
    let Some(state) = state().await else {
        return;
    };
    let pool = state.badges.pool();
    let (before, broken, after) = (fresh_address(), fresh_address(), fresh_address());
    let name = format!("backfill_fail_{}", &broken.as_str()[2..]);
    sqlx::query(AssertSqlSafe(format!(
        "CREATE FUNCTION social.{name}() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN IF NEW.address = '{}' THEN RAISE EXCEPTION 'forced'; END IF; RETURN NEW; END $$",
        broken.as_str()
    )))
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(AssertSqlSafe(format!(
        "CREATE TRIGGER {name} BEFORE INSERT OR UPDATE ON social.user_badge_progress \
         FOR EACH ROW EXECUTE FUNCTION social.{name}()"
    )))
    .execute(pool)
    .await
    .unwrap();

    let at = 1_780_000_000_000_i64;
    let (status, body) = post(
        &state,
        "halloweeks_2026",
        json!([
            registry(&before, json!({ "completedAt": at })),
            registry(&broken, json!({ "completedAt": at })),
            registry(&after, json!({ "completedAt": at })),
        ]),
    )
    .await;

    sqlx::query(AssertSqlSafe(format!(
        "DROP TRIGGER {name} ON social.user_badge_progress"
    )))
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(AssertSqlSafe(format!("DROP FUNCTION social.{name}()")))
        .execute(pool)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["userProgressesMerged"], json!(2));
    assert_eq!(
        body["data"]["failures"],
        json!([{ "address": broken.as_str(), "errors": ["could not write the progress"] }])
    );
    assert!(row(pool, &before, "halloweeks_2026").await.is_some());
    assert!(row(pool, &broken, "halloweeks_2026").await.is_none());
    assert!(row(pool, &after, "halloweeks_2026").await.is_some());
}
