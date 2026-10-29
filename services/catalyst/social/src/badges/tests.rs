//! The upstream-parity bodies and the grant/revoke contracts over the router
//! as `serve` mounts it (nested under `/badges`), on the process's scratch
//! database (`CATALYST_TEST_DATABASE_URL`; skipped without it). The seed
//! rows come from the schema; every per-user row is scoped by a fresh
//! address, and the listings are compared on the four seeded ids because a
//! sibling test may hold a throwaway definition at the same time.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use chrono::{DateTime, NaiveDateTime};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use super::{DEFAULT_ASSET_BASE_URL, Settings, SharedState, State, router};
use crate::identity::Address;
use crate::notifications::errors::ApiError;
use crate::notifications::store::test_support::fresh_address;

const T_STARTER: &str = "2024-10-20T00:00:00.250Z";
const T_BRONZE: &str = "2024-10-23T00:00:00.500Z";
const T_BUSINESS: &str = "2024-10-25T00:00:00.750Z";
const SEEDED: [&str; 4] = [
    "open_for_business",
    "decentraland_citizen",
    "walkabout",
    "emotionista",
];

fn ms(rfc3339: &str) -> i64 {
    DateTime::parse_from_rfc3339(rfc3339)
        .unwrap()
        .timestamp_millis()
}

fn naive(rfc3339: &str) -> NaiveDateTime {
    DateTime::parse_from_rfc3339(rfc3339).unwrap().naive_utc()
}

async fn state(settings: Settings) -> Option<SharedState> {
    let pool = crate::db::scratch::pool(4).await?;
    Some(Arc::new(State::new(pool, settings)))
}

async fn seed(pool: &PgPool, addr: &Address) {
    sqlx::query(
        "INSERT INTO social.user_badge_progress \
           (address, badge_id, steps_done, completed_at, last_completed_tier_id) \
         VALUES ($1, 'walkabout', 15000, $2, 'walkabout-bronze'), \
                ($1, 'open_for_business', 1, $3, NULL)",
    )
    .bind(addr)
    .bind(naive(T_STARTER))
    .bind(naive(T_BUSINESS))
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO social.user_achieved_tiers (address, badge_id, tier_id, completed_at) \
         VALUES ($1, 'walkabout', 'walkabout-starter', $2), \
                ($1, 'walkabout', 'walkabout-bronze', $3)",
    )
    .bind(addr)
    .bind(naive(T_STARTER))
    .bind(naive(T_BRONZE))
    .execute(pool)
    .await
    .unwrap();
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

fn walkabout_tier(
    tier_id: &str,
    tier_name: &str,
    description: &str,
    asset: &str,
    steps: i64,
) -> Value {
    json!({
        "tierId": tier_id,
        "tierName": tier_name,
        "description": description,
        "assets": { "2d": { "normal": asset } },
        "criteria": { "steps": steps },
    })
}

fn business_assets(base: &str) -> Value {
    json!({
        "2d": {
            "normal": format!("{base}/assets/open_for_business/2d/normal.png"),
            "hrm": format!("{base}/assets/open_for_business/2d/hrm.png"),
            "baseColor": format!("{base}/assets/open_for_business/2d/baseColor.png")
        },
        "3d": {
            "normal": format!("{base}/assets/open_for_business/3d/normal.png"),
            "hrm": format!("{base}/assets/open_for_business/3d/hrm.png"),
            "baseColor": format!("{base}/assets/open_for_business/3d/baseColor.png")
        }
    })
}

fn simple_assets(slug: &str) -> Value {
    json!({
        "2d": { "normal": format!("https://badges.decentraland.org/assets/{slug}/2d/normal.png"), "hrm": "", "baseColor": "" },
        "3d": {
            "normal": format!("https://badges.decentraland.org/assets/{slug}/3d/normal.png"),
            "hrm": format!("https://badges.decentraland.org/assets/{slug}/3d/hrm.png"),
            "baseColor": format!("https://badges.decentraland.org/assets/{slug}/3d/baseColor.png")
        }
    })
}

fn only_seeded(list: &Value) -> Vec<Value> {
    list.as_array()
        .unwrap()
        .iter()
        .filter(|b| SEEDED.contains(&b["id"].as_str().unwrap()))
        .cloned()
        .collect()
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
async fn categories_exact_body() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    let (status, body) = get(app(state), "/badges/categories").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({ "data": { "categories": ["Builder", "Explorer", "Socializer"] } })
    );
}

