//! The builder's wire contract, as builder-server answers it: successes are
//! `{"ok":true,"data":...}` ([`ApiData`]); failures are
//! `{"ok":false,"data":...,"error":msg}` with the refusal's status and
//! `data` (`{}` unless the refusal names more). A database or internal
//! failure answers 500 `Internal Server Error` and never its detail. Every
//! timestamp is written as a JavaScript `Date` serializes:
//! `YYYY-MM-DDTHH:MM:SS.mmmZ` ([`iso`]).

use std::fmt;

use axum::Json;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use serde::Serializer;
use serde_json::{Value, json};

pub use crate::shared::errors::ApiData;

const INTERNAL_MESSAGE: &str = "Internal Server Error";
const TOO_MANY: &str = "Too many requests, try again in a minute";

/// What a published or locked refusal is about: upstream words the two
/// differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subject {
    Collection,
    Item,
}

#[derive(Debug)]
pub enum ApiError {
    Http {
        status: u16,
        error: String,
        data: Value,
    },
    Database(sqlx::Error),
    Internal(String),
    /// 429 with a `Retry-After` of the wait, in whole seconds.
    Limited(std::time::Duration),
}

impl ApiError {
    pub fn http(status: u16, error: impl Into<String>) -> Self {
        Self::Http {
            status,
            error: error.into(),
            data: json!({}),
        }
    }
    pub fn bad_request(error: impl Into<String>) -> Self {
        Self::http(400, error)
    }
    pub fn unauthorized(error: impl Into<String>) -> Self {
        Self::http(401, error)
    }
    pub fn forbidden(error: impl Into<String>) -> Self {
        Self::http(403, error)
    }
    pub fn not_found(error: impl Into<String>) -> Self {
        Self::http(404, error)
    }
    pub fn conflict(error: impl Into<String>) -> Self {
        Self::http(409, error)
    }
    pub fn service_unavailable(error: impl Into<String>) -> Self {
        Self::http(503, error)
    }
    pub fn internal(detail: impl Into<String>) -> Self {
        Self::Internal(detail.into())
    }

    /// The refusal with upstream's `data` for it.
    pub fn with_data(self, data: Value) -> Self {
        match self {
            Self::Http { status, error, .. } => Self::Http {
                status,
                error,
                data,
            },
            other => other,
        }
    }

    /// `withAuthentication`'s refusal: 401 `Unauthenticated`, `message`
    /// under `data`.
    pub fn unauthenticated(message: &str) -> Self {
        Self::http(401, "Unauthenticated").with_data(json!({ "message": message }))
    }

    /// 409 for a write to published content; `action` is upstream's
    /// (`deleted`, `updated`, `updated or inserted`, `inserted`,
    /// `inserted or updated`, `updated with a new rarity`).
    pub fn published(subject: Subject, id: &str, action: &str) -> Self {
        let error = match subject {
            Subject::Collection => format!("The collection is published. It can't be {action}."),
            Subject::Item => format!(
                "The collection that contains this item has been already published. The item can't be {action}."
            ),
        };
        Self::http(409, error).with_data(json!({ "id": id }))
    }

    /// 423 for a write to a locked collection or one of its items.
    pub fn locked(subject: Subject, id: &str, action: &str) -> Self {
        let error = match subject {
            Subject::Collection => format!("The collection is locked. It can't be {action}."),
            Subject::Item => {
                format!("The collection for the item is locked. The item can't be {action}.")
            }
        };
        Self::http(423, error).with_data(json!({ "id": id }))
    }

    /// `GET /v1/items/:collectionAddress/:itemId/contents` for an item no
    /// published collection holds: upstream's handler throws without a
    /// status, so it answers 200 with `ok:false` (its `Item.router.spec.ts`
    /// asserts it).
    pub fn item_does_not_exist() -> Self {
        Self::http(200, "The item doesn't exist.")
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::Http { status, .. } => *status,
            Self::Database(_) | Self::Internal(_) => 500,
            Self::Limited(_) => 429,
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http { error, .. } => f.write_str(error),
            Self::Database(e) => write!(f, "database error: {e}"),
            Self::Internal(m) => f.write_str(m),
            Self::Limited(_) => f.write_str(TOO_MANY),
        }
    }
}

impl std::error::Error for ApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Database(e) => Some(e),
            _ => None,
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        Self::Database(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error, data) = match self {
            Self::Limited(wait) => {
                let seconds = wait.as_secs() + u64::from(wait.subsec_nanos() > 0);
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    [(header::RETRY_AFTER, seconds.clamp(1, 86_400).to_string())],
                    Json(json!({ "ok": false, "data": {}, "error": TOO_MANY })),
                )
                    .into_response();
            }
            Self::Http {
                status,
                error,
                data,
            } => (status, error, data),
            Self::Database(e) => {
                tracing::error!(error = %e, "sqlx error on a builder route");
                (500, INTERNAL_MESSAGE.to_string(), json!({}))
            }
            Self::Internal(m) => {
                tracing::error!(error = %m, "internal error on a builder route");
                (500, INTERNAL_MESSAGE.to_string(), json!({}))
            }
        };
        let status = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (
            status,
            Json(json!({ "ok": false, "data": data, "error": error })),
        )
            .into_response()
    }
}

/// `t` as `Date.prototype.toISOString` writes it.
pub fn iso(t: &DateTime<Utc>) -> String {
    t.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// `serialize_with` for a timestamp field.
pub fn iso_timestamp<S: Serializer>(t: &DateTime<Utc>, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&iso(t))
}

