//! The camera-reel service: how many photos the player may keep, the photo upload, and the
//! places lookup that names the scene a photo was taken in.

use std::io::Read;

use bevy::{prelude::*, tasks::Task};
use common::base_domain::{url, Service};
use serde::Deserialize;
use wallet::{sign_service_request, Wallet};

use crate::metadata::{ScreenshotMetadata, UNKNOWN_PLACE};

/// How long after a failed status fetch an open photo mode asks the service again.
const STORAGE_RETRY_SECS: f64 = 5.0;

/// How many photos the player keeps and may keep. Nothing is known until the service answers,
/// and until then there is no room for a photo.
#[derive(Resource, Default, Debug)]
pub struct CameraReelStorage {
    pub current: u32,
    pub max: u32,
    pub loaded: bool,
    /// The address the counts belong to.
    pub address: Option<String>,
    pub(crate) fetch: Option<Task<Result<StorageResponse, String>>>,
    /// When a failed fetch is asked again while photo mode is open, in elapsed seconds.
    pub(crate) retry_at: Option<f64>,
}

impl CameraReelStorage {
    pub fn has_free_space(&self) -> bool {
        self.loaded && self.current < self.max
    }

    pub fn set(&mut self, current: u32, max: u32) {
        self.current = current;
        self.max = max;
        self.loaded = true;
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StorageResponse {
    pub current_images: u32,
    pub max_images: u32,
}

/// The stored photo; only its id is read here (the HUD's gallery reads the rest).
#[derive(Deserialize, Debug, Clone)]
pub struct UploadedImage {
    pub id: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UploadResponse {
    pub current_images: u32,
    pub max_images: u32,
    pub image: UploadedImage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UploadError {
    /// The service refused the photo because the player's storage is full.
    LimitReached,
    Other(String),
}

impl std::fmt::Display for UploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UploadError::LimitReached => write!(f, "storage limit reached"),
            UploadError::Other(message) => write!(f, "{message}"),
        }
    }
}

#[derive(Deserialize)]
struct ErrorEnvelope {
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

pub fn images_url() -> String {
    url(Service::CameraReel, "/api/images")
}

pub fn user_url(address: &str) -> String {
    url(Service::CameraReel, &format!("/api/users/{address}"))
}

async fn signed_headers(
    method: &str,
    url: &str,
    wallet: &Wallet,
) -> Result<Vec<(String, String)>, String> {
    sign_service_request(method, url, wallet, "{}".to_owned())
        .await
        .map_err(|e| format!("signing failed: {e}"))
}

pub async fn fetch_storage(
    client: reqwest::Client,
    wallet: Wallet,
    address: String,
) -> Result<StorageResponse, String> {
    let url = user_url(&address);
    let mut request = client.get(&url).timeout(std::time::Duration::from_secs(20));
    for (key, value) in signed_headers("GET", &url, &wallet).await? {
        request = request.header(key, value);
    }
    let response = platform::TracedJsonResponse::send(request)
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("storage status {}", response.discard()));
    }
    response
        .json::<StorageResponse>()
        .await
        .map_err(|e| format!("storage body: {e}"))
}

/// Deletes one of the signer's photos; the service answers 403 for another owner's and 404 for
/// an absent one.
pub async fn delete_photo(
    client: reqwest::Client,
    wallet: Wallet,
    id: String,
) -> Result<(), String> {
    let url = url(
        Service::CameraReel,
        &format!("/api/images/{}", urlencoding::encode(&id)),
    );
    let mut request = client
        .delete(&url)
        .timeout(std::time::Duration::from_secs(20));
    for (key, value) in signed_headers("DELETE", &url, &wallet).await? {
        request = request.header(key, value);
    }
    let response = platform::TracedJsonResponse::send(request)
        .await
        .map_err(|e| e.to_string())?;
    let status = response.discard();
    if status.is_success() {
        Ok(())
    } else {
        Err(format!("delete status {status}"))
    }
}

