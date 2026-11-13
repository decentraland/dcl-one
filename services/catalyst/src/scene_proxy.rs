//! The scene workers' way to the third-party services scenes call.
//!
//! A scene in the browser explorer runs in a worker on the page's origin, and
//! the services scenes call (the fishing game's Colyseus room, the marketing
//! files bucket) answer no CORS preflight, so the browser refuses the calls.
//! The worker's fetch (`deploy/web/engine/scene_fetch.js`) reads `GET /_scene`
//! once, a map of host to a path prefix on this node, and sends what the map
//! names through this node instead: the node's own `/places` and
//! `/marketplace-api` routes stand in for those services, and every host in
//! `SCENE_PROXY_HOSTS` is forwarded under `/_scene/<host>/...`.
//!
//! The forward is narrow. GET, HEAD and POST only. Of the request, the headers
//! a scene's `fetch` and `signedFetch` carry (the content type, authorization,
//! the `x-identity-*` auth chain, ranges and validators) and nothing else, so
//! neither the page's cookies nor the node's own headers reach the upstream,
//! and a bounded body. Of the answer, its status, the body's type, range and
//! validators, and the body, bounded too; a `set-cookie` or a CORS grant never
//! pass. The upstream is dialled through the guarded client, like every
//! mirror: public hosts only, over pinned DNS.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{any, get};
use futures::StreamExt;
use serde_json::json;

use crate::http::Outbound;

pub type Shared = Arc<SceneProxy>;

/// Upstream services this node serves itself: the prefix stands in for the host.
pub const FIRST_PARTY: &[(&str, &str)] = &[
    ("places.decentraland.org", "/places"),
    ("marketplace-api.decentraland.org", "/marketplace-api"),
];

const REQUEST_HEADERS: &[&str] = &[
    "accept",
    "accept-language",
    "content-type",
    "authorization",
    "range",
    "if-none-match",
    "if-modified-since",
];
const IDENTITY_HEADER_PREFIX: &str = "x-identity-";
const RESPONSE_HEADERS: &[&str] = &[
    "content-type",
    "content-length",
    "content-range",
    "accept-ranges",
    "cache-control",
    "etag",
    "last-modified",
    "vary",
];
pub const REQUEST_BODY_LIMIT: usize = 1 << 20;
pub const RESPONSE_BODY_LIMIT: usize = 32 << 20;
const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(20);

pub struct SceneProxy {
    hosts: Vec<String>,
    http: Outbound,
    scheme: &'static str,
}

impl SceneProxy {
    pub fn new(hosts: impl IntoIterator<Item = String>, http: Outbound) -> Self {
        let mut hosts: Vec<String> = hosts
            .into_iter()
            .map(|host| host.trim().to_ascii_lowercase())
            .filter(|host| !host.is_empty())
            .collect();
        hosts.sort();
        hosts.dedup();
        Self {
            hosts,
            http,
            scheme: "https",
        }
    }

    #[cfg(test)]
    fn over_plain_http(mut self) -> Self {
        self.scheme = "http";
        self
    }

    pub fn hosts(&self) -> &[String] {
        &self.hosts
    }

    /// The map the workers read: host to the prefix on this node that stands in for it.
    pub fn routes(&self) -> BTreeMap<String, String> {
        let mut routes: BTreeMap<String, String> = FIRST_PARTY
            .iter()
            .map(|(host, prefix)| (host.to_string(), prefix.to_string()))
            .collect();
        for host in &self.hosts {
            routes.insert(host.clone(), format!("/_scene/{host}"));
        }
        routes
    }

    fn allows(&self, host: &str) -> bool {
        self.hosts.iter().any(|allowed| allowed == host)
    }
}

pub fn router(state: Shared) -> Router {
    Router::new()
        .route("/", get(routing_map))
        .route("/{host}/{*path}", any(forward))
        .with_state(state)
}

async fn routing_map(State(state): State<Shared>) -> Response {
    Json(json!({ "routes": state.routes() })).into_response()
}

