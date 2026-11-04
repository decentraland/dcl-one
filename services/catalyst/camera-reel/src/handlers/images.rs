use std::io::Cursor;
use std::time::Duration;

use tokio::time::Instant;

use axum::Json;
use axum::body::Body;
use axum::extract::{Multipart, OriginalUri, Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use base64::Engine as _;
use bytes::Bytes;
use image::guess_format;
use uuid::Uuid;

use crate::db::{BlobVerdict, DeletedImage, NewImage};
use crate::dto::{
    Image, Metadata, UpdateReview, UpdateVisibility, UploadResponse, UserDataResponse,
};
use crate::http::ApiError;
use crate::storage::{Storage, StorageError};
use crate::{SharedState, signed_path};

pub const MAX_MULTIPART_IMAGE_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_JSON_IMAGE_BYTES: usize = 15 * 1024 * 1024;

pub async fn upload_image(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Response, ApiError> {
    let address = state
        .require_auth(&headers, "post", &signed_path(&uri, "/api/images"))
        .await?;

    let mut image_bytes: Option<Bytes> = None;
    let mut image_content_type: Option<String> = None;
    let mut metadata_bytes: Option<Bytes> = None;
    let mut is_public = false;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(format!("invalid multipart: {e}")))?
    {
        match field.name() {
            Some("image") => {
                image_content_type = field.content_type().map(|s| s.to_string());
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::BadRequest(format!("invalid image field: {e}")))?;
                if data.len() > MAX_MULTIPART_IMAGE_BYTES {
                    return Err(ApiError::BadRequest("image too large".to_string()));
                }
                image_bytes = Some(data);
            }
            Some("metadata") => {
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::BadRequest(format!("invalid metadata field: {e}")))?;
                metadata_bytes = Some(data);
            }
            Some("is_public") => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| ApiError::BadRequest(format!("invalid is_public field: {e}")))?;
                is_public = text.trim().parse::<bool>().unwrap_or(false);
            }
            _ => {
                let _ = field.bytes().await;
            }
        }
    }

    let image_bytes =
        image_bytes.ok_or_else(|| ApiError::BadRequest("missing image".to_string()))?;
    let metadata_bytes =
        metadata_bytes.ok_or_else(|| ApiError::BadRequest("missing metadata".to_string()))?;

    let metadata: Metadata = serde_json::from_slice(&metadata_bytes).map_err(|e| {
        tracing::error!("failed to parse metadata: {e}");
        ApiError::BadRequest("invalid metadata".to_string())
    })?;

    finalize_upload(
        &state,
        address.as_str(),
        image_bytes,
        image_content_type,
        metadata,
        is_public,
    )
    .await
}

#[derive(serde::Deserialize)]
pub struct JsonUpload {
    pub image: String,
    pub content_type: String,
    pub metadata: Metadata,
    #[serde(default)]
    pub is_public: bool,
}

pub async fn upload_image_json(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Json(req): Json<JsonUpload>,
) -> Result<Response, ApiError> {
    let address = state
        .require_auth(&headers, "post", &signed_path(&uri, "/api/images-json"))
        .await?;

    let image_bytes = Bytes::from(
        base64::engine::general_purpose::STANDARD
            .decode(req.image.trim())
            .map_err(|_| ApiError::BadRequest("invalid base64 image".to_string()))?,
    );
    if image_bytes.len() > MAX_JSON_IMAGE_BYTES {
        return Err(ApiError::BadRequest("image too large".to_string()));
    }

    finalize_upload(
        &state,
        address.as_str(),
        image_bytes,
        Some(req.content_type),
        req.metadata,
        req.is_public,
    )
    .await
}

