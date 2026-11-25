//! A web page on another origin calls the builder routes, at both mounts as
//! `serve` mounts them: the origins `CORS_ORIGIN` lists (the Decentraland
//! front-ends when it is unset) may read the answers, any other origin may
//! not; a preflight is answered before authentication, the database and the
//! request budgets; a signed request is answered as it would be without an
//! `Origin`, since signatures, not origins, authorize; and the public file
//! route answers every origin, as the Marketplace and players read it.
//! Every node here has a pool that never connects.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request};
use tower::ServiceExt;
use uuid::Uuid;

use super::support::{OWNER_KEY, lazy_pool, temp_storage};

const DAPP: &str = "https://create.decentraland.example";
const APEX: &str = "https://decentraland.example";
const EXACT: &str = "https://partner.example";
const FOREIGN: &str = "https://decentraland.example.attacker.example";
const ORIGINS: &str =
    r" regex:^https://(.{1,50}\.)?decentraland\.example$ ; https://partner.example ;";
const HASH: &str = "bafkreigh2akiscaildcqabsyg3dfr6chu3fgpregiymsck7e7aqa4s52zy";
const EXPOSED: &str =
    "etag,cache-control,content-language,content-type,expires,last-modified,pragma,retry-after";
const METHODS: &str = "GET,HEAD,POST,PUT,PATCH,DELETE";

fn raw(origin: Option<&str>, method: Option<&str>) -> crate::RawSettings {
    crate::RawSettings {
        writes_enabled: true,
        cors_origin: origin.map(str::to_string),
        cors_method: method.map(str::to_string),
        ..crate::RawSettings::default()
    }
}

async fn app_with(settings: crate::Settings, tune: impl FnOnce(&mut crate::State)) -> Router {
    let mut state = crate::State::new(
        lazy_pool(),
        temp_storage().await,
        Arc::default(),
        dcl_http::Outbound::for_tests(),
        None,
        None,
        settings,
    )
    .unwrap();
    tune(&mut state);
    let state = Arc::new(state);
    Router::new()
        .merge(crate::root_router(state.clone()))
        .nest("/builder-api", crate::router(state))
}

async fn app(origins: Option<&str>) -> Router {
    app_with(crate::Settings::parse(raw(origins, None)).unwrap(), |_| ()).await
}