async fn forward(
    State(state): State<Shared>,
    Path((host, path)): Path<(String, String)>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let host = host.to_ascii_lowercase();
    if !state.allows(&host) {
        return (StatusCode::NOT_FOUND, "no such scene service").into_response();
    }
    if !matches!(method.as_str(), "GET" | "HEAD" | "POST") {
        return (StatusCode::METHOD_NOT_ALLOWED, "GET, HEAD or POST").into_response();
    }
    let method = reqwest::Method::from_bytes(method.as_str().as_bytes()).expect("one of three");
    let body = match axum::body::to_bytes(body, REQUEST_BODY_LIMIT).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return (StatusCode::PAYLOAD_TOO_LARGE, "request body over the limit").into_response();
        }
    };
    let query = uri.query().map(|q| format!("?{q}")).unwrap_or_default();
    let url = format!("{}://{host}/{path}{query}", state.scheme);
    let mut request = state
        .http
        .request(method, &url)
        .timeout(UPSTREAM_TIMEOUT)
        .headers(forwarded(&headers));
    if !body.is_empty() {
        request = request.body(body);
    }
    let upstream = match request.send().await {
        Ok(upstream) => upstream,
        Err(e) => {
            tracing::warn!(host, "scene proxy: the upstream did not answer: {e}");
            return (StatusCode::BAD_GATEWAY, "the scene service did not answer").into_response();
        }
    };
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut head = HeaderMap::new();
    for name in RESPONSE_HEADERS {
        for value in upstream.headers().get_all(*name) {
            if let Ok(value) = HeaderValue::from_bytes(value.as_bytes()) {
                head.append(HeaderName::from_static(name), value);
            }
        }
    }
    let mut seen = 0usize;
    let body = Body::from_stream(upstream.bytes_stream().map(move |chunk| match chunk {
        Ok(bytes) => {
            seen += bytes.len();
            if seen > RESPONSE_BODY_LIMIT {
                Err(std::io::Error::other("scene service answer over the limit"))
            } else {
                Ok(bytes)
            }
        }
        Err(e) => Err(std::io::Error::other(e.without_url())),
    }));
    (status, head, body).into_response()
}

