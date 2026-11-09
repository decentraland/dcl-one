//! The worlds series, named as worlds-content-server's `src/metrics.ts` and
//! its default HTTP metrics so its dashboard queries carry over. Recorded
//! through the `metrics` facade; `catalyst serve` installs the exporter
//! (`comms::metrics`), which also sets these histograms' buckets.

use std::time::{Duration, Instant};

use axum::extract::{MatchedPath, Request};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;

use super::upload_limits;

pub const HTTP_REQUESTS_TOTAL: &str = "http_requests_total";
pub const HTTP_REQUEST_DURATION_SECONDS: &str = "http_request_duration_seconds";
pub const WORLD_DEPLOYMENTS_COUNTER: &str = "world_deployments_counter";
pub const DEPLOYMENT_PROCESSING_STAGE_DURATION_SECONDS: &str =
    "deployment_processing_stage_duration_seconds";
pub const DEPLOYMENT_PROCESSING_FAILURES: &str = "deployment_processing_failures";
pub const MULTIPART_UPLOAD_RECEIVE_DURATION_SECONDS: &str =
    "multipart_upload_receive_duration_seconds";
pub const MULTIPART_UPLOAD_SIZE_BYTES: &str = "multipart_upload_size_bytes";
pub const MULTIPART_UPLOAD_REJECTIONS: &str = "multipart_upload_rejections";
pub const GARBAGE_COLLECTION_RUNS: &str = "garbage_collection_runs";
pub const GARBAGE_COLLECTION_DURATION_SECONDS: &str = "garbage_collection_duration_seconds";
pub const GARBAGE_COLLECTION_REMOVED_KEYS: &str = "garbage_collection_removed_keys";
pub const GARBAGE_COLLECTION_REMOVED_BYTES: &str = "garbage_collection_removed_bytes";
pub const GARBAGE_COLLECTION_LAST_SUCCESS: &str =
    "garbage_collection_last_success_timestamp_seconds";
pub const CONTENT_LOCK_WRITER_TIMEOUTS: &str = "content_lock_writer_timeouts";

/// `POST /entities`' route label, as upstream.
pub const ENTITIES_ROUTE: &str = "entities";

/// `success`, `timeout`, `client-error` or `error`, as upstream's processing outcomes.
pub fn outcome(status: StatusCode) -> &'static str {
    match status.as_u16() {
        200..=399 => "success",
        408 => "timeout",
        400..=499 => "client-error",
        _ => "error",
    }
}

pub fn upload_rejected(route: &'static str, reason: &'static str) {
    metrics::counter!(MULTIPART_UPLOAD_REJECTIONS, "route" => route, "reason" => reason)
        .increment(1);
}

/// A multipart body received (or refused) after `elapsed`, `bytes` of payload parsed.
pub fn upload_received(
    route: &'static str,
    content_length: bool,
    outcome: &'static str,
    elapsed: Duration,
    bytes: u64,
) {
    metrics::histogram!(MULTIPART_UPLOAD_RECEIVE_DURATION_SECONDS, "route" => route, "outcome" => outcome)
        .record(elapsed.as_secs_f64());
    metrics::histogram!(
        MULTIPART_UPLOAD_SIZE_BYTES,
        "route" => route,
        "content_length" => if content_length { "present" } else { "absent" },
        "outcome" => outcome
    )
    .record(bytes as f64);
    upload_gauges();
}

/// The process-wide upload budget's current use.
pub fn upload_gauges() {
    metrics::gauge!("multipart_upload_active").set(upload_limits::active_uploads() as f64);
    metrics::gauge!("multipart_upload_reserved_bytes")
        .set(upload_limits::in_flight_upload_bytes() as f64);
    metrics::gauge!("multipart_upload_reserved_files")
        .set(upload_limits::in_flight_upload_files() as f64);
}

pub fn upload_capacity(limits: &upload_limits::UploadLimits) {
    metrics::gauge!("multipart_upload_capacity_bytes")
        .set(limits.max_in_flight_upload_bytes as f64);
}

/// Processing after the body was parsed, as upstream's `total` stage; `mode` is `regular` or `partial`.
pub fn deployment_processed(mode: &'static str, status: StatusCode, elapsed: Duration) {
    let outcome = outcome(status);
    metrics::histogram!(
        DEPLOYMENT_PROCESSING_STAGE_DURATION_SECONDS,
        "stage" => "total",
        "mode" => mode,
        "outcome" => outcome
    )
    .record(elapsed.as_secs_f64());
    if outcome != "success" {
        metrics::counter!(
            DEPLOYMENT_PROCESSING_FAILURES,
            "stage" => "total",
            "mode" => mode,
            "outcome" => outcome
        )
        .increment(1);
    }
    if mode == PARTIAL_MODE {
        metrics::counter!(PARTIAL_UPLOAD_REQUESTS, "outcome" => partial_outcome(status))
            .increment(1);
    }
}

pub const REGULAR_MODE: &str = "regular";
pub const PARTIAL_MODE: &str = "partial";

