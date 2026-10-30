//! Native equivalent of EVENT_ENDED's community metric effects. Facts,
//! contributions and the bounded scan cursor commit together; an occurrence
//! can contribute only once. This does not calculate partial ranking scores.

use chrono::NaiveDateTime;
use sqlx::PgPool;

const PAGE: i64 = 100;

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
    let mut tx = pool.begin().await?;
    let initialized = sqlx::query(
        "INSERT INTO social.event_ended_cursor (due_at) VALUES ($1) ON CONFLICT DO NOTHING",
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
    let (since, after_id, after_start): (NaiveDateTime, String, NaiveDateTime) = sqlx::query_as(
        "SELECT due_at,event_id,occurrence_start FROM social.event_ended_cursor WHERE singleton FOR UPDATE",
    ).fetch_one(&mut *tx).await?;
    if now < since {
        tx.commit().await?;
        return Ok(0);
    }
    let rows: Vec<Ended> = sqlx::query_as(
        "SELECT DISTINCT e.id AS event_id,d.starts_at, \
            d.starts_at + e.duration_ms * interval '1 millisecond' AS ends_at,c.id AS community_id, \
            (SELECT count(*) FROM explore.event_attendance_local a WHERE a.event_id=e.id AND a.action='going') AS attendees \
         FROM explore.event e CROSS JOIN LATERAL ( \
            SELECT CASE WHEN pg_input_is_valid(value,'timestamp with time zone') \
                THEN value::timestamptz AT TIME ZONE 'UTC' END AS starts_at \
            FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(e.raw->'recurrent_dates')='array' \
                AND e.raw->'recurrent_dates'<>'[]'::jsonb THEN e.raw->'recurrent_dates' \
                ELSE jsonb_build_array(e.raw->'start_at') END) \
         ) d LEFT JOIN social.communities c ON c.id::text=e.community_id AND c.origin='local' AND c.active AND NOT c.suspended \
         WHERE e.origin='local' AND e.approved AND e.duration_ms >= 0 \
            AND COALESCE(e.raw->>'rejected','false')<>'true' \
            AND COALESCE(e.raw->>'deleted_by_user','false')<>'true' \
            AND COALESCE(e.raw->>'deleted_by_admin','false')<>'true' \
            AND (d.starts_at + e.duration_ms * interval '1 millisecond',e.id,d.starts_at)>($1,$2,$3) \
            AND d.starts_at + e.duration_ms * interval '1 millisecond'<=$4 \
         ORDER BY ends_at,event_id,starts_at LIMIT $5",
    ).bind(since).bind(&after_id).bind(after_start).bind(now).bind(PAGE).fetch_all(&mut *tx).await?;
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
    if let Some(last) = rows.last() {
        sqlx::query("UPDATE social.event_ended_cursor SET due_at=$1,event_id=$2,occurrence_start=$3 WHERE singleton")
            .bind(last.ends_at).bind(&last.event_id).bind(last.starts_at).execute(&mut *tx).await?;
    }
    if rows.len() < PAGE as usize && rows.last().is_none_or(|last| last.ends_at < now) {
        sqlx::query("UPDATE social.event_ended_cursor SET due_at=$1,event_id='',occurrence_start='0001-01-01' WHERE singleton")
            .bind(now).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(inserted as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use serde_json::json;
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
        sqlx::query("INSERT INTO explore.event (id,name,approved,origin,community_id,duration_ms,raw) VALUES ($1,'Ending event',true,$2,$3,3600000,$4)")
            .bind(id).bind(origin).bind(community.to_string()).bind(json!({"start_at":dates[0],"recurrent_dates":dates,"latest_attendees":["0xpreview"],"total_attendees":100}))
            .execute(pool).await.unwrap();
    }

    #[tokio::test]
    async fn native_endings_commit_once_with_local_attendance_and_recover_after_failed_projection()
    {
        let _guard = TEST_LOCK.lock().await;
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        sqlx::query("TRUNCATE social.event_ended_occurrences,social.event_ended_cursor,social.community_event_metrics").execute(&pool).await.unwrap();
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
        assert_eq!(facts, 0, "fact, cursor and projection roll back together");
        sqlx::query(
            "ALTER TABLE social.community_event_metrics DROP CONSTRAINT simulate_failed_projection",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(run_once(&pool, ended).await.unwrap(), 1);
        assert_eq!(run_once(&pool, ended).await.unwrap(), 0);
        // Even a cursor replay cannot contribute an occurrence twice.
        sqlx::query("UPDATE social.event_ended_cursor SET due_at=$1,event_id='',occurrence_start='0001-01-01'").bind(now).execute(&pool).await.unwrap();
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
    async fn equal_time_endings_page_and_concurrent_scans_deduplicate() {
        let _guard = TEST_LOCK.lock().await;
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        sqlx::query("TRUNCATE social.event_ended_occurrences,social.event_ended_cursor,social.community_event_metrics").execute(&pool).await.unwrap();
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