/// The request headers that go upstream: the allow-list and the signed-fetch
/// identity headers, each copied by value so nothing else of the request leaks.
fn forwarded(headers: &HeaderMap) -> reqwest::header::HeaderMap {
    let mut out = reqwest::header::HeaderMap::new();
    for (name, value) in headers {
        let name = name.as_str();
        if !(REQUEST_HEADERS.contains(&name) || name.starts_with(IDENTITY_HEADER_PREFIX)) {
            continue;
        }
        if let (Ok(name), Ok(value)) = (
            reqwest::header::HeaderName::from_bytes(name.as_bytes()),
            reqwest::header::HeaderValue::from_bytes(value.as_bytes()),
        ) {
            out.append(name, value);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use axum::body::to_bytes;
    use axum::http::Request;
    use tower::ServiceExt;

    use super::*;

    struct Seen {
        method: String,
        path: String,
        headers: Vec<String>,
        body: String,
    }

    /// A loopback upstream recording what reaches it and answering with headers a
    /// scene must not see beside the ones it should.
    async fn stub() -> (String, Arc<Mutex<Vec<Seen>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let record = seen.clone();
        let app = Router::new().route(
            "/{*path}",
            any(
                move |method: Method, uri: Uri, headers: HeaderMap, body: String| {
                    let record = record.clone();
                    async move {
                        record.lock().unwrap().push(Seen {
                            method: method.to_string(),
                            path: uri
                                .path_and_query()
                                .map(|p| p.to_string())
                                .unwrap_or_default(),
                            headers: headers.keys().map(|k| k.to_string()).collect(),
                            body,
                        });
                        (
                            StatusCode::CREATED,
                            [
                                ("content-type", "application/json"),
                                ("cache-control", "max-age=60"),
                                ("set-cookie", "upstream=1"),
                                ("access-control-allow-origin", "*"),
                            ],
                            r#"{"ok":true}"#,
                        )
                    }
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (addr.to_string(), seen)
    }

    fn app(hosts: &[&str]) -> Router {
        let proxy = SceneProxy::new(
            hosts.iter().map(|host| host.to_string()),
            Outbound::for_tests(),
        )
        .over_plain_http();
        Router::new().nest("/_scene", router(Arc::new(proxy)))
    }

    async fn text(response: Response) -> String {
        String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn the_map_names_the_nodes_own_routes_and_the_allow_listed_hosts() {
        let app = app(&[
            "us-mia-43272b05.colyseus.cloud",
            " Marketing-Files.decentraland.org ",
            "",
        ]);
        let response = app
            .oneshot(Request::get("/_scene").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let map: serde_json::Value = serde_json::from_str(&text(response).await).unwrap();
        let routes = map["routes"].as_object().unwrap();
        assert_eq!(routes["places.decentraland.org"], "/places");
        assert_eq!(
            routes["marketplace-api.decentraland.org"],
            "/marketplace-api"
        );
        assert_eq!(
            routes["us-mia-43272b05.colyseus.cloud"],
            "/_scene/us-mia-43272b05.colyseus.cloud"
        );
        assert_eq!(
            routes["marketing-files.decentraland.org"],
            "/_scene/marketing-files.decentraland.org"
        );
        assert_eq!(routes.len(), 4);
    }

    #[tokio::test]
    async fn a_host_off_the_list_and_a_method_off_the_three_are_refused() {
        let app = app(&["example.invalid"]);
        let response = app
            .clone()
            .oneshot(
                Request::get("/_scene/other.invalid/x")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let response = app
            .oneshot(
                Request::put("/_scene/example.invalid/x")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn the_forward_carries_the_scenes_headers_and_body_and_nothing_of_the_pages() {
        let (upstream, seen) = stub().await;
        let app = app(&[&upstream]);
        let request = Request::post(format!(
            "/_scene/{upstream}/matchmake/joinOrCreate/fishing_room?x=1"
        ))
        .header("content-type", "application/json")
        .header("x-identity-auth-chain-0", "{}")
        .header("x-identity-timestamp", "1")
        .header("cookie", "session=secret")
        .header("origin", "https://site.example")
        .header("x-forwarded-for", "203.0.113.9")
        .body(Body::from(r#"{"hello":"room"}"#))
        .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/json"
        );
        assert_eq!(
            response.headers().get("cache-control").unwrap(),
            "max-age=60"
        );
        assert!(response.headers().get("set-cookie").is_none());
        assert!(
            response
                .headers()
                .get("access-control-allow-origin")
                .is_none()
        );
        assert_eq!(text(response).await, r#"{"ok":true}"#);
        let seen = seen.lock().unwrap();
        let call = &seen[0];
        assert_eq!(call.method, "POST");
        assert_eq!(call.path, "/matchmake/joinOrCreate/fishing_room?x=1");
        assert_eq!(call.body, r#"{"hello":"room"}"#);
        for carried in [
            "content-type",
            "x-identity-auth-chain-0",
            "x-identity-timestamp",
        ] {
            assert!(
                call.headers.iter().any(|h| h == carried),
                "{carried} missing: {:?}",
                call.headers
            );
        }
        for kept_here in ["cookie", "origin", "x-forwarded-for"] {
            assert!(
                !call.headers.iter().any(|h| h == kept_here),
                "{kept_here} reached the upstream"
            );
        }
    }

    #[tokio::test]
    async fn a_body_over_the_limit_is_refused_before_anything_leaves() {
        let (upstream, seen) = stub().await;
        let app = app(&[&upstream]);
        let response = app
            .oneshot(
                Request::post(format!("/_scene/{upstream}/x"))
                    .body(Body::from(vec![b'a'; REQUEST_BODY_LIMIT + 1]))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert!(seen.lock().unwrap().is_empty());
    }
}
