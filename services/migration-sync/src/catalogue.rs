//! Authoritative Genesis catalogue projection for an explicitly scoped follower.
//! No provider is called. Existing removal triggers retain their intents inert
//! until a separately reviewed ownership transfer starts normal services.
use anyhow::{Result, ensure};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

pub fn validate(entity_id: &str, document: &Value) -> Result<()> {
    ensure!(
        Uuid::parse_str(entity_id).is_ok_and(|id| !id.is_nil() && id.to_string() == entity_id),
        "catalogue ID must be canonical UUID"
    );
    let row = document
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("catalogue document must be an object"))?;
    ensure!(
        document["id"] == entity_id && document["world"] == false,
        "catalogue identity or Genesis scope differs"
    );
    ensure!(
        document["source"] == "places" && document["local"] == true,
        "catalogue provenance differs"
    );
    ensure!(
        row.len() <= 100 && serde_json::to_vec(document)?.len() <= 65536,
        "catalogue document exceeds pilot bound"
    );
    ensure!(
        document["base_position"]
            .as_str()
            .is_some_and(|value| !value.is_empty() && value.len() <= 32),
        "catalogue base position missing or oversized"
    );
    ensure!(
        document["disabled"].is_boolean() && document["highlighted"].is_boolean(),
        "catalogue visibility flags must be boolean"
    );
    ensure!(
        document["categories"]
            .as_array()
            .is_some_and(|values| values.len() <= 100
                && values
                    .iter()
                    .all(|value| value.as_str().is_some_and(|s| s.len() <= 128))),
        "catalogue categories invalid"
    );
    for field in [
        "title",
        "description",
        "creator_address",
        "owner",
        "content_rating",
        "image",
        "contact_name",
        "contact_email",
        "disabled_reason",
        "world_name",
    ] {
        ensure!(
            document[field].is_null() || document[field].is_string(),
            "catalogue text field has invalid type"
        );
    }
    for field in ["created_at", "updated_at", "deployed_at", "disabled_at"] {
        if let Some(value) = document[field].as_str() {
            DateTime::parse_from_rfc3339(value)
                .map_err(|_| anyhow::anyhow!("catalogue timestamp invalid"))?;
        } else {
            ensure!(document[field].is_null(), "catalogue timestamp invalid");
        }
    }
    ensure!(
        document["world_name"].is_null() && document["world_id"].is_null(),
        "world catalogue data is excluded"
    );
    Ok(())
}

pub async fn apply(
    connection: &mut PgConnection,
    entity_id: &str,
    document: Option<&Value>,
) -> Result<()> {
    if let Some(document) = document {
        validate(entity_id, document)?;
    }
    let conflict:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM explore.place_world_local WHERE id=$1) OR EXISTS(SELECT 1 FROM explore.place WHERE id=$1 AND (world OR origin<>'local' OR raw->>'source' IS DISTINCT FROM 'places' OR raw->'local' IS DISTINCT FROM 'true'::jsonb)) OR EXISTS(SELECT 1 FROM explore.place_moderation WHERE place_id=$1 AND set_by IS DISTINCT FROM 'migration:places') OR EXISTS(SELECT 1 FROM explore.place_ranking WHERE place_id=$1 AND set_by IS DISTINCT FROM 'migration:places')")
        .bind(entity_id).fetch_one(&mut *connection).await?;
    ensure!(
        !conflict,
        "catalogue identity has conflicting local ownership"
    );
    // Source rows already carry effective decisions; remove only overlays owned
    // by the earlier batch importer so stale copies cannot supersede new facts.
    sqlx::query(
        "DELETE FROM explore.place_moderation WHERE place_id=$1 AND set_by='migration:places'",
    )
    .bind(entity_id)
    .execute(&mut *connection)
    .await?;
    sqlx::query(
        "DELETE FROM explore.place_ranking WHERE place_id=$1 AND set_by='migration:places'",
    )
    .bind(entity_id)
    .execute(&mut *connection)
    .await?;
    if let Some(document) = document {
        let categories: Vec<String> = document["categories"]
            .as_array()
            .expect("validated categories")
            .iter()
            .map(|value| value.as_str().expect("validated category").to_owned())
            .collect();
        let deployed = document["deployed_at"]
            .as_str()
            .map(DateTime::parse_from_rfc3339)
            .transpose()?
            .map(|value| value.with_timezone(&Utc).naive_utc());
        let creator = document["creator_address"]
            .as_str()
            .or(document["owner"].as_str())
            .map(|value| value.trim().to_lowercase())
            .filter(|value| !value.is_empty());
        sqlx::query("INSERT INTO explore.place(id,base_position,title,description,creator_address,content_rating,categories,deployed_at,disabled,highlighted,raw,origin) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,'local') ON CONFLICT(id) DO UPDATE SET base_position=EXCLUDED.base_position,title=EXCLUDED.title,description=EXCLUDED.description,creator_address=EXCLUDED.creator_address,content_rating=EXCLUDED.content_rating,categories=EXCLUDED.categories,deployed_at=EXCLUDED.deployed_at,disabled=EXCLUDED.disabled,highlighted=EXCLUDED.highlighted,raw=EXCLUDED.raw,fetched_at=timezone('UTC',now())")
            .bind(entity_id).bind(document["base_position"].as_str()).bind(document["title"].as_str()).bind(document["description"].as_str()).bind(creator).bind(document["content_rating"].as_str()).bind(categories).bind(deployed).bind(document["disabled"].as_bool()).bind(document["highlighted"].as_bool()).bind(document).execute(&mut *connection).await?;
    } else {
        sqlx::query("DELETE FROM explore.place WHERE id=$1")
            .bind(entity_id)
            .execute(&mut *connection)
            .await?;
    }
    Ok(())
}