#[tokio::test]
async fn tiers_bodies_and_404() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    let (status, body) = get(
        app(state.clone()),
        "/badges/badges/decentraland_citizen/tiers",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({ "data": { "tiers": [] } }),
        "a non-tiered badge lists no tiers, not 404"
    );

    let (status, body) = get(app(state.clone()), "/badges/badges/doesnotexist/tiers").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        body,
        json!({ "ok": false, "error": "Badge not found", "message": "Badge not found" })
    );

    let (status, body) = get(app(state), "/badges/badges/walkabout/tiers").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({ "data": { "tiers": [
            walkabout_tier("walkabout-starter", "Starter", "Walk 1,000 meters.", "https://badges.decentraland.org/assets/walkabout/starter/2d/normal.png", 1000),
            walkabout_tier("walkabout-bronze", "Bronze", "Walk 10,000 meters.", "https://badges.decentraland.org/assets/walkabout/bronze/2d/normal.png", 10000),
            walkabout_tier("walkabout-silver", "Silver", "Walk 100,000 meters.", "https://badges.decentraland.org/assets/walkabout/silver/2d/normal.png", 100000),
            walkabout_tier("walkabout-gold", "Gold", "Walk 1,000,000 meters.", "https://badges.decentraland.org/assets/walkabout/gold/2d/normal.png", 1000000),
        ] } })
    );
}

#[tokio::test]
async fn user_badges_full_body_and_epoch_types() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    let addr = fresh_address();
    seed(state.badges.pool(), &addr).await;
    let (status, body) = get(
        app(state.clone()),
        &format!("/badges/users/{addr}/badges?includeNotAchieved=true"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let business = json!({
        "id": "open_for_business",
        "name": "Open for Business",
        "description": "Set up a store and publish a collection.",
        "category": "Builder",
        "isTier": false,
        "completedAt": ms(T_BUSINESS).to_string(),
        "assets": business_assets(DEFAULT_ASSET_BASE_URL),
        "progress": {
            "stepsDone": 1,
            "nextStepsTarget": null,
            "totalStepsTarget": 1,
            "lastCompletedTierAt": null,
            "lastCompletedTierName": null,
            "lastCompletedTierImage": null,
            "achievedTiers": []
        }
    });
    let walkabout = json!({
        "id": "walkabout",
        "name": "Walkabout",
        "description": "Walk a cumulative distance across Decentraland.",
        "category": "Explorer",
        "isTier": true,
        "completedAt": ms(T_STARTER).to_string(),
        "assets": simple_assets("walkabout"),
        "progress": {
            "stepsDone": 15000,
            "nextStepsTarget": 100000,
            "totalStepsTarget": 1000000,
            "lastCompletedTierAt": ms(T_BRONZE),
            "lastCompletedTierName": "Bronze",
            "lastCompletedTierImage": "https://badges.decentraland.org/assets/walkabout/bronze/2d/normal.png",
            "achievedTiers": [
                { "tierId": "walkabout-starter", "completedAt": ms(T_STARTER) },
                { "tierId": "walkabout-bronze", "completedAt": ms(T_BRONZE) }
            ]
        }
    });
    let citizen = json!({
        "id": "decentraland_citizen",
        "name": "Decentraland Citizen",
        "description": "Log in to Decentraland for the first time.",
        "category": "Explorer",
        "isTier": false,
        "completedAt": null,
        "assets": simple_assets("decentraland_citizen"),
        "progress": {
            "stepsDone": 0,
            "nextStepsTarget": 1,
            "totalStepsTarget": 1,
            "lastCompletedTierAt": null,
            "lastCompletedTierName": null,
            "lastCompletedTierImage": null,
            "achievedTiers": []
        }
    });
    let emotionista = json!({
        "id": "emotionista",
        "name": "Emotionista",
        "description": "Play emotes in-world.",
        "category": "Socializer",
        "isTier": true,
        "completedAt": null,
        "assets": simple_assets("emotionista"),
        "progress": {
            "stepsDone": 0,
            "nextStepsTarget": 10,
            "totalStepsTarget": 1000,
            "lastCompletedTierAt": null,
            "lastCompletedTierName": null,
            "lastCompletedTierImage": null,
            "achievedTiers": []
        }
    });
    assert_eq!(
        only_seeded(&body["data"]["achieved"]),
        vec![business, walkabout]
    );
    assert_eq!(
        only_seeded(&body["data"]["notAchieved"]),
        vec![citizen, emotionista]
    );

    let w = body["data"]["achieved"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] == json!("walkabout"))
        .unwrap();
    assert!(w["completedAt"].is_string());
    assert!(w["progress"]["lastCompletedTierAt"].is_i64());
    assert!(w["progress"]["achievedTiers"][0]["completedAt"].is_i64());

    let (status, body) = get(app(state.clone()), &format!("/badges/users/{addr}/badges")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["data"]["notAchieved"],
        json!([]),
        "not achieved is opt-in"
    );
    assert_eq!(body["data"]["achieved"].as_array().unwrap().len(), 2);

    let (status, checksum) = get(
        app(state.clone()),
        &format!(
            "/badges/users/{}/badges?includeNotAchieved=true",
            addr.checksummed()
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, lower) = get(
        app(state.clone()),
        &format!("/badges/users/{addr}/badges?includeNotAchieved=true"),
    )
    .await;
    assert_eq!(lower, checksum, "a checksummed address reads the same rows");

    let (status, body) = get(app(state.clone()), "/badges/users/not-an-address/badges").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({ "data": { "achieved": [], "notAchieved": [] } })
    );
    let (status, body) = get(app(state.clone()), "/badges/users/not-an-address/preview").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "data": { "latestAchievedBadges": [] } }));

    let (status, body) = get(app(state), &format!("/badges/users/{addr}/preview")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({ "data": { "latestAchievedBadges": [
            {
                "id": "open_for_business",
                "name": "Open for Business",
                "tierName": null,
                "image": "https://badges.decentraland.org/assets/open_for_business/2d/normal.png"
            },
            {
                "id": "walkabout",
                "name": "Walkabout",
                "tierName": "Bronze",
                "image": "https://badges.decentraland.org/assets/walkabout/bronze/2d/normal.png"
            }
        ] } })
    );
}

