//! Camera reel response envelopes and request extraction. Domain errors use
//! JSON; malformed requests and signed-fetch failures use plain text.

use axum::Json;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ForbiddenReason {
    MaxLimitReached,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForbiddenError {
    pub reason: ForbiddenReason,
    pub message: String,
}

impl ForbiddenError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            reason: ForbiddenReason::MaxLimitReached,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseError {
    pub message: String,
}

impl ResponseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("{1}")]
    PlainText(StatusCode, String),
    #[error("Unathorized")]
    SignedFetchUnauthorized,
    #[error("{0}")]
    BadRequest(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden(String),
    #[error("max limit reached")]
    MaxLimitReached(String),
    #[error("{0}")]
    NotFound(String),
    #[error("bad gateway: {0}")]
    BadGateway(String),
    #[error("{0}")]
    Internal(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            ApiError::PlainText(status, message) => (status, message).into_response(),
            ApiError::SignedFetchUnauthorized => {
                (StatusCode::UNAUTHORIZED, "Unathorized").into_response()
            }
            ApiError::MaxLimitReached(m) => {
                (StatusCode::FORBIDDEN, Json(ForbiddenError::new(m))).into_response()
            }
            ApiError::BadRequest(m) => {
                (StatusCode::BAD_REQUEST, Json(ResponseError::new(m))).into_response()
            }
            ApiError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                Json(ResponseError::new("Unauthorized")),
            )
                .into_response(),
            ApiError::Forbidden(m) => {
                (StatusCode::FORBIDDEN, Json(ResponseError::new(m))).into_response()
            }
            ApiError::NotFound(m) => {
                (StatusCode::NOT_FOUND, Json(ResponseError::new(m))).into_response()
            }
            ApiError::BadGateway(m) => {
                (StatusCode::BAD_GATEWAY, Json(ResponseError::new(m))).into_response()
            }
            ApiError::Internal(m) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ResponseError::new(m)),
            )
                .into_response(),
            ApiError::Database(e) => {
                tracing::error!(error = %e, "sqlx error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ResponseError::new("database error")),
                )
                    .into_response()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn error_envelope_wire_shape() {
        let resp = ApiError::MaxLimitReached("gallery is full".to_string()).into_response();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            v,
            json!({ "reason": "maxLimitReached", "message": "gallery is full" })
        );
    }
}

/// Query parsing preserves the client-visible form parser and its error envelope.
#[derive(Debug)]
pub struct Query<T>(pub T);

impl<T, S> axum::extract::FromRequestParts<S> for Query<T>
where
    T: serde::de::DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        serde_html_form::from_str(parts.uri.query().unwrap_or_default())
            .map(Self)
            .map_err(|error| {
                ApiError::PlainText(
                    StatusCode::BAD_REQUEST,
                    format!("Query deserialize error: {error}"),
                )
            })
    }
}

/// JSON requests retain the 2 MiB limit and plain-text parsing failures.
pub struct JsonBody<T>(pub T);

impl<T, S> axum::extract::FromRequest<S> for JsonBody<T>
where
    T: serde::de::DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(
        request: axum::extract::Request,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        const LIMIT: usize = 2_097_152;
        let content_type = request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        let json = content_type
            .split_once('/')
            .is_some_and(|(_, subtype)| subtype == "json" || subtype.ends_with("+json"));
        if !json {
            return Err(ApiError::PlainText(
                StatusCode::BAD_REQUEST,
                "Content type error".into(),
            ));
        }
        if let Some(length) = request
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|length| *length > LIMIT)
        {
            return Err(ApiError::PlainText(
                StatusCode::PAYLOAD_TOO_LARGE,
                format!(
                    "JSON payload ({length} bytes) is larger than allowed (limit: {LIMIT} bytes)."
                ),
            ));
        }
        let body = axum::body::to_bytes(request.into_body(), LIMIT)
            .await
            .map_err(|_| {
                ApiError::PlainText(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    format!("JSON payload has exceeded limit ({LIMIT} bytes)."),
                )
            })?;
        serde_json::from_slice(&body).map(Self).map_err(|error| {
            ApiError::PlainText(
                StatusCode::BAD_REQUEST,
                format!("Json deserialize error: {error}"),
            )
        })
    }
}

/// Camera Reel's API and health scopes have different CORS policies.
pub(crate) async fn cors(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    use axum::http::{HeaderValue, Method};
    let path = request.uri().path();
    let health = path == "/health" || path.starts_with("/health/");
    let api = path == "/api" || path.starts_with("/api/");
    // Documentation is registered outside the CORS-wrapped API scope.
    if (!health && !api) || path.starts_with("/api/docs/") {
        return next.run(request).await;
    }
    let origin = request.headers().get(header::ORIGIN).cloned();
    let requested_method = request
        .headers()
        .get(header::ACCESS_CONTROL_REQUEST_METHOD)
        .and_then(|value| Method::from_bytes(value.as_bytes()).ok());
    let vary = HeaderValue::from_static(
        "Origin, Access-Control-Request-Method, Access-Control-Request-Headers",
    );
    if request.method() == Method::OPTIONS
        && let Some(method) = requested_method
    {
        let allowed = if health {
            method == Method::GET || method == Method::OPTIONS
        } else {
            matches!(
                method,
                Method::GET | Method::POST | Method::PATCH | Method::DELETE
            )
        };
        if !allowed {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(axum::body::Body::from("Requested method is not allowed"))
                .unwrap();
        }
        let mut response = StatusCode::OK.into_response();
        let headers = response.headers_mut();
        if let Some(origin) = origin {
            headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        }
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static(if health {
                "GET, OPTIONS"
            } else {
                "DELETE, GET, POST, PATCH"
            }),
        );
        if let Some(requested) = request
            .headers()
            .get(header::ACCESS_CONTROL_REQUEST_HEADERS)
        {
            headers.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, requested.clone());
        }
        headers.insert(
            header::ACCESS_CONTROL_MAX_AGE,
            HeaderValue::from_static(if health { "3600" } else { "300" }),
        );
        headers.insert(header::VARY, vary);
        return response;
    }
    let mut response = if request.method() == Method::HEAD {
        StatusCode::NOT_FOUND.into_response()
    } else {
        next.run(request).await
    };
    let headers = response.headers_mut();
    if let Some(origin) = origin {
        headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    }
    if api && !headers.is_empty() {
        let names = headers
            .keys()
            .map(|name| name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        if let Ok(names) = HeaderValue::from_str(&names) {
            headers.insert(header::ACCESS_CONTROL_EXPOSE_HEADERS, names);
        }
    }
    headers.append(header::VARY, vary);
    response
}

