//! Process-wide multipart upload budgets shared by every multipart route: admission
//! takes a concurrency slot plus a zero-byte lease grown from actual parsed payload
//! bytes -- never from Content-Length, which includes multipart framing.

use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::http::{HeaderMap, header};
use parking_lot::Mutex;

pub const DEFAULT_MAX_IN_FLIGHT_UPLOAD_BYTES: u64 = 4 * 1024 * 1024 * 1024;

pub const DEFAULT_MAX_CONCURRENT_UPLOADS: u64 = 40;

pub const DEFAULT_MAX_IN_FLIGHT_UPLOAD_FILES: u64 = 40_000;

pub const DEFAULT_MULTIPART_UPLOAD_TIMEOUT_MS: u64 = 300_000;

pub const DEFAULT_DEPLOYMENT_PROCESSING_TIMEOUT_MS: u64 = 300_000;

pub const BYTES_SHED_MESSAGE: &str = "Server is buffering too many uploads, please retry shortly.";
pub const CONCURRENCY_SHED_MESSAGE: &str =
    "Server is handling too many concurrent uploads, please retry shortly.";
pub const FILES_SHED_MESSAGE: &str =
    "Server is buffering too many upload files, please retry shortly.";
pub const MULTIPART_TIMEOUT_MESSAGE: &str = "The multipart upload timed out.";
pub const PAYLOAD_TOO_LARGE_MESSAGE: &str = "The multipart request is too large.";
pub const INVALID_CONTENT_LENGTH_MESSAGE: &str = "Invalid Content-Length header.";
pub const TOO_MANY_FIELDS_MESSAGE: &str = "The multipart request has too many fields.";
pub const TOO_MANY_PARTS_MESSAGE: &str = "The multipart request has too many parts.";

/// The parser's field-count refusal (`multipart/middleware.ts:316`).
pub fn too_many_fields_message(max_fields: usize) -> String {
    format!("The multipart request has too many fields. The maximum allowed is {max_fields}.")
}

pub const MAX_MULTIPART_FIELD_VALUE_BYTES: usize = 1024 * 1024;

pub const MAX_MULTIPART_FIELDS: usize = 100;

pub const MAX_MULTIPART_PARTS: usize = 10_100;

static IN_FLIGHT_UPLOAD_BYTES: AtomicU64 = AtomicU64::new(0);
static IN_FLIGHT_UPLOAD_FILES: AtomicU64 = AtomicU64::new(0);
static ACTIVE_UPLOADS: AtomicU64 = AtomicU64::new(0);

/// The knobs the old `MAX_IN_FLIGHT_UPLOAD_BYTES`, `MAX_CONCURRENT_UPLOADS`,
/// `MAX_IN_FLIGHT_UPLOAD_FILES`, `MULTIPART_UPLOAD_TIMEOUT_MS` and
/// `DEPLOYMENT_PROCESSING_TIMEOUT_MS` carried; every one must be positive
/// and the byte budget must cover one full deploy payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadLimits {
    pub max_in_flight_upload_bytes: u64,
    pub max_concurrent_uploads: u64,
    pub max_in_flight_upload_files: u64,
    pub multipart_upload_timeout_ms: u64,
    pub deployment_processing_timeout_ms: u64,
    pub max_concurrent_uploads_per_source: u64,
    pub max_in_flight_upload_bytes_per_source: u64,
}

impl Default for UploadLimits {
    fn default() -> Self {
        Self {
            max_in_flight_upload_bytes: DEFAULT_MAX_IN_FLIGHT_UPLOAD_BYTES,
            max_concurrent_uploads: DEFAULT_MAX_CONCURRENT_UPLOADS,
            max_in_flight_upload_files: DEFAULT_MAX_IN_FLIGHT_UPLOAD_FILES,
            multipart_upload_timeout_ms: DEFAULT_MULTIPART_UPLOAD_TIMEOUT_MS,
            deployment_processing_timeout_ms: DEFAULT_DEPLOYMENT_PROCESSING_TIMEOUT_MS,
            max_concurrent_uploads_per_source: DEFAULT_MAX_CONCURRENT_UPLOADS_PER_SOURCE,
            max_in_flight_upload_bytes_per_source: DEFAULT_MAX_IN_FLIGHT_UPLOAD_BYTES_PER_SOURCE,
        }
    }
}

