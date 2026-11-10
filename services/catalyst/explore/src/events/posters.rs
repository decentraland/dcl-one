//! `POST /api/poster` and `/api/poster-vertical` (multipart, field
//! `poster`, a signed fetch, behind `DATA_WRITES_ENABLED`): an event poster
//! the events form uploads before it creates the event. The signer needs a
//! profile this node holds (the old `withAuthProfile()`, 404 `Not found
//! profile for "<user>"`; `Poster/routes.ts:67-80`). The bytes must be one
//! of the accepted image types (sniffed, and equal to the part's declared
//! type), at most 500 KiB; they land in the ONE blob store and in
//! `explore.poster` as a `local` row named `<sha256>.<ext>`, which
//! `GET /poster/{file}` (`explore::api::posters`) serves. The answer is the
//! old `{ok:true,data:{filename,url,size,type}}` (`Poster/routes.ts:132-137`)
//! with `url` absolute on this node's `/events` mount
//! (`PUBLIC_URL/events/poster/<file>`), where the old service named its
//! bucket; `filename` is `poster/<file>` with the content hash where the old
//! service minted a uuid (`docs/events.md`).

use axum::Json;
use axum::extract::{Multipart, OriginalUri, State};
use axum::http::{HeaderMap, Method};
use bytes::Bytes;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::shared::errors::ApiError;
use crate::{Settings, SharedState};

pub const MAX_POSTER_BYTES: usize = 500 * 1024;
pub const DEFAULT_EVENT_IMAGE: &str = "/assets/event-default.webp";
const POSTER_FILE_TYPES: [&str; 4] = ["image/jpeg", "image/png", "image/gif", "image/webp"];
const POSTER_VERTICAL_FILE_TYPES: [&str; 3] = ["image/jpeg", "image/png", "image/webp"];

/// Absolute upload links must name a configured deployment origin; stored
/// URLs use the canonical local path so content is always served here.
pub(super) fn normalize_image_input(
    image: &str,
    settings: &Settings,
    expected_kind: &str,
) -> Result<String, ApiError> {
    let image = image.trim();
    let path = if image.starts_with('/') && !image.starts_with("//") {
        image.to_owned()
    } else {
        let url = url::Url::parse(image)
            .map_err(|_| ApiError::bad_request("Upload the image through the poster endpoint"))?;
        let own_origin = [
            &settings.events_asset_base_url,
            &settings.content_public_url,
        ]
        .iter()
        .any(|base| url::Url::parse(base).is_ok_and(|base| base.origin() == url.origin()));
        if !own_origin
            || !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(ApiError::bad_request(
                "Image must come from this deployment's poster upload endpoint",
            ));
        }
        let path = url.path();
        [
            &settings.events_asset_base_url,
            &settings.content_public_url,
        ]
        .iter()
        .filter_map(|base| url::Url::parse(base).ok())
        .filter(|base| base.origin() == url.origin())
        .filter_map(|base| {
            let prefix = base.path().trim_end_matches('/');
            (!prefix.is_empty())
                .then(|| path.strip_prefix(prefix))
                .flatten()
                .filter(|candidate| {
                    *candidate == DEFAULT_EVENT_IMAGE || poster_key(candidate).is_some()
                })
                .map(str::to_owned)
        })
        .next()
        .unwrap_or_else(|| path.to_owned())
    };
    if path == DEFAULT_EVENT_IMAGE
        || poster_key(&path).is_some_and(|(kind, _)| kind == expected_kind)
    {
        Ok(path)
    } else {
        Err(ApiError::bad_request(
            "Upload the image through the poster endpoint",
        ))
    }
}

fn poster_key(image: &str) -> Option<(&'static str, &str)> {
    for kind in ["poster", "poster-vertical"] {
        if let Some(file) = image.strip_prefix(&format!("/{kind}/")) {
            return crate::api::posters::valid_file(file).then_some((kind, file));
        }
    }
    None
}