/// Check every mapped representation, not just retained JSON or row count.
pub async fn equivalent(
    connection: &mut PgConnection,
    entity_id: &str,
    document: &Value,
) -> Result<bool> {
    validate(entity_id, document)?;
    let row=sqlx::query("SELECT base_position,title,description,creator_address,content_rating,categories,deployed_at,disabled,highlighted,raw,origin,world, NOT EXISTS(SELECT 1 FROM explore.place_moderation WHERE place_id=$1) AND NOT EXISTS(SELECT 1 FROM explore.place_ranking WHERE place_id=$1) AND NOT EXISTS(SELECT 1 FROM explore.place_world_local WHERE id=$1) AS unopposed FROM explore.place WHERE id=$1")
        .bind(entity_id).fetch_optional(&mut *connection).await?;
    let Some(row) = row else { return Ok(false) };
    let creator = document["creator_address"]
        .as_str()
        .or(document["owner"].as_str())
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty());
    let deployed = document["deployed_at"]
        .as_str()
        .map(DateTime::parse_from_rfc3339)
        .transpose()?
        .map(|value| value.with_timezone(&Utc).naive_utc());
    Ok(row.try_get::<bool, _>("unopposed")?
        && row.try_get::<Value, _>("raw")? == *document
        && row.try_get::<String, _>("base_position")?
            == document["base_position"].as_str().unwrap_or_default()
        && json!(row.try_get::<Option<String>, _>("title")?) == document["title"]
        && json!(row.try_get::<Option<String>, _>("description")?) == document["description"]
        && row.try_get::<Option<String>, _>("creator_address")? == creator
        && json!(row.try_get::<Option<String>, _>("content_rating")?) == document["content_rating"]
        && json!(row.try_get::<Vec<String>, _>("categories")?) == document["categories"]
        && row.try_get::<Option<chrono::NaiveDateTime>, _>("deployed_at")? == deployed
        && json!(row.try_get::<bool, _>("disabled")?) == document["disabled"]
        && json!(row.try_get::<bool, _>("highlighted")?) == document["highlighted"]
        && row.try_get::<String, _>("origin")? == "local"
        && !row.try_get::<bool, _>("world")?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn document() -> Value {
        json!({"id":"11111111-1111-4111-8111-111111111111","world":false,"source":"places","local":true,"base_position":"0,0","disabled":false,"highlighted":false,"categories":["poi"],"world_name":null,"world_id":null})
    }
    #[test]
    fn canonical_genesis_document_is_accepted() {
        assert!(validate("11111111-1111-4111-8111-111111111111", &document()).is_ok());
    }
    #[test]
    fn worlds_and_wrong_types_are_rejected() {
        for (field, value) in [
            ("world", json!(true)),
            ("world_name", json!("world.dcl.eth")),
            ("categories", json!([1])),
            ("disabled", json!("false")),
            ("deployed_at", json!("yesterday")),
            ("source", json!("content")),
        ] {
            let mut row = document();
            row[field] = value;
            assert!(validate("11111111-1111-4111-8111-111111111111", &row).is_err());
        }
    }
}