fn positive_limit(name: &str, value: u64) -> anyhow::Result<u64> {
    if value == 0 {
        anyhow::bail!("{name} must be a positive integer, got {value}");
    }
    Ok(value)
}

impl UploadLimits {
    pub fn validate(self, max_upload_size_bytes: u64) -> anyhow::Result<Self> {
        positive_limit(
            "MAX_IN_FLIGHT_UPLOAD_BYTES",
            self.max_in_flight_upload_bytes,
        )?;
        positive_limit("MAX_CONCURRENT_UPLOADS", self.max_concurrent_uploads)?;
        positive_limit(
            "MAX_IN_FLIGHT_UPLOAD_FILES",
            self.max_in_flight_upload_files,
        )?;
        positive_limit(
            "MULTIPART_UPLOAD_TIMEOUT_MS",
            self.multipart_upload_timeout_ms,
        )?;
        positive_limit(
            "DEPLOYMENT_PROCESSING_TIMEOUT_MS",
            self.deployment_processing_timeout_ms,
        )?;
        positive_limit(
            "MAX_CONCURRENT_UPLOADS_PER_SOURCE",
            self.max_concurrent_uploads_per_source,
        )?;
        if self.max_in_flight_upload_bytes_per_source < max_upload_size_bytes {
            anyhow::bail!(
                "MAX_IN_FLIGHT_UPLOAD_BYTES_PER_SOURCE ({}) must fit one maximum-size upload ({max_upload_size_bytes} bytes).",
                self.max_in_flight_upload_bytes_per_source
            );
        }
        if self.max_in_flight_upload_bytes < max_upload_size_bytes {
            anyhow::bail!(
                "MAX_IN_FLIGHT_UPLOAD_BYTES ({}) must be greater than or equal to maxSizeInBytes ({max_upload_size_bytes})",
                self.max_in_flight_upload_bytes
            );
        }
        Ok(self)
    }
}

fn try_resize_counter(counter: &AtomicU64, lease: &mut u64, new_total: u64, max: u64) -> bool {
    let mut current = counter.load(Ordering::Acquire);
    loop {
        let others = current.saturating_sub(*lease);
        let Some(next) = others.checked_add(new_total) else {
            return false;
        };
        if next > max {
            return false;
        }
        match counter.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => {
                *lease = new_total;
                return true;
            }
            Err(actual) => current = actual,
        }
    }
}

pub struct InFlightBytesGuard(u64);

impl InFlightBytesGuard {
    pub fn reserved(&self) -> u64 {
        self.0
    }

    /// Re-targets the lease at `new_total` reserved bytes; on failure the reservation is unchanged.
    pub fn try_resize(&mut self, new_total: u64, max: u64) -> bool {
        try_resize_counter(&IN_FLIGHT_UPLOAD_BYTES, &mut self.0, new_total, max)
    }
}

impl Drop for InFlightBytesGuard {
    fn drop(&mut self) {
        IN_FLIGHT_UPLOAD_BYTES.fetch_sub(self.0, Ordering::AcqRel);
    }
}

pub fn reserve_in_flight() -> InFlightBytesGuard {
    InFlightBytesGuard(0)
}

pub fn in_flight_upload_bytes() -> u64 {
    IN_FLIGHT_UPLOAD_BYTES.load(Ordering::Acquire)
}

pub struct InFlightFilesGuard(u64);

impl InFlightFilesGuard {
    pub fn reserved(&self) -> u64 {
        self.0
    }

    /// Re-targets the lease at `new_total` reserved files; on failure the reservation is unchanged.
    pub fn try_resize(&mut self, new_total: u64, max: u64) -> bool {
        try_resize_counter(&IN_FLIGHT_UPLOAD_FILES, &mut self.0, new_total, max)
    }
}

impl Drop for InFlightFilesGuard {
    fn drop(&mut self) {
        IN_FLIGHT_UPLOAD_FILES.fetch_sub(self.0, Ordering::AcqRel);
    }
}

