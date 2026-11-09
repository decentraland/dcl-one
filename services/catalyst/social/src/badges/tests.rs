//! The upstream-parity bodies, the progress write path and the admin
//! contracts over the router as `serve` mounts it (nested under `/badges`),
//! on the process's scratch database (`CATALYST_TEST_DATABASE_URL`; skipped
//! without it). The catalog rows come from the schema; every per-user row is
//! scoped by a fresh address (or a captured one only one test uses), and a
//! test that needs to archive or unpublish a badge makes its own, because
//! sibling tests share the catalog.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use chrono::{DateTime, NaiveDateTime};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use super::progress::{Applied, ApplyOptions, apply_progress};
use super::{DEFAULT_ASSET_BASE_URL, Settings, SharedState, State, router};
use crate::identity::Address;
use crate::notifications::errors::ApiError;
use crate::notifications::store::test_support::fresh_address;

const CDN: &str = DEFAULT_ASSET_BASE_URL;

fn naive_ms(ms: i64) -> NaiveDateTime {
    DateTime::from_timestamp_millis(ms).unwrap().naive_utc()
}

async fn state(settings: Settings) -> Option<SharedState> {
    let pool = crate::db::scratch::pool(4).await?;
    Some(Arc::new(State::new(pool, settings)))
}

fn app(state: SharedState) -> Router {
    Router::new().nest("/badges", router(state))
}

async fn send(app: Router, req: Request<Body>) -> (StatusCode, HeaderMap, Value) {
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 24)
        .await
        .unwrap();
    let value = if bytes.is_empty()
        || headers
            .get(header::CONTENT_TYPE)
            .is_some_and(|c| c == "image/png")
    {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, headers, value)
}

async fn get(app: Router, uri: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let (status, _, value) = send(app, req).await;
    (status, value)
}

fn assets(dir: &str) -> Value {
    json!({
        "2d": { "normal": format!("{CDN}/{dir}/2d/normal.png") },
        "3d": {
            "normal": format!("{CDN}/{dir}/3d/normal.png"),
            "hrm": format!("{CDN}/{dir}/3d/hrm.png"),
            "basecolor": format!("{CDN}/{dir}/3d/basecolor.png")
        }
    })
}

fn find<'a>(list: &'a Value, id: &str) -> Option<&'a Value> {
    list.as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] == json!(id))
}

/// A published single badge of this test's own, so archiving it touches no
/// sibling test; `steps` is its target.
async fn own_badge(pool: &PgPool, steps: i32) -> String {
    let id = format!("own_{}", Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO social.badge_definitions \
           (id, name, description, category, is_tier, criteria_steps, assets, published_at) \
         VALUES ($1, 'Own', 'Own badge', 'Socializer', false, $2, \
                 jsonb_build_object('2d', jsonb_build_object('normal', $1 || '/2d/normal.png')), \
                 timezone('UTC', now()))",
    )
    .bind(&id)
    .bind(steps)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn drop_badge(pool: &PgPool, id: &str) {
    sqlx::query("DELETE FROM social.badge_definitions WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn cors_headers_on_get_and_preflight() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    let req = Request::builder()
        .method("GET")
        .uri("/badges/categories")
        .header("origin", "https://example.com")
        .body(Body::empty())
        .unwrap();
    let (status, headers, _) = send(app(state.clone()), req).await;
    assert!(status.is_success());
    assert_eq!(headers.get("access-control-allow-origin").unwrap(), "*");

    let req = Request::builder()
        .method("OPTIONS")
        .uri("/badges/categories")
        .header("origin", "https://example.com")
        .header("access-control-request-method", "GET")
        .body(Body::empty())
        .unwrap();
    let (status, headers, _) = send(app(state), req).await;
    assert!(status.is_success());
    assert_eq!(headers.get("access-control-allow-origin").unwrap(), "*");
    assert!(headers.get("access-control-allow-methods").is_some());
}

#[tokio::test]
async fn the_schema_seeds_upstreams_catalog() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    let pool = state.badges.pool();
    let (badges, tiered, single_with_target, published): (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE is_tier), \
                count(*) FILTER (WHERE NOT is_tier AND criteria_steps > 0), \
                count(*) FILTER (WHERE published_at = '2026-10-07 00:00:00' AND archived_at IS NULL) \
         FROM social.badge_definitions WHERE created_at < '2026-10-08'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let tiers: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM social.badge_tiers t JOIN social.badge_definitions d ON d.id = t.badge_id \
         WHERE d.created_at < '2026-10-08'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        (badges, tiered, single_with_target, published),
        (34, 10, 24, 34)
    );
    assert_eq!(tiers + single_with_target, 84, "upstream's 84 tiers");
    let order: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM social.badge_definitions WHERE created_at < '2026-10-08' ORDER BY created_at, id",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(order.first().map(String::as_str), Some("dclcitizen"));
    assert_eq!(order[7], "open_for_business");
    assert_eq!(order.last().map(String::as_str), Some("halloweeks_2026"));
    for gone in ["decentraland_citizen", "walkabout"] {
        assert!(!order.iter().any(|id| id == gone), "{gone}");
    }
}

