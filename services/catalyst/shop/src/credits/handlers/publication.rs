//! `POST /credits/authorize-publication` and its canonical
//! `/shop-api/v1/credit-authorizations/publication`: a collection's
//! publication fee paid with credits (`credits::publication`).

use axum::Json;
use axum::extract::{OriginalUri, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};

use super::authorize::{body_json, refuse};
use super::signer_from;
use crate::credits::SharedState;
use crate::credits::batch::AuthorizeError;
use crate::credits::publication;
use crate::shared::signed_fetch::AUTH_TIMESTAMP_HEADER;

pub async fn authorize_publication(
    State(state): State<SharedState>,
    original: OriginalUri,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let signer = match signer_from(
        &state,
        &headers,
        "post",
        "/credits/authorize-publication",
        &original,
    )
    .await
    {
        Ok(signer) => signer,
        Err(e) => return e.into_response(),
    };
    let request = match publication::parse(&body_json(&body)) {
        Ok(request) => request,
        Err(message) => return refuse(AuthorizeError::Line { index: 0, message }, true),
    };
    let auth_ts = headers
        .get(AUTH_TIMESTAMP_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    match publication::authorize(&state, &signer, &request, auth_ts).await {
        Ok(out) => Json(out).into_response(),
        Err(e) => refuse(e, true),
    }
}