pub fn reserve_in_flight_files() -> InFlightFilesGuard {
    InFlightFilesGuard(0)
}

pub fn in_flight_upload_files() -> u64 {
    IN_FLIGHT_UPLOAD_FILES.load(Ordering::Acquire)
}

pub enum PayloadAccountError {
    PayloadTooLarge,
    BudgetExhausted,
}

/// Accounts newly parsed payload bytes, growing `lease` to the running total; the per-request payload cap is checked before the shared budget.
pub fn account_payload_bytes(
    total_bytes: &mut usize,
    added: usize,
    max_payload_bytes: usize,
    lease: &mut InFlightBytesGuard,
    max_in_flight_bytes: u64,
) -> Result<(), PayloadAccountError> {
    *total_bytes = total_bytes.saturating_add(added);
    if *total_bytes > max_payload_bytes {
        return Err(PayloadAccountError::PayloadTooLarge);
    }
    if !lease.try_resize(*total_bytes as u64, max_in_flight_bytes) {
        return Err(PayloadAccountError::BudgetExhausted);
    }
    Ok(())
}

/// A request's declared Content-Length, distinguishing an absent header from a malformed one.
pub enum DeclaredContentLength {
    Absent,
    /// Present but not matching `^[0-9]+$`.
    Invalid,
    Bytes(u64),
}

pub fn declared_content_length(headers: &HeaderMap) -> DeclaredContentLength {
    let Some(value) = headers.get(header::CONTENT_LENGTH) else {
        return DeclaredContentLength::Absent;
    };
    let Ok(s) = value.to_str() else {
        return DeclaredContentLength::Invalid;
    };
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return DeclaredContentLength::Invalid;
    }
    match s.parse::<u64>() {
        Ok(n) => DeclaredContentLength::Bytes(n),
        Err(_) => DeclaredContentLength::Bytes(u64::MAX),
    }
}

pub struct UploadSlotGuard(());

impl Drop for UploadSlotGuard {
    fn drop(&mut self) {
        ACTIVE_UPLOADS.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Admits an upload only while fewer than `max` uploads are active, independently of the byte budget.
pub fn try_acquire_upload_slot(max: u64) -> Option<UploadSlotGuard> {
    let mut current = ACTIVE_UPLOADS.load(Ordering::Acquire);
    loop {
        if current >= max {
            return None;
        }
        match ACTIVE_UPLOADS.compare_exchange_weak(
            current,
            current + 1,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return Some(UploadSlotGuard(())),
            Err(actual) => current = actual,
        }
    }
}

pub fn active_uploads() -> u64 {
    ACTIVE_UPLOADS.load(Ordering::Acquire)
}

pub const DEFAULT_MAX_CONCURRENT_UPLOADS_PER_SOURCE: u64 = 4;

pub const DEFAULT_MAX_IN_FLIGHT_UPLOAD_BYTES_PER_SOURCE: u64 = 350 * 1024 * 1024;

pub const SOURCE_LIMIT_MESSAGE: &str =
    "Too many uploads in progress from this client, please retry shortly.";

const SOURCE_RETRY_AFTER_SECONDS: u64 = 5;

/// Uploads and charged bytes in flight per client source; only sources with an upload hold an entry.
static SOURCES: LazyLock<Mutex<HashMap<String, (u64, u64)>>> = LazyLock::new(Default::default);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceLimitReason {
    Concurrency,
    Bytes,
}

impl SourceLimitReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Concurrency => "source_concurrency",
            Self::Bytes => "source_bytes",
        }
    }
}

/// One admitted upload's share of its source's allowance, returned on drop.
pub struct SourceUploadLease {
    source: String,
    bytes: u64,
}

impl Drop for SourceUploadLease {
    fn drop(&mut self) {
        let mut sources = SOURCES.lock();
        if let Some((uploads, bytes)) = sources.get_mut(&self.source) {
            *uploads = uploads.saturating_sub(1);
            *bytes = bytes.saturating_sub(self.bytes);
            if *uploads == 0 {
                sources.remove(&self.source);
            }
        }
    }
}

