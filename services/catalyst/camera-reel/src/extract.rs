//! The query and JSON body extractors of the read and visibility routes,
//! answering their rejections the way camera-reel-service's actix
//! extractors do: 400 `text/plain; charset=utf-8` with
//! `Query deserialize error: <serde message>`, `Content type error` or
//! `Json deserialize error: <serde message>`, and 413 for a JSON body over
//! actix's default 2 MiB.

use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::http::request::Parts;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::de::DeserializeOwned;

/// actix-web's default `JsonConfig` limit.
pub const MAX_JSON_BODY_BYTES: usize = 2 * 1024 * 1024;

fn plain(status: StatusCode, body: String) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        body,
    )
        .into_response()
}

/// The query string, decoded without the field-path prefix axum's own
/// `Query` adds to its message.
#[derive(Debug, Clone, Copy, Default)]
pub struct Query<T>(pub T);

impl<T, S> FromRequestParts<S> for Query<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        serde_urlencoded::from_str(parts.uri.query().unwrap_or(""))
            .map(Query)
            .map_err(|e| {
                plain(
                    StatusCode::BAD_REQUEST,
                    format!("Query deserialize error: {e}"),
                )
            })
    }
}

/// A JSON body: `application/json` or a `+json` type required, at most
/// [`MAX_JSON_BODY_BYTES`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Json<T>(pub T);

fn is_json(content_type: &str) -> bool {
    let essence = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let Some((kind, subtype)) = essence.split_once('/') else {
        return false;
    };
    kind == "application" && (subtype == "json" || subtype.ends_with("+json"))
}

impl<T, S> FromRequest<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let json = req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(is_json);
        if !json {
            return Err(plain(
                StatusCode::BAD_REQUEST,
                "Content type error".to_string(),
            ));
        }
        let known = req
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok());
        if let Some(length) = known.filter(|l| *l > MAX_JSON_BODY_BYTES) {
            return Err(plain(
                StatusCode::PAYLOAD_TOO_LARGE,
                format!(
                    "JSON payload ({length} bytes) is larger than allowed (limit: {MAX_JSON_BODY_BYTES} bytes)."
                ),
            ));
        }
        let bytes = axum::body::to_bytes(req.into_body(), MAX_JSON_BODY_BYTES)
            .await
            .map_err(|_| {
                plain(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    format!("JSON payload has exceeded limit ({MAX_JSON_BODY_BYTES} bytes)."),
                )
            })?;
        serde_json::from_slice(&bytes).map(Json).map_err(|e| {
            plain(
                StatusCode::BAD_REQUEST,
                format!("Json deserialize error: {e}"),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::{Body, to_bytes};
    use axum::routing::{get, post};
    use serde::Deserialize;
    use tower::ServiceExt;

    #[derive(Deserialize)]
    struct Page {
        #[serde(default)]
        #[allow(dead_code)]
        limit: u64,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Ids {
        places_ids: Vec<String>,
    }

    async fn call(app: Router, req: Request) -> (StatusCode, String, String) {
        let res = app.oneshot(req).await.unwrap();
        let status = res.status();
        let ct = res
            .headers()
            .get(header::CONTENT_TYPE)
            .map(|v| v.to_str().unwrap().to_string())
            .unwrap_or_default();
        let body = to_bytes(res.into_body(), 1 << 16).await.unwrap();
        (status, ct, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn a_bad_query_answers_the_upstream_plain_text() {
        let app = Router::new().route("/x", get(|_: Query<Page>| async { "ok" }));
        let req = Request::builder()
            .uri("/x?limit=abc")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            call(app.clone(), req).await,
            (
                StatusCode::BAD_REQUEST,
                "text/plain; charset=utf-8".to_string(),
                "Query deserialize error: invalid digit found in string".to_string()
            )
        );
        let req = Request::builder()
            .uri("/x?limit=3&other=1")
            .body(Body::empty())
            .unwrap();
        assert_eq!(call(app, req).await.0, StatusCode::OK);
    }

    #[tokio::test]
    async fn json_rejections_are_actix_s_400_and_413() {
        let app = Router::new().route(
            "/b",
            post(|Json(b): Json<Ids>| async move { b.places_ids.len().to_string() }),
        );
        let send = |body: Vec<u8>, ct: Option<&str>| {
            let mut req = Request::builder().method("POST").uri("/b");
            if let Some(ct) = ct {
                req = req.header(header::CONTENT_TYPE, ct);
            }
            req.body(Body::from(body)).unwrap()
        };
        for (body, ct, status, text) in [
            (
                "{}",
                Some("application/json"),
                StatusCode::BAD_REQUEST,
                "Json deserialize error: missing field `placesIds` at line 1 column 2",
            ),
            (
                r#"{"placesIds":"x"}"#,
                Some("application/json"),
                StatusCode::BAD_REQUEST,
                "Json deserialize error: invalid type: string \"x\", expected a sequence at line 1 column 16",
            ),
            (
                r#"{"placesIds":["a"]}"#,
                None,
                StatusCode::BAD_REQUEST,
                "Content type error",
            ),
            (
                r#"{"placesIds":["a"]}"#,
                Some("text/plain"),
                StatusCode::BAD_REQUEST,
                "Content type error",
            ),
            (
                r#"{"placesIds":["a"]}"#,
                Some("application/json; charset=utf-8"),
                StatusCode::OK,
                "1",
            ),
            (
                r#"{"placesIds":["a","b"]}"#,
                Some("application/vnd.x+json"),
                StatusCode::OK,
                "2",
            ),
        ] {
            let (got, _, got_text) = call(app.clone(), send(body.as_bytes().to_vec(), ct)).await;
            assert_eq!((got, got_text.as_str()), (status, text), "{body} {ct:?}");
        }
        let big = vec![b' '; MAX_JSON_BODY_BYTES + 1];
        let (status, ct, _) = call(app, send(big, Some("application/json"))).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(ct, "text/plain; charset=utf-8");
    }
}
