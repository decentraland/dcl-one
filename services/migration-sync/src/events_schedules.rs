//! Source-only Events schedule facts. No request replay, event mutations or delivery.
use anyhow::{Result, ensure};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

pub const SOURCE: &str = "events";
pub const ADAPTER: &str = "events_schedules_outbox_v1";
pub const SOURCE_SQL: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/services/migration-sync/source/events_schedules.sql"
));

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub image: Option<String>,
    pub theme: Option<String>,
    pub background: Vec<String>,
    pub active_since: DateTime<Utc>,
    pub active_until: DateTime<Utc>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "op", deny_unknown_fields)]
pub enum ScheduleChange {
    #[serde(rename = "schedule_upsert")]
    Upsert { schedule: ScheduleRecord },
    #[serde(rename = "schedule_delete")]
    Delete { id: String },
}

fn validate_id(id: &str) -> Result<()> {
    ensure!(
        Uuid::parse_str(id).is_ok_and(|value| !value.is_nil() && value.to_string() == id),
        "schedule ID must be a canonical nonnil UUID"
    );
    Ok(())
}

impl ScheduleRecord {
    pub fn validate(&self) -> Result<()> {
        validate_id(&self.id)?;
        ensure!(
            self.name.chars().count() <= 50,
            "schedule name exceeds source column bound"
        );
        for value in [&self.description, &self.image] {
            ensure!(
                value.as_ref().is_none_or(|s| s.chars().count() <= 255),
                "schedule text exceeds source column bound"
            );
        }
        ensure!(
            self.theme.as_ref().is_none_or(|s| s.chars().count() <= 25),
            "schedule theme exceeds source column bound"
        );
        ensure!(
            self.background.len() <= 1000
                && self.background.iter().all(|s| s.chars().count() <= 30),
            "schedule background exceeds capture bound"
        );
        ensure!(
            [
                self.active_since,
                self.active_until,
                self.created_at,
                self.updated_at
            ]
            .iter()
            .all(|time| time.timestamp_subsec_nanos() % 1000 == 0),
            "schedule timestamp exceeds source precision"
        );
        // Preserve active intervals and updated_at verbatim. Source PATCH does
        // not guarantee updated_at advances, so application is ordered by commit.
        Ok(())
    }
}
impl ScheduleChange {
    pub fn id(&self) -> &str {
        match self {
            Self::Upsert { schedule } => &schedule.id,
            Self::Delete { id } => id,
        }
    }
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Upsert { schedule } => schedule.validate(),
            Self::Delete { id } => validate_id(id),
        }
    }
}

/// Caller owns the source-ordered transaction and writes its receipt/checkpoint.
/// This function changes facts only; it cannot emit event notification intents.
pub async fn apply(connection: &mut PgConnection, change: &ScheduleChange) -> Result<()> {
    change.validate()?;
    match change {
        ScheduleChange::Upsert { schedule } => {
            sqlx::query("INSERT INTO explore.schedules_local(id,name,description,image,theme,background,active_since,active_until,active,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
                ON CONFLICT(id) DO UPDATE SET name=EXCLUDED.name,description=EXCLUDED.description,image=EXCLUDED.image,theme=EXCLUDED.theme,background=EXCLUDED.background,active_since=EXCLUDED.active_since,active_until=EXCLUDED.active_until,active=EXCLUDED.active,created_at=EXCLUDED.created_at,updated_at=EXCLUDED.updated_at")
                .bind(&schedule.id).bind(&schedule.name).bind(&schedule.description).bind(&schedule.image).bind(&schedule.theme)
                .bind(serde_json::to_value(&schedule.background)?).bind(schedule.active_since.naive_utc()).bind(schedule.active_until.naive_utc())
                .bind(schedule.active).bind(schedule.created_at.naive_utc()).bind(schedule.updated_at.naive_utc())
                .execute(connection).await?;
        }
        ScheduleChange::Delete { id } => {
            sqlx::query("DELETE FROM explore.schedules_local WHERE id=$1")
                .bind(id)
                .execute(connection)
                .await?;
        }
    }
    Ok(())
}

/// Read one source snapshot under the capture adapter's shared boundary.
pub async fn read_snapshot(connection: &mut PgConnection) -> Result<Vec<ScheduleRecord>> {
    // PostgreSQL's varchar(30)[] bounds each string, but not the array itself.
    // Check inside the caller's snapshot before transferring any array to Rust.
    let oversized: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM public.schedule s JOIN migration_capture_events.scope c ON c.entity_id=s.id WHERE cardinality(s.background)>1000)")
        .fetch_one(&mut *connection).await?;
    ensure!(
        !oversized,
        "schedule source background exceeds bounded array"
    );
    let rows=sqlx::query("SELECT s.id::text AS id,s.name,s.description,s.image,s.theme,s.background,s.active_since,s.active_until,s.active,s.created_at,s.updated_at FROM public.schedule s JOIN migration_capture_events.scope c ON c.entity_id=s.id ORDER BY s.id LIMIT 1001")
        .fetch_all(connection).await?;
    ensure!(
        rows.len() <= 1000,
        "schedule baseline exceeds bounded scope"
    );
    let mut schedules = Vec::with_capacity(rows.len());
    for row in rows {
        let schedule = ScheduleRecord {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            description: row.try_get("description")?,
            image: row.try_get("image")?,
            theme: row.try_get("theme")?,
            background: row.try_get("background")?,
            active_since: row.try_get("active_since")?,
            active_until: row.try_get("active_until")?,
            active: row.try_get("active")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        };
        schedule.validate()?;
        schedules.push(schedule);
    }
    Ok(schedules)
}

