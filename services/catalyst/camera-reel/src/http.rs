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
    /// The client source spent its contract-wallet checks (`eip1654_budget`).
    #[error("{0}")]
    TooManyRequests(String),
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
            ApiError::TooManyRequests(m) => (
                StatusCode::TOO_MANY_REQUESTS,
                [(axum::http::header::RETRY_AFTER, "1")],
                Json(ResponseError::new(m)),
            )
                .into_response(),
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

    /// A contract-wallet check the client source's budget refused is a 429
    /// with `Retry-After`, never the signed-fetch 401.
    #[tokio::test]
    async fn a_rate_limited_signature_check_is_a_429_with_retry_after() {
        let resp = ApiError::TooManyRequests("slow down".to_string()).into_response();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(resp.headers()[axum::http::header::RETRY_AFTER], "1");
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
        let json = request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<mime::Mime>().ok())
            .is_some_and(|value| {
                value.subtype() == mime::JSON || value.suffix() == Some(mime::JSON)
            });
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
    // Only the registered GET documentation resources bypass the API scope.
    // Unknown documentation paths and other methods still use the scope's CORS policy.
    let documentation = request.method() == Method::GET
        && (path == "/api/docs/openapi.json" || path.starts_with("/api/docs/ui/"));
    if (!health && !api) || documentation {
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
                .expect("a constant status always builds a response");
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

/// Decode an asset name once; malformed escapes stay literal and unknown
/// names, including invalid UTF-8, are not present in the asset whitelist.
pub(crate) fn decoded_path_component(value: &str) -> Option<String> {
    String::from_utf8(decoded_path_bytes(value)).ok()
}

fn decoded_path_bytes(value: &str) -> Vec<u8> {
    fn hex(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let (Some(high), Some(low)) = (
                bytes.get(index + 1).copied().and_then(hex),
                bytes.get(index + 2).copied().and_then(hex),
            )
        {
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    decoded
}

/// Match encoded spellings of static API segments before routing. Valid
/// parameter bytes remain untouched. Invalid UTF-8 is converted to replacement
/// characters and re-escaped so the Path extractor still decodes exactly once.
/// OriginalUri retains the bytes the client signed in both cases.
pub(crate) fn routing_request(mut request: axum::extract::Request) -> axum::extract::Request {
    const ROUTES: &[&str] = &[
        "api/images",
        "api/images/:id",
        "api/images/:id/metadata",
        "api/images/:id/visibility",
        "api/users/:address",
        "api/users/:address/images",
        "api/places/:id/images",
        "api/places/images",
        "api/wearables/:item/images",
        "api/docs/openapi.json",
        "api/docs/ui/*",
        "health/live",
    ];
    let path = request.uri().path();
    if !path.contains('%') {
        return request;
    }
    let raw: Vec<&str> = path.strip_prefix('/').unwrap_or(path).split('/').collect();
    let decoded: Vec<Vec<u8>> = raw.iter().map(|part| decoded_path_bytes(part)).collect();
    let mut segments: Vec<std::borrow::Cow<'_, str>> =
        raw.iter().map(|part| (*part).into()).collect();
    // The two scopes must also recognize encoded prefixes for their fallbacks.
    if let Some(prefix) = decoded.first() {
        if prefix == b"api" {
            segments[0] = "api".into();
        }
        if prefix == b"health" {
            segments[0] = "health".into();
        }
    }
    for route in ROUTES {
        let pattern: Vec<&str> = route.split('/').collect();
        let wildcard = pattern.last() == Some(&"*");
        let required = pattern.len() - usize::from(wildcard);
        if (!wildcard && pattern.len() != raw.len()) || (wildcard && raw.len() < required) {
            continue;
        }
        if !pattern
            .iter()
            .take(required)
            .enumerate()
            .all(|(index, part)| part.starts_with(':') || decoded[index] == part.as_bytes())
        {
            continue;
        }
        for (index, part) in pattern.iter().take(required).enumerate() {
            if !part.starts_with(':') {
                segments[index] = (*part).into();
            } else if let std::borrow::Cow::Owned(value) = String::from_utf8_lossy(&decoded[index])
            {
                // Escape reserved bytes too: a percent or slash inside a parameter
                // must not acquire a second meaning when Axum decodes the new URI.
                use std::fmt::Write;
                let mut encoded = String::new();
                for byte in value.bytes() {
                    if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
                        encoded.push(char::from(byte));
                    } else {
                        write!(encoded, "%{byte:02X}").expect("writing to a string");
                    }
                }
                segments[index] = encoded.into();
            }
        }
        break;
    }
    let canonical = format!("/{}", segments.join("/"));
    if canonical == path {
        return request;
    }
    let original = request.uri().clone();
    let path_and_query = if let Some(query) = original.query() {
        format!("{canonical}?{query}")
    } else {
        canonical
    };
    let mut parts = original.clone().into_parts();
    let Ok(path_and_query) = path_and_query.parse() else {
        return request;
    };
    parts.path_and_query = Some(path_and_query);
    let Ok(uri) = axum::http::Uri::from_parts(parts) else {
        return request;
    };
    if request
        .extensions()
        .get::<axum::extract::OriginalUri>()
        .is_none()
    {
        request
            .extensions_mut()
            .insert(axum::extract::OriginalUri(original));
    }
    *request.uri_mut() = uri;
    request
}