/// What a request is charged against its source before its body is read: the declared length, else the route's cap.
pub fn source_charge(declared: Option<u64>, max_request_bytes: u64) -> u64 {
    declared.map_or(max_request_bytes, |d| d.min(max_request_bytes))
}

/// Admits an upload of up to `bytes` from `source` while it has fewer than `max_uploads` and `max_bytes` in flight.
pub fn try_acquire_source(
    source: &str,
    bytes: u64,
    max_uploads: u64,
    max_bytes: u64,
) -> Result<SourceUploadLease, SourceLimitReason> {
    let mut sources = SOURCES.lock();
    let entry = sources.entry(source.to_string()).or_insert((0, 0));
    let result = if entry.0 >= max_uploads {
        Err(SourceLimitReason::Concurrency)
    } else if entry.1.saturating_add(bytes) > max_bytes {
        Err(SourceLimitReason::Bytes)
    } else {
        entry.0 += 1;
        entry.1 += bytes;
        Ok(SourceUploadLease {
            source: source.to_string(),
            bytes,
        })
    };
    if entry.0 == 0 {
        sources.remove(source);
    }
    result
}

/// Per-source admission for a multipart route (`route` labels the metrics). A request without a
/// client address (internal callers, no gateway) is not limited per source, only counted.
pub fn admit_source(
    headers: &HeaderMap,
    limits: &UploadLimits,
    route: &'static str,
    max_request_bytes: u64,
) -> Result<Option<SourceUploadLease>, SourceLimitReason> {
    let ip = crate::shared::client_ip::client_ip(headers);
    if ip.is_empty() {
        metrics::counter!("multipart_upload_unattributed", "route" => route).increment(1);
        return Ok(None);
    }
    let declared = match declared_content_length(headers) {
        DeclaredContentLength::Bytes(n) => Some(n),
        _ => None,
    };
    let source = crate::shared::client_ip::source_key(&ip);
    try_acquire_source(
        &source,
        source_charge(declared, max_request_bytes),
        limits.max_concurrent_uploads_per_source,
        limits.max_in_flight_upload_bytes_per_source,
    )
    .map(Some)
    .inspect_err(|reason| {
        metrics::counter!("multipart_upload_rejections", "route" => route, "reason" => reason.as_str())
            .increment(1);
        tracing::warn!(route, source = %source, reason = reason.as_str(), "multipart upload shed: per-source limit");
    })
}

pub fn source_limit_response() -> axum::response::Response {
    use axum::response::IntoResponse;
    (
        axum::http::StatusCode::TOO_MANY_REQUESTS,
        [("Retry-After", SOURCE_RETRY_AFTER_SECONDS.to_string())],
        axum::Json(DeployRejection {
            error: "Too Many Requests".to_string(),
            message: SOURCE_LIMIT_MESSAGE.to_string(),
        }),
    )
        .into_response()
}

// The shed and timeout answers live with the worlds envelope, whose
// `ApiError` renders them too.
pub use crate::comms::worlds_envelope::{DeployRejection, shed_response, timeout_response};

