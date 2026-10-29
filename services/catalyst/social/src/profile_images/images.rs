//! `GET /entities/{entity}/{face,body}.png`: the answer of
//! [`ProfileImagesState::image`] (`produce.rs`). 404 only when the entity is
//! not a profile or no source has an image; a store or source that failed or
//! ran out of time is a 503 with `Retry-After`, never a 404. An entity the
//! store withholds (`content::Denylist`) is 404 before any source: the store
//! refuses its snapshot bytes itself, and the check here keeps a stored
//! image, a fresh render and the origin's copy off the wire.

use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;

use super::SharedState;
use super::cache::ImageKind;
use super::produce::Answer;
use crate::auth::is_canonical_cid;

pub async fn face(State(state): State<SharedState>, Path(entity): Path<String>) -> Response {
    serve(&state, &entity, ImageKind::Face).await
}

pub async fn body(State(state): State<SharedState>, Path(entity): Path<String>) -> Response {
    serve(&state, &entity, ImageKind::Body).await
}

async fn serve(state: &SharedState, entity: &str, kind: ImageKind) -> Response {
    if !is_canonical_cid(entity) {
        return (StatusCode::BAD_REQUEST, "invalid entity id").into_response();
    }
    if state.storage.withholds(entity) {
        return (StatusCode::NOT_FOUND, "image not available").into_response();
    }
    match state.image(entity, kind).await {
        Answer::Image(bytes) => png_response(bytes),
        Answer::NotFound => (StatusCode::NOT_FOUND, "image not available").into_response(),
        Answer::Unavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::RETRY_AFTER, HeaderValue::from_static("5"))],
            "profile image temporarily unavailable",
        )
            .into_response(),
    }
}

fn png_response(bytes: Bytes) -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("image/png")),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=86400"),
            ),
        ],
        bytes,
    )
        .into_response()
}
