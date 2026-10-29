//! The wire shapes of camera-reel-service (`library/catalyst-api/camera-reel`
//! was generated from the old crate's `ts` feature, which this package does
//! not carry). `Image.url`/`thumbnailUrl` are rendered from the stored hashes
//! by [`super::db`] as `<api_url>/api/images/<hash>`.

use serde::{Deserialize, Serialize};

use super::db::DbImage;

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "camera-reel/")
)]
pub struct Image {
    pub id: String,
    pub url: String,
    pub thumbnail_url: String,
    pub is_public: bool,
    pub metadata: Metadata,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "camera-reel/")
)]
pub struct GalleryImage {
    pub id: String,
    pub url: String,
    pub thumbnail_url: String,
    pub is_public: bool,
    pub date_time: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "camera-reel/")
)]
pub struct GalleryImageWithPlace {
    pub id: String,
    pub url: String,
    pub thumbnail_url: String,
    pub is_public: bool,
    pub date_time: String,
    pub place_id: String,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "camera-reel/")
)]
pub struct Metadata {
    pub user_name: String,
    pub user_address: String,
    pub date_time: String,
    pub realm: String,
    pub scene: Scene,
    pub visible_people: Vec<User>,
    pub place_id: String,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "camera-reel/")
)]
pub struct Scene {
    pub name: String,
    pub location: Location,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "camera-reel/")
)]
pub struct Location {
    pub x: String,
    pub y: String,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "camera-reel/")
)]
pub struct User {
    pub user_name: String,
    pub user_address: String,
    pub wearables: Vec<String>,
    #[serde(default)]
    pub is_guest: bool,
    #[serde(default)]
    pub is_emoting: Option<bool>,
}

impl From<DbImage> for Image {
    fn from(value: DbImage) -> Self {
        Self {
            id: value.id.to_string(),
            url: value.url,
            thumbnail_url: value.thumbnail_url,
            is_public: value.is_public,
            metadata: value.metadata.0,
        }
    }
}

impl From<DbImage> for GalleryImage {
    fn from(value: DbImage) -> Self {
        Self {
            id: value.id.to_string(),
            url: value.url,
            thumbnail_url: value.thumbnail_url,
            is_public: value.is_public,
            date_time: value.metadata.0.date_time,
        }
    }
}

impl From<DbImage> for GalleryImageWithPlace {
    fn from(value: DbImage) -> Self {
        Self {
            id: value.id.to_string(),
            url: value.url,
            thumbnail_url: value.thumbnail_url,
            is_public: value.is_public,
            date_time: value.metadata.0.date_time,
            place_id: value.metadata.0.place_id,
        }
    }
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "camera-reel/")
)]
pub struct UserDataResponse {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub current_images: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub max_images: u64,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "camera-reel/")
)]
pub struct PlaceDataResponse {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub max_images: u64,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "camera-reel/")
)]
pub struct UploadResponse {
    pub image: Image,
    #[serde(flatten)]
    pub user_data: UserDataResponse,
}

/// The gallery pages. `nextCursor` is present only on a keyset request
/// (`?cursor=`), so the offset shape stays what the old service answered.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "camera-reel/")
)]
pub struct GetImagesResponse {
    pub images: Vec<Image>,
    #[serde(flatten)]
    pub user_data: UserDataResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub next_cursor: Option<Option<String>>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "camera-reel/")
)]
pub struct GetGalleryImagesResponse {
    pub images: Vec<GalleryImage>,
    #[serde(flatten)]
    pub user_data: UserDataResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub next_cursor: Option<Option<String>>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "camera-reel/")
)]
pub struct GetPlaceImagesResponse {
    pub images: Vec<GalleryImage>,
    #[serde(flatten)]
    pub place_data: PlaceDataResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub next_cursor: Option<Option<String>>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "camera-reel/")
)]
pub struct GetMultiplePlacesImagesResponse {
    pub images: Vec<GalleryImageWithPlace>,
    #[serde(flatten)]
    pub place_data: PlaceDataResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub next_cursor: Option<Option<String>>,
}

/// `GET /api/wearables/{item}/images`: full images (who is in the shot and
/// where it was taken, without one metadata request per photo) and the
/// matching set's size; offset pages only, as upstream answers them.
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetWearableImagesResponse {
    pub images: Vec<Image>,
    pub max_images: u64,
}

#[derive(Deserialize, Debug)]
pub struct UpdateVisibility {
    pub is_public: bool,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReview {
    pub review_status: String,
}

impl UpdateReview {
    pub fn is_valid(&self) -> bool {
        matches!(self.review_status.as_str(), "ok" | "flagged" | "rejected")
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetMultiplePlacesImagesBody {
    pub places_ids: Vec<String>,
}