/// Normalize and verify a poster in the same policy path for HTTP and console authoring.
pub(super) async fn resolve<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    image: &str,
    settings: &Settings,
    expected_kind: &str,
) -> Result<String, ApiError> {
    let image = normalize_image_input(image, settings, expected_kind)?;
    if image == DEFAULT_EVENT_IMAGE {
        return Ok(image);
    }
    let (kind, file) = poster_key(&image)
        .ok_or_else(|| ApiError::bad_request("Upload the image through the poster endpoint"))?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM explore.poster WHERE kind=$1 AND file=$2 AND origin='local')",
    )
    .bind(kind)
    .bind(file)
    .fetch_one(executor)
    .await?;
    if !exists {
        return Err(ApiError::bad_request("Poster upload not found"));
    }
    Ok(image)
}

pub fn absolute_default_image(image: &str, events_base_url: &str) -> String {
    if image == DEFAULT_EVENT_IMAGE {
        format!(
            "{}{}",
            events_base_url.trim_end_matches('/'),
            DEFAULT_EVENT_IMAGE
        )
    } else {
        image.to_owned()
    }
}

#[cfg(test)]
pub(super) fn default_image_url(settings: &Settings) -> String {
    format!(
        "{}{}",
        settings.events_asset_base_url.trim_end_matches('/'),
        DEFAULT_EVENT_IMAGE
    )
}

/// Bundled first-party fallback, independent of the optional upstream mirror.
/// Source: events@770e43f8baaf47844bf4ff5768e5493a380508bc,
/// public/images/event-default.jpg (its encoded format is WebP).
pub async fn default_image() -> axum::response::Response {
    use axum::response::IntoResponse;
    (
        [
            (axum::http::header::CONTENT_TYPE, "image/webp"),
            (axum::http::header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        include_bytes!(concat!(
            env!("DCL_ONE_ROOT"),
            "/apps/ui3/public/assets/event-default.webp"
        ))
        .as_slice(),
    )
        .into_response()
}

fn extension(mime: &str) -> &'static str {
    match mime {
        "image/gif" => ".gif",
        "image/png" => ".png",
        "image/jpeg" => ".jpg",
        "image/webp" => ".webp",
        _ => "",
    }
}

/// The sniffed type, which must be the declared one and an allowed one.
pub fn detect_and_validate_mime(
    data: &[u8],
    declared: &str,
    allowed: &[&str],
) -> Result<&'static str, ApiError> {
    let declared = declared
        .split(';')
        .next()
        .unwrap_or(declared)
        .trim()
        .to_ascii_lowercase();
    let reject = || {
        ApiError::bad_request(format!(
            "Invalid file content; expected one of {}",
            allowed.join(", ")
        ))
    };
    let detected = crate::api::posters::sniff(data).ok_or_else(reject)?;
    if detected != declared.as_str() || !allowed.contains(&detected) {
        return Err(reject());
    }
    Ok(detected)
}

struct Uploaded {
    data: Bytes,
    mime: String,
}

/// The 413 of each route: the horizontal upload limit's text and the
/// vertical one's (`Poster/routes.ts:45-66`; fixtures `events.poster.too_large`,
/// `events.poster_vertical.too_large`).
fn too_large(kind: &str) -> ApiError {
    if kind == "poster-vertical" {
        ApiError::http(
            413,
            format!(
                "File size limit has been reached (max {} KB)",
                MAX_POSTER_BYTES / 1024
            ),
        )
    } else {
        ApiError::http(413, "File size limit has been reached")
    }
}