#[tokio::test]
async fn public_asset_base_url_rewrites_every_advertised_url_and_assets_are_served() {
    let Some(state) = state(Settings {
        public_asset_base_url: "https://badges.example.test/".into(),
        ..Settings::default()
    })
    .await
    else {
        return;
    };
    let addr = fresh_address();
    seed(state.badges.pool(), &addr).await;
    let (_, body) = get(app(state.clone()), &format!("/badges/users/{addr}/badges")).await;
    let business = body["data"]["achieved"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] == json!("open_for_business"))
        .unwrap();
    assert_eq!(
        business["assets"],
        business_assets("https://badges.example.test")
    );
    let (_, body) = get(app(state.clone()), &format!("/badges/users/{addr}/preview")).await;
    assert_eq!(
        body["data"]["latestAchievedBadges"][1]["image"],
        json!("https://badges.example.test/assets/walkabout/bronze/2d/normal.png")
    );

    let req = Request::builder()
        .method("GET")
        .uri("/badges/assets/walkabout/bronze/2d/normal.png")
        .body(Body::empty())
        .unwrap();
    let (status, headers, _) = send(app(state.clone()), req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/png");
    let req = Request::builder()
        .method("GET")
        .uri("/badges/assets/walkabout/2d/missing.png")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(app(state), req).await.0, StatusCode::NOT_FOUND);
}

const ACTOR: &str = "tests";

async fn audit(pool: &PgPool, addr: &Address) -> Vec<(String, String, Option<String>, String)> {
    sqlx::query_as("SELECT action, badge_id, tier_id, actor FROM social.badge_admin_audit WHERE address = $1 ORDER BY id")
        .bind(addr)
        .fetch_all(pool)
        .await
        .unwrap()
}