#[tokio::test]
async fn categories_status_and_catalog_routes() {
    let Some(state) = state(Settings {
        commit_hash: Some("abc123".into()),
        ..Settings::default()
    })
    .await
    else {
        return;
    };
    let (status, body) = get(app(state.clone()), "/badges/categories").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({ "data": { "categories": ["Explorer", "Collector", "Creator", "Socializer", "Builder"] } })
    );

    let (status, body) = get(app(state.clone()), "/badges/status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["commitHash"], "abc123");
    assert!(body["data"]["currentTime"].is_i64());
    assert!(body["data"]["version"].is_string());

    let (status, body) = get(app(state.clone()), "/badges/badges").await;
    assert_eq!(status, StatusCode::OK);
    let list = &body["data"];
    let ids: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["id"].as_str().unwrap())
        .collect();
    assert_eq!(&ids[..3], ["dclcitizen", "epic_ensemble", "regally_rare"]);
    assert_eq!(
        find(list, "open_for_business").unwrap(),
        &json!({
            "id": "open_for_business",
            "name": "Open for Business",
            "category": "Creator",
            "description": "Complete Store Information and submit at least 1 collection",
            "criteria": { "steps": 2 },
            "assets": assets("open_for_business"),
            "archived": false
        })
    );
    let traveler = find(list, "traveler").unwrap();
    assert!(traveler.get("criteria").is_none() && traveler.get("assets").is_none());
    assert_eq!(traveler["tiers"].as_array().unwrap().len(), 6);
    assert_eq!(
        traveler["tiers"][1],
        json!({
            "tierId": "traveler-bronze",
            "tierName": "Bronze",
            "description": "50 Genesis City scenes explored (for at least 1 min)",
            "criteria": { "steps": 50 },
            "assets": assets("traveler/bronze")
        })
    );

    let (status, body) = get(app(state.clone()), "/badges/badges/traveler").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(&body["data"]["badge"], traveler);
    let (status, body) = get(app(state.clone()), "/badges/badges/traveler/tiers").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["tiers"], traveler["tiers"]);
    for missing in [
        "/badges/badges/doesnotexist",
        "/badges/badges/doesnotexist/tiers",
        "/badges/badges/walkabout",
    ] {
        let (status, body) = get(app(state.clone()), missing).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{missing}");
        assert_eq!(body, json!({ "error": "Badge not found" }), "{missing}");
    }
}

