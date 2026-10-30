// from: camera-reel-service@69947bc697ee712c1b4cb4560ca5307773c14157 src/api/get.rs (get_wearable_images, is_item_id)
//! `GET /api/wearables/{item}/images`: the public photos of people wearing
//! a collection item. No auth: these are photos of other people, offered
//! for an item rather than for the person in them, so the public,
//! non-rejected filter is not one a caller can lift.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::SharedState;
use crate::dto::{GetWearableImagesResponse, Image};
use crate::handlers::{capped_limit, default_limit, default_offset};
use crate::http::ApiError;

#[derive(Deserialize, Debug)]
pub struct GetWearableImagesQuery {
    #[serde(default = "default_offset")]
    pub offset: u64,
    #[serde(default = "default_limit")]
    pub limit: u64,
}

/// `<contract>-<itemId>`, the marketplace's identity for an item: a 0x +
/// 40 hex contract, a dash, a decimal item id. Not a URN and no token id.
/// Checked before the database so a malformed id is a 400, not a scan that
/// can only find nothing.
pub fn is_item_id(value: &str) -> bool {
    let Some((contract, item_id)) = value.split_once('-') else {
        return false;
    };
    contract.len() == 42
        && contract.starts_with("0x")
        && contract[2..].bytes().all(|b| b.is_ascii_hexdigit())
        && !item_id.is_empty()
        && item_id.bytes().all(|b| b.is_ascii_digit())
}

pub async fn get_wearable_images(
    State(state): State<SharedState>,
    Path(item): Path<String>,
    Query(q): Query<GetWearableImagesQuery>,
) -> Result<Response, ApiError> {
    let item = item.to_lowercase();
    if !is_item_id(&item) {
        return Err(ApiError::BadRequest(
            "the item must be an item id, as `<contract>-<itemId>`".to_string(),
        ));
    }

    let offset = q.offset.min(i64::MAX as u64) as i64;
    let found = state
        .db
        .get_wearable_images_page(&item, offset, capped_limit(q.limit))
        .await
        .map_err(|e| {
            tracing::error!("failed to fetch the images of item {item}: {e}");
            ApiError::Internal("failed to fetch the images of the item".to_string())
        })?;

    let images = found.images.into_iter().map(Image::from).collect();
    Ok((
        StatusCode::OK,
        Json(GetWearableImagesResponse {
            images,
            max_images: found.total,
        }),
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::is_item_id;

    #[test]
    fn accepts_a_marketplace_item_id() {
        assert!(is_item_id("0x0bf152a83a6fc55066c2b664b164ca2916ad38f5-2"));
        assert!(is_item_id("0x0BF152A83A6FC55066C2B664B164CA2916AD38F5-0"));
    }

    #[test]
    fn refuses_anything_that_is_not_one() {
        assert!(!is_item_id(
            "urn:decentraland:matic:collections-v2:0x0bf152a83a6fc55066c2b664b164ca2916ad38f5:2"
        ));
        assert!(!is_item_id("0x0bf152a83a6fc55066c2b664b164ca2916ad38f5"));
        assert!(!is_item_id("0x0bf152a83a6fc55066c2b664b164ca2916ad38f5-"));
        assert!(
            !is_item_id("0x0bf152a83a6fc55066c2b664b164ca2916ad38f5-2-7"),
            "a token id is not an item id"
        );
        assert!(
            !is_item_id("0x0bf152a83a6fc55066c2b664b164ca2916ad38f-2"),
            "41 characters"
        );
        assert!(
            !is_item_id("0x0bf152a83a6fc55066c2b664b164ca2916ad38fg-2"),
            "not hex"
        );
        assert!(!is_item_id("-2"));
        assert!(!is_item_id("not-2"));
        assert!(!is_item_id(""));
    }
}