/// Read only the approved target projection for canonical reconciliation.
pub async fn read_target(
    connection: &mut PgConnection,
    scope: &[String],
) -> Result<Vec<ScheduleRecord>> {
    // The target JSON column also admits non-arrays and arbitrarily long strings.
    // CASE avoids evaluating jsonb_array_length for an invalid JSON shape.
    let invalid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM explore.schedules_local WHERE id=ANY($1) AND CASE WHEN jsonb_typeof(background)='array' THEN jsonb_array_length(background)>1000 OR octet_length(background::text)>1048576 ELSE true END)")
        .bind(scope).fetch_one(&mut *connection).await?;
    ensure!(
        !invalid,
        "schedule target background exceeds bounded array or has invalid shape"
    );
    let rows=sqlx::query("SELECT id,name,description,image,theme,background,active_since,active_until,active,created_at,updated_at FROM explore.schedules_local WHERE id=ANY($1) ORDER BY id LIMIT 1001")
        .bind(scope).fetch_all(connection).await?;
    ensure!(rows.len() <= 1000, "schedule target exceeds bounded scope");
    let mut schedules = Vec::with_capacity(rows.len());
    for row in rows {
        let background: serde_json::Value = row.try_get("background")?;
        let schedule = ScheduleRecord {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            description: row.try_get("description")?,
            image: row.try_get("image")?,
            theme: row.try_get("theme")?,
            background: serde_json::from_value(background)?,
            active_since: row
                .try_get::<chrono::NaiveDateTime, _>("active_since")?
                .and_utc(),
            active_until: row
                .try_get::<chrono::NaiveDateTime, _>("active_until")?
                .and_utc(),
            active: row.try_get("active")?,
            created_at: row
                .try_get::<chrono::NaiveDateTime, _>("created_at")?
                .and_utc(),
            updated_at: row
                .try_get::<chrono::NaiveDateTime, _>("updated_at")?
                .and_utc(),
        };
        schedule.validate()?;
        schedules.push(schedule);
    }
    Ok(schedules)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record() -> ScheduleRecord {
        let time = DateTime::parse_from_rfc3339("2026-11-12T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        ScheduleRecord {
            id: "11111111-1111-4111-8111-111111111111".into(),
            name: "Festival".into(),
            description: None,
            image: None,
            theme: None,
            background: vec![],
            active_since: time,
            active_until: time,
            active: false,
            created_at: time,
            updated_at: time,
        }
    }
    #[test]
    fn preserves_optional_values_and_source_interval_semantics() {
        let expected = ScheduleChange::Upsert { schedule: record() };
        let actual: ScheduleChange =
            serde_json::from_value(serde_json::to_value(&expected).unwrap()).unwrap();
        assert_eq!(actual, expected);
        assert!(actual.validate().is_ok());
    }
    #[test]
    fn rejects_noncanonical_keys_and_unbounded_input() {
        let mut schedule = record();
        schedule.id = "not-a-uuid".into();
        assert!(schedule.validate().is_err());
        schedule = record();
        schedule.background = vec!["x".repeat(31)];
        assert!(schedule.validate().is_err());
        schedule = record();
        schedule.theme = Some("x".repeat(26));
        assert!(schedule.validate().is_err());
        assert!(
            ScheduleChange::Delete {
                id: "00000000-0000-0000-0000-000000000000".into()
            }
            .validate()
            .is_err()
        );
    }
    #[test]
    fn unknown_fields_cannot_smuggle_additional_mutations() {
        let mut payload =
            serde_json::to_value(ScheduleChange::Upsert { schedule: record() }).unwrap();
        payload["extra"] = serde_json::json!("ignored command");
        assert!(serde_json::from_value::<ScheduleChange>(payload).is_err());
    }
}