/// `partial_upload_requests`' outcome label of a partial batch's answer, as upstream.
pub fn partial_outcome(status: StatusCode) -> &'static str {
    match status.as_u16() {
        200 => "finalized",
        202 => "accepted",
        408 => "timeout",
        429 => "throttled",
        499 => "aborted",
        400..=499 => "validation_error",
        _ => "error",
    }
}

pub const PARTIAL_UPLOAD_METADATA_CHECKS: &str = "partial_upload_metadata_checks";
pub const PARTIAL_UPLOAD_BATCHES: &str = "partial_upload_batches";
pub const PARTIAL_UPLOAD_RESERVED_BYTES: &str = "partial_upload_reserved_bytes";
pub const PARTIAL_UPLOAD_CLEANUP_BACKLOG_BYTES: &str = "partial_upload_cleanup_backlog_bytes";
pub const PARTIAL_UPLOAD_CAPACITY_BYTES: &str = "partial_upload_capacity_bytes";
pub const PARTIAL_UPLOAD_REQUESTS: &str = "partial_upload_requests";
pub const PARTIAL_UPLOAD_THROTTLED: &str = "partial_upload_throttled";
pub const PARTIAL_UPLOADS_STARTED: &str = "partial_uploads_started";
pub const PARTIAL_UPLOADS_COMPLETED: &str = "partial_uploads_completed";
pub const PARTIAL_UPLOADS_PENDING: &str = "partial_uploads_pending";
pub const PARTIAL_UPLOAD_DURATION_SECONDS: &str = "partial_upload_duration_seconds";
pub const PARTIAL_UPLOAD_BATCHES_PER_UPLOAD: &str = "partial_upload_batches_per_upload";
pub const PARTIAL_UPLOAD_EXPIRED_UPLOADS: &str = "partial_upload_expired_uploads";
pub const PARTIAL_UPLOAD_CLEANUP_FAILED_UPLOADS: &str = "partial_upload_cleanup_failed_uploads";
pub const PARTIAL_UPLOAD_CLEANUP_RUNS: &str = "partial_upload_cleanup_runs";
pub const PARTIAL_UPLOAD_CLEANUP_DURATION_SECONDS: &str = "partial_upload_cleanup_duration_seconds";
pub const PARTIAL_UPLOAD_CLEANUP_LAST_SUCCESS: &str =
    "partial_upload_cleanup_last_success_timestamp_seconds";

fn unix_now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

pub fn partial_metadata_checks(hashes: usize) {
    metrics::counter!(PARTIAL_UPLOAD_METADATA_CHECKS).increment(hashes as u64);
}

/// A stored batch: `incomplete` (202) or `finalizing`.
pub fn partial_batch(complete: bool) {
    let outcome = if complete { "finalizing" } else { "incomplete" };
    metrics::counter!(PARTIAL_UPLOAD_BATCHES, "outcome" => outcome).increment(1);
}

pub fn partial_throttled(reason: &'static str) {
    metrics::counter!(PARTIAL_UPLOAD_THROTTLED, "reason" => reason).increment(1);
}

pub fn partial_started() {
    metrics::counter!(PARTIAL_UPLOADS_STARTED).increment(1);
}

/// A published upload: its age since the first batch and its stored batches.
pub fn partial_completed(age: Duration, batches: i32) {
    metrics::counter!(PARTIAL_UPLOADS_COMPLETED).increment(1);
    metrics::histogram!(PARTIAL_UPLOAD_DURATION_SECONDS).record(age.as_secs_f64());
    metrics::histogram!(PARTIAL_UPLOAD_BATCHES_PER_UPLOAD).record(f64::from(batches));
}

pub fn partial_reserved_bytes(bytes: i64) {
    metrics::gauge!(PARTIAL_UPLOAD_RESERVED_BYTES).set(bytes as f64);
}

pub fn partial_capacity(bytes: u64) {
    metrics::gauge!(PARTIAL_UPLOAD_CAPACITY_BYTES).set(bytes as f64);
}

/// The database snapshot a cleanup run reports: charged and expired bytes, live and expired uploads.
pub fn partial_snapshot(charged: i64, expired_bytes: i64, live: i64, expired: i64) {
    partial_reserved_bytes(charged);
    metrics::gauge!(PARTIAL_UPLOAD_CLEANUP_BACKLOG_BYTES).set(expired_bytes as f64);
    metrics::gauge!(PARTIAL_UPLOADS_PENDING, "state" => "live").set(live as f64);
    metrics::gauge!(PARTIAL_UPLOADS_PENDING, "state" => "expired").set(expired as f64);
}

pub fn partial_expired_reclaimed() {
    metrics::counter!(PARTIAL_UPLOAD_EXPIRED_UPLOADS).increment(1);
}

pub fn partial_cleanup_failed() {
    metrics::counter!(PARTIAL_UPLOAD_CLEANUP_FAILED_UPLOADS).increment(1);
}

