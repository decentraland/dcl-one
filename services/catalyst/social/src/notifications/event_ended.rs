//! Native equivalent of EVENT_ENDED's community metric effects. Facts and
//! contributions commit together. An activation cutoff excludes historical
//! imports; each occurrence contributes once, including after schedule edits.
//! This does not calculate partial ranking scores.

use chrono::NaiveDateTime;
use sqlx::PgPool;

const PAGE: i64 = 100;

const ENDED_SQL: &str = "WITH candidates AS MATERIALIZED ( \
         SELECT DISTINCT e.id AS event_id,d.starts_at, \
            d.starts_at + e.duration_ms * interval '1 millisecond' AS ends_at,e.community_id \
         FROM (SELECT * FROM explore.event WHERE approved AND finish_at>$1) e \
         CROSS JOIN LATERAL ( \
            SELECT CASE WHEN pg_input_is_valid(value,'timestamp with time zone') \
                THEN value::timestamptz AT TIME ZONE 'UTC' END AS starts_at \
            FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(e.raw->'recurrent_dates')='array' \
                AND e.raw->'recurrent_dates'<>'[]'::jsonb THEN e.raw->'recurrent_dates' \
                ELSE jsonb_build_array(e.raw->'start_at') END) \
         ) d \
         WHERE e.origin='local' AND e.duration_ms >= 0 \
            AND COALESCE(e.raw->>'rejected','false')<>'true' \
            AND COALESCE(e.raw->>'deleted_by_user','false')<>'true' \
            AND COALESCE(e.raw->>'deleted_by_admin','false')<>'true' \
            AND d.starts_at + e.duration_ms * interval '1 millisecond'>$1 \
            AND d.starts_at + e.duration_ms * interval '1 millisecond'<=$2 \
            AND d.starts_at >= CASE WHEN pg_input_is_valid(e.raw->>'_metrics_schedule_after','timestamp with time zone') \
                THEN (e.raw->>'_metrics_schedule_after')::timestamptz AT TIME ZONE 'UTC' ELSE '-infinity'::timestamp END \
            AND NOT EXISTS (SELECT 1 FROM social.event_ended_occurrences done \
                WHERE done.event_id=e.id AND done.occurrence_start=d.starts_at) \
         ORDER BY ends_at,event_id,starts_at LIMIT $3) \
         SELECT p.event_id,p.starts_at,p.ends_at,c.id AS community_id, \
            (SELECT count(*) FROM explore.event_attendance_local a WHERE a.event_id=p.event_id AND a.action='going') AS attendees \
         FROM candidates p LEFT JOIN social.communities c ON \
            c.id=CASE WHEN pg_input_is_valid(p.community_id,'uuid') THEN p.community_id::uuid END \
            AND c.origin='local' AND c.active AND NOT c.suspended \
         ORDER BY p.ends_at,p.event_id,p.starts_at";

#[cfg(test)]
pub(super) static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(sqlx::FromRow)]
struct Ended {
    event_id: String,
    starts_at: NaiveDateTime,
    ends_at: NaiveDateTime,
    community_id: Option<uuid::Uuid>,
    attendees: i64,
}

