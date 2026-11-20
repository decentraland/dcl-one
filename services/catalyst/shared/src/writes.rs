//! The write switches every domain applies: the route-local [`Gate`]
//! middleware (the creator plane's, reused by camera-reel), the data plane's [`DataWrites`] (`DATA_WRITES_ENABLED`) and the
//! operator plane's [`OPS_WRITES_ENABLED`]. Off, a gated mutating request
//! answers 503 with a `Retry-After`; the reads keep answering. On, the
//! plane's writes go through.

use std::sync::atomic::{AtomicBool, Ordering};

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::json;

/// Every authoritative write of the operator slice answers 503 until this is
/// set; the twin of `SOCIAL_WRITES_ENABLED`.
pub const OPS_WRITES_ENABLED: &str = "OPS_WRITES_ENABLED";

pub const WRITES_DISABLED_MESSAGE: &str =
    "operator-plane writes are disabled on this node (OPS_WRITES_ENABLED=false)";

/// A background writer's switch, asked at the start of each pass: a skipped
/// pass and the resume are logged once per transition, not once per tick.
pub struct PassGate {
    job: &'static str,
    held: AtomicBool,
}

impl PassGate {
    pub const fn new(job: &'static str) -> Self {
        Self {
            job,
            held: AtomicBool::new(false),
        }
    }

    /// Whether this pass may write.
    pub fn allows(&self, switch_on: bool) -> bool {
        self.decide(switch_on).0
    }

    /// Whether this pass may write and whether it logged a transition.
    fn decide(&self, switch_on: bool) -> (bool, bool) {
        let held = !switch_on;
        let turned = self.held.swap(held, Ordering::SeqCst) != held;
        if turned {
            if held {
                tracing::warn!(
                    job = self.job,
                    "the job's write switch is off; its writes are skipped"
                );
            } else {
                tracing::info!(
                    job = self.job,
                    "the job's write switch is on; writes resume"
                );
            }
        }
        (switch_on, turned)
    }
}

/// Whether `method` is a write this gate refuses while disabled.
pub fn is_gated(method: &Method) -> bool {
    !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

/// One router's view of the switch: the paths (as the router sees them,
/// after any nest prefix is stripped) whose mutating methods stay open, and
/// the 503 it answers.
#[derive(Clone, Copy)]
pub struct Gate {
    pub enabled: bool,
    pub open_while_disabled: &'static [&'static str],
    pub refuse: fn() -> Response,
}

impl Gate {
    /// Whether the switch refuses `method` on `path`.
    pub fn refuses(&self, method: &Method, path: &str) -> bool {
        !self.enabled && is_gated(method) && !self.open_while_disabled.contains(&path)
    }
}

/// The gate as a middleware function, for a router that layers it on its
/// write routes alone (`MethodRouter::route_layer`) rather than through
/// [`guard_with`].
pub async fn gate(State(gate): State<Gate>, req: Request, next: Next) -> Response {
    if gate.refuses(req.method(), req.uri().path()) {
        return (gate.refuse)();
    }
    next.run(req).await
}

/// Wraps a router in `gate`. Applied per router, before nesting, so the path
/// the gate sees is the route-local one and the same rule holds at the root
/// and behind the prefix.
pub fn guard_with(router: Router, gate: Gate) -> Router {
    router.layer(middleware::from_fn_with_state(gate, self::gate))
}

/// `DATA_WRITES_ENABLED`: the one switch in front of every data-plane write
/// (the social switch's twin, `social::writes`). Off (the default), a
/// mutating request to a gated router answers 503 with the lambdas envelope
/// and `Retry-After`; the reads keep answering. `POST /v1/payments/verify`
/// reads a receipt and changes nothing, so it stays open, as do the
/// economy's two relayer controls (`/v1/admin/relayer/{toggle,signer}`):
/// they flip process-local atomics behind `CATALYRST_ECONOMY_ADMIN_TOKEN`
/// and write no durable row, the social gate's `disconnect` rule, so an
/// operator can pause broadcasting or pick the signer before turning
/// writes on.
pub struct DataWrites;

