//! Stats HTTP surface. Root merging claims only statistics paths; the standalone
//! router owns its complete fallback, methods and CORS contract.
use super::{Reader, content, render};
use axum::{
    Router,
    body::Body,
    extract::{Request, State as AxumState},
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::Response,
};
use serde::{Serialize, ser::SerializeMap};
use std::sync::Arc;

#[derive(Clone)]
pub struct Settings {
    /// Host-resolved commit identity; standalone callers can retain the environment reader.
    pub commit_hash: Option<String>,
    /// Base of `/hot-scenes` thumbnails. `serve` sets it from `CONTENT_URL` or this
    /// node's own `/content` (`stats_content_url`); the default serves tests.
    /// Never ends in `/`: thumbnails append `/contents/<hash>` literally.
    pub content_url: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            commit_hash: None,
            content_url: "https://peer.decentraland.org/content".into(),
        }
    }
}
/// Lifecycle and named probe results from the running host, never inferred from peers.
/// The host maps rejected probes to false and supplies their registration order.
#[derive(Clone, Default)]
pub struct Lifecycle {
    pub initialized: bool,
    pub live: bool,
    pub started: bool,
    pub checks: Vec<(String, bool)>,
}
#[async_trait::async_trait]
pub trait Hooks: Send + Sync {
    async fn probe(&self, startup: bool) -> Lifecycle;
    /// Existing dcl-one `/stats/health` diagnostics, supplied by the connector host.
    async fn diagnostics(&self) -> Result<String, content::Error>;
}
#[derive(Clone)]
pub struct State {
    pub reader: Arc<Reader>,
    pub content: Arc<dyn content::Source>,
    pub hooks: Arc<dyn Hooks>,
    pub settings: Settings,
    pub now: Arc<dyn Fn() -> i64 + Send + Sync>,
    pub environment: Arc<dyn Fn() -> (String, String) + Send + Sync>,
}
impl State {
    pub fn new(
        reader: Arc<Reader>,
        content: Arc<dyn content::Source>,
        hooks: Arc<dyn Hooks>,
        settings: Settings,
    ) -> Self {
        Self {
            reader,
            content,
            hooks,
            settings,
            now: Arc::new(|| chrono::Utc::now().timestamp_millis()),
            environment: Arc::new(|| {
                (
                    std::env::var("CURRENT_VERSION").unwrap_or_default(),
                    std::env::var("COMMIT_HASH").unwrap_or_default(),
                )
            }),
        }
    }
}
pub fn standalone(state: State) -> Router {
    Router::new()
        .fallback(crate::shared::routing::erase(dispatch))
        .with_state(state)
}
/// Ordinary requests to the socket alias retain the standalone host's fallback.
pub fn socket_alias(router: Router, state: State) -> Router {
    router.layer(middleware::from_fn(move |request: Request, next: Next| {
        let state = state.clone();
        async move {
            let upgrade = request.method() == axum::http::Method::GET
                && request
                    .headers()
                    .get("upgrade")
                    .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"websocket"));
            if request.uri().path() == "/ws" && !upgrade {
                dispatch_scoped(state, request, false).await
            } else {
                next.run(request).await
            }
        }
    }))
}
/// Install the complete public hosts outside shared path normalization and middleware.
pub fn mount_services(shared: Router, state: State, stats: Router, connector: Router) -> Router {
    // Camera Reel keeps its route spelling, including the Swagger UI directory.
    let camera_reel = shared.clone();
    let shared = tower_http::normalize_path::NormalizePath::trim_trailing_slash(shared);
    let assembled = merge_root(
        Router::new()
            .fallback_service(shared)
            .nest(
                "/archipelago-ea-stats",
                dcl_metrics::http::component(stats, "stats"),
            )
            .nest(
                "/archipelago-ea-ws-connector",
                dcl_metrics::http::component(connector, "connector"),
            )
            .layer(middleware::from_fn(move |request: Request, next: Next| {
                let camera_reel = camera_reel.clone();
                async move {
                    let path = request.uri().path();
                    if path == "/camera-reel-service" || path.starts_with("/camera-reel-service/") {
                        // Dispatch without an outer route match: wildcard captures would leak
                        // into the inner Path extractor and alter its metrics route label.
                        use tower::ServiceExt;
                        camera_reel.oneshot(request).await.unwrap()
                    } else {
                        next.run(request).await
                    }
                }
            })),
        state,
    );
    dcl_metrics::http::instrument(assembled, "catalyst")
}
/// Apply after assembling root routes. Unknown paths, `/status`, `/ping`, health
/// probes remain the other services' responsibility at the shared root.
pub fn merge_root<S: Clone + Send + Sync + 'static>(router: Router<S>, state: State) -> Router<S> {
    router.layer(middleware::from_fn(move |request: Request, next: Next| {
        let state = state.clone();
        async move {
            let candidate = structural_route(request.uri().path(), true);
            if !candidate.as_ref().is_some_and(Route::root) {
                return next.run(request).await;
            }
            dispatch_scoped(state, request, true).await
        }
    }))
}
#[derive(Clone)]
enum Route {
    Parcels,
    Peers,
    Peer(String),
    Islands,
    Island(String),
    HotScenes,
    CoreStatus,
    Status,
    Live,
    Ready,
    Startup,
    Ping,
    Diagnostics,
}
impl Route {
    fn template(&self) -> &'static str {
        match self {
            Self::Parcels => "/parcels",
            Self::Peers => "/peers",
            Self::Peer(_) => "/peers/{id}",
            Self::Islands => "/islands",
            Self::Island(_) => "/islands/{id}",
            Self::HotScenes => "/hot-scenes",
            Self::CoreStatus => "/core-status",
            Self::Status => "/status",
            Self::Live => "/health/live",
            Self::Ready => "/health/ready",
            Self::Startup => "/health/startup",
            Self::Ping => "/ping",
            Self::Diagnostics => "/stats/health",
        }
    }
    fn root(&self) -> bool {
        matches!(
            self,
            Self::Parcels
                | Self::Peers
                | Self::Peer(_)
                | Self::Islands
                | Self::Island(_)
                | Self::HotScenes
                | Self::CoreStatus
                | Self::Diagnostics
        )
    }
}
fn structural_route(path: &str, shared_root: bool) -> Option<Route> {
    let path = path.strip_suffix('/').unwrap_or(path);
    // Probe middleware run before the application router.
    for (p, r) in [
        ("/health/live", Route::Live),
        ("/health/ready", Route::Ready),
        ("/health/startup", Route::Startup),
    ] {
        if path.eq_ignore_ascii_case(p) {
            return Some(r);
        }
    }
    for prefix in ["", "/comms", "/stats"] {
        if prefix == "/stats" && !shared_root {
            continue;
        }
        for (suffix, list) in [
            ("parcels", Route::Parcels),
            ("peers", Route::Peers),
            ("islands", Route::Islands),
        ] {
            let pattern = format!("{prefix}/{suffix}");
            if path.eq_ignore_ascii_case(&pattern) {
                return Some(list);
            }
            if suffix != "parcels"
                && path.len() > pattern.len() + 1
                && path
                    .get(..pattern.len())
                    .is_some_and(|p| p.eq_ignore_ascii_case(&pattern))
                && path.as_bytes()[pattern.len()] == b'/'
            {
                let id = &path[pattern.len() + 1..];
                if !id.contains('/') {
                    return Some(if suffix == "peers" {
                        Route::Peer(decode_path(id))
                    } else {
                        Route::Island(decode_path(id))
                    });
                }
            }
        }
    }
    for (p, r) in [
        ("/hot-scenes", Route::HotScenes),
        ("/core-status", Route::CoreStatus),
        ("/status", Route::Status),
        ("/ping", Route::Ping),
        ("/stats/health", Route::Diagnostics),
    ] {
        if path.eq_ignore_ascii_case(p) && (shared_root || !matches!(r, Route::Diagnostics)) {
            return Some(r);
        }
    }
    None
}
fn decode_path(id: &str) -> String {
    let mut bytes = Vec::with_capacity(id.len());
    let raw = id.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' {
            if i + 2 >= raw.len() {
                return id.into();
            }
            let hex = |b: u8| (b as char).to_digit(16).map(|v| v as u8);
            let (Some(a), Some(b)) = (hex(raw[i + 1]), hex(raw[i + 2])) else {
                return id.into();
            };
            bytes.push(a * 16 + b);
            i += 3;
        } else {
            bytes.push(raw[i]);
            i += 1;
        }
    }
    String::from_utf8(bytes).unwrap_or_else(|_| id.into())
}
struct Reply {
    status: u16,
    body: Option<String>,
    kind: Option<&'static str>,
    explicit_type: bool,
    headers: HeaderMap,
}
impl Reply {
    fn empty(status: u16) -> Self {
        Self {
            status,
            body: None,
            kind: None,
            explicit_type: false,
            headers: HeaderMap::new(),
        }
    }
    fn json(status: u16, body: String) -> Self {
        Self {
            body: Some(body),
            kind: Some("application/json"),
            ..Self::empty(status)
        }
    }
    fn text(status: u16, body: impl Into<String>) -> Self {
        Self {
            body: Some(body.into()),
            kind: Some("text/plain; charset=utf-8"),
            ..Self::empty(status)
        }
    }
    fn header(mut self, key: &'static str, value: &str) -> Self {
        self.headers.insert(
            key,
            HeaderValue::from_str(value).expect("statistics header"),
        );
        self
    }
    fn explicit(mut self, kind: &'static str) -> Self {
        self.kind = Some(kind);
        self.explicit_type = true;
        self
    }
    fn response(mut self, head: bool) -> Response {
        let suppressed = head || self.status == 204;
        if self.status != 204
            && (self.explicit_type || !head)
            && let Some(kind) = self.kind
        {
            self.headers
                .insert("content-type", HeaderValue::from_static(kind));
        }
        let body = if suppressed {
            // Unknown size avoids Axum synthesizing a Content-Length absent from HEAD/204.
            Body::from_stream(futures::stream::empty::<Result<bytes::Bytes, std::io::Error>>())
        } else {
            let body = self.body.unwrap_or_default();
            self.headers
                .insert("content-length", HeaderValue::from(body.len()));
            Body::from(body)
        };
        let mut response = Response::new(body);
        *response.status_mut() =
            StatusCode::from_u16(self.status).expect("replies use valid HTTP status codes");
        *response.headers_mut() = self.headers;
        response
    }
}
fn cors(reply: &mut Reply, headers: &HeaderMap, options: bool) {
    if let Some(origin) = headers.get("origin").filter(|origin| !origin.is_empty()) {
        reply
            .headers
            .insert("access-control-allow-origin", origin.clone());
        reply
            .headers
            .insert("vary", HeaderValue::from_static("Origin"));
    }
    if options || headers.contains_key("origin") {
        reply.headers.insert(
            "access-control-allow-credentials",
            HeaderValue::from_static("true"),
        );
    }
    if options {
        for (k, v) in [
            (
                "access-control-allow-methods",
                "GET,HEAD,POST,PUT,DELETE,CONNECT,TRACE,PATCH",
            ),
            (
                "access-control-allow-headers",
                "Cache-Control,Content-Type,Origin,Accept,User-Agent,X-Upload-Origin",
            ),
            ("access-control-max-age", "86400"),
        ] {
            reply.headers.insert(k, HeaderValue::from_static(v));
        }
    }
}
async fn dispatch(AxumState(state): AxumState<State>, request: Request) -> Response {
    dispatch_scoped(state, request, false).await
}
// The HTTP/1 parser accepts a finite method set.
const NODE_METHODS: &[&str] = &[
    "ACL",
    "BIND",
    "CHECKOUT",
    "CONNECT",
    "COPY",
    "DELETE",
    "GET",
    "HEAD",
    "LINK",
    "LOCK",
    "M-SEARCH",
    "MERGE",
    "MKACTIVITY",
    "MKCALENDAR",
    "MKCOL",
    "MOVE",
    "NOTIFY",
    "OPTIONS",
    "PATCH",
    "POST",
    "PROPFIND",
    "PROPPATCH",
    "PURGE",
    "PUT",
    "QUERY",
    "REBIND",
    "REPORT",
    "SEARCH",
    "SOURCE",
    "SUBSCRIBE",
    "TRACE",
    "UNBIND",
    "UNLINK",
    "UNLOCK",
    "UNSUBSCRIBE",
];
fn rejected_method(method: &str) -> Option<Reply> {
    if !NODE_METHODS.contains(&method) {
        Some(Reply::empty(400))
    } else if method == "TRACE" {
        Some(Reply::empty(500))
    } else {
        None
    }
}
async fn dispatch_scoped(state: State, request: Request, shared_root: bool) -> Response {
    let (request, _body) = request.into_parts();
    let method = request.method.as_str();
    let label_early_response = || {
        dcl_metrics::http::route(
            &request.extensions,
            "stats",
            structural_route(request.uri.path(), shared_root)
                .as_ref()
                .map_or("unmatched", Route::template),
        );
    };
    if let Some(reply) = rejected_method(method) {
        label_early_response();
        return reply.response(false);
    }
    let head = method == "HEAD";
    if method == "OPTIONS" {
        label_early_response();
        let mut reply = Reply::empty(204);
        cors(&mut reply, &request.headers, true);
        return reply.response(false);
    }
    // URL parsing normalizes dot segments while retaining encoded slashes and repeats.
    let normalized =
        url::Url::parse(&format!("http://statistics.invalid{}", request.uri.path())).ok();
    let path = normalized
        .as_ref()
        .map_or(request.uri.path(), url::Url::path);
    let matched = structural_route(path, shared_root);
    dcl_metrics::http::route(
        &request.extensions,
        "stats",
        matched.as_ref().map_or("unmatched", Route::template),
    );
    let result = if !["GET", "HEAD", "PUT", "PATCH", "POST", "DELETE"].contains(&method) {
        Ok(Reply::empty(501).header("allow", if matched.is_some() { "HEAD, GET" } else { "" }))
    } else if matched.is_some() && !matches!(method, "GET" | "HEAD") {
        Ok(Reply::empty(405).header("allow", "HEAD, GET"))
    } else {
        handle(&state, matched.as_ref(), &request).await
    };
    match result {
        Ok(mut reply) => {
            cors(&mut reply, &request.headers, false);
            reply.response(head)
        }
        Err(_) => Reply::empty(500).response(head),
    }
}
async fn handle(
    state: &State,
    route: Option<&Route>,
    request: &axum::http::request::Parts,
) -> Result<Reply, content::Error> {
    let snapshot = state.reader.snapshot();
    Ok(match route {
        Some(Route::Parcels) => Reply::json(200, render::parcels(&snapshot)),
        Some(Route::Peers) => {
            let ids = url::form_urlencoded::parse(request.uri.query().unwrap_or("").as_bytes())
                .filter_map(|(k, v)| (k == "id").then(|| v.into_owned()))
                .collect::<Vec<_>>();
            Reply::json(200, render::peers(&snapshot, &ids))
        }
        Some(Route::Peer(id)) => {
            let (status, body) = render::peer(&snapshot, id);
            Reply::json(status, body)
        }
        Some(Route::Islands) => Reply::json(200, render::islands(&snapshot)),
        Some(Route::Island(id)) => render::island(&snapshot, id)
            .map_or_else(|| Reply::empty(404), |body| Reply::json(200, body)),
        Some(Route::CoreStatus) => Reply::json(200, render::core_status(&snapshot, (state.now)())),
        Some(Route::HotScenes) => {
            let tiles = render::tiles(&snapshot);
            let scenes = content::tile_documents(state.content.as_ref(), &tiles).await?;
            Reply::json(
                200,
                content::hot_scenes(&scenes, &tiles, &state.settings.content_url)?,
            )
        }
        Some(Route::Status) => {
            #[derive(Serialize)]
            struct Status {
                version: String,
                #[serde(rename = "currentTime")]
                current_time: i64,
                #[serde(rename = "commitHash")]
                commit_hash: String,
            }
            let (version, commit_hash) = (state.environment)();
            let commit_hash = state.settings.commit_hash.clone().unwrap_or(commit_hash);
            Reply::json(
                200,
                render::json(&Status {
                    version,
                    current_time: (state.now)(),
                    commit_hash,
                }),
            )
            .header("access-control-allow-origin", "*")
        }
        Some(Route::Live) => Reply::text(200, "alive"),
        Some(Route::Ready) | Some(Route::Startup) => {
            let startup = matches!(route, Some(Route::Startup));
            let lifecycle = state.hooks.probe(startup).await;
            probe_reply(lifecycle, startup).explicit("application/health+json; charset=utf-8")
        }
        Some(Route::Ping) => Reply::text(200, "/ping"),
        Some(Route::Diagnostics) => Reply::json(200, state.hooks.diagnostics().await?),
        None => Reply::text(404, "Not found"),
    })
}
fn probe_reply(lifecycle: Lifecycle, startup: bool) -> Reply {
    if !lifecycle.initialized {
        return Reply::json(
            503,
            format!(
                r#"{{"status":"{}"}}"#,
                if startup {
                    "bootstrapping"
                } else {
                    "initializing"
                }
            ),
        );
    }
    if !lifecycle.started {
        return if startup {
            Reply::json(503, r#"{"status":"starting"}"#.into())
        } else {
            Reply::text(503, if lifecycle.live { "unready" } else { "waiting" })
        };
    }
    struct Details(Vec<(String, bool)>);
    impl Serialize for Details {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            #[derive(Serialize)]
            struct Check {
                status: &'static str,
            }
            let mut map = s.serialize_map(Some(self.0.len()))?;
            for (name, pass) in &self.0 {
                map.serialize_entry(
                    name,
                    &Check {
                        status: if *pass { "pass" } else { "fail" },
                    },
                )?;
            }
            map.end()
        }
    }
    #[derive(Serialize)]
    struct Probe {
        details: Details,
        status: &'static str,
    }
    let pass = lifecycle.checks.iter().all(|(_, pass)| *pass);
    Reply::json(
        if pass { 200 } else { 503 },
        render::json(&Probe {
            details: Details(lifecycle.checks),
            status: if pass { "pass" } else { "fail" },
        }),
    )
}