async fn answer(app: &Router, request: Request<Body>) -> (u16, HeaderMap, String) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (
        status,
        headers,
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

fn request(method: &str, path: &str, origin: Option<&str>) -> Request<Body> {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(origin) = origin {
        request = request.header("origin", origin);
    }
    request.body(Body::empty()).unwrap()
}

fn preflight(path: &str, origin: &str, method: &str, headers: &str) -> Request<Body> {
    Request::builder()
        .method("OPTIONS")
        .uri(path)
        .header("origin", origin)
        .header("access-control-request-method", method)
        .header("access-control-request-headers", headers)
        .header("x-real-ip", "203.0.113.9")
        .body(Body::empty())
        .unwrap()
}

fn allowed(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("access-control-allow-origin")
        .map(|v| v.to_str().unwrap())
}

fn varies_by_origin(headers: &HeaderMap) -> bool {
    headers
        .get_all("vary")
        .iter()
        .any(|v| v.to_str().unwrap().to_ascii_lowercase().contains("origin"))
}

fn listed_routes(id: Uuid) -> Vec<(&'static str, String)> {
    vec![
        ("GET", format!("/v1/collections/{id}")),
        ("PUT", format!("/v1/items/{id}")),
        ("POST", "/v1/validator/claims".to_string()),
        ("GET", format!("/v1/storage/contents/{HASH}/exists")),
        ("POST", "/v1/newsletter".to_string()),
    ]
}

#[tokio::test]
async fn a_listed_origin_reads_the_answers_and_any_other_origin_does_not_at_both_mounts() {
    let app = app(Some(ORIGINS)).await;
    let id = Uuid::new_v4();
    for mount in ["", "/builder-api"] {
        for (method, path) in listed_routes(id) {
            let path = format!("{mount}{path}");
            for origin in [DAPP, APEX, EXACT] {
                let (_, headers, _) = answer(&app, request(method, &path, Some(origin))).await;
                assert_eq!(
                    allowed(&headers),
                    Some(origin),
                    "{method} {path} from {origin}"
                );
                assert!(varies_by_origin(&headers), "{method} {path}");
                assert!(headers.get("access-control-allow-credentials").is_none());
                assert_eq!(
                    headers
                        .get("access-control-expose-headers")
                        .map(|v| v.to_str().unwrap().to_ascii_lowercase()),
                    Some(EXPOSED.to_string()),
                    "{method} {path}"
                );
            }
            for origin in [FOREIGN, "https://partner.example.attacker.example", "null"] {
                let (_, headers, _) = answer(&app, request(method, &path, Some(origin))).await;
                assert_eq!(allowed(&headers), None, "{method} {path} from {origin}");
                assert!(varies_by_origin(&headers), "{method} {path}");
            }
        }
    }
}

#[tokio::test]
async fn the_public_file_route_and_info_answer_every_origin() {
    let app = app(Some(ORIGINS)).await;
    for mount in ["", "/builder-api"] {
        for (method, path) in [
            ("GET", format!("{mount}/v1/storage/contents/{HASH}")),
            ("HEAD", format!("{mount}/v1/storage/contents/{HASH}")),
            ("GET", format!("{mount}/v1/info")),
        ] {
            let (_, headers, _) = answer(&app, request(method, &path, Some(FOREIGN))).await;
            assert_eq!(allowed(&headers), Some(FOREIGN), "{method} {path}");
            assert!(headers.get("access-control-allow-credentials").is_none());
            let (_, headers, _) = answer(
                &app,
                preflight(&path, FOREIGN, method, "x-identity-auth-chain-0"),
            )
            .await;
            assert_eq!(allowed(&headers), Some(FOREIGN), "preflight {path}");
        }
    }
}

#[tokio::test]
async fn a_preflight_is_answered_before_authentication_the_database_and_any_budget() {
    let app = app_with(
        crate::Settings::parse(raw(Some(ORIGINS), None)).unwrap(),
        |state| {
            state.limits.source_reads =
                crate::limits::budget(1, std::time::Duration::from_secs(60));
            state.limits.source_writes =
                crate::limits::budget(1, std::time::Duration::from_secs(60));
        },
    )
    .await;
    let id = Uuid::new_v4();
    let signed = "content-type,x-identity-auth-chain-0,x-identity-auth-chain-1,x-identity-auth-chain-9,x-identity-timestamp,x-identity-metadata";
    let validator = "content-type,x-wearable-validator-signature,x-wearable-validator-timestamp";
    for mount in ["", "/builder-api"] {
        for (path, method, headers) in [
            (format!("{mount}/v1/items/{id}"), "PUT", signed),
            (format!("{mount}/v1/collections/{id}"), "GET", signed),
            (format!("{mount}/v1/items/{id}/files"), "POST", signed),
            (format!("{mount}/v1/validator/claims"), "POST", validator),
            (
                format!("{mount}/v1/collections/{id}/validation-result"),
                "POST",
                validator,
            ),
        ] {
            for _ in 0..3 {
                let (status, answered, body) =
                    answer(&app, preflight(&path, DAPP, method, headers)).await;
                assert!((200..300).contains(&status), "{path}: {status} {body}");
                assert_eq!(allowed(&answered), Some(DAPP), "{path}");
                assert_eq!(
                    answered.get("access-control-allow-methods").unwrap(),
                    METHODS
                );
                let allowed_headers = answered
                    .get("access-control-allow-headers")
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_ascii_lowercase();
                for header in headers.split(',') {
                    assert!(
                        allowed_headers.split(',').any(|h| h.trim() == header),
                        "{path}: {header} not in {allowed_headers}"
                    );
                }
                assert!(!allowed_headers.contains('*'), "{allowed_headers}");
            }
            let (status, answered, _) =
                answer(&app, preflight(&path, FOREIGN, method, headers)).await;
            assert!((200..300).contains(&status));
            assert_eq!(allowed(&answered), None, "{path}");
        }
    }
    let read = || {
        Request::builder()
            .uri(format!("/v1/collections/{id}"))
            .header("x-real-ip", "203.0.113.9")
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(
        answer(&app, read()).await.0,
        401,
        "the preflights spent nothing"
    );
    assert_eq!(answer(&app, read()).await.0, 429, "the budget is live");
}

#[tokio::test]
async fn a_signed_request_from_any_origin_is_answered_as_without_one() {
    let app = app(Some(ORIGINS)).await;
    for path in [
        format!("/v1/items/{}", Uuid::new_v4()),
        "/builder-api/v1/drafts/items".to_string(),
    ] {
        let mut answers = Vec::new();
        for origin in [None, Some(DAPP), Some(FOREIGN)] {
            let mut request = super::support::request(Some(OWNER_KEY), "GET", &path).await;
            if let Some(origin) = origin {
                request = request.header("origin", origin);
            }
            let (status, _, body) = answer(&app, request.body(Body::empty()).unwrap()).await;
            answers.push((status, body));
        }
        assert_eq!(answers[0], answers[1], "{path}");
        assert_eq!(answers[0], answers[2], "{path}");
        assert_ne!(answers[0].0, 401, "{path}: {:?}", answers[0]);
    }
}

#[tokio::test]
async fn unset_the_origins_are_the_decentraland_front_ends_and_their_local_development() {
    let app = app(None).await;
    let path = format!("/v1/items/{}", Uuid::new_v4());
    for origin in [
        "https://decentraland.org",
        "https://create.decentraland.zone",
        "http://localhost:5173",
    ] {
        let (_, headers, _) = answer(&app, request("PUT", &path, Some(origin))).await;
        assert_eq!(allowed(&headers), Some(origin), "{origin}");
    }
    for origin in [
        "https://attacker.example",
        "https://decentraland.org.attacker.example",
        "http://localhost.attacker.example:5173",
        "null",
    ] {
        let (_, headers, _) = answer(&app, request("PUT", &path, Some(origin))).await;
        assert_eq!(allowed(&headers), None, "{origin}");
    }
}

#[test]
fn the_settings_are_read_as_builder_server_is_deployed_and_refuse_what_the_node_cannot_honour() {
    let parsed =
        |origin: Option<&str>, method: Option<&str>| crate::Settings::parse(raw(origin, method));
    let any = parsed(Some("*"), None).unwrap();
    assert!(any.cors.allows("https://attacker.example"));
    let listed = parsed(Some(ORIGINS), Some(" get, POST ,head ")).unwrap();
    assert!(listed.cors.allows(DAPP) && listed.cors.allows(EXACT));
    assert!(!listed.cors.allows("https://partner.example/"));
    assert_eq!(listed.cors.methods(), "GET,POST,HEAD");
    assert_eq!(parsed(None, Some("*")).unwrap().cors.methods(), METHODS);
    for (origin, method) in [
        (Some(r"regex:^https://(?=x)decentraland\.example$"), None),
        (Some("regex:("), None),
        (Some(" ; "), None),
        (None, Some("BREW")),
        (None, Some("GET,")),
    ] {
        let error = parsed(origin, method).unwrap_err().to_string();
        assert!(
            error.contains(if origin.is_some() {
                "CORS_ORIGIN"
            } else {
                "CORS_METHOD"
            }),
            "{error}"
        );
    }
}

/// A page on a listed origin reads the refusal and when to try again.
#[tokio::test]
async fn a_listed_origin_reads_the_refusal_of_a_node_whose_writes_are_off() {
    let mut raw = raw(Some(ORIGINS), None);
    raw.writes_enabled = false;
    let app = app_with(crate::Settings::parse(raw).unwrap(), |_| ()).await;
    for path in [
        format!("/v1/items/{}", Uuid::new_v4()),
        format!("/builder-api/v1/items/{}", Uuid::new_v4()),
    ] {
        let (status, headers, _) = answer(&app, request("PUT", &path, Some(DAPP))).await;
        assert_eq!((status, allowed(&headers)), (503, Some(DAPP)), "{path}");
        assert!(headers.contains_key("retry-after"), "{path}");
        assert!(
            headers
                .get("access-control-expose-headers")
                .is_some_and(|v| v.to_str().unwrap().split(',').any(|h| h == "retry-after")),
            "{path}"
        );
    }
}