fn capture(name: &str) -> Value {
    let path = format!(
        "{}/services/catalyst/tests/fixtures/conformance/social/badges/{name}.json",
        env!("DCL_ONE_ROOT")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Writes the rows a captured `/users/{a}/badges` answer implies, so the
/// replay compares the whole body.
async fn seed_capture(pool: &PgPool, address: &str, achieved: &Value) {
    for table in ["user_achieved_tiers", "user_badge_progress"] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DELETE FROM social.{table} WHERE address = $1"
        )))
        .bind(address)
        .execute(pool)
        .await
        .unwrap();
    }
    for badge in achieved.as_array().unwrap() {
        let completed = badge["completedAt"]
            .as_str()
            .map(|ms| naive_ms(ms.parse().unwrap()));
        sqlx::query(
            "INSERT INTO social.user_badge_progress (address, badge_id, steps_done, completed_at) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(address)
        .bind(badge["id"].as_str().unwrap())
        .bind(badge["progress"]["stepsDone"].as_i64().unwrap() as i32)
        .bind(completed)
        .execute(pool)
        .await
        .unwrap();
        for tier in badge["progress"]["achievedTiers"]
            .as_array()
            .into_iter()
            .flatten()
        {
            sqlx::query(
                "INSERT INTO social.user_achieved_tiers (address, badge_id, tier_id, completed_at) \
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(address)
            .bind(badge["id"].as_str().unwrap())
            .bind(tier["tierId"].as_str().unwrap())
            .bind(naive_ms(tier["completedAt"].as_i64().unwrap()))
            .execute(pool)
            .await
            .unwrap();
        }
    }
}

#[tokio::test]
async fn the_production_captures_replay_exactly() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    for name in ["user-badges", "user-badges-second-address"] {
        let fixture = capture(name);
        let path = fixture["request"]["path"].as_str().unwrap();
        let address = path.split('/').nth(2).unwrap();
        let expected = &fixture["response"]["body_json"];
        seed_capture(state.badges.pool(), address, &expected["data"]["achieved"]).await;
        let (status, body) = get(app(state.clone()), &format!("/badges{path}")).await;
        assert_eq!(status, StatusCode::OK, "{name}");
        assert_eq!(&body, expected, "{name}");
    }
    for name in [
        "user-preview",
        "user-badges-unknown-address",
        "user-badges-bad-address",
        "user-preview-unknown-address",
        "categories",
        "badge-tiers",
    ] {
        let fixture = capture(name);
        let path = fixture["request"]["path"].as_str().unwrap();
        let (status, body) = get(app(state.clone()), &format!("/badges{path}")).await;
        assert_eq!(
            status.as_u16() as u64,
            fixture["response"]["status"].as_u64().unwrap(),
            "{name}"
        );
        assert_eq!(&body, &fixture["response"]["body_json"], "{name}");
    }
}

#[tokio::test]
async fn user_badges_shapes_and_include_not_achieved_is_exact() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    let pool = state.badges.pool();
    let addr = fresh_address();
    sqlx::query(
        "INSERT INTO social.user_badge_progress (address, badge_id, steps_done) VALUES ($1, 'open_for_business', 1)",
    )
    .bind(&addr)
    .execute(pool)
    .await
    .unwrap();
    for flag in ["TRUE", "1", "yes"] {
        let (_, body) = get(
            app(state.clone()),
            &format!("/badges/users/{addr}/badges?includeNotAchieved={flag}"),
        )
        .await;
        assert_eq!(body["data"]["notAchieved"], json!([]), "{flag}");
    }
    let (_, body) = get(
        app(state.clone()),
        &format!(
            "/badges/users/{}/badges?includeNotAchieved=true",
            addr.checksummed()
        ),
    )
    .await;
    let not_achieved = &body["data"]["notAchieved"];
    assert_eq!(body["data"]["achieved"], json!([]));
    assert_eq!(
        find(not_achieved, "open_for_business").unwrap(),
        &json!({
            "id": "open_for_business",
            "name": "Open for Business",
            "description": "Complete Store Information and submit at least 1 collection",
            "category": "Creator",
            "isTier": false,
            "completedAt": null,
            "assets": assets("open_for_business"),
            "progress": { "stepsDone": 1, "nextStepsTarget": 2, "totalStepsTarget": 2 }
        }),
        "a single badge's partial progress and real target"
    );
    let emotionista = find(not_achieved, "emotionista").unwrap();
    assert_eq!(emotionista["assets"], assets("emotionista/starter"));
    assert_eq!(
        emotionista["progress"],
        json!({ "stepsDone": 0, "nextStepsTarget": 1, "totalStepsTarget": 300 })
    );
    let ids: Vec<&str> = not_achieved
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["id"].as_str().unwrap())
        .collect();
    assert_eq!(&ids[..2], ["dclcitizen", "epic_ensemble"], "catalog order");
}

