//! HTTP observations are recorded once at ingress. Routers behind service fallbacks
//! identify their matched templates after Axum has routed the request.
use crate::{CounterFamily, Descriptor, GaugeFamily, HistogramFamily, SharedString, Unit};
use axum::{
    Router,
    extract::{MatchedPath, Request},
    http::{Extensions, Method, header},
    middleware::{Next, from_fn},
};
use std::{
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

const LABELS: &[&str; 4] = &["component", "route", "method", "status_code"];

const REQUESTS: CounterFamily<4> = CounterFamily::new(
    "dcl_http_requests_total",
    "HTTP requests completed or cancelled before response headers.",
    None,
    LABELS,
);
const IN_FLIGHT: GaugeFamily<1> = GaugeFamily::new(
    "dcl_http_requests_in_flight",
    "HTTP requests awaiting response headers.",
    None,
    &["method"],
);
const DURATION: HistogramFamily<4> = HistogramFamily::new(
    "dcl_http_request_duration_seconds",
    "Seconds from ingress to response headers or cancellation.",
    Some(Unit::Seconds),
    LABELS,
    &[
        0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1., 2.5, 5., 10., 30.,
    ],
);
const DECLARED_SIZE: HistogramFamily<4> = HistogramFamily::new(
    "dcl_http_request_declared_size_bytes",
    "Request size reported by a valid Content-Length header.",
    Some(Unit::Bytes),
    LABELS,
    &[
        0., 128., 512., 1024., 4096., 16384., 65536., 262144., 1048576., 16777216., 268435456.,
    ],
);

pub const FAMILIES: &[Descriptor] = &[
    REQUESTS.descriptor,
    IN_FLIGHT.descriptor,
    DURATION.descriptor,
    DECLARED_SIZE.descriptor,
];

#[derive(Default)]
struct Context {
    component: OnceLock<&'static str>,
    matched: OnceLock<MatchedPath>,
    manual: OnceLock<(&'static str, &'static str)>,
    exclude: AtomicBool,
}

/// For dispatchers that resolve their own finite set of route templates.
pub fn route(extensions: &Extensions, component: &'static str, template: &'static str) {
    if let Some(context) = extensions.get::<Arc<Context>>() {
        let _ = context.manual.set((component, template));
    }
}

/// Called only when a compatibility exporter has actually selected a scrape.
pub fn exclude(extensions: &Extensions) {
    if let Some(context) = extensions.get::<Arc<Context>>() {
        context.exclude.store(true, Ordering::Relaxed);
    }
}

/// Attach immediately outside the routes belonging to this component, before merging.
pub fn component(router: Router, component: &'static str) -> Router {
    router.layer(from_fn(move |req: Request, next: Next| async move {
        if let Some(context) = req.extensions().get::<Arc<Context>>() {
            let _ = context.component.set(component);
            if let Some(template) = req.extensions().get::<MatchedPath>() {
                let _ = context.matched.set(template.clone());
            }
        }
        next.run(req).await
    }))
}

fn method(method: &Method) -> &'static str {
    match method.as_str() {
        "GET" => "GET",
        "HEAD" => "HEAD",
        "POST" => "POST",
        "PUT" => "PUT",
        "PATCH" => "PATCH",
        "DELETE" => "DELETE",
        "CONNECT" => "CONNECT",
        "OPTIONS" => "OPTIONS",
        "TRACE" => "TRACE",
        _ => "OTHER",
    }
}

