use std::io::Cursor;
use std::time::Duration;

use tokio::time::Instant;

use axum::Json;
use axum::body::Body;
use axum::extract::{FromRequest, Multipart, OriginalUri, Path, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use base64::Engine as _;
use bytes::Bytes;
use image::{ImageFormat, ImageReader, Limits, guess_format};
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

/// Decode bounds: an upload's pixel count, not its byte size, sets the cost.
pub const MAX_IMAGE_DIMENSION: u32 = 8192;
pub const MAX_DECODE_ALLOC_BYTES: u64 = 128 * 1024 * 1024;

/// Decodes run off the async workers, a few at a time.
static DECODES: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

/// Runs `work` on the blocking pool holding a `permits` permit until it
/// finishes, even when the caller is dropped first.
pub(crate) async fn decode_with<T: Send + 'static>(
    permits: &'static tokio::sync::Semaphore,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ApiError> {
    let permit = permits
        .acquire()
        .await
        .map_err(|_| ApiError::Internal("decoder unavailable".to_string()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        work()
    })
    .await
    .map_err(|_| ApiError::Internal("couldn't create thumbnail".to_string()))
}

pub async fn upload_image(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Response, ApiError> {
    let address = state
        .require_auth(&headers, "post", &signed_path(&uri, "/api/images"))
        .await?;

    // Upstream refuses a full gallery before reading the form; the insert
    // below stays the race-safe authority. A failed count reads as 0, as
    // upstream's does.
    let max_images = state.settings.max_images_per_user;
    let count = state
        .db
        .get_user_images_count(address.as_str(), false)
        .await
        .unwrap_or(0);
    if count >= max_images {
        return Err(max_limit_reached(max_images));
    }

    // The first of a repeated field wins, as upstream's form reader keeps it.
    let mut image_bytes: Option<Bytes> = None;
    let mut image_content_type: Option<String> = None;
    let mut image_file_name: Option<String> = None;
    let mut metadata_bytes: Option<Bytes> = None;
    let mut is_public: Option<bool> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(format!("invalid multipart: {e}")))?
    {
        match field.name() {
            Some("image") if image_bytes.is_none() => {
                image_content_type = field.content_type().map(|s| s.to_string());
                image_file_name = field.file_name().map(|s| s.to_string());
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::BadRequest(format!("invalid image field: {e}")))?;
                if data.len() > MAX_MULTIPART_IMAGE_BYTES {
                    return Err(ApiError::BadRequest("image too large".to_string()));
                }
                image_bytes = Some(data);
            }
            Some("metadata") if metadata_bytes.is_none() => {
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::BadRequest(format!("invalid metadata field: {e}")))?;
                metadata_bytes = Some(data);
            }
            Some("is_public") if is_public.is_none() => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| ApiError::BadRequest(format!("invalid is_public field: {e}")))?;
                // Exactly `true` or `false`, as upstream's `Text<bool>` parses it.
                is_public = Some(match text.as_str() {
                    "true" => true,
                    "false" => false,
                    _ => {
                        return Err(ApiError::BadRequest("invalid is_public field".to_string()));
                    }
                });
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
        Upload {
            image_bytes,
            image_content_type,
            image_file_name,
            metadata,
            is_public: is_public.unwrap_or(false),
        },
    )
    .await
}

fn max_limit_reached(max_images: u64) -> ApiError {
    ApiError::MaxLimitReached(format!(
        "you have reached the limit of {max_images} max images"
    ))
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
        Upload {
            image_bytes,
            image_content_type: Some(req.content_type),
            image_file_name: None,
            metadata: req.metadata,
            is_public: req.is_public,
        },
    )
    .await
}