/// The 640x360 thumbnail in the image's own format; decode failures are the
/// client's (400), as before.
pub fn thumbnail_of(image_bytes: &[u8]) -> Result<Bytes, ApiError> {
    let format = guess_format(image_bytes)
        .map_err(|_| ApiError::BadRequest("invalid image format".to_string()))?;
    let img = image::load_from_memory_with_format(image_bytes, format).map_err(|e| {
        tracing::error!("failed to parse image: {e}");
        ApiError::BadRequest("invalid image".to_string())
    })?;
    let thumb = img.thumbnail(640, 360);
    let mut buffer = Cursor::new(Vec::new());
    thumb.write_to(&mut buffer, format).map_err(|e| {
        tracing::error!("couldn't generate thumbnail: {e}");
        ApiError::BadRequest("couldn't create thumbnail".to_string())
    })?;
    Ok(Bytes::from(buffer.into_inner()))
}

/// `hash` must be `auth::hash_bytes_v1(&data)`; an existing blob with that
/// hash is the same bytes, so it is left alone.
async fn store_blob(storage: &Storage, hash: &str, data: Bytes) -> Result<(), StorageError> {
    if storage.exist(hash).await? {
        return Ok(());
    }
    storage.store(hash, data).await
}

async fn finalize_upload(
    state: &SharedState,
    address: &str,
    image_bytes: Bytes,
    image_content_type: Option<String>,
    metadata: Metadata,
    is_public: bool,
) -> Result<Response, ApiError> {
    if !metadata.user_address.eq_ignore_ascii_case(address) {
        return Err(ApiError::BadRequest("invalid user address".to_string()));
    }

    let content_type = image_content_type
        .ok_or_else(|| ApiError::BadRequest("invalid content type".to_string()))?;
    match content_type.as_str() {
        "image/png" | "image/jpeg" => {}
        _ => return Err(ApiError::BadRequest("unsupported content type".to_string())),
    }

    let thumbnail = thumbnail_of(&image_bytes)?;

    let new_image = NewImage {
        id: Uuid::new_v4(),
        image_hash: crate::auth::hash_bytes_v1(&image_bytes),
        thumbnail_hash: crate::auth::hash_bytes_v1(&thumbnail),
        is_public,
        metadata,
    };
    let image = new_image.wire(&state.settings.api_url);

    // The row is the limit gate, so it lands before the blobs: a refused upload
    // never touches the store, and a failed blob write takes its row back out.
    let max_images = state.settings.max_images_per_user;
    let current_images = state
        .db
        .insert_image_within_limit(&new_image, max_images)
        .await
        .map_err(|e| {
            tracing::error!("failed to store image metadata: {e}");
            ApiError::Internal("failed to store image metadata".to_string())
        })?
        .ok_or_else(|| {
            ApiError::MaxLimitReached(format!(
                "you have reached the limit of {max_images} max images"
            ))
        })?;

    let stored = async {
        store_blob(&state.storage, &new_image.image_hash, image_bytes)
            .await
            .map_err(|e| ApiError::Internal(format!("failed to store image: {e}")))?;
        store_blob(&state.storage, &new_image.thumbnail_hash, thumbnail)
            .await
            .map_err(|e| ApiError::Internal(format!("failed to store thumbnail: {e}")))
    }
    .await;
    if let Err(err) = stored {
        let _ = state.db.delete_image(new_image.id).await;
        return Err(err);
    }

    // PhotoTaken's counter handler is best-effort upstream. Retain the false
    // publication marker on failure so the repair pass can retry after restart.
    if let Err(error) = state.db.publish_ranking(new_image.id).await {
        tracing::error!(%error, image_id=%new_image.id, "photo ranking publication deferred");
    }

    let response = UploadResponse {
        image,
        user_data: UserDataResponse {
            current_images,
            max_images,
        },
    };
    Ok((StatusCode::OK, Json(response)).into_response())
}

// Candidate retry policy: recent uploads get the existing minute cadence; old
// incomplete uploads are probed again an hour after a complete cold sweep.
const RANKING_RECENT_AGE: chrono::TimeDelta = chrono::TimeDelta::minutes(10);
const RANKING_TICK: Duration = Duration::from_secs(60);
const RANKING_COLD_RETRY: Duration = Duration::from_secs(60 * 60);
const RANKING_PASS_BUDGET: Duration = Duration::from_secs(3);
const RANKING_MAX_PAGES: usize = 32;

pub(crate) struct RankingSweep {
    pub(crate) after: Uuid,
    pub(crate) cutoff: Option<chrono::NaiveDateTime>,
    pub(crate) next_due: Instant,
}

