//! The explore routes' error and success envelopes.
//!
//! An error renders as the old services' `{"ok":false,"error":msg,"code":...}`
//! body (decentraland-gatsby `entities/Route/error.js:8-29` and
//! `entities/Route/wkc/response/ErrorResponse.js:9-38`), with `data` when the
//! refusal carries detail (a validation failure). A host that still wants the
//! lambdas-style duplicate `message` key gets it by default; the explore
//! router layers [`strict_envelope`] to drop it. Database and internal errors
//! never leak their detail, and no envelope ever carries a `stack`.

use axum::Json;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::{Value, json};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("{message}")]
    Http { status: u16, message: String },

    /// A refusal with upstream's `data` object beside the message: the ajv
    /// validation detail (`errors`, `messages`, `body`).
    #[error("{message}")]
    HttpData {
        status: u16,
        message: String,
        data: Value,
    },

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("{0}")]
    Internal(String),

    /// A write refused by the node's write switch (`explore::writes`), or
    /// held by its restore stamps (`writes::refusal`): 503 with
    /// `Retry-After`, the social switch's answer in this envelope.
    #[error("{0}")]
    WritesDisabled(std::borrow::Cow<'static, str>),
}

impl ApiError {
    pub fn http(status: u16, message: impl Into<String>) -> Self {
        Self::Http {
            status,
            message: message.into(),
        }
    }
    /// An error whose envelope carries `data` (gatsby `RequestError`'s
    /// `extra`, `error.js:30-40`; wkc `ErrorResponse`'s `data`).
    pub fn with_data(status: u16, message: impl Into<String>, data: Value) -> Self {
        Self::HttpData {
            status,
            message: message.into(),
            data,
        }
    }
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::http(400, msg)
    }
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::http(401, msg)
    }
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::http(403, msg)
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::http(404, msg)
    }
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::http(409, msg)
    }
    pub fn service_unavailable(msg: impl Into<String>) -> Self {
        Self::http(503, msg)
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    /// The HTTP status the error answers.
    pub fn status(&self) -> u16 {
        match self {
            ApiError::Http { status, .. } | ApiError::HttpData { status, .. } => *status,
            ApiError::Database(_) | ApiError::Internal(_) => 500,
            ApiError::WritesDisabled(_) => 503,
        }
    }
}

/// gatsby `Route/error.js:53-64` and wkc `Response.StatusCode`
/// (`Response.js:349-`): the snake-cased status text.
pub fn code_for_status(status: u16) -> &'static str {
    match status {
        400 => "bad_request",
        401 => "unauthorized",
        403 => "forbidden",
        404 => "not_found",
        405 => "method_not_allowed",
        408 => "request_timeout",
        409 => "conflict",
        410 => "gone",
        413 => "payload_too_large",
        415 => "unsupported_media_type",
        418 => "i_am_a_teapot",
        422 => "unprocessable_entity",
        429 => "too_many_requests",
        500 => "internal_server_error",
        501 => "not_implemented",
        502 => "bad_gateway",
        503 => "service_unavailable",
        504 => "gateway_timeout",
        _ => "error",
    }
}

/// What an [`ApiError`] answered, attached to its response as an extension so
/// a router can re-render the body in its own envelope ([`strict_envelope`]).
#[derive(Debug, Clone)]
pub struct ErrorEnvelope {
    pub status: u16,
    pub message: String,
    pub code: &'static str,
    pub data: Option<Value>,
}

impl ErrorEnvelope {
    /// Upstream's body: `{ok:false, error, code[, data]}` and nothing else
    /// (`error.js:8-29`; `data` only when it has keys, `error.js:22-24`).
    pub fn upstream_body(&self) -> Value {
        let mut body = json!({
            "ok": false,
            "error": self.message,
            "code": self.code,
        });
        if let Some(data) = &self.data
            && data.as_object().is_some_and(|o| !o.is_empty())
        {
            body["data"] = data.clone();
        }
        body
    }

    /// The default rendering: upstream's body plus the duplicate `message`
    /// key the lambdas-style hosts still answer.
    fn legacy_body(&self) -> Value {
        let mut body = self.upstream_body();
        body["message"] = Value::String(self.message.clone());
        body
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let retry_after = matches!(self, ApiError::WritesDisabled(_));
        let (status, message, data) = match self {
            ApiError::WritesDisabled(message) => (503, message.into_owned(), None),
            ApiError::Http { status, message } => (status, message, None),
            ApiError::HttpData {
                status,
                message,
                data,
            } => (status, message, Some(data)),
            ApiError::Database(err) => {
                tracing::error!(error = %err, "sqlx error on an explore route");
                (500, "database error".to_string(), None)
            }
            ApiError::Internal(m) => {
                tracing::error!(error = %m, "internal error on an explore route");
                (500, "internal error".to_string(), None)
            }
        };
        let envelope = ErrorEnvelope {
            status,
            message,
            code: code_for_status(status),
            data,
        };
        let status_code = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut response = (status_code, Json(envelope.legacy_body())).into_response();
        if retry_after {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from_static("3600"));
        }
        response.extensions_mut().insert(envelope);
        response
    }
}

