//! HTTP families shared by the Social and Worlds compatibility views.
use crate::{Descriptor, Unit};

/// Both legacy families use prom-client's default buckets.
const SOURCE_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];
pub const HTTP_REQUEST_DURATION_SECONDS: Descriptor = Descriptor::histogram(
    "http_request_duration_seconds",
    "Request duration in seconds.",
    Some(Unit::Seconds),
    &["code", "handler", "method"],
    SOURCE_BUCKETS,
);
pub const HTTP_REQUESTS_TOTAL: Descriptor = Descriptor::counter(
    "http_requests_total",
    "Total number of HTTP requests",
    Some(Unit::Count),
    &["code", "handler", "method"],
);
pub const HTTP_REQUEST_SIZE_BYTES: Descriptor = Descriptor::histogram(
    "http_request_size_bytes",
    "Duration of HTTP requests size in bytes",
    Some(Unit::Bytes),
    &["code", "handler", "method"],
    SOURCE_BUCKETS,
);
pub const FAMILIES: &[Descriptor] = &[
    HTTP_REQUEST_DURATION_SECONDS,
    HTTP_REQUEST_SIZE_BYTES,
    HTTP_REQUESTS_TOTAL,
];