impl RankingSweep {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            after: Uuid::nil(),
            cutoff: None,
            next_due: now,
        }
    }
}

pub(crate) struct RankingSchedule {
    // Fixed slots: recent first, cold second. Only these two sweep records persist.
    pub(crate) sweeps: [RankingSweep; 2],
    next: usize,
}

impl RankingSchedule {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            sweeps: [RankingSweep::new(now), RankingSweep::new(now)],
            next: 0,
        }
    }

    pub(crate) fn due(&mut self, now: Instant) -> Option<usize> {
        for offset in 0..2 {
            let index = (self.next + offset) % 2;
            if self.sweeps[index].next_due <= now {
                self.next = (index + 1) % 2;
                return Some(index);
            }
        }
        None
    }
}

#[derive(Debug, Default)]
pub(crate) struct RankingRepairProgress {
    pub(crate) pages: usize,
    pub(crate) fetched: usize,
    pub(crate) inspected: usize,
    pub(crate) deferred: usize,
    pub(crate) probes: usize,
    pub(crate) published: usize,
    pub(crate) errors: usize,
    pub(crate) timed_out: bool,
}

impl crate::State {
    pub(crate) async fn repair_ranking_publications(
        &self,
        sweep: &mut RankingSweep,
        cold: bool,
        deadline: Instant,
    ) -> anyhow::Result<RankingRepairProgress> {
        let cutoff = *sweep
            .cutoff
            .get_or_insert_with(|| chrono::Utc::now().naive_utc() - RANKING_RECENT_AGE);
        let mut progress = RankingRepairProgress::default();
        let page = async {
            let rows = self.db.pending_rankings(sweep.after).await?;
            progress.fetched = rows.len();
            if rows.is_empty() {
                sweep.after = Uuid::nil();
                sweep.cutoff = None;
                sweep.next_due = Instant::now()
                    + if cold {
                        RANKING_COLD_RETRY
                    } else {
                        RANKING_TICK
                    };
            }
            for (id, image, thumbnail, created_at) in rows {
                if (created_at < cutoff) != cold {
                    progress.deferred += 1;
                } else {
                    // No database transaction or row lock is held during storage I/O.
                    let publication: anyhow::Result<bool> = async {
                        progress.probes += 1;
                        if !self.storage.exist(&image).await? {
                            return Ok(false);
                        }
                        progress.probes += 1;
                        if !self.storage.exist(&thumbnail).await? {
                            return Ok(false);
                        }
                        self.db.publish_ranking(id).await?;
                        Ok(true)
                    }
                    .await;
                    match publication {
                        Ok(true) => progress.published += 1,
                        Ok(false) => {}
                        Err(error) => {
                            progress.errors += 1;
                            tracing::error!(%id, %error, "photo ranking row repair failed");
                        }
                    }
                    progress.inspected += 1;
                }
                // Only completed/deferred rows advance. Dropping this future on
                // deadline or cancellation retains the in-flight row and unvisited tail.
                sweep.after = id;
            }
            Ok::<_, anyhow::Error>(())
        };
        match tokio::time::timeout_at(deadline, page).await {
            Ok(result) => result?,
            Err(_) => progress.timed_out = true,
        }
        Ok(progress)
    }

    pub(crate) async fn repair_ranking_pass(
        &self,
        schedule: &mut RankingSchedule,
        deadline: Instant,
    ) -> anyhow::Result<RankingRepairProgress> {
        let mut progress = RankingRepairProgress::default();
        for _ in 0..RANKING_MAX_PAGES {
            if Instant::now() >= deadline {
                progress.timed_out = true;
                break;
            }
            let Some(index) = schedule.due(Instant::now()) else {
                break;
            };
            let page = self
                .repair_ranking_publications(&mut schedule.sweeps[index], index == 1, deadline)
                .await?;
            progress.pages += 1;
            progress.fetched += page.fetched;
            progress.inspected += page.inspected;
            progress.deferred += page.deferred;
            progress.probes += page.probes;
            progress.published += page.published;
            progress.errors += page.errors;
            if page.timed_out {
                progress.timed_out = true;
                break;
            }
        }
        Ok(progress)
    }