/// Re-renders every [`ApiError`] answer that passed through as upstream's
/// envelope (`{ok:false, error, code[, data]}`), keeping its status and
/// headers. Layer it on a router whose clients were written against the old
/// places and events services.
pub async fn strict_envelope(request: Request, next: Next) -> Response {
    let response = next.run(request).await;
    let Some(envelope) = response.extensions().get::<ErrorEnvelope>().cloned() else {
        return response;
    };
    let (mut parts, _) = response.into_parts();
    let body = serde_json::to_vec(&envelope.upstream_body()).unwrap_or_default();
    parts.headers.remove(header::CONTENT_LENGTH);
    parts.headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    Response::from_parts(parts, Body::from(body))
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "places/"))]
pub struct ApiData<T> {
    pub ok: bool,
    pub data: T,
}

impl<T: Serialize> ApiData<T> {
    pub fn ok(data: T) -> Self {
        Self { ok: true, data }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "places/"))]
pub struct ApiDataTotal<T> {
    pub ok: bool,
    pub data: Vec<T>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total: i64,
}

impl<T: Serialize> ApiDataTotal<T> {
    pub fn ok(data: Vec<T>, total: i64) -> Self {
        Self {
            ok: true,
            data,
            total,
        }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "places/"))]
pub struct ApiDataTotalMap<T> {
    pub ok: bool,
    pub data: std::collections::BTreeMap<String, T>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total: i64,
}

impl<T: Serialize> ApiDataTotalMap<T> {
    pub fn ok(data: std::collections::BTreeMap<String, T>, total: i64) -> Self {
        Self {
            ok: true,
            data,
            total,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::routing::get;
    use tower::ServiceExt;

    async fn body(resp: Response) -> Value {
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn the_default_envelope_keeps_the_legacy_message_key_and_a_code() {
        let resp = ApiError::not_found("Not found place \"x\"").into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            body(resp).await,
            json!({
                "ok": false,
                "error": "Not found place \"x\"",
                "message": "Not found place \"x\"",
                "code": "not_found"
            })
        );
    }

    #[tokio::test]
    async fn database_errors_do_not_expose_details() {
        let resp = ApiError::Database(sqlx::Error::PoolClosed).into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let v = body(resp).await;
        assert_eq!(v["error"], "database error");
        assert_eq!(v["code"], "internal_server_error");
        assert!(v.get("stack").is_none());
    }

    /// gatsby `error.js:8-29` / wkc `ErrorResponse.js:9-38`: `error`, `code`
    /// and `data` only; the strict layer drops the duplicate `message`.
    #[tokio::test]
    async fn the_strict_layer_renders_upstream_envelopes_with_data() {
        let app = Router::new()
            .route(
                "/plain",
                get(|| async { ApiError::unauthorized("Missing Authorization") }),
            )
            .route(
                "/data",
                get(|| async {
                    ApiError::with_data(
                        400,
                        "Invalid data was sent to the server",
                        json!({"messages": ["x"], "body": {}}),
                    )
                }),
            )
            .route(
                "/empty-data",
                get(|| async { ApiError::with_data(400, "nothing", json!({})) }),
            )
            .route("/ok", get(|| async { Json(json!({"ok": true})) }))
            .route(
                "/held",
                get(|| async { ApiError::WritesDisabled("held".into()) }),
            )
            .layer(axum::middleware::from_fn(strict_envelope));
        let get_json = |path: &'static str| {
            let app = app.clone();
            async move {
                let resp = app
                    .oneshot(Request::get(path).body(Body::empty()).unwrap())
                    .await
                    .unwrap();
                let status = resp.status();
                let retry = resp.headers().get(header::RETRY_AFTER).cloned();
                let content_type = resp.headers().get(header::CONTENT_TYPE).cloned();
                (status, retry, content_type, body(resp).await)
            }
        };
        let (status, _, content_type, v) = get_json("/plain").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(content_type.unwrap(), "application/json");
        assert_eq!(
            v,
            json!({"ok": false, "error": "Missing Authorization", "code": "unauthorized"})
        );
        let (status, _, _, v) = get_json("/data").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            v,
            json!({
                "ok": false,
                "error": "Invalid data was sent to the server",
                "code": "bad_request",
                "data": {"messages": ["x"], "body": {}}
            })
        );
        let (_, _, _, v) = get_json("/empty-data").await;
        assert!(v.get("data").is_none(), "an empty data object is omitted");
        let (status, _, _, v) = get_json("/ok").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v, json!({"ok": true}));
        let (status, retry, _, v) = get_json("/held").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(retry.unwrap(), "3600");
        assert_eq!(
            v,
            json!({"ok": false, "error": "held", "code": "service_unavailable"})
        );
    }

    #[test]
    fn status_codes_map_to_upstream_names() {
        assert_eq!(code_for_status(400), "bad_request");
        assert_eq!(code_for_status(413), "payload_too_large");
        assert_eq!(code_for_status(503), "service_unavailable");
        assert_eq!(ApiError::with_data(400, "x", json!({})).status(), 400);
        assert_eq!(ApiError::WritesDisabled("x".into()).status(), 503);
    }

    #[test]
    fn wire_identity_api_ok_envelope() {
        let v = serde_json::to_value(ApiData::ok(vec![1, 2])).unwrap();
        assert_eq!(v, json!({ "ok": true, "data": [1, 2] }));
        let v = serde_json::to_value(ApiDataTotal::ok(vec!["a"], 7)).unwrap();
        assert_eq!(v, json!({ "ok": true, "data": ["a"], "total": 7 }));
    }
}
