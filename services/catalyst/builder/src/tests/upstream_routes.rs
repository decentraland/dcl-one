//! The route ledger: every route `github.com/decentraland/builder-server`
//! registers under `/v1` at `98897af692f26063a088aebb2ea5bb3f683168a2`, and
//! whether this crate's router answers its method and path. A `Routed` route
//! reaches a handler, which says nothing about matching upstream's
//! behaviour; an `Unrouted` one is inside the port but this router doesn't
//! answer it; a `Removed` route is outside the port and stays unanswered:
//! the node keeps no scenes (projects, manifests, templates, deployments,
//! asset packs, assets and their files), analytics or NFT reads.
//! [`SCENE_ROUTES`] lists the scene routes the node itself added beyond
//! upstream's, which stay unanswered under both mounts.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use sqlx::postgres::PgPoolOptions;

use super::support::temp_storage;
use crate::shared::test_http::call;
use crate::{Settings, State};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Route {
    Routed,
    Unrouted,
    Removed,
}
use Route::{Removed, Routed, Unrouted};

pub(super) const UPSTREAM: &[(&str, &str, Route)] = &[
    ("GET", "/info", Routed),
    ("GET", "/assetPacks", Removed),
    ("GET", "/assetPacks/:id", Removed),
    ("GET", "/assets/:id", Removed),
    ("GET", "/assets", Removed),
    ("GET", "/templates", Removed),
    ("GET", "/projects", Removed),
    ("GET", "/projects/:id", Removed),
    ("DELETE", "/projects/:id", Removed),
    ("DELETE", "/projects/:coords/coords", Removed),
    ("GET", "/projects/:id/public", Removed),
    ("GET", "/projects/:id/media/:filename", Removed),
    ("GET", "/projects/:id/contents/:content", Removed),
    ("GET", "/projects/:id/about", Removed),
    ("GET", "/projects/:id/crdt", Removed),
    ("DELETE", "/projects_by_coords/:coords/coords", Removed),
    ("GET", "/items", Routed),
    ("GET", "/:address/items", Routed),
    ("GET", "/items/:id", Routed),
    (
        "GET",
        "/published-collections/:address/items/:id/utility",
        Routed,
    ),
    ("GET", "/collections/:id/items", Routed),
    ("PUT", "/items/:idOrURN", Routed),
    ("DELETE", "/items/:id", Routed),
    ("POST", "/items/:id/files", Routed),
    ("POST", "/items/:id/videos", Routed),
    ("GET", "/items/:collectionAddress/:itemId/contents", Routed),
    ("GET", "/collections", Routed),
    ("GET", "/:address/collections", Routed),
    ("GET", "/collections/:id", Routed),
    ("POST", "/collections/:id/publish", Routed),
    ("POST", "/collections/:id/owner/sync", Routed),
    ("POST", "/collections/:id/tos", Routed),
    ("POST", "/collections/:id/lock", Routed),
    ("GET", "/collections/:id/approvalData", Routed),
    ("PUT", "/collections/:id", Routed),
    ("DELETE", "/collections/:id", Routed),
    ("GET", "/addresses", Routed),
    ("GET", "/curations", Unrouted),
    ("GET", "/collections/:id/itemCurations", Unrouted),
    ("GET", "/collections/:id/curation", Unrouted),
    ("PATCH", "/collections/:id/curation", Unrouted),
    ("POST", "/collections/:id/curation", Unrouted),
    ("POST", "/collections/:id/curation/post", Routed),
    ("GET", "/items/:id/curation", Unrouted),
    ("PATCH", "/items/:id/curation", Unrouted),
    ("POST", "/items/:id/curation", Unrouted),
    ("POST", "/collections/:id/validations", Unrouted),
    ("POST", "/collections/:id/curation/appeal", Unrouted),
    ("GET", "/collections/:id/events", Unrouted),
    ("POST", "/collections/:id/comments", Unrouted),
    ("POST", "/collections/:id/validation-result", Unrouted),
    ("GET", "/committee", Routed),
    ("GET", "/thirdParties", Routed),
    ("GET", "/thirdParties/:id/slots", Routed),
    ("GET", "/thirdParties/:id", Routed),
    ("DELETE", "/thirdParties/:id", Routed),
    ("PATCH", "/thirdParties/:id", Routed),
    ("GET", "/rarities", Routed),
    ("GET", "/rarities/:name", Routed),
    ("POST", "/collections/:id/post", Routed),
    ("GET", "/projects/:id/manifest", Removed),
    ("GET", "/manifests", Removed),
    ("GET", "/publics/:id/manifest", Removed),
    ("GET", "/templates/:id/manifest", Removed),
    ("DELETE", "/projects/:id/manifest", Removed),
    ("GET", "/deployments", Removed),
    ("GET", "/projects/:id/deployment", Removed),
    ("DELETE", "/projects/:id/deployment", Removed),
    ("GET", "/storage/assetPacks/:filename", Removed),
    ("GET", "/storage/contents/:filename", Routed),
    ("HEAD", "/storage/contents/:filename", Routed),
    ("GET", "/storage/contents/:filename/exists", Routed),
    ("GET", "/analytics/weekly", Removed),
    ("GET", "/analytics/status", Removed),
    ("GET", "/nfts", Removed),
    ("GET", "/nfts/:contractAddress/:tokenId", Removed),
    ("POST", "/newsletter", Routed),
];