/// The `poster` part, checked in the old order: the upload middleware's size
/// limit first (`Poster/routes.ts:45-66`), then the handler's param, multiple,
/// empty and content checks (`Poster/routes.ts:84-107`).
async fn read_poster(mut multipart: Multipart, kind: &str) -> Result<Uploaded, ApiError> {
    let mut poster: Option<Uploaded> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::bad_request(format!("invalid multipart: {e}")))?
    {
        if field.name() == Some("poster") {
            if poster.is_some() {
                return Err(ApiError::bad_request("Multiple files are not allowed"));
            }
            let mime = field
                .content_type()
                .map(|s| s.split(';').next().unwrap_or(s).to_string())
                .unwrap_or_default();
            let data = field
                .bytes()
                .await
                .map_err(|e| ApiError::bad_request(format!("invalid poster field: {e}")))?;
            if data.len() > MAX_POSTER_BYTES {
                return Err(too_large(kind));
            }
            poster = Some(Uploaded { data, mime });
        } else {
            let _ = field.bytes().await;
        }
    }
    let poster = poster.ok_or_else(|| ApiError::bad_request("Poster param is required"))?;
    if poster.data.is_empty() {
        return Err(ApiError::bad_request("Empty files are not allowed"));
    }
    Ok(poster)
}

/// `RequestError("Service unavailable", ServiceUnavailable, {...err})`
/// (`Poster/routes.ts:273-279`): the failing S3 call's `operation` and
/// `options` (`routes.ts:289-293`, set on the error by the wrappers) and the
/// error's own fields spread into `data`. This node has no bucket, so
/// `Bucket` is `null` and the error's field is its `message`. Fixture
/// `events.poster.bucket_unavailable`.
fn bucket_unavailable(operation: &str, options: Value, message: &str) -> ApiError {
    ApiError::with_data(
        503,
        "Service unavailable",
        json!({ "operation": operation, "options": options, "message": message }),
    )
}

async fn upload(
    state: SharedState,
    method: Method,
    path: &str,
    headers: HeaderMap,
    multipart: Multipart,
    kind: &'static str,
    allowed: &[&str],
) -> Result<Json<Value>, ApiError> {
    state.user_writes()?;
    let signer = state.signer(&headers, method.as_str(), path).await?;
    super::profiles::require_profile(&state.pool, &signer).await?;
    let poster = read_poster(multipart, kind).await?;
    let mime = detect_and_validate_mime(&poster.data, &poster.mime, allowed)?;
    let size = poster.data.len();
    // `ensure()` (`Poster/routes.ts:249-285`) checks the bucket before the
    // upload; this node's missing blob store is its `headBucket` failure.
    let storage = state.storage.clone().ok_or_else(|| {
        bucket_unavailable(
            "headBucket",
            json!({ "Bucket": null }),
            "no blob store is configured on this node",
        )
    })?;
    let name = format!(
        "{}{}",
        hex::encode(Sha256::digest(&poster.data)),
        extension(mime)
    );
    let cid = {
        let body = poster.data.clone();
        tokio::task::spawn_blocking(move || crate::auth::hash::hash_bytes_v1(&body))
            .await
            .map_err(|e| ApiError::internal(format!("poster hash task: {e}")))?
    };
    let stored = async {
        if !storage.exist(&cid).await? {
            storage.store(&cid, poster.data.clone()).await?;
        }
        Ok::<_, crate::storage::StorageError>(())
    }
    .await;
    if let Err(e) = stored {
        tracing::error!(error = %e, "poster blob store failed");
        return Err(bucket_unavailable(
            "putObject",
            json!({ "Bucket": null, "Key": name }),
            "the blob store refused the poster",
        ));
    }
    sqlx::query(
        "INSERT INTO explore.poster (kind, file, blob_cid, content_type, byte_size, origin, fetched_at)
         VALUES ($1, $2, $3, $4, $5, 'local', timezone('UTC', now()))
         ON CONFLICT (kind, file) DO UPDATE SET blob_cid = EXCLUDED.blob_cid, content_type = EXCLUDED.content_type,
             byte_size = EXCLUDED.byte_size, origin = 'local', fetched_at = EXCLUDED.fetched_at",
    )
    .bind(kind)
    .bind(&name)
    .bind(&cid)
    .bind(mime)
    .bind(size as i64)
    .execute(&state.pool)
    .await?;
    Ok(Json(poster_response(
        &state.settings.events_asset_base_url,
        kind,
        &name,
        size,
        mime,
    )))
}

