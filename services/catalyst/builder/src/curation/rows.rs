//! The item curation rows a third-party publish answers, as builder-server
//! answers them: each row with every column.

use chrono::NaiveDateTime;
use serde::{Serialize, Serializer};
use uuid::Uuid;

use crate::http::iso_timestamp;

fn naive_iso<S: Serializer>(t: &NaiveDateTime, s: S) -> Result<S::Ok, S::Error> {
    iso_timestamp(&t.and_utc(), s)
}

/// A `builder.item_curations` row.
#[derive(Debug, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct ItemCurationOut {
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub id: Uuid,
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub item_id: Uuid,
    pub status: String,
    #[serde(serialize_with = "naive_iso")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub created_at: NaiveDateTime,
    #[serde(serialize_with = "naive_iso")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub updated_at: NaiveDateTime,
    pub content_hash: String,
    pub is_mapping_complete: Option<bool>,
}