    pub fn spawn_ranking_job(self: &std::sync::Arc<Self>, jobs: &crate::jobs::Jobs) {
        let state = self.clone();
        jobs.spawn_leased("camera-photo-ranking", move |ctx| {
            let state = state.clone();
            async move {
                let mut schedule = RankingSchedule::new(Instant::now());
                loop {
                    let result = tokio::select! {
                        _ = ctx.cancel.cancelled() => return Ok(()),
                        result = state.repair_ranking_pass(
                            &mut schedule, Instant::now() + RANKING_PASS_BUDGET,
                        ) => result,
                    };
                    match result {
                        Ok(progress) => ctx.progress(format!(
                            "{} photo ranking publications repaired; {} pages, {} inspected, {} deferred, {} row errors; pass timed out: {}",
                            progress.published, progress.pages, progress.inspected,
                            progress.deferred, progress.errors, progress.timed_out,
                        )),
                        Err(error) => {
                            ctx.error(error.to_string());
                            tracing::error!(%error, "photo ranking repair pass failed");
                        }
                    }
                    ctx.pass_done();
                    tokio::select! {
                        _ = ctx.cancel.cancelled() => return Ok(()),
                        _ = tokio::time::sleep(RANKING_TICK) => {},
                    }
                }
            }
        });
    }
}

pub async fn delete_image(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(image_id): Path<String>,
) -> Result<Response, ApiError> {
    let address = state
        .require_auth(
            &headers,
            "delete",
            &signed_path(&uri, format!("/api/images/{image_id}")),
        )
        .await?;

    let deleted = delete_image_row(&state, &image_id, Some(address.as_str())).await?;
    if !deleted.deleted {
        return Err(ApiError::Forbidden("forbidden".to_string()));
    }

    Ok((
        StatusCode::OK,
        Json(UserDataResponse {
            current_images: deleted.remaining,
            max_images: state.settings.max_images_per_user,
        }),
    )
        .into_response())
}

/// The row goes; the blobs stay, withheld. The ONE store is shared with
/// content, worlds and builder files and keeps hash-addressed bytes
/// immutable, so nothing here unlinks (the old service's per-row refcount
/// over its private tree is gone); the row was what made the hash reachable
/// through `/api/images/{hash}`, and the `withheld_blobs` record written in
/// the same transaction is what keeps it off `/contents/{hash}` everywhere.
async fn delete_image_row(
    state: &SharedState,
    image_id: &str,
    owner: Option<&str>,
) -> Result<DeletedImage, ApiError> {
    let deleted = state
        .db
        .delete_image_owned(image_id, owner, chrono::Utc::now().naive_utc())
        .await
        .map_err(|e| {
            tracing::error!("failed to delete image metadata: {e}");
            ApiError::Internal("failed to delete image".to_string())
        })?
        .ok_or_else(|| ApiError::NotFound("image not found".to_string()))?;
    state.apply_verdict(&BlobVerdict {
        withheld: deleted.withheld.clone(),
        released: Vec::new(),
    });
    Ok(deleted)
}

pub async fn update_image_visibility(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(image_id): Path<String>,
    Json(update): Json<UpdateVisibility>,
) -> Result<Response, ApiError> {
    let address = state
        .require_auth(
            &headers,
            "patch",
            &signed_path(&uri, format!("/api/images/{image_id}/visibility")),
        )
        .await?;

    let owned = state
        .db
        .set_image_visibility(&image_id, address.as_str(), update.is_public)
        .await
        .map_err(|e| {
            tracing::error!("failed to update image metadata: {e}");
            ApiError::Internal("failed to update image metadata".to_string())
        })?
        .ok_or_else(|| ApiError::NotFound("image not found".to_string()))?;
    if !owned {
        return Err(ApiError::Forbidden("forbidden".to_string()));
    }

    Ok(StatusCode::OK.into_response())
}

