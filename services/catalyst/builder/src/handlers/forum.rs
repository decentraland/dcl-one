//! The forum routes: a collection's topic and the committee's replies in it
//! ([`crate::curation::forum_posts`]).

use axum::Json;
use axum::extract::{Path, State};
use bytes::Bytes;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::SharedState;
use crate::access;
use crate::auth::Signed;
use crate::curation::forum_posts;
use crate::http::{ApiData, ApiError};

/// `POST /v1/collections/:id/post`. The request's body is not read: the
/// node writes the post.
pub async fn post(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
) -> Result<Json<ApiData<String>>, ApiError> {
    let manage = access::manage_collection(&state, &signer, &id).await?;
    Ok(Json(ApiData::ok(forum_posts::post(&state, &manage).await?)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplyBody {
    #[serde(rename = "forumPost")]
    forum_post: ReplyPost,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplyPost {
    raw: String,
    #[serde(rename = "topic_id")]
    _topic_id: Option<Value>,
}

/// `POST /v1/collections/:id/curation/post` with `{forumPost: {raw}}`; a
/// client's `topic_id` is ignored.
pub async fn reply(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let grant = access::reply_on_forum(&state, &signer, &id).await?;
    let raw = serde_json::from_slice::<ReplyBody>(&body)
        .ok()
        .map(|body| body.forum_post.raw);
    forum_posts::reply(&state, &grant, raw.as_deref()).await?;
    Ok(Json(json!({ "ok": true })))
}