async fn progress(
    pool: &PgPool,
    addr: &Address,
    badge_id: &str,
) -> Option<(i32, Option<NaiveDateTime>, Option<String>, Option<String>)> {
    sqlx::query_as(
        "SELECT steps_done, completed_at, last_completed_tier_id, granted_by \
         FROM social.user_badge_progress WHERE address = $1 AND badge_id = $2",
    )
    .bind(addr)
    .bind(badge_id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn achieved(pool: &PgPool, addr: &Address, badge_id: &str) -> Vec<(String, Option<String>)> {
    sqlx::query_as(
        "SELECT tier_id, granted_by FROM social.user_achieved_tiers \
         WHERE address = $1 AND badge_id = $2 ORDER BY completed_at, tier_id",
    )
    .bind(addr)
    .bind(badge_id)
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
async fn unknown_badge_is_false_and_writes_nothing() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    let addr = fresh_address();
    assert!(
        !state
            .badges
            .grant_badge(&addr, "doesnotexist", None, ACTOR)
            .await
            .unwrap()
    );
    assert!(
        !state
            .badges
            .revoke_badge(&addr, "doesnotexist", ACTOR)
            .await
            .unwrap()
    );
    assert!(audit(state.badges.pool(), &addr).await.is_empty());
}

#[tokio::test]
async fn plain_badge_grant_is_idempotent_and_audited() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    let (pool, addr) = (state.badges.pool().clone(), fresh_address());
    assert!(
        state
            .badges
            .grant_badge(&addr, "open_for_business", Some("ignored-on-plain"), ACTOR)
            .await
            .unwrap()
    );
    let first = progress(&pool, &addr, "open_for_business")
        .await
        .expect("progress row");
    assert_eq!(first.0, 1);
    assert!(first.1.is_some());
    assert_eq!(first.2, None);
    assert_eq!(first.3.as_deref(), Some(ACTOR));
    assert!(achieved(&pool, &addr, "open_for_business").await.is_empty());

    assert!(
        state
            .badges
            .grant_badge(&addr, "open_for_business", None, "again")
            .await
            .unwrap()
    );
    let second = progress(&pool, &addr, "open_for_business").await.unwrap();
    assert_eq!((second.0, second.1, second.2), (1, first.1, None));
    assert_eq!(second.3.as_deref(), Some("again"));

    assert_eq!(
        audit(&pool, &addr).await,
        vec![
            (
                "grant".into(),
                "open_for_business".into(),
                Some("ignored-on-plain".into()),
                ACTOR.into()
            ),
            (
                "grant".into(),
                "open_for_business".into(),
                None,
                "again".into()
            ),
        ]
    );
    let (ach, _) = state.badges.user_badges(addr.as_str(), true).await.unwrap();
    assert_eq!(
        ach.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(),
        vec!["open_for_business"]
    );
}

#[tokio::test]
async fn tiered_grant_resolves_the_top_tier_or_the_requested_one() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    let (pool, addr) = (state.badges.pool().clone(), fresh_address());

    assert!(
        state
            .badges
            .grant_badge(&addr, "walkabout", None, ACTOR)
            .await
            .unwrap()
    );
    let p = progress(&pool, &addr, "walkabout").await.unwrap();
    assert_eq!(
        (p.0, p.2.as_deref()),
        (1_000_000, Some("walkabout-gold")),
        "no tierId grants the highest ordinal"
    );
    assert_eq!(
        achieved(&pool, &addr, "walkabout").await,
        vec![("walkabout-gold".to_string(), Some(ACTOR.to_string()))]
    );

    assert!(
        state
            .badges
            .grant_badge(&addr, "walkabout", Some("walkabout-starter"), "later")
            .await
            .unwrap()
    );
    let p = progress(&pool, &addr, "walkabout").await.unwrap();
    assert_eq!(
        (p.0, p.2.as_deref(), p.3.as_deref()),
        (1_000_000, Some("walkabout-starter"), Some("later")),
        "steps never drop; the last granted tier wins"
    );
    assert_eq!(
        achieved(&pool, &addr, "walkabout")
            .await
            .into_iter()
            .map(|(t, _)| t)
            .collect::<Vec<_>>(),
        vec![
            "walkabout-gold".to_string(),
            "walkabout-starter".to_string()
        ]
    );

    let err = state
        .badges
        .grant_badge(&addr, "walkabout", Some("walkabout-platinum"), ACTOR)
        .await
        .unwrap_err();
    assert_eq!(status_of(&err), 404);
    assert!(err.to_string().contains("walkabout-platinum"));

    let lonely = format!("lonely-{}", Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO social.badge_definitions (id, name, is_tier) VALUES ($1, 'Lonely', true)",
    )
    .bind(&lonely)
    .execute(&pool)
    .await
    .unwrap();
    let err = state
        .badges
        .grant_badge(&addr, &lonely, None, ACTOR)
        .await
        .unwrap_err();
    assert_eq!(status_of(&err), 400);
    assert!(progress(&pool, &addr, &lonely).await.is_none());
    sqlx::query("DELETE FROM social.badge_definitions WHERE id = $1")
        .bind(&lonely)
        .execute(&pool)
        .await
        .unwrap();
    state.badges.invalidate_catalog();

    assert_eq!(
        audit(&pool, &addr)
            .await
            .into_iter()
            .map(|(a, b, t, _)| (a, b, t))
            .collect::<Vec<_>>(),
        vec![
            ("grant".to_string(), "walkabout".to_string(), None),
            (
                "grant".to_string(),
                "walkabout".to_string(),
                Some("walkabout-starter".to_string())
            ),
        ],
        "refused grants leave no audit row"
    );

    let (ach, _) = state
        .badges
        .user_badges(addr.as_str(), false)
        .await
        .unwrap();
    let walkabout = ach.iter().find(|b| b.id == "walkabout").expect("achieved");
    assert_eq!(
        walkabout
            .progress
            .achieved_tiers
            .iter()
            .map(|t| t.tier_id.as_str())
            .collect::<Vec<_>>(),
        vec!["walkabout-gold", "walkabout-starter"]
    );
    let latest = state
        .badges
        .latest_achieved(addr.as_str(), 5)
        .await
        .unwrap();
    assert_eq!(latest[0].id, "walkabout");
    assert_eq!(latest[0].tier_name.as_deref(), Some("Starter"));
}

