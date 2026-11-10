//! `GET /api/schedules` and `GET /api/schedules/{id}` over the local
//! `explore.schedules_local` table (schedules are local authority; the writes
//! are `events::profiles`). The list answers the schedules still running, as
//! the old service's `getSchedules` did (`Schedule/model.ts:15-20`:
//! `WHERE active_until > now()`); by id (`Schedule/routes.ts:38-46`,
//! `findOne`) and the sitemap (`Sitemap/routes.ts:84-94`, `find()`) read
//! every row.

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

    /// Every schedule row, in the old order (`active_since`, then id).
    pub async fn all(&self) -> Result<Vec<ScheduleRecord>, ApiError> {
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

    /// `GET /api/schedules`: the schedules whose `active_until` is still ahead
    /// (`Schedule/model.ts:15-20`; fixture `events.get_schedules.list`). A schedule
    /// without an end never lists: the old column was NOT NULL.
    pub async fn list(&self) -> Result<Vec<ScheduleRecord>, ApiError> {
        let now = chrono::Utc::now();
        Ok(still_running(self.all().await?, now))
    }

    /// `GET /api/schedules/{id}`: any row, expired or not (`Schedule/routes.ts:38-46`).
    pub async fn get(&self, schedule_id: &str) -> Result<Option<ScheduleRecord>, ApiError> {
        Ok(self.all().await?.into_iter().find(|s| s.id == schedule_id))
    }
}

fn still_running(
    schedules: Vec<ScheduleRecord>,
    now: chrono::DateTime<chrono::Utc>,
) -> Vec<ScheduleRecord> {
    schedules
        .into_iter()
        .filter(|s| s.active_until.is_some_and(|until| until > now))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schedule(id: &str, until: Option<&str>) -> ScheduleRecord {
        ScheduleRecord {
            id: id.into(),
            name: id.into(),
            description: None,
            image: None,
            theme: None,
            background: vec![],
            active_since: None,
            active_until: until.map(|u| u.parse().unwrap()),
            active: true,
            created_at: None,
            updated_at: None,
        }
    }

    /// `Schedule/model.ts:15-20`: `active_until > now()`, strictly.
    #[test]
    fn the_list_keeps_only_schedules_whose_end_is_ahead() {
        let now = "2030-01-01T00:00:00Z".parse().unwrap();
        let kept = still_running(
            vec![
                schedule("past", Some("2029-12-31T23:59:59Z")),
                schedule("ending-now", Some("2030-01-01T00:00:00Z")),
                schedule("ahead", Some("2030-01-01T00:00:01Z")),
                schedule("open", None),
            ],
            now,
        );
        assert_eq!(
            kept.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["ahead"]
        );
    }

    #[tokio::test]
    async fn expired_schedules_are_read_by_id_and_listed_nowhere_else() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let expired = uuid::Uuid::new_v4().to_string();
        let running = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO explore.schedules_local (id, name, active_since, active_until) VALUES \
             ($1, 'Expired', timezone('UTC', now()) - interval '2 days', timezone('UTC', now()) - interval '1 day'), \
             ($2, 'Running', timezone('UTC', now()) - interval '1 day', timezone('UTC', now()) + interval '1 day')",
        )
        .bind(&expired)
        .bind(&running)
        .execute(&pool)
        .await
        .unwrap();
        let schedules = SchedulesComponent::new(pool);
        let listed: Vec<String> = schedules
            .list()
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert!(listed.contains(&running));
        assert!(!listed.contains(&expired));
        assert_eq!(
            schedules.get(&expired).await.unwrap().unwrap().name,
            "Expired"
        );
        let all: Vec<String> = schedules
            .all()
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert!(all.contains(&expired) && all.contains(&running));
    }
}
