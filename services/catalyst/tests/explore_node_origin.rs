//! The public explore reads project mirrored asset and jump URLs on the node.
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use catalyst::explore::events::mirror::upsert_event;
use catalyst::explore::places::mirror::{extract_place, upsert_rows, upsert_world};
use catalyst::explore::{ExploreState, Settings, router};
use chrono::{Duration, Utc};
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;

use crate::db;

const EVENT_ID: &str = "9a1e0000-0000-4000-8000-000000000001";
const LOCAL_EVENT_ID: &str = "9a1e0000-0000-4000-8000-000000000003";
const LOCAL_POSTERS: &[&str] = &[
    "/events-assets-099ac00/poster/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.png",
    "/events-assets-099ac00/poster-vertical/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.png",
];
const PLACE_ID: &str = "6b7c1d2e-0000-4000-8000-000000000001";
const MAP_PLACE_ID: &str = "6b7c1d2e-0000-4000-8000-000000000002";
const WORLD: &str = "harbour.dcl.eth";

const POSTER: (&str, &str) = (
    "https://events-assets-099ac00.decentraland.org/poster/poster.webp",
    "/events-assets-099ac00/poster/poster.webp",
);
const VERTICAL: (&str, &str) = (
    "https://events-assets-099ac00.decentraland.org/poster-vertical/vertical.webp",
    "/events-assets-099ac00/poster-vertical/vertical.webp",
);
const JUMP: (&str, &str) = (
    "https://decentraland.org/jump/events?position=-97%2C-99",
    "/jump/events?position=-97%2C-99",
);
const PARCEL: (&str, &str) = (
    "https://peer-ec1.decentraland.org/content/contents/bafyParcel",
    "/content/contents/bafyParcel",
);
const THUMBNAIL: (&str, &str) = (
    "https://worlds-content-server.decentraland.org/contents/bafyWorld",
    "/worlds-content-server/contents/bafyWorld",
);
const MAP: (&str, &str) = (
    "https://api.decentraland.org/v2/map.png?center=11,11",
    "/api/v2/map.png?center=11,11",
);
const PRODUCTION_REFERENCES: &[&str] = &[
    "events-assets-099ac00.decentraland.org",
    "events-assets-099ac00.node.example",
    "peer.decentraland.org",
    "peer-ec1.decentraland.org",
    "worlds-content-server.decentraland.org",
    "api.decentraland.org",
    "decentraland.org/jump",
];

async fn seed(pool: &PgPool) {
    let start = Utc::now() + Duration::hours(1);
    let finish = start + Duration::hours(1);
    assert!(
        upsert_event(
            pool,
            &json!({
                "id": EVENT_ID,
                "name": "Mirrored event",
                "approved": true,
                "start_at": start,
                "finish_at": finish,
                "next_start_at": start,
                "next_finish_at": finish,
                "image": POSTER.0,
                "image_vertical": VERTICAL.0,
                "url": JUMP.0,
                "x": -97,
                "y": -99,
            }),
        )
        .await
        .expect("seed event")
    );

    assert!(
        upsert_event(pool, &json!({
            "id": LOCAL_EVENT_ID,
            "name": "Uploaded local event",
            "approved": true,
            "start_at": start,
            "finish_at": finish,
            "next_start_at": start,
            "next_finish_at": finish,
            "image": "/poster/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.png",
            "image_vertical": "/poster-vertical/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.png",
            "x": -97,
            "y": -99,
        })).await.expect("seed event with local posters")
    );
    sqlx::query("UPDATE explore.event SET origin='local' WHERE id=$1")
        .bind(LOCAL_EVENT_ID)
        .execute(pool)
        .await
        .expect("adopt local event");

    let places = [
        (PLACE_ID, "-97,-99", PARCEL.0),
        (MAP_PLACE_ID, "11,11", MAP.0),
    ]
    .map(|(id, position, image)| {
        extract_place(&json!({
            "id": id,
            "title": "Mirrored place",
                "owner": "0x00000000000000000000000000000000000000bb",
                "highlighted": id == MAP_PLACE_ID,
            "base_position": position,
            "positions": [position],
            "image": image,
        }))
        .expect("valid place fixture")
    });
    upsert_rows(pool, &places).await.expect("seed places");

    assert!(
        upsert_world(
            pool,
            &json!({
                "id": WORLD,
                "world_name": WORLD,
                "title": "Mirrored world",
                "owner": "0x00000000000000000000000000000000000000bb",
                "show_in_places": true,
                "image": THUMBNAIL.0,
            }),
        )
        .await
        .expect("seed world")
    );
}

async fn check_explore_reads(public_url: Option<&str>, rewrite_upstream: bool) {
    let Some(database) = db::scratch_database().await else {
        return;
    };
    seed(&database.pool).await;
    let rewrite_base = Settings::asset_rewrite_base_for(public_url);
    assert_eq!(
        rewrite_base.as_deref(),
        public_url.filter(|_| rewrite_upstream)
    );
    let state = Arc::new(ExploreState::new(
        database.pool.clone(),
        Settings {
            asset_rewrite_base: rewrite_base.clone(),
            configured_public_base: Settings::configured_public_base_for(public_url),
            ..Settings::for_tests()
        },
        None,
    ));
    let app = router(state.clone()).nest("/places", router(state));
    let event = format!("/api/events/{EVENT_ID}");
    let place = format!("/api/places/{PLACE_ID}");
    let events = &[POSTER, VERTICAL, JUMP];
    let routes: [(&str, &[(&str, &str)]); 7] = [
        ("/api/events", events),
        ("/v1/events", events),
        (&event, events),
        ("/api/places", &[PARCEL, MAP]),
        (&place, &[PARCEL]),
        ("/api/worlds", &[THUMBNAIL]),
        ("/places/api/destinations", &[PARCEL, MAP, THUMBNAIL]),
    ];

    for (route, urls) in routes {
        let response = app
            .clone()
            .oneshot(Request::get(route).body(Body::empty()).unwrap())
            .await
            .expect("route response");
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .expect("route body");
        let body = std::str::from_utf8(&bytes).expect("UTF-8 response");
        assert_eq!(status, StatusCode::OK, "{route}: {body}");

        for &(upstream, path) in urls {
            let expected = rewrite_base
                .as_deref()
                .map(|origin| format!("{origin}{path}"))
                .unwrap_or_else(|| upstream.to_owned());
            assert!(
                body.contains(&expected),
                "{route}: expected {expected}: {body}"
            );
        }
        if matches!(route, "/api/events" | "/v1/events") {
            for path in LOCAL_POSTERS {
                let expected = format!("{}{path}", public_url.unwrap_or(""));
                let url = serde_json::to_string(&expected).unwrap();
                assert!(body.contains(&url), "{route}: expected {url}: {body}");
            }
        }
        if rewrite_base.is_some() {
            for reference in PRODUCTION_REFERENCES {
                assert!(
                    !body.contains(reference),
                    "{route}: unexpected {reference}: {body}"
                );
            }
        }
    }

    drop(app);
    database.teardown().await;
}

#[tokio::test]
async fn no_answer_names_a_production_asset_host() {
    check_explore_reads(Some("https://node.example"), true).await;
}

#[tokio::test]
async fn answers_keep_stored_urls_without_a_public_url() {
    check_explore_reads(None, false).await;
}

#[tokio::test]
async fn answers_keep_the_public_prefix_and_port() {
    check_explore_reads(Some("https://node.example:8443/prefix"), true).await;
}

#[tokio::test]
async fn decentraland_subdomain_projects_only_local_posters() {
    check_explore_reads(Some("https://events.decentraland.org"), false).await;
}