async fn audit(pool: &PgPool, addr: &Address) -> Vec<(String, String, Option<String>, String)> {
    sqlx::query_as("SELECT action, badge_id, tier_id, actor FROM social.badge_admin_audit WHERE address = $1 ORDER BY id")
        .bind(addr)
        .fetch_all(pool)
        .await
        .unwrap()
}

type ProgressTuple = (
    i32,
    Option<NaiveDateTime>,
    Option<String>,
    Option<String>,
    Value,
);

async fn progress(pool: &PgPool, addr: &Address, badge_id: &str) -> Option<ProgressTuple> {
    sqlx::query_as(
        "SELECT steps_done, completed_at, last_completed_tier_id, granted_by, progress \
         FROM social.user_badge_progress WHERE address = $1 AND badge_id = $2",
    )
    .bind(addr)
    .bind(badge_id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn achieved(pool: &PgPool, addr: &Address, badge_id: &str) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT a.tier_id FROM social.user_achieved_tiers a \
         JOIN social.badge_tiers t ON t.badge_id = a.badge_id AND t.tier_id = a.tier_id \
         WHERE a.address = $1 AND a.badge_id = $2 ORDER BY t.ordinal",
    )
    .bind(addr)
    .bind(badge_id)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn badge_notifications(pool: &PgPool, addr: &Address) -> Vec<Value> {
    sqlx::query_scalar(
        "SELECT metadata FROM social.notifications WHERE address = $1 AND type = 'badge_granted' \
         ORDER BY metadata->>'id', metadata->>'description'",
    )
    .bind(addr)
    .fetch_all(pool)
    .await
    .unwrap()
}

fn status_of(err: &ApiError) -> u16 {
    match err {
        ApiError::Http { status, .. } => *status,
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[tokio::test]
async fn unknown_or_unpublished_badge_is_false_and_writes_nothing() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    let addr = fresh_address();
    for id in ["doesnotexist", "walkabout"] {
        assert!(
            !state
                .badges
                .grant_badge(&addr, id, None, "t")
                .await
                .unwrap(),
            "{id}"
        );
    }
    assert!(
        !state
            .badges
            .revoke_badge(&addr, "doesnotexist", "t")
            .await
            .unwrap()
    );
    assert!(audit(state.badges.pool(), &addr).await.is_empty());
}

#[tokio::test]
async fn a_tiered_grant_earns_every_tier_up_to_the_requested_one() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    let (pool, addr) = (state.badges.pool().clone(), fresh_address());

    let err = state
        .badges
        .grant_badge(&addr, "emotionista", None, "t")
        .await
        .unwrap_err();
    assert_eq!(status_of(&err), 400, "a tiered grant names its tier");
    let err = state
        .badges
        .grant_badge(&addr, "emotionista", Some("emotionista-mythic"), "t")
        .await
        .unwrap_err();
    assert_eq!(status_of(&err), 404);
    assert!(progress(&pool, &addr, "emotionista").await.is_none());

    assert!(
        state
            .badges
            .grant_badge(&addr, "emotionista", Some("emotionista-silver"), "t")
            .await
            .unwrap()
    );
    let p = progress(&pool, &addr, "emotionista").await.unwrap();
    assert_eq!(
        (p.0, p.1, p.2.as_deref(), p.3.as_deref()),
        (25, None, Some("emotionista-silver"), Some("t"))
    );
    assert_eq!(p.4, json!({ "steps": 25 }));
    assert_eq!(
        achieved(&pool, &addr, "emotionista").await,
        [
            "emotionista-starter",
            "emotionista-bronze",
            "emotionista-silver"
        ]
    );
    let (ach, _) = state
        .badges
        .user_badges(addr.as_str(), false)
        .await
        .unwrap();
    let emotionista = ach.iter().find(|b| b.id == "emotionista").unwrap();
    assert_eq!(
        emotionista.completed_at, None,
        "silver is not the last tier"
    );
    assert_eq!(emotionista.progress.next_steps_target, Some(50));
    assert_eq!(emotionista.assets, Some(assets("emotionista/silver")));

    state
        .badges
        .grant_badge(&addr, "emotionista", Some("emotionista-diamond"), "t2")
        .await
        .unwrap();
    let p = progress(&pool, &addr, "emotionista").await.unwrap();
    assert_eq!((p.0, p.2.as_deref()), (300, Some("emotionista-diamond")));
    assert!(p.1.is_some(), "the last tier completes the badge");
    assert_eq!(achieved(&pool, &addr, "emotionista").await.len(), 6);

    let notes = badge_notifications(&pool, &addr).await;
    assert_eq!(notes.len(), 6, "one per tier, never twice");
    assert!(notes.contains(&json!({
        "id": "emotionista",
        "title": "New Badge Unlocked!",
        "description": "Emotionista Silver",
        "image": format!("{CDN}/emotionista/silver/2d/normal.png"),
    })));
    assert_eq!(
        audit(&pool, &addr)
            .await
            .iter()
            .map(|r| r.2.clone())
            .collect::<Vec<_>>(),
        [
            Some("emotionista-silver".into()),
            Some("emotionista-diamond".into())
        ],
        "refused grants leave no audit row"
    );
}

#[tokio::test]
async fn a_single_grant_reaches_the_target_and_is_idempotent() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    let (pool, addr) = (state.badges.pool().clone(), fresh_address());
    let err = state
        .badges
        .grant_badge(&addr, "open_for_business", Some("bogus"), "t")
        .await
        .unwrap_err();
    assert_eq!(status_of(&err), 400, "a single badge takes no tierId");
    assert!(progress(&pool, &addr, "open_for_business").await.is_none());
    for actor in ["first", "again"] {
        assert!(
            state
                .badges
                .grant_badge(&addr, "open_for_business", None, actor)
                .await
                .unwrap()
        );
    }
    let p = progress(&pool, &addr, "open_for_business").await.unwrap();
    assert_eq!((p.0, p.2, p.3.as_deref()), (2, None, Some("again")));
    assert!(p.1.is_some());
    assert!(achieved(&pool, &addr, "open_for_business").await.is_empty());
    assert_eq!(
        badge_notifications(&pool, &addr).await,
        vec![json!({
            "id": "open_for_business",
            "title": "New Badge Unlocked!",
            "description": "Open for Business",
            "image": format!("{CDN}/open_for_business/2d/normal.png"),
        })]
    );
    assert_eq!(audit(&pool, &addr).await.len(), 2);
}

