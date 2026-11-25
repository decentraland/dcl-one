//! Source-compatible HTTP metric names shared by Social and Worlds.
use crate::{Descriptor, PROM_CLIENT_DEFAULT_BUCKETS, Unit};

pub const HTTP_REQUEST_DURATION_SECONDS: Descriptor = Descriptor::histogram(
    "http_request_duration_seconds",
    "Request duration in seconds.",
    Some(Unit::Seconds),
    &["code", "handler", "method"],
    PROM_CLIENT_DEFAULT_BUCKETS,
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
    PROM_CLIENT_DEFAULT_BUCKETS,
);
pub const FAMILIES: &[Descriptor] = &[
    HTTP_REQUEST_DURATION_SECONDS,
    HTTP_REQUEST_SIZE_BYTES,
    HTTP_REQUESTS_TOTAL,
];
