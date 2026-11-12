//! The sole residual fact is POI membership produced by the source daily job.
//! Caller owns the stream transaction and commits the outcome with its cursor.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgConnection, Row};
use uuid::Uuid;

pub const PRODUCER: &str = "places_poi_category_update_v1";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PoiResult {
    pub producer: String,
    pub entity_id: String,
    pub poi: bool,
}

impl PoiResult {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.producer == PRODUCER, "unsupported POI producer");
        let id = Uuid::parse_str(&self.entity_id).context("invalid POI entity ID")?;
        ensure!(
            !id.is_nil() && id.to_string() == self.entity_id,
            "noncanonical POI entity ID"
        );
        Ok(())
    }
}

/// Apply only the POI bit to an already migrated, exclusively owned Genesis row.
/// This function never creates catalogue records or changes unrelated categories.
pub async fn apply(connection: &mut PgConnection, result: &PoiResult) -> Result<()> {
    result.validate()?;
    let row = sqlx::query(
        "SELECT CASE WHEN cardinality(categories)<=100 AND pg_column_size(categories)<=8192
           THEN categories END AS categories,
         CASE WHEN jsonb_typeof(raw->'categories')='array'
           THEN CASE WHEN jsonb_array_length(raw->'categories')<=100 AND pg_column_size(raw->'categories')<=8192
             THEN raw->'categories' END END AS raw_categories,
         origin='local' AND raw->>'source'='places' AND raw->'local'='true'::jsonb
           AND NOT world AND raw->'world'='false'::jsonb AND world_name IS NULL AS owned,
         EXISTS(SELECT 1 FROM explore.place_world_local w WHERE w.id=p.id)
           OR EXISTS(SELECT 1 FROM explore.place_moderation m WHERE m.place_id=p.id)
           OR EXISTS(SELECT 1 FROM explore.place_ranking r WHERE r.place_id=p.id) AS overlay
         FROM explore.place p WHERE id=$1 FOR UPDATE",
    )
    .bind(&result.entity_id)
    .fetch_optional(&mut *connection)
    .await?
    .context("POI target catalogue row is missing")?;
    ensure!(
        row.try_get::<Option<bool>, _>("owned")? == Some(true),
        "POI target ownership is unsupported"
    );
    ensure!(
        !row.try_get::<bool, _>("overlay")?,
        "POI target has native overlays"
    );
    let categories: Vec<String> = row.try_get("categories")?;
    ensure!(
        categories.len() <= 100 && categories.iter().all(|value| value.len() <= 64),
        "POI categories exceed bounds"
    );
    let raw_categories: Value = row.try_get("raw_categories")?;
    ensure!(
        raw_categories == serde_json::to_value(&categories)?,
        "POI category projections disagree"
    );
    let mut next: Vec<String> = categories
        .into_iter()
        .filter(|value| value != "poi")
        .collect();
    if result.poi {
        next.push("poi".to_owned());
    }
    ensure!(next.len() <= 100, "POI categories exceed bounds");
    sqlx::query("UPDATE explore.place SET categories=$2,raw=jsonb_set(raw,'{categories}',to_jsonb($2::text[]),true) WHERE id=$1")
        .bind(&result.entity_id).bind(&next).execute(connection).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_canonical_job_outcomes_are_admitted() {
        let valid = PoiResult {
            producer: PRODUCER.into(),
            entity_id: "a1111111-1111-4111-8111-111111111111".into(),
            poi: true,
        };
        valid.validate().unwrap();
        let mut invalid = valid.clone();
        invalid.producer = "http".into();
        assert!(invalid.validate().is_err());
        invalid = valid;
        invalid.entity_id = invalid.entity_id.to_uppercase();
        assert!(invalid.validate().is_err());
        assert!(serde_json::from_value::<PoiResult>(serde_json::json!({"producer":PRODUCER,"entity_id":"a1111111-1111-4111-8111-111111111111","poi":true,"categories":["other"]})).is_err());
    }
}