#[tokio::test]
async fn apply_progress_keeps_the_document_and_only_awards_awardable_badges() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    let pool = state.badges.pool().clone();
    let addr = fresh_address();
    let badge = own_badge(&pool, 2).await;
    let at = naive_ms(1_790_000_000_000);
    let quiet = ApplyOptions {
        at,
        notify: None,
        granted_by: None,
    };
    let visit = |profile: &'static str| {
        move |p: &mut super::Progress| {
            let seen = p.state.entry("profiles_visited").or_insert(json!([]));
            let list = seen.as_array_mut().unwrap();
            if !list.contains(&json!(profile)) {
                list.push(json!(profile));
            }
            p.steps = list.len() as i32;
        }
    };

    let mut tx = pool.begin().await.unwrap();
    let first = apply_progress(&mut tx, &addr, &badge, &quiet, visit("a"))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let Applied::Written(change) = first else {
        panic!("{first:?}")
    };
    assert_eq!(
        (change.steps_before, change.steps, change.completed),
        (0, 1, false)
    );

    let mut tx = pool.begin().await.unwrap();
    let again = apply_progress(&mut tx, &addr, &badge, &quiet, visit("a"))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        again,
        Applied::Unchanged,
        "a repeated event is deduplicated"
    );

    let mut tx = pool.begin().await.unwrap();
    let done = apply_progress(&mut tx, &addr, &badge, &quiet, visit("b"))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(matches!(done, Applied::Written(ref c) if c.completed && c.notified == 0));
    let p = progress(&pool, &addr, &badge).await.unwrap();
    assert_eq!((p.0, p.1), (2, Some(at)));
    assert_eq!(p.4, json!({ "steps": 2, "profiles_visited": ["a", "b"] }));
    assert!(
        badge_notifications(&pool, &addr).await.is_empty(),
        "suppressed"
    );

    // archived: no award, still listed for its earner with nothing left to reach
    sqlx::query(
        "UPDATE social.badge_definitions SET archived_at = timezone('UTC', now()) WHERE id = $1",
    )
    .bind(&badge)
    .execute(&pool)
    .await
    .unwrap();
    let other = fresh_address();
    let mut tx = pool.begin().await.unwrap();
    let refused = apply_progress(&mut tx, &other, &badge, &quiet, visit("a"))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(refused, Applied::NotAwardable);
    assert!(progress(&pool, &other, &badge).await.is_none());
    let err = state
        .badges
        .grant_badge(&other, &badge, None, "t")
        .await
        .unwrap_err();
    assert_eq!(status_of(&err), 400, "an archived badge is not granted");

    let (_, body) = get(app(state.clone()), "/badges/badges").await;
    assert_eq!(
        find(&body["data"], &badge).unwrap()["archived"],
        json!(true)
    );
    let (_, body) = get(
        app(state.clone()),
        &format!("/badges/users/{addr}/badges?includeNotAchieved=true"),
    )
    .await;
    let earned = find(&body["data"]["achieved"], &badge).unwrap();
    assert_eq!(earned["progress"]["nextStepsTarget"], Value::Null);
    let (_, body) = get(
        app(state.clone()),
        &format!("/badges/users/{other}/badges?includeNotAchieved=true"),
    )
    .await;
    assert!(find(&body["data"]["notAchieved"], &badge).is_none());

    // unpublished: invisible and not awardable
    sqlx::query(
        "UPDATE social.badge_definitions SET archived_at = NULL, published_at = NULL WHERE id = $1",
    )
    .bind(&badge)
    .execute(&pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let refused = apply_progress(&mut tx, &other, &badge, &quiet, visit("a"))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(refused, Applied::NotAwardable);
    state.badges.invalidate_catalog();
    let (status, _) = get(app(state.clone()), &format!("/badges/badges/{badge}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    drop_badge(&pool, &badge).await;
}

#[tokio::test]
async fn the_preview_has_a_row_per_tier_and_pages() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    let (pool, addr) = (state.badges.pool().clone(), fresh_address());
    let rows = [
        ("traveler", "traveler-starter", 1_000),
        ("traveler", "traveler-bronze", 2_000),
        ("emotionista", "emotionista-starter", 3_000),
    ];
    for (badge, tier, ms) in rows {
        sqlx::query(
            "INSERT INTO social.user_achieved_tiers (address, badge_id, tier_id, completed_at) VALUES ($1, $2, $3, $4)",
        )
        .bind(&addr)
        .bind(badge)
        .bind(tier)
        .bind(naive_ms(1_780_000_000_000 + ms))
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO social.user_badge_progress (address, badge_id, steps_done, completed_at) VALUES \
         ($1, 'traveler', 50, NULL), ($1, 'emotionista', 1, NULL), ($1, 'dclcitizen', 1, $2)",
    )
    .bind(&addr)
    .bind(naive_ms(1_780_000_004_000))
    .execute(&pool)
    .await
    .unwrap();
    let (_, body) = get(app(state.clone()), &format!("/badges/users/{addr}/preview")).await;
    assert_eq!(
        body,
        json!({ "data": { "latestAchievedBadges": [
            { "id": "dclcitizen", "name": "Decentraland Citizen", "image": format!("{CDN}/dclcitizen/2d/normal.png") },
            { "id": "emotionista", "name": "Emotionista", "tierName": "Starter", "image": format!("{CDN}/emotionista/starter/2d/normal.png") },
            { "id": "traveler", "name": "Traveler", "tierName": "Bronze", "image": format!("{CDN}/traveler/bronze/2d/normal.png") },
            { "id": "traveler", "name": "Traveler", "tierName": "Starter", "image": format!("{CDN}/traveler/starter/2d/normal.png") },
        ] } })
    );
    let (_, body) = get(
        app(state.clone()),
        &format!("/badges/users/{addr}/preview?limit=2&offset=1"),
    )
    .await;
    let names: Vec<_> = body["data"]["latestAchievedBadges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| (b["id"].as_str().unwrap(), b["tierName"].as_str()))
        .collect();
    assert_eq!(
        names,
        [
            ("emotionista", Some("Starter")),
            ("traveler", Some("Bronze"))
        ]
    );
}

#[tokio::test]
async fn the_asset_base_resolves_relative_and_legacy_paths() {
    let base = "https://node.example/badges/assets";
    let Some(state) = state(Settings {
        public_asset_base_url: format!("{base}/"),
        ..Settings::default()
    })
    .await
    else {
        return;
    };
    let pool = state.badges.pool().clone();
    let badge = own_badge(&pool, 1).await;
    sqlx::query(
        "UPDATE social.badge_definitions SET assets = jsonb_build_object('2d', jsonb_build_object('normal', \
         'https://badges.decentraland.org/assets/' || id || '/2d/normal.png')) WHERE id = $1",
    )
    .bind(&badge)
    .execute(&pool)
    .await
    .unwrap();
    let (_, body) = get(app(state.clone()), "/badges/badges").await;
    assert_eq!(
        find(&body["data"], "dclcitizen").unwrap()["assets"]["3d"]["basecolor"],
        json!(format!("{base}/dclcitizen/3d/basecolor.png"))
    );
    assert_eq!(
        find(&body["data"], &badge).unwrap()["assets"],
        json!({ "2d": { "normal": format!("{base}/{badge}/2d/normal.png") } })
    );
    drop_badge(&pool, &badge).await;
}

#[tokio::test]
async fn revoke_clears_both_tables_and_audits_once() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    let (pool, addr) = (state.badges.pool().clone(), fresh_address());
    state
        .badges
        .grant_badge(&addr, "traveler", Some("traveler-bronze"), "t")
        .await
        .unwrap();
    state
        .badges
        .grant_badge(&addr, "open_for_business", None, "t")
        .await
        .unwrap();
    assert!(
        state
            .badges
            .revoke_badge(&addr, "traveler", "mod")
            .await
            .unwrap()
    );
    assert!(achieved(&pool, &addr, "traveler").await.is_empty());
    assert!(progress(&pool, &addr, "traveler").await.is_none());
    assert!(progress(&pool, &addr, "open_for_business").await.is_some());
    assert!(
        state
            .badges
            .revoke_badge(&addr, "traveler", "mod")
            .await
            .unwrap(),
        "known badge with nothing to delete still reports true"
    );
    let rows = audit(&pool, &addr).await;
    assert_eq!(rows.len(), 4);
    assert_eq!(
        (
            rows[2].0.as_str(),
            rows[2].1.as_str(),
            rows[2].2.as_deref(),
            rows[2].3.as_str()
        ),
        ("revoke", "traveler", None, "mod")
    );
}

#[tokio::test]
async fn the_admin_routes_are_bearer_gated() {
    let Some(unset) = state(Settings::default()).await else {
        return;
    };
    let addr = fresh_address();
    let path = |badge: &str| format!("/badges/users/{}/badges/{badge}", addr.checksummed());
    let request = |method: &str, uri: &str, authorization: Option<&str>, body: Option<Value>| {
        let mut req = Request::builder().method(method).uri(uri);
        if let Some(a) = authorization {
            req = req.header(header::AUTHORIZATION, a);
        }
        match body {
            Some(b) => req
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(b.to_string()))
                .unwrap(),
            None => req.body(Body::empty()).unwrap(),
        }
    };
    for method in ["POST", "GET"] {
        let (status, _, body) = send(
            app(unset.clone()),
            request(
                method,
                &path("open_for_business"),
                Some("Bearer anything"),
                None,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method}");
        assert_eq!(body["error"], json!("admin token not configured"));
    }

    let state = state(Settings {
        admin_token: Some("s3cret".into()),
        ..Settings::default()
    })
    .await
    .unwrap();
    for presented in [None, Some("Bearer wrong"), Some("Basic s3cret")] {
        let (status, _, body) = send(
            app(state.clone()),
            request("DELETE", &path("open_for_business"), presented, None),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["error"], json!("missing or invalid bearer token"));
    }
    let bearer = Some("Bearer s3cret");
    let (status, _, body) = send(
        app(state.clone()),
        request("GET", &path("open_for_business"), bearer, None),
    )
    .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::NOT_FOUND,
            json!({ "error": "Not Found", "message": "User progress not found for this badge" })
        )
    );
    let (status, _, body) = send(
        app(state.clone()),
        request("GET", &path("nope"), bearer, None),
    )
    .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::NOT_FOUND,
            json!({ "error": "Not Found", "message": "Badge does not exists" })
        )
    );

    let (status, _, body) = send(
        app(state.clone()),
        request(
            "POST",
            &path("open_for_business"),
            bearer,
            Some(json!({ "tierId": " " })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "data": { "granted": true, "address": addr.as_str(), "badgeId": "open_for_business", "tierId": null } })
    );
    assert_eq!(
        audit(state.badges.pool(), &addr).await[0].3,
        "service-token:CATALYRST_BADGES_ADMIN_TOKEN"
    );
    state
        .badges
        .grant_badge(&addr, "traveler", Some("traveler-starter"), "t")
        .await
        .unwrap();
    for (badge, achieved_tiers) in [
        ("open_for_business", Value::Null),
        ("traveler", json!(["traveler-starter"])),
    ] {
        let (status, _, body) = send(
            app(state.clone()),
            request("GET", &path(badge), bearer, None),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let row = &body["data"];
        assert_eq!(row["badge_id"], json!(badge));
        assert_eq!(row["user_address"], json!(addr.as_str()));
        assert!(row["completed_at"].is_string() || badge == "traveler");
        assert!(row["updated_at"].is_string());
        assert_eq!(
            row["achieved_tiers"]
                .as_array()
                .map(|a| a.iter().map(|t| t["tier_id"].clone()).collect::<Vec<_>>()),
            achieved_tiers.as_array().cloned()
        );
    }
    let (_, _, body) = send(
        app(state.clone()),
        request("GET", &path("open_for_business"), bearer, None),
    )
    .await;
    assert_eq!(body["data"]["progress"], json!({ "steps": 2 }));

    let (status, _, body) = send(
        app(state.clone()),
        request("DELETE", &path("open_for_business"), bearer, None),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({ "data": { "revoked": true, "address": addr.as_str(), "badgeId": "open_for_business" } })
    );
    let (status, _, body) = send(
        app(state.clone()),
        request(
            "POST",
            "/badges/users/not-an-address/badges/open_for_business",
            bearer,
            None,
        ),
    )
    .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("invalid address"))
    );
    let (status, _, body) = send(
        app(state),
        request("POST", &path("doesnotexist"), bearer, None),
    )
    .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (
            StatusCode::NOT_FOUND,
            Some("no badge found with id: doesnotexist")
        )
    );
}

#[tokio::test]
async fn the_assets_route_serves_the_mirror_directory() {
    let dir =
        std::env::temp_dir().join(format!("catalyst-badges-route-{}", Uuid::new_v4().simple()));
    std::fs::create_dir_all(dir.join("dclcitizen/2d")).unwrap();
    std::fs::write(dir.join("dclcitizen/2d/normal.png"), b"png").unwrap();
    let Some(state) = state(Settings {
        assets_dir: Some(dir.clone()),
        ..Settings::default()
    })
    .await
    else {
        return;
    };
    let req = |uri: &str| Request::builder().uri(uri).body(Body::empty()).unwrap();
    let (status, headers, _) = send(
        app(state.clone()),
        req("/badges/assets/dclcitizen/2d/normal.png"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/png");
    let (status, _, _) = send(app(state), req("/badges/assets/dclcitizen/3d/normal.png")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(&dir);
}