struct Observation {
    context: Arc<Context>,
    fallback: (&'static str, Option<MatchedPath>),
    method: &'static str,
    size: Option<u64>,
    start: Instant,
    status: Option<u16>,
    inflight: metrics::Gauge,
}
impl Drop for Observation {
    fn drop(&mut self) {
        self.inflight.decrement(1.);
        if self.context.exclude.load(Ordering::Relaxed) {
            return;
        }
        let (component, route) = self.context.manual.get().copied().unwrap_or_else(|| {
            let (default_component, template) = &self.fallback;
            (
                self.context
                    .component
                    .get()
                    .copied()
                    .unwrap_or(default_component),
                self.context
                    .matched
                    .get()
                    .or(template.as_ref())
                    .map_or("unmatched", MatchedPath::as_str),
            )
        });
        let status_code = self
            .status
            .map_or_else(|| "cancelled".to_owned(), |code| code.to_string());
        let values = [
            SharedString::const_str(component),
            route.to_owned().into(),
            SharedString::const_str(self.method),
            status_code.into(),
        ];
        REQUESTS.handle_owned(values.clone()).increment(1);
        DURATION
            .handle_owned(values.clone())
            .record(self.start.elapsed().as_secs_f64());
        if let Some(size) = self.size {
            DECLARED_SIZE.handle_owned(values).record(size as f64);
        }
    }
}

/// Install once on the assembled host. Dropping the service future records cancellation.
pub fn instrument(router: Router, default_component: &'static str) -> Router {
    router.layer(from_fn(move |mut req: Request, next: Next| async move {
        let method = method(req.method());
        let inflight = IN_FLIGHT.handle([method]);
        inflight.increment(1.);
        let context = Arc::new(Context::default());
        let mut observation = Observation {
            context: context.clone(),
            method,
            start: Instant::now(),
            inflight,
            fallback: (
                default_component,
                req.extensions().get::<MatchedPath>().cloned(),
            ),
            size: req
                .headers()
                .get(header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .filter(|v| v.bytes().all(|byte| byte.is_ascii_digit()))
                .and_then(|v| v.parse().ok()),
            status: None,
        };
        req.extensions_mut().insert(context);
        let response = next.run(req).await;
        observation.status = Some(response.status().as_u16());
        response
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, routing::get};
    use tower::ServiceExt;

    #[tokio::test]
    async fn service_fallbacks_preserve_nested_templates_and_count_each_request_once() {
        let recorder = crate::Registry::new(&[])
            .unwrap()
            .builder(crate::PrometheusBuilder::new())
            .unwrap()
            .build_recorder();
        let _local = metrics::set_default_local_recorder(&recorder);
        let shop = Router::new().route("/items/{id}", get(|| async { "ok" }));
        let inner = Router::new().nest("/shop", component(shop, "shop"));
        let app = instrument(Router::new().fallback_service(inner), "serve");

        for (method, path, status) in [
            ("GET", "/shop/items/private-id?token=secret", 200),
            ("POST", "/shop/items/another-id", 405),
            ("CUSTOM", "/unknown-id", 404),
        ] {
            let res = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .header(header::CONTENT_LENGTH, "17")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(res.status(), status);
        }
        let scrape = recorder.handle().render();
        let counts: Vec<_> = scrape
            .lines()
            .filter(|l| l.starts_with("dcl_http_requests_total{"))
            .collect();
        assert_eq!(counts.len(), 3, "{scrape}");
        for expected in ["200", "405"] {
            assert!(
                counts
                    .iter()
                    .any(|line| line.contains("route=\"/shop/items/{id}\"")
                        && line.contains("component=\"shop\"")
                        && line.contains(&format!("status_code=\"{expected}\""))
                        && line.ends_with(" 1")),
                "{scrape}"
            );
        }
        assert!(
            counts
                .iter()
                .any(|line| line.contains("route=\"unmatched\"")
                    && line.contains("method=\"OTHER\"")),
            "{scrape}"
        );
        assert!(
            scrape.contains("dcl_http_request_declared_size_bytes_sum{")
                && scrape.contains(" 17\n")
        );
        for private in ["private-id", "unknown-id", "another-id", "secret", "CUSTOM"] {
            assert!(!scrape.contains(private), "{scrape}");
        }
    }

    #[tokio::test]
    async fn cancellation_releases_inflight_and_scrapes_are_excluded() {
        let recorder = crate::PrometheusBuilder::new().build_recorder();
        let _local = metrics::set_default_local_recorder(&recorder);
        let app = instrument(
            Router::new()
                .route(
                    "/busy",
                    get(|| async { std::future::pending::<()>().await }),
                )
                .route(
                    "/custom-scrape",
                    get(|req: Request| async move {
                        exclude(req.extensions());
                        "scrape"
                    }),
                ),
            "test",
        );
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/custom-scrape")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let mut future =
            Box::pin(app.oneshot(Request::builder().uri("/busy").body(Body::empty()).unwrap()));
        assert!(futures::poll!(&mut future).is_pending());
        assert!(
            recorder
                .handle()
                .render()
                .contains("dcl_http_requests_in_flight{method=\"GET\"} 1")
        );
        drop(future);
        let scrape = recorder.handle().render();
        assert!(scrape.contains("dcl_http_requests_in_flight{method=\"GET\"} 0"));
        assert!(scrape.contains("status_code=\"cancelled\""), "{scrape}");
        assert!(!scrape.contains("custom-scrape"), "{scrape}");
        assert!(!scrape.contains("declared_size_bytes"));
    }
}