pub(super) async fn run_once(pool: &PgPool, now: NaiveDateTime) -> anyhow::Result<u64> {
    let mut tx = super::events::timed_transaction(pool).await?;
    sqlx::query("SET LOCAL statement_timeout = '2s'")
        .execute(&mut *tx)
        .await?;
    let initialized = sqlx::query(
        "INSERT INTO social.event_ended_activation (activated_at) VALUES ($1) ON CONFLICT DO NOTHING",
    )
    .bind(now)
    .execute(&mut *tx)
    .await?
    .rows_affected()
        > 0;
    if initialized {
        tx.commit().await?;
        return Ok(0);
    }
    let activated_at: NaiveDateTime = sqlx::query_scalar(
        "SELECT activated_at FROM social.event_ended_activation WHERE singleton FOR UPDATE",
    )
    .fetch_one(&mut *tx)
    .await?;
    // Recover the recent outage window using the existing approved finish index.
    // Attendance is read only for the bounded set of unrecorded occurrences.
    let since = activated_at.max(now - chrono::Duration::days(30));
    let rows: Vec<Ended> = sqlx::query_as(ENDED_SQL)
        .bind(since)
        .bind(now)
        .bind(PAGE)
        .fetch_all(&mut *tx)
        .await?;
    let event_ids: Vec<&str> = rows.iter().map(|row| row.event_id.as_str()).collect();
    let starts: Vec<NaiveDateTime> = rows.iter().map(|row| row.starts_at).collect();
    let ends: Vec<NaiveDateTime> = rows.iter().map(|row| row.ends_at).collect();
    let communities: Vec<Option<uuid::Uuid>> = rows.iter().map(|row| row.community_id).collect();
    let attendees: Vec<i64> = rows.iter().map(|row| row.attendees).collect();
    let inserted = sqlx::query(
        "WITH added AS ( \
            INSERT INTO social.event_ended_occurrences (event_id,occurrence_start,ended_at,community_id,attendees) \
            SELECT * FROM UNNEST($1::text[],$2::timestamp[],$3::timestamp[],$4::uuid[],$5::bigint[]) \
            ON CONFLICT DO NOTHING RETURNING community_id,attendees \
         ), contribution AS ( \
            INSERT INTO social.community_event_metrics (community_id,events_count,events_total_attendees) \
            SELECT community_id,count(*),sum(attendees)::bigint FROM added WHERE community_id IS NOT NULL GROUP BY community_id \
            ON CONFLICT (community_id) DO UPDATE SET \
                events_count=social.community_event_metrics.events_count+EXCLUDED.events_count, \
                events_total_attendees=social.community_event_metrics.events_total_attendees+EXCLUDED.events_total_attendees \
         ) SELECT count(*) FROM added",
    ).bind(event_ids).bind(starts).bind(ends).bind(communities).bind(attendees)
        .fetch_one(&mut *tx).await?;
    use sqlx::Row;
    let inserted: i64 = inserted.get(0);
    tx.commit().await?;
    Ok(inserted as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use serde_json::{Value, json};
    use uuid::Uuid;

    async fn event(
        pool: &PgPool,
        id: &str,
        community: Uuid,
        starts: &[NaiveDateTime],
        origin: &str,
    ) {
        let dates: Vec<String> = starts
            .iter()
            .map(|date| date.and_utc().to_rfc3339())
            .collect();
        sqlx::query("INSERT INTO explore.event (id,name,approved,origin,community_id,duration_ms,raw,finish_at) VALUES ($1,'Ending event',true,$2,$3,3600000,$4,$5)")
            .bind(id).bind(origin).bind(community.to_string().to_uppercase()).bind(json!({"start_at":dates[0],"recurrent_dates":dates,"latest_attendees":["0xpreview"],"total_attendees":100})).bind(starts.iter().max().copied().unwrap()+Duration::hours(1))
            .execute(pool).await.unwrap();
    }

    #[tokio::test]
    async fn native_endings_commit_once_with_local_attendance_and_recover_after_failed_projection()
    {
        let _guard = TEST_LOCK.lock().await;
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        sqlx::query("TRUNCATE social.event_ended_occurrences,social.event_ended_activation,social.community_event_metrics").execute(&pool).await.unwrap();
        let now = chrono::DateTime::parse_from_rfc3339("2099-06-01T18:00:00Z")
            .unwrap()
            .naive_utc();
        let community = Uuid::new_v4();
        sqlx::query("INSERT INTO social.communities (id,name,owner_address) VALUES ($1,'Ending community','0xhost')")
            .bind(community).execute(&pool).await.unwrap();
        event(
            &pool,
            "past",
            community,
            &[now - Duration::hours(2)],
            "local",
        )
        .await;
        assert_eq!(
            run_once(&pool, now).await.unwrap(),
            0,
            "first boot never replays historical endings"
        );
        event(
            &pool,
            "series",
            community,
            &[now, now + Duration::days(1)],
            "local",
        )
        .await;
        event(&pool, "optional-mirror", community, &[now], "upstream").await;
        sqlx::query("INSERT INTO explore.event_attendance_local (event_id,signer,action) VALUES ('series','0xgoing','going'),('series','0xcancelled','cancel')")
            .execute(&pool).await.unwrap();
        sqlx::query("ALTER TABLE social.community_event_metrics ADD CONSTRAINT simulate_failed_projection CHECK (events_count=0)")
            .execute(&pool).await.unwrap();
        let ended = now + Duration::hours(1);
        assert!(run_once(&pool, ended).await.is_err());
        let facts: i64 = sqlx::query_scalar("SELECT count(*) FROM social.event_ended_occurrences")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(facts, 0, "fact and projection roll back together");
        sqlx::query(
            "ALTER TABLE social.community_event_metrics DROP CONSTRAINT simulate_failed_projection",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(run_once(&pool, ended).await.unwrap(), 1);
        assert_eq!(run_once(&pool, ended).await.unwrap(), 0);
        // Repeated scans cannot contribute an occurrence twice.
        assert_eq!(run_once(&pool, ended).await.unwrap(), 0);
        sqlx::query(
            "UPDATE explore.event_attendance_local SET action='cancel' WHERE signer='0xgoing'",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(run_once(&pool, ended + Duration::days(1)).await.unwrap(), 1);
        let metrics:(i64,i64)=sqlx::query_as("SELECT events_count,events_total_attendees FROM social.community_event_metrics WHERE community_id=$1")
            .bind(community).fetch_one(&pool).await.unwrap();
        assert_eq!(
            metrics,
            (2, 1),
            "each occurrence uses current explicit local RSVPs, never previews"
        );
    }

    #[tokio::test]
    async fn recurring_duration_edits_reconcile_earlier_endings_after_prior_scans() {
        use crate::explore::events::{moderation, writes};
        use axum::extract::{OriginalUri, Path, State};
        use axum::http::{HeaderMap, Method};
        let _guard = TEST_LOCK.lock().await;
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        sqlx::query("TRUNCATE social.event_ended_occurrences,social.event_ended_activation,social.community_event_metrics").execute(&pool).await.unwrap();
        let now = chrono::DateTime::parse_from_rfc3339("2099-09-01T18:00:00Z")
            .unwrap()
            .naive_utc();
        let owner = "0x1111111111111111111111111111111111111111";
        let community = Uuid::new_v4();
        sqlx::query("INSERT INTO social.communities(id,name,owner_address) VALUES($1,'Mutable schedule',$2)")
            .bind(community).bind(owner).execute(&pool).await.unwrap();
        let body: moderation::CreateEvent = serde_json::from_value(json!({
            "name":"Mutable occurrence", "start_at":now.and_utc().to_rfc3339(),
            "duration":1800000,"x":0,"y":0,"community_id":community.to_string(),
            "recurrent":true,"recurrent_frequency":"DAILY","recurrent_count":2
        }))
        .unwrap();
        let mut settings = catalyst_explore::Settings::for_tests();
        settings.events_admin_token = Some("metric-fixture".into());
        settings.operator_writes_enabled = true;
        let event_id = moderation::create_as(&pool, &body, owner, owner, &settings)
            .await
            .unwrap();
        moderation::moderate(
            &pool,
            &event_id,
            &moderation::AdminPatch {
                approved: Some(true),
                ..Default::default()
            },
            "operator",
            &settings,
        )
        .await
        .unwrap();
        crate::explore::events::attendees::rsvp_going(&pool, &event_id, owner, None)
            .await
            .unwrap();
        assert_eq!(run_once(&pool, now).await.unwrap(), 0);
        assert_eq!(
            run_once(&pool, now + Duration::minutes(10)).await.unwrap(),
            0
        );
        let state = std::sync::Arc::new(catalyst_explore::ExploreState::new(
            pool.clone(),
            settings,
            None,
        ));
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer metric-fixture".parse().unwrap());
        let axum::Json(updated) = writes::patch_event(
            State(state),
            Method::PATCH,
            OriginalUri(format!("/api/events/{event_id}").parse().unwrap()),
            headers,
            Path(event_id.clone()),
            serde_json::to_vec(&json!({"duration":300000}))
                .unwrap()
                .into(),
        )
        .await
        .unwrap();
        assert_eq!(updated.data.duration, Some(300000));
        assert_eq!(
            run_once(&pool, now + Duration::minutes(11)).await.unwrap(),
            1
        );
        assert_eq!(
            run_once(&pool, now + Duration::minutes(12)).await.unwrap(),
            0
        );
        let metric:(i64,i64) = sqlx::query_as("SELECT events_count,events_total_attendees FROM social.community_event_metrics WHERE community_id=$1").bind(community).fetch_one(&pool).await.unwrap();
        assert_eq!(metric, (1, 1));
        let ended: NaiveDateTime = sqlx::query_scalar(
            "SELECT ended_at FROM social.event_ended_occurrences WHERE event_id=$1",
        )
        .bind(event_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(ended, now + Duration::minutes(5));
    }

    #[tokio::test]
    async fn rescheduling_recorded_series_never_recounts_new_historical_starts() {
        use crate::explore::events::{moderation, writes};
        use axum::extract::{OriginalUri, Path, State};
        use axum::http::{HeaderMap, Method};
        let _guard = TEST_LOCK.lock().await;
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        sqlx::query("TRUNCATE social.event_ended_occurrences,social.event_ended_activation,social.community_event_metrics").execute(&pool).await.unwrap();
        let now = chrono::Utc::now().naive_utc();
        assert_eq!(run_once(&pool, now - Duration::days(6)).await.unwrap(), 0);
        for minutes in [30, -30] {
            let owner = "0x1111111111111111111111111111111111111111";
            let community = Uuid::new_v4();
            sqlx::query("INSERT INTO social.communities(id,name,owner_address) VALUES($1,'Rescheduled series',$2)")
                .bind(community).bind(owner).execute(&pool).await.unwrap();
            let start = now - Duration::days(5) - Duration::hours(2);
            let body: moderation::CreateEvent = serde_json::from_value(json!({
                "name":"Rescheduled series", "start_at":start.and_utc().to_rfc3339(),
                "duration":3600000,"x":0,"y":0,"community_id":community.to_string(),
                "recurrent":true,"recurrent_frequency":"DAILY","recurrent_count":12
            }))
            .unwrap();
            let mut settings = catalyst_explore::Settings::for_tests();
            settings.events_admin_token = Some("reschedule-fixture".into());
            settings.operator_writes_enabled = true;
            let event_id = moderation::create_as(&pool, &body, owner, owner, &settings)
                .await
                .unwrap();
            moderation::moderate(
                &pool,
                &event_id,
                &moderation::AdminPatch {
                    approved: Some(true),
                    ..Default::default()
                },
                "operator",
                &settings,
            )
            .await
            .unwrap();
            crate::explore::events::attendees::rsvp_going(&pool, &event_id, owner, None)
                .await
                .unwrap();
            assert_eq!(run_once(&pool, now).await.unwrap(), 6);
            let state = std::sync::Arc::new(catalyst_explore::ExploreState::new(
                pool.clone(),
                settings,
                None,
            ));
            let mut headers = HeaderMap::new();
            headers.insert(
                "authorization",
                "Bearer reschedule-fixture".parse().unwrap(),
            );
            let axum::Json(updated) = writes::patch_event(
                State(state),
                Method::PATCH,
                OriginalUri(format!("/api/events/{event_id}").parse().unwrap()),
                headers,
                Path(event_id.clone()),
                serde_json::to_vec(
                    &json!({"start_at":(start+Duration::minutes(minutes)).and_utc().to_rfc3339()}),
                )
                .unwrap()
                .into(),
            )
            .await
            .unwrap();
            assert!(
                !serde_json::to_value(updated)
                    .unwrap()
                    .to_string()
                    .contains("_metrics_schedule_after")
            );
            assert_eq!(
                run_once(&pool, now + Duration::minutes(1)).await.unwrap(),
                0
            );
            assert_eq!(run_once(&pool, now + Duration::days(1)).await.unwrap(), 1);
            assert_eq!(run_once(&pool, now + Duration::days(1)).await.unwrap(), 0);
            let metrics:(i64,i64)=sqlx::query_as("SELECT events_count,events_total_attendees FROM social.community_event_metrics WHERE community_id=$1")
                .bind(community).fetch_one(&pool).await.unwrap();
            assert_eq!(metrics, (7, 7));
            let leaked:i64=sqlx::query_scalar("SELECT count(*) FROM social.event_notification_intents WHERE event_id=$1 AND event ? '_metrics_schedule_after'")
                .bind(&event_id).fetch_one(&pool).await.unwrap();
            assert_eq!(leaked, 0);
            // Keep the second loop's recovery clock independent of this future scan.
            sqlx::query("DELETE FROM explore.event WHERE id=$1")
                .bind(&event_id)
                .execute(&pool)
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    async fn metric_query_prefilters_expired_series_and_counts_attendance_only_after_page_limit() {
        let _guard = TEST_LOCK.lock().await;
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        let prefix = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().naive_utc();
        let since = now - Duration::days(30);
        sqlx::query("INSERT INTO explore.event(id,name,approved,origin,finish_at,duration_ms,raw) \
            SELECT $1||'-old-'||n,'Expired series',true,'local',$2-interval '60 days',60000, \
            jsonb_build_object('start_at',$2-interval '60 days','recurrent_dates', \
                (SELECT jsonb_agg(to_char($2-interval '60 days','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')) FROM generate_series(1,100))) \
            FROM generate_series(1,10000) n")
            .bind(&prefix).bind(now).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO explore.event(id,name,approved,origin,finish_at,duration_ms,raw) \
            SELECT $1||'-current-'||n,'Current ending',true,'local',$2-interval '59 minutes',60000, \
            jsonb_build_object('start_at',to_char($2-interval '1 hour','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')) FROM generate_series(1,120) n")
            .bind(&prefix).bind(now).execute(&pool).await.unwrap();
        sqlx::query("ANALYZE explore.event")
            .execute(&pool)
            .await
            .unwrap();
        // The only interpolated SQL is the static audited production statement.
        let plan: serde_json::Value = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {ENDED_SQL}"
        )))
        .bind(since)
        .bind(now)
        .bind(PAGE)
        .fetch_one(&pool)
        .await
        .unwrap();
        fn inspect(
            value: &serde_json::Value,
            attendee_loops: &mut Vec<u64>,
            indexes: &mut Vec<String>,
        ) {
            if let Some(object) = value.as_object() {
                if object.get("Relation Name").and_then(Value::as_str)
                    == Some("event_attendance_local")
                {
                    attendee_loops
                        .push(object.get("Actual Loops").and_then(Value::as_u64).unwrap());
                }
                if let Some(index) = object.get("Index Name").and_then(Value::as_str) {
                    indexes.push(index.to_string());
                }
                for value in object.values() {
                    inspect(value, attendee_loops, indexes);
                }
            } else if let Some(array) = value.as_array() {
                for value in array {
                    inspect(value, attendee_loops, indexes);
                }
            }
        }
        let mut attendee_loops = Vec::new();
        let mut indexes = Vec::new();
        inspect(&plan, &mut attendee_loops, &mut indexes);
        sqlx::query("DELETE FROM explore.event WHERE id LIKE $1")
            .bind(format!("{prefix}-%"))
            .execute(&pool)
            .await
            .unwrap();
        assert!(!attendee_loops.is_empty());
        assert!(attendee_loops.iter().all(|loops| *loops <= PAGE as u64));
        // Choice of index versus sequential scan is PostgreSQL's cost decision;
        // print the measured plan rather than pinning a platform-specific choice.
        println!("metric query smoke measurement: {plan}");
    }

    #[tokio::test]
    async fn equal_time_endings_page_and_concurrent_scans_deduplicate() {
        let _guard = TEST_LOCK.lock().await;
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        sqlx::query("TRUNCATE social.event_ended_occurrences,social.event_ended_activation,social.community_event_metrics").execute(&pool).await.unwrap();
        let now = chrono::DateTime::parse_from_rfc3339("2099-07-01T18:00:00Z")
            .unwrap()
            .naive_utc();
        let community = Uuid::new_v4();
        run_once(&pool, now).await.unwrap();
        for id in 0..101 {
            event(
                &pool,
                &format!("ending-{id:03}"),
                community,
                &[now],
                "local",
            )
            .await;
        }
        let end = now + Duration::hours(1);
        let (one, two) = tokio::join!(run_once(&pool, end), run_once(&pool, end));
        assert_eq!(one.unwrap() + two.unwrap(), 101);
        assert_eq!(run_once(&pool, end).await.unwrap(), 0);
        let facts: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM social.event_ended_occurrences WHERE community_id IS NULL",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            facts, 101,
            "unknown communities cannot authorize metric attribution"
        );
    }
}
