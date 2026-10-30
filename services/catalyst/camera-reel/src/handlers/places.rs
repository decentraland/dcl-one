use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::SharedState;
use crate::dto::{
    GalleryImage, GalleryImageWithPlace, GetMultiplePlacesImagesBody,
    GetMultiplePlacesImagesResponse, GetPlaceImagesResponse, PlaceDataResponse,
};
use crate::handlers::{PageQuery, next_cursor};
use crate::http::ApiError;

pub const MAX_PLACES_IDS: usize = 100;

pub fn validate_places_ids(places_ids: &[String]) -> Result<(), ApiError> {
    if places_ids.is_empty() {
        return Err(ApiError::BadRequest("no place IDs provided".to_string()));
    }
    if places_ids.len() > MAX_PLACES_IDS {
        return Err(ApiError::BadRequest(format!(
            "too many place IDs provided, maximum is {MAX_PLACES_IDS}"
        )));
    }
    Ok(())
}

async fn world_place_ids(state: &SharedState, name: &str) -> Result<Vec<String>, ApiError> {
    state.db.world_place_ids(name).await.map_err(|e| {
        tracing::error!("failed to resolve world name '{name}': {e}");
        ApiError::Internal(format!("failed to resolve world name: {e}"))
    })
}

pub async fn get_place_images(
    State(state): State<SharedState>,
    Path(place_id): Path<String>,
    Query(q): Query<PageQuery>,
) -> Result<Response, ApiError> {
    let limit = q.capped_limit();
    let page = q.page()?;
    if place_id.ends_with(".eth") {
        let place_ids = world_place_ids(&state, &place_id).await?;

        if place_ids.is_empty() {
            return Ok((
                StatusCode::OK,
                Json(GetPlaceImagesResponse {
                    images: vec![],
                    place_data: PlaceDataResponse { max_images: 0 },
                    next_cursor: None,
                }),
            )
                .into_response());
        }

        let found = state
            .db
            .get_multiple_places_images_page(&place_ids, &page, limit)
            .await
            .map_err(|_| ApiError::NotFound("place not found".to_string()))?;

        let next_cursor = next_cursor(found.next);
        let images = found.images.into_iter().map(GalleryImage::from).collect();
        return Ok((
            StatusCode::OK,
            Json(GetPlaceImagesResponse {
                images,
                place_data: PlaceDataResponse {
                    max_images: found.total,
                },
                next_cursor,
            }),
        )
            .into_response());
    }

    let found = state
        .db
        .get_place_images_page(&place_id, &page, limit)
        .await
        .map_err(|_| ApiError::NotFound("place not found".to_string()))?;

    let next_cursor = next_cursor(found.next);
    let images = found.images.into_iter().map(GalleryImage::from).collect();
    Ok((
        StatusCode::OK,
        Json(GetPlaceImagesResponse {
            images,
            place_data: PlaceDataResponse {
                max_images: found.total,
            },
            next_cursor,
        }),
    )
        .into_response())
}

/// A read behind a `POST` (the id list is the body); `writes::guard` keeps
/// it open while creator writes are disabled.
pub async fn get_multiple_places_images(
    State(state): State<SharedState>,
    Query(q): Query<PageQuery>,
    Json(body): Json<GetMultiplePlacesImagesBody>,
) -> Result<Response, ApiError> {
    validate_places_ids(&body.places_ids)?;

    let limit = q.capped_limit();
    let page = q.page()?;
    let resolved_ids = resolve_places_ids(&state, body.places_ids).await?;

    if resolved_ids.is_empty() {
        return Ok((
            StatusCode::OK,
            Json(GetMultiplePlacesImagesResponse {
                images: vec![],
                place_data: PlaceDataResponse { max_images: 0 },
                next_cursor: None,
            }),
        )
            .into_response());
    }

    let found = state
        .db
        .get_multiple_places_images_page(&resolved_ids, &page, limit)
        .await
        .map_err(|_| ApiError::NotFound("places not found".to_string()))?;

    let next_cursor = next_cursor(found.next);
    let images = found
        .images
        .into_iter()
        .map(GalleryImageWithPlace::from)
        .collect();
    Ok((
        StatusCode::OK,
        Json(GetMultiplePlacesImagesResponse {
            images,
            place_data: PlaceDataResponse {
                max_images: found.total,
            },
            next_cursor,
        }),
    )
        .into_response())
}

/// World names resolve to their place ids, in input order (a local read now,
/// so the old bounded concurrency is gone).
async fn resolve_places_ids(
    state: &SharedState,
    places_ids: Vec<String>,
) -> Result<Vec<String>, ApiError> {
    let mut resolved = Vec::with_capacity(places_ids.len());
    for id in places_ids {
        if id.ends_with(".eth") {
            resolved.extend(world_place_ids(state, &id).await?);
        } else {
            resolved.push(id);
        }
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::{MAX_PLACES_IDS, validate_places_ids};
    use crate::handlers::{MAX_LIMIT, capped_limit};
    use crate::http::ApiError;

    #[test]
    fn capped_limit_passes_values_at_or_below_max() {
        assert_eq!(capped_limit(0), 0);
        assert_eq!(capped_limit(20), 20);
        assert_eq!(capped_limit(MAX_LIMIT), MAX_LIMIT as i64);
    }

    #[test]
    fn capped_limit_clamps_values_above_max() {
        assert_eq!(capped_limit(MAX_LIMIT + 1), MAX_LIMIT as i64);
        assert_eq!(capped_limit(1_000), MAX_LIMIT as i64);
        assert_eq!(capped_limit(u64::MAX), MAX_LIMIT as i64);
    }

    #[test]
    fn validate_places_ids_rejects_empty() {
        let err = validate_places_ids(&[]).unwrap_err();
        assert!(matches!(err, ApiError::BadRequest(_)));
    }

    #[test]
    fn validate_places_ids_accepts_up_to_max() {
        let ids: Vec<String> = (0..MAX_PLACES_IDS).map(|i| i.to_string()).collect();
        assert_eq!(ids.len(), MAX_PLACES_IDS);
        assert!(validate_places_ids(&ids).is_ok());
    }

    #[test]
    fn validate_places_ids_rejects_over_max() {
        let ids: Vec<String> = (0..=MAX_PLACES_IDS).map(|i| i.to_string()).collect();
        assert_eq!(ids.len(), MAX_PLACES_IDS + 1);
        let err = validate_places_ids(&ids).unwrap_err();
        assert!(matches!(err, ApiError::BadRequest(_)));
    }
}
