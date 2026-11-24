//! A minted item's public facts by contract and on-chain id: its `utility`,
//! and its kind and video for the contents answer.

use serde_json::{Map, Value};

use crate::access::MintedItem;
use crate::http::ApiError;

use super::ItemsComponent;

const VIDEO_PATH: &str = "video.mp4";

/// Whether a minted item is a smart wearable (a wearable with a `.js` file)
/// and the video it keeps.
pub struct MintedKind {
    pub smart: bool,
    pub video: Option<String>,
}

/// builder-server's contents answer over the entries anyone may read: the
/// entries, and for a smart wearable `video.mp4` naming `video`, the kept
/// video (`null` without one; left out when nobody may read it).
pub fn contents_answer(
    open: Vec<(String, String)>,
    smart: bool,
    video: Option<Option<String>>,
) -> Value {
    let mut map: Map<String, Value> = open
        .into_iter()
        .map(|(file, hash)| (file, Value::String(hash)))
        .collect();
    if smart {
        match video {
            Some(video) => {
                map.insert(VIDEO_PATH.into(), video.map_or(Value::Null, Value::String));
            }
            None => {
                map.remove(VIDEO_PATH);
            }
        }
    }
    Value::Object(map)
}

impl ItemsComponent {
    /// The item's `utility`.
    pub async fn utility(&self, item: &MintedItem) -> Result<Option<String>, ApiError> {
        Ok(
            sqlx::query_scalar("SELECT utility FROM builder.items WHERE id = $1")
                .bind(item.id())
                .fetch_one(&self.pool)
                .await?,
        )
    }

    /// Whether the item is a smart wearable, and its kept video.
    pub async fn minted_kind(&self, item: &MintedItem) -> Result<MintedKind, ApiError> {
        let (smart, video): (bool, Option<String>) = sqlx::query_as(
            "SELECT i.type = 'wearable' AND EXISTS (SELECT 1 FROM builder.item_contents f \
               WHERE f.item_id = i.id AND right(f.file, 3) = '.js'), i.video \
             FROM builder.items i WHERE i.id = $1",
        )
        .bind(item.id())
        .fetch_one(&self.pool)
        .await?;
        Ok(MintedKind { smart, video })
    }
}