/// `GET /api/images/{hash}`: the blob, while a non-rejected row names it
/// and the store does not withhold it (the operator's `content.denylist`
/// or a reel verdict; the store's own reads refuse a listed hash too, this
/// check keeps the bucket redirect from pointing at one).
pub async fn get_image(
    State(state): State<SharedState>,
    Path(image_id): Path<String>,
) -> Result<Response, ApiError> {
    if state.storage.withholds(&image_id) {
        return Err(ApiError::NotFound("image not found".to_string()));
    }
    let servable = state.db.hash_is_servable(&image_id).await.map_err(|e| {
        tracing::error!("failed to check image review status: {e}");
        ApiError::NotFound("image not found".to_string())
    })?;
    if !servable {
        return Err(ApiError::NotFound("image not found".to_string()));
    }

    if let Some(bucket_url) = &state.settings.bucket_url {
        let location = format!("{}/{}", bucket_url.trim_end_matches('/'), image_id);
        return Ok((
            StatusCode::TEMPORARY_REDIRECT,
            [(header::LOCATION, location)],
        )
            .into_response());
    }

    let bytes = state
        .storage
        .retrieve(&image_id)
        .await
        .map_err(|e| match e {
            StorageError::InvalidId(_) | StorageError::PathTraversal(_) => {
                ApiError::BadRequest(format!("invalid image id: {e}"))
            }
            other => ApiError::Internal(format!("failed to read image: {other}")),
        })?
        .ok_or_else(|| ApiError::NotFound("image not found".to_string()))?;

    let content_type = match guess_format(&bytes) {
        Ok(image::ImageFormat::Png) => "image/png",
        Ok(image::ImageFormat::Jpeg) => "image/jpeg",
        _ => "application/octet-stream",
    };

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, content_type)],
        Body::from(bytes),
    )
        .into_response())
}

pub async fn admin_delete_image(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(image_id): Path<String>,
) -> Result<Response, ApiError> {
    state.authorize_admin(&headers)?;

    let deleted = delete_image_row(&state, &image_id, None).await?;

    Ok((
        StatusCode::OK,
        Json(UserDataResponse {
            current_images: deleted.remaining,
            max_images: state.settings.max_images_per_user,
        }),
    )
        .into_response())
}

pub async fn admin_update_image_review(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(image_id): Path<String>,
    Json(update): Json<UpdateReview>,
) -> Result<Response, ApiError> {
    state.authorize_admin(&headers)?;

    if !update.is_valid() {
        return Err(ApiError::BadRequest(
            "reviewStatus must be one of: ok, flagged, rejected".to_string(),
        ));
    }

    let verdict = state
        .db
        .update_image_review_status(
            &image_id,
            &update.review_status,
            chrono::Utc::now().naive_utc(),
        )
        .await
        .map_err(|e| {
            tracing::error!("failed to update image review status: {e}");
            ApiError::Internal("failed to update image review status".to_string())
        })?
        .ok_or_else(|| ApiError::NotFound("image not found".to_string()))?;
    state.apply_verdict(&verdict);

    Ok(StatusCode::OK.into_response())
}

/// Public metadata: private images answer too (upstream parity; the
/// `metadata_public` contract), a rejected or a store-withheld one is 404.
pub async fn get_metadata(
    State(state): State<SharedState>,
    Path(image_id): Path<String>,
) -> Result<Response, ApiError> {
    if state.storage.withholds(&image_id) {
        return Err(ApiError::NotFound("image not found".to_string()));
    }
    let db_image = match state.db.get_image(&image_id).await {
        Ok(img) => img,
        Err(sqlx::Error::ColumnDecode { source, .. }) => {
            tracing::debug!("couldn't decode image metadata: {source:?}");
            return Err(ApiError::Internal("couldn't decode image".to_string()));
        }
        Err(e) => {
            tracing::debug!("image not found: {e:?}");
            return Err(ApiError::NotFound("image not found".to_string()));
        }
    };

    if db_image.review_status == "rejected" {
        return Err(ApiError::NotFound("image not found".to_string()));
    }

    let image: Image = db_image.into();
    Ok((StatusCode::OK, Json(image)).into_response())
}