const PARAMETER: &str = "6f1c0a52-5d0e-4c4b-9c55-0d6a1a3b7e21";

const SCENE_ROUTES: &[(&str, &str)] = &[
    ("PUT", "/v1/projects/:id"),
    ("PUT", "/v1/projects/:id/manifest"),
    ("GET", "/v1/projects/:id/files"),
    ("PUT", "/v1/projects/:id/files"),
    ("PUT", "/v1/projects/:id/blobs/:hash"),
    ("GET", "/v1/projects/:id/assetPacks"),
    ("PUT", "/v1/projects/:id/assetPacks/:pack"),
    ("DELETE", "/v1/projects/:id/assetPacks/:pack"),
    ("GET", "/contents/:hash"),
];

fn filled(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            if segment.starts_with(':') {
                PARAMETER
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn request_path(upstream: &str) -> String {
    format!("/v1{}", filled(upstream))
}

async fn app() -> Router {
    let pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(100))
        .connect_lazy("postgres://127.0.0.1:1/route_ledger")
        .unwrap();
    let state = Arc::new(
        State::new(
            pool,
            temp_storage().await,
            Arc::default(),
            dcl_http::Outbound::for_tests(),
            None,
            None,
            Settings {
                writes_enabled: true,
                ..Settings::default()
            },
        )
        .unwrap(),
    );
    Router::new()
        .merge(crate::root_router(state.clone()))
        .nest("/builder-api", crate::router(state))
        .fallback(|| async { StatusCode::IM_A_TEAPOT })
}

async fn answers(app: &Router, method: &str, path: &str) -> (bool, StatusCode) {
    let (status, _) = call(app, method, path).await;
    (
        status != StatusCode::IM_A_TEAPOT && status != StatusCode::METHOD_NOT_ALLOWED,
        status,
    )
}

#[tokio::test]
async fn the_ledger_matches_the_router() {
    let app = app().await;
    let mut disagreements = Vec::new();
    for &(method, path, route) in UPSTREAM {
        let (routed, status) = answers(&app, method, &request_path(path)).await;
        if routed != (route == Routed) {
            disagreements.push(format!(
                "{method} {path}: {route:?} in the ledger, answered {status}"
            ));
        }
    }
    assert!(disagreements.is_empty(), "{}", disagreements.join("\n"));
}

#[tokio::test]
async fn no_scene_route_answers_under_either_mount() {
    let app = app().await;
    let mut answered = Vec::new();
    for mount in ["", "/builder-api"] {
        let removed = UPSTREAM
            .iter()
            .filter(|(_, _, route)| *route == Removed)
            .map(|&(method, path, _)| (method, request_path(path)));
        let node_only = SCENE_ROUTES
            .iter()
            .map(|&(method, path)| (method, filled(path)));
        for (method, path) in removed.chain(node_only) {
            let path = format!("{mount}{path}");
            if let (true, status) = answers(&app, method, &path).await {
                answered.push(format!("{method} {path}: answered {status}"));
            }
        }
    }
    assert!(answered.is_empty(), "{}", answered.join("\n"));
}