const METRIC_BUCKETS: [f64; 14] = [
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 20.0, 30.0, 60.0,
];

#[derive(Default)]
struct Samples {
    count: u64,
    seconds: f64,
    buckets: [u64; 14],
}

/// The reel's counters are scoped to this router, independently of other services.
type MetricSeries = std::collections::BTreeMap<(String, String, u16), Samples>;

#[derive(Default)]
pub(crate) struct Metrics(std::sync::Mutex<MetricSeries>);

impl Metrics {
    fn record(&self, method: String, handler: String, status: u16, seconds: f64) {
        let mut values = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let sample = values.entry((method, handler, status)).or_default();
        sample.count += 1;
        sample.seconds += seconds;
        for (bucket, limit) in sample.buckets.iter_mut().zip(METRIC_BUCKETS) {
            if seconds <= limit {
                *bucket += 1;
            }
        }
    }

    fn render(&self) -> String {
        use std::fmt::Write;
        let values = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if values.is_empty() {
            return String::new();
        }
        let mut output = String::from(
            "# HELP http_request_duration_seconds HTTP request duration in seconds for all requests\n# TYPE http_request_duration_seconds histogram\n",
        );
        let labels = |method: &str, handler: &str, code| {
            let escaped = handler
                .replace('\\', "\\\\")
                .replace('\n', "\\n")
                .replace('"', "\\\"");
            format!("code=\"{code}\",handler=\"{escaped}\",method=\"{method}\"")
        };
        for ((method, handler, code), sample) in values.iter() {
            let labels = labels(method, handler, code);
            for (limit, count) in METRIC_BUCKETS.iter().zip(sample.buckets) {
                writeln!(
                    output,
                    "http_request_duration_seconds_bucket{{{labels},le=\"{limit}\"}} {count}"
                )
                .unwrap();
            }
            writeln!(
                output,
                "http_request_duration_seconds_bucket{{{labels},le=\"+Inf\"}} {}",
                sample.count
            )
            .unwrap();
            writeln!(
                output,
                "http_request_duration_seconds_sum{{{labels}}} {}",
                sample.seconds
            )
            .unwrap();
            writeln!(
                output,
                "http_request_duration_seconds_count{{{labels}}} {}",
                sample.count
            )
            .unwrap();
        }
        output.push_str("# HELP http_requests_total Total number of HTTP requests\n# TYPE http_requests_total counter\n");
        for ((method, handler, code), sample) in values.iter() {
            writeln!(
                output,
                "http_requests_total{{{}}} {}",
                labels(method, handler, code),
                sample.count
            )
            .unwrap();
        }
        output
    }
}

pub(crate) async fn metrics(
    axum::extract::State(state): axum::extract::State<crate::SharedState>,
) -> String {
    state.metrics.render()
}

pub(crate) async fn observe(
    axum::extract::State(state): axum::extract::State<crate::SharedState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if request.uri().path() == "/metrics" {
        let Some(expected) = state
            .settings
            .metrics_bearer_token
            .as_deref()
            .filter(|token| !token.is_empty())
        else {
            return (StatusCode::INTERNAL_SERVER_ERROR, "").into_response();
        };
        let supplied = request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .filter(|value| value.len() > 7);
        let error = match supplied {
            None => Some("Token not present"),
            Some(value) => match value.split_once(' ') {
                Some(("Bearer", token))
                    if crate::auth::admin_bearer::timing_safe_eq(
                        token.as_bytes(),
                        expected.as_bytes(),
                    ) =>
                {
                    None
                }
                Some(("Bearer", _)) => Some("Invalid token"),
                _ => Some("Wrong schema"),
            },
        };
        if let Some(error) = error {
            return (StatusCode::UNAUTHORIZED, error).into_response();
        }
    }
    let started = std::time::Instant::now();
    let method = request.method().as_str().to_owned();
    let handler = if request.uri().path() == "/metrics" {
        "*".to_owned()
    } else {
        request
            .extensions()
            .get::<axum::extract::MatchedPath>()
            .map(|matched| {
                matched
                    .as_str()
                    .strip_prefix("/camera-reel-service")
                    .unwrap_or(matched.as_str())
                    .to_owned()
            })
            .unwrap_or_else(|| "*".to_owned())
    };
    let response = next.run(request).await;
    state.metrics.record(
        method,
        handler,
        response.status().as_u16(),
        started.elapsed().as_secs_f64(),
    );
    response
}
