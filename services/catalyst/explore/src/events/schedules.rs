//! `GET /api/schedules` and `GET /api/schedules/{id}` over the local
//! `explore.schedules_local` table (schedules are local authority; the signed
//! upsert arrives with stage 5).

use std::time::Duration;

use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::PgPool;

use super::ScheduleRecord;
use crate::shared::cache::{TtlCache, cache_get, cache_put};
use crate::shared::errors::ApiError;

/// Local writes will invalidate, so the memo only delays what other nodes wrote.
const SCHEDULE_CACHE_TTL: Duration = Duration::from_secs(30);

pub struct SchedulesComponent {
    pool: PgPool,
    cache: TtlCache<Vec<ScheduleRecord>>,
}

#[derive(sqlx::FromRow)]
struct ScheduleLocalRow {
    id: String,
    name: String,
    description: Option<String>,
    image: Option<String>,
    theme: Option<String>,
    background: Value,
    active_since: Option<NaiveDateTime>,
    active_until: Option<NaiveDateTime>,
    active: bool,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

impl ScheduleLocalRow {
    fn into_record(self) -> ScheduleRecord {
        let background = self
            .background
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        ScheduleRecord {
            id: self.id,
            name: self.name,
            description: self.description,
            image: self.image,
            theme: self.theme,
            background,
            active_since: self.active_since.map(|t| t.and_utc()),
            active_until: self.active_until.map(|t| t.and_utc()),
            active: self.active,
            created_at: Some(self.created_at.and_utc()),
            updated_at: Some(self.updated_at.and_utc()),
        }
    }
}

impl SchedulesComponent {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            cache: Default::default(),
        }
    }

    pub fn invalidate(&self) {
        self.cache.lock().unwrap().clear();
    }

    pub async fn list(&self) -> Result<Vec<ScheduleRecord>, ApiError> {
        if let Some(list) = cache_get(&self.cache, "all") {
            return Ok(list);
        }
        let rows: Vec<ScheduleLocalRow> = sqlx::query_as(
            "SELECT id, name, description, image, theme, background, active_since, active_until, \
                    active, created_at, updated_at \
             FROM explore.schedules_local ORDER BY active_since ASC NULLS LAST, id ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        let list: Vec<ScheduleRecord> = rows
            .into_iter()
            .map(ScheduleLocalRow::into_record)
            .collect();
        cache_put(
            &self.cache,
            "all".to_string(),
            list.clone(),
            SCHEDULE_CACHE_TTL,
        );
        Ok(list)
    }

    pub async fn get(&self, schedule_id: &str) -> Result<Option<ScheduleRecord>, ApiError> {
        Ok(self.list().await?.into_iter().find(|s| s.id == schedule_id))
    }
}