/// `serialize_with` for an optional timestamp field (`null` when absent).
pub fn iso_timestamp_opt<S: Serializer>(
    t: &Option<DateTime<Utc>>,
    s: S,
) -> Result<S::Ok, S::Error> {
    match t {
        Some(t) => iso_timestamp(t, s),
        None => s.serialize_none(),
    }
}

/// A refusal the router answered itself (an extractor's, a method the route
/// does not serve) in the envelope, with the status's own words instead of
/// the parser's; an unusable JSON body is upstream's `Invalid request body`.
pub async fn enveloped(response: Response) -> Response {
    let status = response.status();
    let is_json = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"));
    if is_json || !(status.is_client_error() || status.is_server_error()) {
        return response;
    }
    let (status, error) = match status {
        StatusCode::UNPROCESSABLE_ENTITY | StatusCode::UNSUPPORTED_MEDIA_TYPE => {
            (400, "Invalid request body")
        }
        other => (other.as_u16(), other.canonical_reason().unwrap_or("Error")),
    };
    let (mut parts, _) = response.into_parts();
    parts.headers.remove(header::CONTENT_TYPE);
    parts.headers.remove(header::CONTENT_LENGTH);
    let mut answer = ApiError::http(status, error).into_response();
    answer.headers_mut().extend(parts.headers);
    answer
}

/// `Cache-Control: private, no-store` on a route that reads a signature.
pub async fn private_response(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("private, no-store"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn wire(err: ApiError) -> (StatusCode, Value) {
        let resp = err.into_response();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn a_refusal_is_the_upstream_failure_envelope() {
        assert_eq!(
            wire(ApiError::not_found("Not found")).await,
            (
                StatusCode::NOT_FOUND,
                json!({ "ok": false, "data": {}, "error": "Not found" })
            )
        );
        assert_eq!(
            wire(
                ApiError::unauthorized("Unauthorized").with_data(json!({ "eth_address": "0xabc" }))
            )
            .await,
            (
                StatusCode::UNAUTHORIZED,
                json!({ "ok": false, "data": { "eth_address": "0xabc" }, "error": "Unauthorized" })
            )
        );
        assert_eq!(
            wire(ApiError::unauthenticated("Invalid signature")).await,
            (
                StatusCode::UNAUTHORIZED,
                json!({ "ok": false, "data": { "message": "Invalid signature" }, "error": "Unauthenticated" })
            )
        );
    }

    #[tokio::test]
    async fn published_and_locked_refusals_carry_upstreams_text_and_id() {
        assert_eq!(
            wire(ApiError::published(
                Subject::Collection,
                "c1",
                "updated or inserted"
            ))
            .await,
            (
                StatusCode::CONFLICT,
                json!({ "ok": false, "data": { "id": "c1" }, "error": "The collection is published. It can't be updated or inserted." })
            )
        );
        assert_eq!(
            wire(ApiError::published(Subject::Item, "i1", "deleted")).await,
            (
                StatusCode::CONFLICT,
                json!({ "ok": false, "data": { "id": "i1" }, "error": "The collection that contains this item has been already published. The item can't be deleted." })
            )
        );
        assert_eq!(
            wire(ApiError::locked(Subject::Collection, "c1", "deleted")).await,
            (
                StatusCode::LOCKED,
                json!({ "ok": false, "data": { "id": "c1" }, "error": "The collection is locked. It can't be deleted." })
            )
        );
        assert_eq!(
            wire(ApiError::locked(Subject::Item, "i1", "inserted or updated")).await,
            (
                StatusCode::LOCKED,
                json!({ "ok": false, "data": { "id": "i1" }, "error": "The collection for the item is locked. The item can't be inserted or updated." })
            )
        );
    }

    #[tokio::test]
    async fn the_missing_item_contents_answer_ok_false_with_200() {
        assert_eq!(
            wire(ApiError::item_does_not_exist()).await,
            (
                StatusCode::OK,
                json!({ "ok": false, "data": {}, "error": "The item doesn't exist." })
            )
        );
    }

    #[tokio::test]
    async fn database_and_internal_errors_do_not_expose_details() {
        for err in [
            ApiError::Database(sqlx::Error::Protocol(
                "relation \"builder.items\" at /var/lib/x".into(),
            )),
            ApiError::internal("storing bafy: /srv/blobs/ab/cd: permission denied"),
        ] {
            assert_eq!(
                wire(err).await,
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({ "ok": false, "data": {}, "error": "Internal Server Error" })
                )
            );
        }
    }

    #[test]
    fn timestamps_are_written_as_javascript_dates() {
        let t = DateTime::parse_from_rfc3339("2023-11-14T22:13:20.123456+00:00")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(iso(&t), "2023-11-14T22:13:20.123Z");
        let whole = DateTime::from_timestamp_millis(1_767_225_600_000).unwrap();
        assert_eq!(iso(&whole), "2026-01-01T00:00:00.000Z");
        #[derive(serde::Serialize)]
        struct Row {
            #[serde(serialize_with = "iso_timestamp")]
            at: DateTime<Utc>,
            #[serde(serialize_with = "iso_timestamp_opt")]
            maybe: Option<DateTime<Utc>>,
        }
        assert_eq!(
            serde_json::to_value(Row {
                at: whole,
                maybe: None
            })
            .unwrap(),
            json!({ "at": "2026-01-01T00:00:00.000Z", "maybe": null })
        );
    }
}