/// The multipart body the service expects: the JPEG as `image`, the metadata JSON as `metadata`.
pub fn multipart_body(
    jpeg: Vec<u8>,
    metadata: &ScreenshotMetadata,
) -> Result<(String, Vec<u8>), String> {
    let metadata_json = serde_json::to_string(metadata).map_err(|e| e.to_string())?;
    let mut form = multipart::client::lazy::Multipart::new();
    form.add_text("metadata", metadata_json);
    form.add_stream(
        "image",
        std::io::Cursor::new(jpeg),
        Some(format!("{}.jpg", metadata.date_time)),
        Some(
            "image/jpeg"
                .parse::<mime::Mime>()
                .map_err(|e| e.to_string())?,
        ),
    );
    let mut prepared = form.prepare().map_err(|e| e.to_string())?;
    let mut body = Vec::new();
    prepared.read_to_end(&mut body).map_err(|e| e.to_string())?;
    Ok((prepared.boundary().to_owned(), body))
}

pub async fn upload_photo(
    client: reqwest::Client,
    wallet: Wallet,
    jpeg: Vec<u8>,
    metadata: ScreenshotMetadata,
) -> Result<UploadResponse, UploadError> {
    let url = images_url();
    let (boundary, body) = multipart_body(jpeg, &metadata).map_err(UploadError::Other)?;
    let mut request = client
        .post(&url)
        .timeout(std::time::Duration::from_secs(60))
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(body);
    for (key, value) in signed_headers("POST", &url, &wallet)
        .await
        .map_err(UploadError::Other)?
    {
        request = request.header(key, value);
    }
    let response = platform::TracedJsonResponse::send(request)
        .await
        .map_err(|e| UploadError::Other(e.to_string()))?;
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|e| UploadError::Other(e.to_string()))?;
    if status.is_success() {
        return serde_json::from_slice::<UploadResponse>(&bytes)
            .map_err(|e| UploadError::Other(format!("upload body: {e}")));
    }
    let envelope = serde_json::from_slice::<ErrorEnvelope>(&bytes).ok();
    if envelope
        .as_ref()
        .and_then(|e| e.reason.as_deref())
        .is_some_and(|reason| reason == "maxLimitReached")
    {
        return Err(UploadError::LimitReached);
    }
    let message = envelope
        .and_then(|e| e.message)
        .unwrap_or_else(|| String::from_utf8_lossy(&bytes).into_owned());
    Err(UploadError::Other(format!(
        "upload status {status}: {message}"
    )))
}

#[derive(Deserialize)]
struct PlacesEnvelope {
    #[serde(default)]
    ok: Option<bool>,
    #[serde(default)]
    data: Vec<PlaceEntry>,
}

#[derive(Deserialize)]
struct PlaceEntry {
    id: String,
    #[serde(default)]
    title: Option<String>,
}