/// One cleanup run: `success`, `deferred` (the store lock stayed busy) or `error`.
pub fn partial_cleanup_run(outcome: &'static str, elapsed: Duration) {
    metrics::counter!(PARTIAL_UPLOAD_CLEANUP_RUNS, "outcome" => outcome).increment(1);
    metrics::histogram!(PARTIAL_UPLOAD_CLEANUP_DURATION_SECONDS).record(elapsed.as_secs_f64());
    if outcome == "success" {
        metrics::gauge!(PARTIAL_UPLOAD_CLEANUP_LAST_SUCCESS).set(unix_now());
    }
}

/// `dcl-name` for a `.dcl.eth` world, `ens-name` otherwise, as upstream.
pub fn deployment_kind(world: &str) -> &'static str {
    if world.to_ascii_lowercase().ends_with(".dcl.eth") {
        "dcl-name"
    } else {
        "ens-name"
    }
}

pub fn world_deployed(world: &str) {
    metrics::counter!(WORLD_DEPLOYMENTS_COUNTER, "kind" => deployment_kind(world)).increment(1);
}

/// One `/gc` run: `success`, `deferred` (the store lock stayed busy) or `error`.
pub fn gc_run(outcome: &'static str, elapsed: Duration, removed: usize, removed_bytes: u64) {
    metrics::counter!(GARBAGE_COLLECTION_RUNS, "outcome" => outcome).increment(1);
    metrics::histogram!(GARBAGE_COLLECTION_DURATION_SECONDS).record(elapsed.as_secs_f64());
    if outcome == "success" {
        metrics::counter!(GARBAGE_COLLECTION_REMOVED_KEYS).increment(removed as u64);
        metrics::counter!(GARBAGE_COLLECTION_REMOVED_BYTES).increment(removed_bytes);
        metrics::gauge!(GARBAGE_COLLECTION_LAST_SUCCESS).set(unix_now());
    }
    if outcome == "deferred" {
        metrics::counter!(CONTENT_LOCK_WRITER_TIMEOUTS).increment(1);
    }
}

/// The standard methods by name; any other method is `other`, so labels stay bounded.
fn method_label(method: &axum::http::Method) -> &'static str {
    use axum::http::Method;
    match *method {
        Method::GET => "GET",
        Method::HEAD => "HEAD",
        Method::POST => "POST",
        Method::PUT => "PUT",
        Method::DELETE => "DELETE",
        Method::PATCH => "PATCH",
        Method::OPTIONS => "OPTIONS",
        Method::CONNECT => "CONNECT",
        Method::TRACE => "TRACE",
        _ => "other",
    }
}

/// `http_requests_total` and `http_request_duration_seconds` by method, matched route (`handler`)
/// and status (`code`), the labels of upstream's default HTTP metrics.
pub async fn http_metrics(request: Request, next: Next) -> Response {
    let Some(handler) = request
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_string())
    else {
        return next.run(request).await;
    };
    let method = method_label(request.method()).to_string();
    let started = Instant::now();
    let response = next.run(request).await;
    let code = response.status().as_u16().to_string();
    let labels = [("method", method), ("handler", handler), ("code", code)];
    metrics::counter!(HTTP_REQUESTS_TOTAL, &labels).increment(1);
    metrics::histogram!(HTTP_REQUEST_DURATION_SECONDS, &labels)
        .record(started.elapsed().as_secs_f64());
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::Body;
    use axum::routing::get;
    use tower::ServiceExt;

    #[test]
    fn outcomes_and_kinds_follow_upstream_labels() {
        assert_eq!(outcome(StatusCode::OK), "success");
        assert_eq!(outcome(StatusCode::REQUEST_TIMEOUT), "timeout");
        assert_eq!(outcome(StatusCode::BAD_REQUEST), "client-error");
        assert_eq!(outcome(StatusCode::SERVICE_UNAVAILABLE), "error");
        assert_eq!(deployment_kind("Name.DCL.eth"), "dcl-name");
        assert_eq!(deployment_kind("name.eth"), "ens-name");
    }

    #[test]
    fn unknown_http_methods_share_one_label() {
        use axum::http::Method;
        assert_eq!(method_label(&Method::GET), "GET");
        assert_eq!(method_label(&Method::PATCH), "PATCH");
        assert_eq!(method_label(&Method::from_bytes(b"BREW").unwrap()), "other");
        assert_eq!(
            method_label(&Method::from_bytes(b"XYZZY1").unwrap()),
            "other"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn http_metrics_label_the_matched_route() {
        let recorder = crate::comms::metrics::prometheus_builder()
            .unwrap()
            .build_recorder();
        let handle = recorder.handle();
        let app = Router::new()
            .route("/world/{world_name}/about", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(http_metrics));
        let _guard = metrics::set_default_local_recorder(&recorder);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/world/a.dcl.eth/about")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let rendered = handle.render();
        assert!(
            rendered.contains(
                "http_requests_total{method=\"GET\",handler=\"/world/{world_name}/about\",code=\"200\"} 1"
            ),
            "{rendered}"
        );
        assert!(
            rendered.contains("http_request_duration_seconds_bucket{method=\"GET\",handler=\"/world/{world_name}/about\",code=\"200\",le="),
            "a histogram with buckets: {rendered}"
        );
    }
}