impl DataWrites {
    pub const DISABLED_MESSAGE: &'static str =
        "data-plane writes are disabled on this node (DATA_WRITES_ENABLED=false)";

    const OPEN_WHILE_DISABLED: &'static [&'static str] = &[
        "/v1/payments/verify",
        "/v1/admin/relayer/toggle",
        "/v1/admin/relayer/signer",
    ];

    pub fn is_gated(method: &Method, path: &str) -> bool {
        if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
            return false;
        }
        !Self::OPEN_WHILE_DISABLED.contains(&path)
    }

    pub fn disabled_response() -> Response {
        let body = json!({ "ok": false, "error": "Service Unavailable", "message": Self::DISABLED_MESSAGE });
        (
            StatusCode::SERVICE_UNAVAILABLE,
            [
                (header::CONTENT_TYPE, "application/json"),
                (header::RETRY_AFTER, "3600"),
            ],
            body.to_string(),
        )
            .into_response()
    }

    /// Wraps a router in the gate; applied per router, before nesting, so the
    /// path the gate sees is the route-local one.
    pub fn guard(router: Router, writes_enabled: bool) -> Router {
        Self::guard_open(router, writes_enabled, &[])
    }

    /// `guard`, plus the router's own route-local paths that change nothing
    /// and stay open while writes are off (a quote, say).
    pub fn guard_open(
        router: Router,
        writes_enabled: bool,
        open: &'static [&'static str],
    ) -> Router {
        async fn gate(
            State((enabled, open)): State<(bool, &'static [&'static str])>,
            req: Request,
            next: Next,
        ) -> Response {
            let path = req.uri().path();
            if !enabled && DataWrites::is_gated(req.method(), path) && !open.contains(&path) {
                return DataWrites::disabled_response();
            }
            next.run(req).await
        }
        router.layer(middleware::from_fn_with_state((writes_enabled, open), gate))
    }
}

/// The six write switches as `WRITES=` names them, with their variables.
#[derive(Clone, Copy, Debug, Default)]
pub struct Switches {
    pub content: bool,
    pub create: bool,
    pub data: bool,
    pub social: bool,
    pub worlds: bool,
    pub ops: bool,
}

impl Switches {
    pub fn planes(&self) -> [(&'static str, &'static str, bool); 6] {
        [
            ("content", "CONTENT_WRITES_ENABLED", self.content),
            ("create", "CREATE_WRITES_ENABLED", self.create),
            ("data", "DATA_WRITES_ENABLED", self.data),
            ("social", "SOCIAL_WRITES_ENABLED", self.social),
            ("worlds", "WORLDS_WRITES_ENABLED", self.worlds),
            ("ops", OPS_WRITES_ENABLED, self.ops),
        ]
    }
}

/// `GET /writes`: each plane's switch and whether its writes go through,
/// with the reason they do not, for a front that keeps state of its own
/// (the sites' `/create` drafts) to apply the same gate.
pub fn status_document(switches: &Switches) -> serde_json::Value {
    let planes: serde_json::Map<String, serde_json::Value> = switches
        .planes()
        .into_iter()
        .map(|(plane, var, on)| {
            let reason = (!on).then(|| format!("{var}=false"));
            (
                plane.to_string(),
                json!({ "switch": var, "on": on, "open": on, "reason": reason }),
            )
        })
        .collect();
    json!({ "planes": planes })
}