/// 413 `{"error":"Payload Too Large","message":…}`, as upstream's multipart
/// parser answers a size or count limit (`upload-error-handler.ts`).
pub fn too_large(message: impl Into<String>) -> crate::comms::worlds_envelope::ApiError {
    crate::comms::worlds_envelope::ApiError::http(413, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use serde_json::json;

    #[test]
    fn upload_slot_cap_is_enforced_and_released_on_drop() {
        let a = try_acquire_upload_slot(u64::MAX).expect("first slot");
        let base = active_uploads();
        let b = try_acquire_upload_slot(base + 1).expect("slot under cap");
        assert!(try_acquire_upload_slot(active_uploads()).is_none());
        drop(b);
        drop(a);
    }

    #[test]
    fn byte_lease_admission_is_free_and_grows_from_parsed_bytes() {
        let mut a = reserve_in_flight();
        assert_eq!(a.reserved(), 0);
        assert!(a.try_resize(100, u64::MAX));
        assert_eq!(a.reserved(), 100);
        assert!(a.try_resize(40, u64::MAX));
        assert_eq!(a.reserved(), 40);
        assert!(in_flight_upload_bytes() >= 40);

        let cap = in_flight_upload_bytes();
        let mut b = reserve_in_flight();
        assert!(!b.try_resize(cap.saturating_add(1), cap));
        assert_eq!(b.reserved(), 0);
        assert!(b.try_resize(1, u64::MAX));
        drop(b);
        drop(a);
    }

    #[test]
    fn resize_rejects_sums_that_would_overflow_even_under_a_max_budget() {
        let mut a = reserve_in_flight();
        assert!(a.try_resize(2, u64::MAX));
        let mut b = reserve_in_flight();
        assert!(!b.try_resize(u64::MAX, u64::MAX));
        assert_eq!(b.reserved(), 0);
        drop(a);
    }

    #[test]
    fn busboy_default_limits_and_messages_match_upstream() {
        assert_eq!(MAX_MULTIPART_FIELD_VALUE_BYTES, 1024 * 1024);
        assert_eq!(MAX_MULTIPART_FIELDS, 100);
        assert_eq!(MAX_MULTIPART_PARTS, 10_100);
        assert_eq!(
            TOO_MANY_FIELDS_MESSAGE,
            "The multipart request has too many fields."
        );
        assert_eq!(
            TOO_MANY_PARTS_MESSAGE,
            "The multipart request has too many parts."
        );
    }

    #[test]
    fn files_lease_bounds_the_aggregate_and_message_matches_upstream() {
        assert_eq!(
            FILES_SHED_MESSAGE,
            "Server is buffering too many upload files, please retry shortly."
        );
        let mut a = reserve_in_flight_files();
        assert!(a.try_resize(5, u64::MAX));
        assert_eq!(a.reserved(), 5);
        let cap = in_flight_upload_files();
        let mut b = reserve_in_flight_files();
        assert!(!b.try_resize(cap.saturating_add(1), cap));
        assert_eq!(b.reserved(), 0);
        assert!(b.try_resize(1, u64::MAX));
        drop(b);
        drop(a);
    }

    #[test]
    fn payload_accounting_checks_payload_cap_before_budget() {
        let mut lease = reserve_in_flight();
        let mut total = 0usize;
        assert!(account_payload_bytes(&mut total, 10, 100, &mut lease, u64::MAX).is_ok());
        assert_eq!(total, 10);
        assert_eq!(lease.reserved(), 10);

        assert!(matches!(
            account_payload_bytes(&mut total, 1000, 100, &mut lease, 0),
            Err(PayloadAccountError::PayloadTooLarge)
        ));

        let mut lease2 = reserve_in_flight();
        let mut total2 = 0usize;
        assert!(matches!(
            account_payload_bytes(&mut total2, 10, 100, &mut lease2, 0),
            Err(PayloadAccountError::BudgetExhausted)
        ));
        assert_eq!(lease2.reserved(), 0);
    }

    #[test]
    fn content_length_parsing_distinguishes_absent_invalid_and_numeric() {
        use axum::http::HeaderValue;

        let headers = HeaderMap::new();
        assert!(matches!(
            declared_content_length(&headers),
            DeclaredContentLength::Absent
        ));

        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from_static("1234"));
        assert!(matches!(
            declared_content_length(&headers),
            DeclaredContentLength::Bytes(1234)
        ));

        for bad in ["+5", "-5", "abc", "12 ", " 12", "1.5", "0x10", ""] {
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_LENGTH, HeaderValue::from_str(bad).unwrap());
            assert!(
                matches!(
                    declared_content_length(&headers),
                    DeclaredContentLength::Invalid
                ),
                "expected Invalid for {bad:?}"
            );
        }

        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_static("99999999999999999999999999"),
        );
        assert!(matches!(
            declared_content_length(&headers),
            DeclaredContentLength::Bytes(u64::MAX)
        ));
    }

    #[tokio::test]
    async fn shed_and_timeout_responses_match_upstream_wire_shape() {
        let shed = shed_response(CONCURRENCY_SHED_MESSAGE);
        assert_eq!(shed.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            shed.headers().get("Retry-After").unwrap().to_str().unwrap(),
            "5"
        );
        let body = axum::body::to_bytes(shed.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            v,
            json!({
                "error": "Service Unavailable",
                "message": "Server is handling too many concurrent uploads, please retry shortly."
            })
        );

        let to = timeout_response(MULTIPART_TIMEOUT_MESSAGE);
        assert_eq!(to.status(), StatusCode::REQUEST_TIMEOUT);
        assert!(to.headers().get("Retry-After").is_none());
        let body = axum::body::to_bytes(to.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            v,
            json!({ "error": "Request Timeout", "message": "The multipart upload timed out." })
        );
    }

    #[test]
    fn a_source_is_bounded_by_uploads_and_bytes_and_released_on_drop() {
        let source = format!("test-source-{}", rand::random::<u64>());
        let a = try_acquire_source(&source, 100, 2, 250).unwrap();
        assert_eq!(
            try_acquire_source(&source, 151, 2, 250).err(),
            Some(SourceLimitReason::Bytes)
        );
        let b = try_acquire_source(&source, 150, 2, 250).unwrap();
        assert_eq!(
            try_acquire_source(&source, 0, 2, 250).err(),
            Some(SourceLimitReason::Concurrency)
        );
        assert!(try_acquire_source("another-source", 250, 2, 250).is_ok());
        drop(a);
        drop(b);
        assert!(
            !SOURCES.lock().contains_key(&source),
            "an idle source holds no entry"
        );
        assert_eq!(source_charge(None, 350), 350);
        assert_eq!(source_charge(Some(10), 350), 10);
        assert_eq!(source_charge(Some(1_000), 350), 350);
    }

    #[test]
    fn limiter_knobs_reject_zero_with_upstream_message_shape() {
        assert_eq!(positive_limit("MAX_CONCURRENT_UPLOADS", 40).unwrap(), 40);
        let err = positive_limit("MAX_CONCURRENT_UPLOADS", 0).unwrap_err();
        assert_eq!(
            err.to_string(),
            "MAX_CONCURRENT_UPLOADS must be a positive integer, got 0"
        );
    }

    /// `MAX_IN_FLIGHT_UPLOAD_BYTES_PER_SOURCE` is shared with content's
    /// deploys: the config refuses what this check would refuse at startup.
    #[test]
    fn the_shared_per_source_byte_floor_is_one_world_deploy() {
        let max_upload = super::super::deploy::MAX_UPLOAD_SIZE_BYTES as u64;
        let parse = |bytes: u64| {
            crate::config::Config::parse_for_tests(&[
                "x",
                "--database-url",
                "postgres:///c",
                "--max-in-flight-upload-bytes-per-source",
                &bytes.to_string(),
            ])
        };
        assert!(parse(max_upload - 1).is_err());
        let set = parse(max_upload)
            .unwrap()
            .content
            .max_in_flight_upload_bytes_per_source
            .unwrap();
        let limits = UploadLimits {
            max_in_flight_upload_bytes_per_source: set,
            ..UploadLimits::default()
        };
        assert!(limits.validate(max_upload).is_ok());
        assert!(DEFAULT_MAX_IN_FLIGHT_UPLOAD_BYTES_PER_SOURCE >= max_upload);
    }

    #[test]
    fn byte_budget_must_cover_the_deploy_payload_cap() {
        let max_upload = super::super::deploy::MAX_UPLOAD_SIZE_BYTES as u64;
        let limits = |bytes: u64| UploadLimits {
            max_in_flight_upload_bytes: bytes,
            ..UploadLimits::default()
        };
        assert!(limits(max_upload).validate(max_upload).is_ok());
        assert!(limits(max_upload + 1).validate(max_upload).is_ok());
        let err = limits(max_upload - 1).validate(max_upload).unwrap_err();
        assert!(
            err.to_string()
                .contains("must be greater than or equal to maxSizeInBytes"),
            "unexpected message: {err}"
        );
        assert!(
            UploadLimits {
                max_concurrent_uploads: 0,
                ..UploadLimits::default()
            }
            .validate(max_upload)
            .is_err()
        );
    }
}