/// The 640x360 thumbnail in the image's own format, which must be the
/// declared one; decode failures and oversized images are the client's (400).
pub fn thumbnail_of(image_bytes: &[u8], declared: ImageFormat) -> Result<Bytes, ApiError> {
    let format = guess_format(image_bytes)
        .map_err(|_| ApiError::BadRequest("invalid image format".to_string()))?;
    if format != declared {
        return Err(ApiError::BadRequest("invalid image format".to_string()));
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOC_BYTES);
    let mut reader = ImageReader::with_format(Cursor::new(image_bytes), format);
    reader.limits(limits);
    let img = reader.decode().map_err(|e| {
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

struct Upload {
    image_bytes: Bytes,
    image_content_type: Option<String>,
    /// The multipart part's file name; the JSON route has none.
    image_file_name: Option<String>,
    metadata: Metadata,
    is_public: bool,
}

/// The checks in upstream's order after the metadata parses: every
/// `screenRect` valid, the signer is the metadata's `userAddress`, a png or
/// jpeg that decodes, a file name without a path separator; then the row
/// (the limit's authority), the blobs and the ranking.
async fn finalize_upload(
    state: &SharedState,
    address: &str,
    upload: Upload,
) -> Result<Response, ApiError> {
    let Upload {
        image_bytes,
        image_content_type,
        image_file_name,
        metadata,
        is_public,
    } = upload;
    if !metadata.screen_rects_are_valid() {
        tracing::warn!("refused an upload with an invalid screen rect");
        return Err(ApiError::BadRequest("invalid metadata".to_string()));
    }
    // Case-insensitive: upstream compares against the lowercase signer
    // exactly, so it refuses a checksummed `userAddress` this accepts.
    if !metadata.user_address.eq_ignore_ascii_case(address) {
        return Err(ApiError::BadRequest("invalid user address".to_string()));
    }

    let content_type = image_content_type
        .ok_or_else(|| ApiError::BadRequest("invalid content type".to_string()))?;
    let declared = match content_type.as_str() {
        "image/png" => ImageFormat::Png,
        "image/jpeg" => ImageFormat::Jpeg,
        _ => return Err(ApiError::BadRequest("unsupported content type".to_string())),
    };

    // JSON uploads too are refused at the cap before the decode; the insert stays the gate.
    let max_images = state.settings.max_images_per_user;
    let owned = state
        .db
        .get_user_images_count(address, false)
        .await
        .map_err(|e| {
            tracing::error!("failed to count user images: {e}");
            ApiError::Internal("failed to store image metadata".to_string())
        })?;
    if owned >= max_images {
        return Err(max_limit_reached(max_images));
    }

    let bytes = image_bytes.clone();
    let thumbnail = decode_with(&DECODES, move || thumbnail_of(&bytes, declared)).await??;

    if image_file_name
        .as_deref()
        .is_some_and(|name| name.contains('/') || name.contains('\\'))
    {
        return Err(ApiError::BadRequest("invalid file name".to_string()));
    }

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
    let current_images = state
        .db
        .insert_image_within_limit(&new_image, max_images)
        .await
        .map_err(|e| {
            tracing::error!("failed to store image metadata: {e}");
            ApiError::Internal("failed to store image metadata".to_string())
        })?
        .ok_or_else(|| max_limit_reached(max_images))?;

    let stored = async {
        store_blob(&state.storage, &new_image.image_hash, image_bytes).await?;
        store_blob(&state.storage, &new_image.thumbnail_hash, thumbnail).await
    }
    .await;
    if let Err(error) = stored {
        tracing::error!(%error, image_id = %new_image.id, "failed to upload image");
        let _ = state.db.delete_image(new_image.id).await;
        return Err(ApiError::Internal("failed to upload image".to_string()));
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
// Candidate slow-row isolation budget; the pass deadline still takes priority.
const RANKING_ROW_BUDGET: Duration = Duration::from_secs(1);
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
                if Instant::now() >= deadline {
                    progress.timed_out = true;
                    break;
                }
                if (created_at < cutoff) != cold {
                    progress.deferred += 1;
                } else {
                    // No database transaction or row lock is held during storage I/O.
                    let row_deadline = Instant::now() + RANKING_ROW_BUDGET;
                    let publication = async {
                        progress.probes += 1;
                        if !self.storage.exist(&image).await? {
                            return Ok(false);
                        }
                        progress.probes += 1;
                        if !self.storage.exist(&thumbnail).await? {
                            return Ok(false);
                        }
                        self.db
                            .publish_ranking_before(id, Some(row_deadline))
                            .await?;
                        Ok::<_, anyhow::Error>(true)
                    };
                    let publication = match tokio::time::timeout_at(row_deadline, publication).await
                    {
                        Ok(result) => result,
                        Err(error) => Err(error.into()),
                    };
                    // A row budget is a completed retryable failure. A global
                    // deadline retains this row even when both timers fire together.
                    if Instant::now() >= deadline {
                        progress.timed_out = true;
                        break;
                    }
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

/// The signature is checked before the body is read, so an unsigned
/// request is 401 whatever its body; a body that is not `{"is_public": bool}`
/// JSON is then upstream's plain-text 400.
pub async fn update_image_visibility(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(image_id): Path<String>,
    request: Request,
) -> Result<Response, ApiError> {
    let address = state
        .require_auth(
            &headers,
            "patch",
            &signed_path(&uri, format!("/api/images/{image_id}/visibility")),
        )
        .await?;
    let update = match crate::extract::Json::<UpdateVisibility>::from_request(request, &()).await {
        Ok(crate::extract::Json(update)) => update,
        Err(rejection) => return Ok(rejection),
    };

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