pub fn status_router(switches: Switches) -> Router {
    Router::new().route(
        "/writes",
        get(move || async move {
            (
                [(header::CACHE_CONTROL, "no-store")],
                axum::Json(status_document(&switches)),
            )
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_http::call;
    use axum::body::Body;
    use axum::routing::post;
    use tower::ServiceExt;

    fn refuse() -> Response {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "lab writes are disabled (LAB=false)",
        )
            .into_response()
    }

    fn app(enabled: bool) -> Router {
        let inner = Router::new()
            .route("/w", post(|| async { "written" }).get(|| async { "read" }))
            .route("/open", post(|| async { "open" }));
        let gate = Gate {
            enabled,
            open_while_disabled: &["/open"],
            refuse,
        };
        Router::new()
            .nest("/p", guard_with(inner.clone(), gate))
            .merge(DataWrites::guard(
                Router::new().route("/v1/lists", post(|| async { "listed" })),
                enabled,
            ))
    }

    #[tokio::test]
    async fn guard_open_keeps_the_routers_own_open_paths_answering_while_writes_are_off() {
        let inner = || {
            Router::new()
                .route("/quote", post(|| async { "quoted" }))
                .route("/buy", post(|| async { "bought" }))
        };
        let gated = DataWrites::guard_open(inner(), false, &["/quote"]);
        assert_eq!(call(&gated, "POST", "/quote").await.0, StatusCode::OK);
        assert_eq!(
            call(&gated, "POST", "/buy").await.0,
            StatusCode::SERVICE_UNAVAILABLE
        );
        let plain = DataWrites::guard(inner(), false);
        assert_eq!(
            call(&plain, "POST", "/quote").await.0,
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[tokio::test]
    async fn the_switch_alone_decides() {
        let on = app(true);
        assert_eq!(
            call(&on, "POST", "/p/w").await,
            (StatusCode::OK, "written".into())
        );
        assert_eq!(
            call(&on, "POST", "/v1/lists").await,
            (StatusCode::OK, "listed".into())
        );
        let off = app(false);
        assert_eq!(
            call(&off, "POST", "/p/w").await,
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "lab writes are disabled (LAB=false)".into()
            )
        );
        let (status, body) = call(&off, "POST", "/v1/lists").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["message"], DataWrites::DISABLED_MESSAGE);
        assert_eq!(
            call(&off, "GET", "/p/w").await,
            (StatusCode::OK, "read".into()),
            "reads keep answering"
        );
        assert_eq!(
            call(&off, "POST", "/p/open").await,
            (StatusCode::OK, "open".into()),
            "an open path stays open"
        );
    }

    #[test]
    fn a_background_pass_follows_the_switch_and_logs_each_transition_once() {
        let gate = PassGate::new("lab-job");
        assert_eq!(gate.decide(true), (true, false));
        assert_eq!(gate.decide(false), (false, true));
        assert_eq!(
            gate.decide(false),
            (false, false),
            "still off: no second log"
        );
        assert_eq!(gate.decide(true), (true, true), "the resume logs once");
        assert_eq!(gate.decide(true), (true, false));
    }

    /// Every non-test source file that stores into the blob Storage, with
    /// the plane whose switch gates the write; `mirror` is a read-through
    /// copy of upstream bytes, fetched again on a miss.
    #[cfg(unix)]
    const STORAGE_WRITERS: &[(&str, &str)] = &[
        ("builder/src/catalog.rs", "create"),
        ("builder/src/drafts.rs", "create"),
        ("builder/src/projects.rs", "create"),
        ("camera-reel/src/handlers/images.rs", "create"),
        ("camera-reel/src/import.rs", "create"),
        ("comms/src/gatekeeper/reports.rs", "social"),
        ("content/src/deploy_remote_entity.rs", "content"),
        ("content/src/read_through.rs", "mirror"),
        ("content/src/snapshot_gen.rs", "content"),
        ("content/src/write_deployer.rs", "content"),
        ("explore/src/api/posters.rs", "mirror"),
        ("explore/src/api/satellite.rs", "mirror"),
        ("explore/src/api/world_manifest.rs", "mirror"),
        ("explore/src/events/posters.rs", "data"),
        ("shop/src/outfits/mod.rs", "data"),
        ("social/src/communities/fed_content.rs", "social"),
        ("social/src/communities/thumbnail.rs", "social"),
        ("social/src/messaging.rs", "social"),
        ("social/src/profile_images/cache.rs", "mirror"),
        ("worlds/src/core/deploy.rs", "worlds"),
        ("worlds/src/core/partial.rs", "worlds"),
        ("worlds/src/core/settings.rs", "worlds"),
        ("worlds/src/mirror_reconcile.rs", "worlds"),
        ("worlds/src/mirror_upstream.rs", "worlds"),
    ];

    /// The workspace files that call a Storage writer outside their inline
    /// test module, test files aside.
    #[cfg(unix)]
    fn storage_writers(root: &std::path::Path) -> std::collections::BTreeSet<String> {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().into_owned();
                if path.is_dir() {
                    if name != "tests" {
                        walk(&path, out);
                    }
                } else if name.ends_with(".rs")
                    && name != "tests.rs"
                    && !name.ends_with("_tests.rs")
                {
                    out.push(path);
                }
            }
        }
        let mut files = Vec::new();
        walk(&root.join("src"), &mut files);
        for krate in std::fs::read_dir(root).unwrap().flatten() {
            if krate.path().join("Cargo.toml").is_file() && krate.file_name() != "storage" {
                walk(&krate.path().join("src"), &mut files);
            }
        }
        // Every call of a Storage writer: `store`, `store_file`, `rekey` and
        // `create_staging_file`, its arguments read to the closing parenthesis
        // however rustfmt wrapped them. Not a store: an accessor's (no
        // arguments) and an atomic's (its last argument an ordering).
        let writes_blobs = |text: &str| {
            ["store(", "store_file(", "rekey(", "create_staging_file("]
                .iter()
                .any(|method| {
                    text.match_indices(&format!(".{method}")).any(|(at, call)| {
                        let mut depth = 1;
                        let args: String = text[at + call.len()..]
                            .chars()
                            .take_while(|c| {
                                depth += match c {
                                    '(' => 1,
                                    ')' => -1,
                                    _ => 0,
                                };
                                depth > 0
                            })
                            .collect();
                        let last = args
                            .split(',')
                            .map(str::trim)
                            .rfind(|a| !a.is_empty())
                            .unwrap_or("");
                        let ordering = [
                            "Ordering::",
                            "SeqCst",
                            "Relaxed",
                            "Release",
                            "Acquire",
                            "AcqRel",
                        ]
                        .iter()
                        .any(|o| last.starts_with(o))
                            || last.contains("atomic::Ordering::");
                        !args.trim().is_empty() && !ordering
                    })
                })
        };
        files
            .into_iter()
            .filter(|file| {
                let text = std::fs::read_to_string(file).unwrap();
                let tests_at = text
                    .match_indices("#[cfg(test)]\nmod ")
                    .find(|(at, _)| text[*at..].lines().nth(1).is_some_and(|l| l.ends_with('{')))
                    .map_or(text.len(), |(at, _)| at);
                writes_blobs(&text[..tests_at])
            })
            .map(|file| {
                file.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    // unix only: the source scan expects `/` paths and LF line ends, not Windows' `\` and CRLF.
    #[cfg(unix)]
    #[test]
    fn every_blob_writer_names_its_write_plane() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let listed: std::collections::BTreeSet<String> = STORAGE_WRITERS
            .iter()
            .map(|(file, _)| file.to_string())
            .collect();
        assert_eq!(
            storage_writers(&root),
            listed,
            "a file that stores into Storage names its plane in STORAGE_WRITERS"
        );
        let planes = Switches::default().planes().map(|(plane, _, _)| plane);
        for (file, plane) in STORAGE_WRITERS {
            assert!(
                *plane == "mirror" || planes.contains(plane),
                "{file} names {plane}, which is not a write plane"
            );
        }
    }

    #[tokio::test]
    async fn the_status_document_names_each_plane_and_its_switch() {
        let switches = Switches {
            create: true,
            social: true,
            ..Default::default()
        };
        let doc = status_document(&switches);
        assert_eq!(doc.as_object().unwrap().len(), 1, "{doc}");
        assert_eq!(doc["planes"].as_object().unwrap().len(), 6);
        assert_eq!(
            doc["planes"]["create"],
            json!({ "switch": "CREATE_WRITES_ENABLED", "on": true, "open": true, "reason": null })
        );
        assert_eq!(
            doc["planes"]["content"],
            json!({
                "switch": "CONTENT_WRITES_ENABLED",
                "on": false,
                "open": false,
                "reason": "CONTENT_WRITES_ENABLED=false"
            })
        );
        let res = status_router(switches)
            .oneshot(Request::get("/writes").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()[header::CACHE_CONTROL], "no-store");
    }
}