#[tokio::test]
async fn revoke_clears_both_tables_and_audits_once() {
    let Some(state) = state(Settings::default()).await else {
        return;
    };
    let (pool, addr) = (state.badges.pool().clone(), fresh_address());
    state
        .badges
        .grant_badge(&addr, "walkabout", Some("walkabout-bronze"), ACTOR)
        .await
        .unwrap();
    state
        .badges
        .grant_badge(&addr, "open_for_business", None, ACTOR)
        .await
        .unwrap();

    assert!(
        state
            .badges
            .revoke_badge(&addr, "walkabout", "mod")
            .await
            .unwrap()
    );
    assert!(achieved(&pool, &addr, "walkabout").await.is_empty());
    assert!(progress(&pool, &addr, "walkabout").await.is_none());
    assert!(
        progress(&pool, &addr, "open_for_business").await.is_some(),
        "other badges untouched"
    );
    assert!(
        state
            .badges
            .revoke_badge(&addr, "walkabout", "mod")
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
        ("revoke", "walkabout", None, "mod")
    );
    let (ach, _) = state
        .badges
        .user_badges(addr.as_str(), false)
        .await
        .unwrap();
    assert_eq!(
        ach.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(),
        vec!["open_for_business"]
    );
}

#[tokio::test]
async fn the_mutation_routes_are_bearer_gated_and_audit_the_service_token() {
    let Some(unset) = state(Settings::default()).await else {
        return;
    };
    let addr = fresh_address();
    let path = format!(
        "/badges/users/{}/badges/open_for_business",
        addr.checksummed()
    );
    let request = |method: &str, authorization: Option<&str>, body: Option<Value>| {
        let mut req = Request::builder().method(method).uri(&path);
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
    for presented in [Some("Bearer anything"), None] {
        let (status, _, body) = send(app(unset.clone()), request("POST", presented, None)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(
            body,
            json!({ "ok": false, "error": "admin token not configured", "message": "admin token not configured" })
        );
    }

    let state = state(Settings {
        admin_token: Some("s3cret".into()),
        ..Settings::default()
    })
    .await
    .unwrap();
    for presented in [None, Some("Bearer wrong"), Some("Basic s3cret")] {
        let (status, _, body) = send(app(state.clone()), request("DELETE", presented, None)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["error"], json!("missing or invalid bearer token"));
    }
    let (status, _, body) = send(
        app(state.clone()),
        request(
            "POST",
            Some("Bearer s3cret"),
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

    let (status, _, body) = send(
        app(state.clone()),
        request("DELETE", Some("Bearer s3cret"), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({ "data": { "revoked": true, "address": addr.as_str(), "badgeId": "open_for_business" } })
    );

    let bad = "/badges/users/not-an-address/badges/open_for_business";
    let req = Request::builder()
        .method("POST")
        .uri(bad)
        .header(header::AUTHORIZATION, "Bearer s3cret")
        .body(Body::empty())
        .unwrap();
    let (status, _, body) = send(app(state.clone()), req).await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("invalid address"))
    );
    let missing = format!("/badges/users/{addr}/badges/doesnotexist");
    let req = Request::builder()
        .method("POST")
        .uri(&missing)
        .header(header::AUTHORIZATION, "Bearer s3cret")
        .body(Body::empty())
        .unwrap();
    let (status, _, body) = send(app(state), req).await;
    assert_eq!(
        (status, body["error"].as_str()),
        (
            StatusCode::NOT_FOUND,
            Some("no badge found with id: doesnotexist")
        )
    );
}