/// The place a photo was taken in: `(name, id)` from the places API, or the unknown place.
/// A world realm is looked up by name, Genesis City by parcel.
pub async fn lookup_place(
    client: reqwest::Client,
    world_name: Option<String>,
    parcel: IVec2,
) -> (String, String) {
    let request = match world_name {
        Some(name) => url(
            Service::Places,
            &format!("/api/worlds?names={}", urlencoding::encode(&name)),
        ),
        None => url(
            Service::Places,
            &format!("/api/places?positions={},{}", parcel.x, parcel.y),
        ),
    };
    let found = async {
        let response = platform::TracedJsonResponse::send(
            client
                .get(&request)
                .timeout(std::time::Duration::from_secs(20)),
        )
        .await
        .ok()?;
        let envelope = response.json::<PlacesEnvelope>().await.ok()?;
        if envelope.ok == Some(false) {
            return None;
        }
        envelope.data.into_iter().next()
    }
    .await;
    match found {
        Some(place) => (
            place.title.unwrap_or_else(|| UNKNOWN_PLACE.to_owned()),
            place.id,
        ),
        None => (UNKNOWN_PLACE.to_owned(), UNKNOWN_PLACE.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::{Location, Scene};

    fn metadata() -> ScreenshotMetadata {
        ScreenshotMetadata {
            user_name: "me".into(),
            user_address: "0xabc".into(),
            date_time: "1700000000".into(),
            realm: "main".into(),
            scene: Scene {
                name: "Plaza".into(),
                location: Location {
                    x: "0".into(),
                    y: "0".into(),
                },
            },
            visible_people: vec![],
            place_id: "p".into(),
        }
    }

    #[test]
    fn the_multipart_body_carries_the_image_and_metadata_parts() {
        let (boundary, body) = multipart_body(vec![0xFF, 0xD8, 0xFF], &metadata()).unwrap();
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains(&format!("--{boundary}")));
        assert!(text.contains("name=\"metadata\""));
        assert!(text.contains("\"userAddress\":\"0xabc\""));
        assert!(text.contains("name=\"image\""));
        assert!(text.contains("filename=\"1700000000.jpg\""));
        assert!(text.contains("Content-Type: image/jpeg"));
    }

    #[test]
    fn storage_has_no_room_until_the_service_answers() {
        let mut storage = CameraReelStorage::default();
        assert!(!storage.has_free_space());
        storage.set(0, 500);
        assert!(storage.has_free_space());
        storage.set(500, 500);
        assert!(!storage.has_free_space());
    }

    #[test]
    fn the_upload_response_needs_only_the_photo_id() {
        let response: UploadResponse = serde_json::from_str(
            r#"{"currentImages":3,"maxImages":500,"image":{"id":"abc","url":"u","thumbnailUrl":"t","isPublic":false}}"#,
        )
        .unwrap();
        assert_eq!(response.image.id, "abc");
        let minimal: UploadResponse =
            serde_json::from_str(r#"{"currentImages":3,"maxImages":500,"image":{"id":"abc"}}"#)
                .unwrap();
        assert_eq!((minimal.current_images, minimal.max_images), (3, 500));
    }
}

/// Fetches the storage counts for the signed-in address when it changes and whenever photo mode
/// opens, so a photo deleted from the gallery frees its slot here too. A fetch that failed is
/// retried when a photo is asked for and, while photo mode stays open, every few seconds: the
/// HUD's Shoot is disabled until the status is known, so it recovers without reopening photo mode.
pub(crate) fn fetch_storage_status(
    mut storage: ResMut<CameraReelStorage>,
    wallet: Res<Wallet>,
    mut opened: EventReader<crate::mode::PhotoModeChanged>,
    mut takes: EventReader<crate::capture::TakePhotoRequest>,
    in_photo_mode: Query<(), With<crate::PhotoCamera>>,
    time: Res<Time>,
    ipfs: ipfs::IpfsAssetServer,
) {
    use common::util::{TaskCompat, TaskExt};

    let now = time.elapsed_secs_f64();
    if let Some(result) = storage.fetch.as_mut().and_then(|task| task.complete()) {
        storage.fetch = None;
        match result {
            Ok(response) => {
                storage.set(response.current_images, response.max_images);
                storage.retry_at = None;
            }
            Err(message) => {
                warn!("camera reel storage status: {message}");
                storage.retry_at = Some(now + STORAGE_RETRY_SECS);
            }
        }
    }

    let address = wallet.address().map(|a| format!("{a:#x}"));
    let opened = opened.read().any(|change| change.active);
    let asked = takes.read().next().is_some();
    let address_changed = address != storage.address;
    if address_changed {
        // a fetch for the previous address would land as this one's
        storage.fetch = None;
        storage.address = address.clone();
        storage.loaded = false;
        storage.current = 0;
        storage.max = 0;
        storage.retry_at = None;
    }
    let Some(address) = address else {
        return;
    };
    let due = !in_photo_mode.is_empty() && storage.retry_at.is_some_and(|at| now >= at);
    let retry = (asked || due) && !storage.loaded;
    if (address_changed || opened || retry) && storage.fetch.is_none() {
        storage.fetch = Some(bevy::tasks::IoTaskPool::get().spawn_compat(fetch_storage(
            ipfs.ipfs().client(),
            wallet.clone(),
            address,
        )));
    }
}