/// The old `PosterAttributes` answer (`Poster/routes.ts:132-137`), `url`
/// absolute on this node's events mount (fixtures `events.poster.success`,
/// `events.poster_vertical.success`).
fn poster_response(asset_base: &str, kind: &str, name: &str, size: usize, mime: &str) -> Value {
    let filename = format!("{kind}/{name}");
    json!({
        "ok": true,
        "data": {
            "filename": filename,
            "url": format!("{}/{filename}", asset_base.trim_end_matches('/')),
            "size": size,
            "type": mime,
        }
    })
}

pub async fn upload_poster(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    multipart: Multipart,
) -> Result<Json<Value>, ApiError> {
    upload(
        state,
        method,
        uri.path(),
        headers,
        multipart,
        "poster",
        &POSTER_FILE_TYPES,
    )
    .await
}

pub async fn upload_poster_vertical(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    multipart: Multipart,
) -> Result<Json<Value>, ApiError> {
    upload(
        state,
        method,
        uri.path(),
        headers,
        multipart,
        "poster-vertical",
        &POSTER_VERTICAL_FILE_TYPES,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

    fn multipart(content_type: &str, data: &[u8]) -> (Vec<u8>, String) {
        let mut body = Vec::new();
        body.extend_from_slice(format!("--b\r\nContent-Disposition: form-data; name=\"poster\"; filename=\"p\"\r\nContent-Type: {content_type}\r\n\r\n").as_bytes());
        body.extend_from_slice(data);
        body.extend_from_slice(b"\r\n--b--\r\n");
        (body, "multipart/form-data; boundary=b".into())
    }

    /// An upload lands in the blob store and the poster index, and
    /// `GET /poster/{file}` of the explore API would serve it by that name.
    #[tokio::test]
    async fn an_uploaded_poster_is_indexed_by_its_old_name() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use std::sync::Arc;
        use tower::ServiceExt;
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let wallet = alloy_signer_local::PrivateKeySigner::random();
        let address = format!("{:#x}", wallet.address());
        let dir = std::env::temp_dir().join(format!(
            "catalyst-posters-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let storage = Arc::new(crate::storage::Storage::from_config(&dir).await.unwrap());
        let settings = crate::Settings {
            user_writes_enabled: true,
            ..crate::Settings::for_tests()
        };
        let app = crate::router(Arc::new(
            crate::ExploreState::new(pool.clone(), settings, None).with_storage(storage.clone()),
        ));
        let png = [PNG, &[7u8; 64][..]].concat();
        let post = |uri: &'static str, content_type: &'static str, data: Vec<u8>| {
            let app = app.clone();
            let wallet = wallet.clone();
            async move {
                let (body, ct) = multipart(content_type, &data);
                let mut req = Request::post(uri).header("content-type", ct);
                for (k, v) in crate::shared::signed_fetch::test_support::signed_headers_by(
                    &wallet,
                    "post",
                    uri,
                    "{}",
                    true,
                    &chrono::Utc::now().timestamp_millis().to_string(),
                )
                .await
                .iter()
                {
                    req = req.header(k, v);
                }
                let resp = app
                    .oneshot(req.body(Body::from(body)).unwrap())
                    .await
                    .unwrap();
                let status = resp.status();
                let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
                    .await
                    .unwrap();
                (
                    status,
                    serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null),
                )
            }
        };
        // `withAuthProfile()`: a signer without a profile is 404 (fixture
        // `events.poster.no_profile`).
        let (status, body) = post("/api/poster", "image/png", png.clone()).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
        assert_eq!(
            body["error"],
            format!("Not found profile for \"{address}\"")
        );
        super::super::profiles::test_support::deploy_profile(&pool, &address, json!({})).await;
        let (status, body) = post("/api/poster", "image/png", png.clone()).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        let name = format!("{}.png", hex::encode(Sha256::digest(&png)));
        assert_eq!(
            body,
            json!({ "ok": true, "data": { "filename": format!("poster/{name}"), "url": format!("http://localhost/events/poster/{name}"), "size": png.len(), "type": "image/png" } })
        );
        let (cid, origin): (String, String) = sqlx::query_as(
            "SELECT blob_cid, origin FROM explore.poster WHERE kind = 'poster' AND file = $1",
        )
        .bind(&name)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(origin, "local");
        assert_eq!(
            storage.retrieve(&cid).await.unwrap().unwrap().as_ref(),
            png.as_slice()
        );
        let (status, body) =
            post("/api/poster-vertical", "image/gif", b"GIF89a-----".to_vec()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            body["error"],
            "Invalid file content; expected one of image/jpeg, image/png, image/webp"
        );
        // The size limit answers before the content check, with each route's
        // text (`Poster/routes.ts:45-66`).
        let oversized = [b"not an image".as_slice(), &vec![0u8; MAX_POSTER_BYTES][..]].concat();
        let (status, body) = post("/api/poster", "image/png", oversized.clone()).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(body["error"], "File size limit has been reached");
        let (status, body) = post("/api/poster-vertical", "image/png", oversized).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(
            body["error"],
            "File size limit has been reached (max 500 KB)"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_answer_is_the_old_envelope_with_an_absolute_url_on_the_events_mount() {
        assert_eq!(
            poster_response(
                "https://node.example/prefix/events/",
                "poster-vertical",
                "abc.webp",
                7,
                "image/webp"
            ),
            json!({ "ok": true, "data": { "filename": "poster-vertical/abc.webp", "url": "https://node.example/prefix/events/poster-vertical/abc.webp", "size": 7, "type": "image/webp" } })
        );
        assert_eq!(
            too_large("poster").to_string(),
            "File size limit has been reached"
        );
        assert_eq!(
            too_large("poster-vertical").to_string(),
            "File size limit has been reached (max 500 KB)"
        );
    }

    #[test]
    fn the_sniffed_type_must_be_the_declared_and_an_allowed_one() {
        assert_eq!(
            detect_and_validate_mime(PNG, "image/png", &POSTER_FILE_TYPES).unwrap(),
            "image/png"
        );
        assert_eq!(
            detect_and_validate_mime(PNG, "image/png; charset=binary", &POSTER_FILE_TYPES).unwrap(),
            "image/png"
        );
        assert!(detect_and_validate_mime(PNG, "image/jpeg", &POSTER_FILE_TYPES).is_err());
        assert!(
            detect_and_validate_mime(b"GIF89a....", "image/gif", &POSTER_VERTICAL_FILE_TYPES)
                .is_err(),
            "no gif for vertical posters"
        );
        assert!(detect_and_validate_mime(b"<svg/>", "image/svg+xml", &POSTER_FILE_TYPES).is_err());
        assert_eq!(extension("image/jpeg"), ".jpg");
    }
    #[test]
    fn configured_origins_and_field_kinds_are_preserved_during_normalization() {
        let settings = Settings {
            events_asset_base_url: "https://native.example.org/events".into(),
            content_public_url: "https://content.example.org".into(),
            ..Settings::for_tests()
        };
        assert_eq!(
            normalize_image_input(
                "https://native.example.org/events/assets/event-default.webp",
                &settings,
                "poster"
            )
            .unwrap(),
            DEFAULT_EVENT_IMAGE
        );
        assert_eq!(
            normalize_image_input(
                "https://content.example.org/poster-vertical/old.webp",
                &settings,
                "poster-vertical"
            )
            .unwrap(),
            "/poster-vertical/old.webp"
        );
        assert!(
            normalize_image_input("/poster/animated.gif", &settings, "poster-vertical").is_err()
        );
        assert!(normalize_image_input("/poster-vertical/old.webp", &settings, "poster").is_err());
        assert!(
            normalize_image_input(
                "https://native.example.org/eventsevil/poster/old.webp",
                &settings,
                "poster"
            )
            .is_err()
        );
    }
}
