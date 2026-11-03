//! Ranking contracts from social-service-ea b6b2cef, logic/community/ranking.ts,
//! communities-db.ts and its three SQS ranking handlers. Local lifecycle events
//! use durable receipts; external delivery retains the source's at-least-once
//! contribution semantics. Mirrored rankings are never recalculated here.
use std::{sync::Arc, time::Duration};

use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

#[derive(Debug, Default, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub events_count: i64,
    pub photos_count: i64,
    pub streams_count: i64,
    pub places_count: i64,
    pub posts_count: i64,
    pub new_members_count: i64,
    pub events_total_attendees: i64,
    pub streams_total_participants: i64,
    pub has_thumbnail: i64,
    pub has_description: i64,
    pub age_in_days: i32,
}

pub fn score(metrics: &Metrics) -> f64 {
    let values = [
        (metrics.has_thumbnail, 1.0_f64, false, 0.15),
        (metrics.has_description, 1.0, false, 0.1),
        (metrics.new_members_count, 50.0, true, 0.2),
        (metrics.events_count, 50.0, true, 0.15),
        (metrics.events_total_attendees, 1000.0, true, 0.1),
        (metrics.posts_count, 100.0, true, 0.1),
        (metrics.places_count, 10.0, false, 0.08),
        (metrics.streams_count, 20.0, true, 0.07),
        (metrics.streams_total_participants, 500.0, true, 0.04),
        (metrics.photos_count, 100.0, true, 0.01),
    ];
    let base: f64 = values
        .into_iter()
        .map(|(value, max, logarithmic, weight)| {
            let value = value.max(0) as f64;
            let normalized = if logarithmic {
                (1.0 + value).ln() / (1.0 + max).ln()
            } else {
                value / max
            };
            normalized.min(1.0) * weight
        })
        .sum();
    let boost = if metrics.age_in_days >= 30 {
        1.0
    } else {
        let by_age = if metrics.age_in_days <= 7 {
            1.15
        } else {
            1.05 + f64::from(30 - metrics.age_in_days) / 23.0 * (1.15 - 1.05)
        };
        (by_age - base * 0.3).max(1.0)
    };
    (base * boost).clamp(0.0, 1.0)
}

fn contribution(value: Option<f64>, maximum: i64) -> i64 {
    value
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| value.trunc().min(maximum as f64) as i64)
        .unwrap_or(0)
}

const UPDATE: &str = "UPDATE social.communities SET
    ranking_events_count=LEAST(ranking_events_count::bigint+$2,2147483647)::int4,
    ranking_photos_count=LEAST(ranking_photos_count::bigint+$3,2147483647)::int4,
    ranking_streams_count=LEAST(ranking_streams_count::bigint+$4,2147483647)::int4,
    ranking_events_total_attendees=LEAST(ranking_events_total_attendees::bigint+$5,2147483647)::int4,
    ranking_streams_total_participants=LEAST(ranking_streams_total_participants::bigint+$6,2147483647)::int4
    WHERE id=$1";

async fn add(conn: &mut PgConnection, community: Uuid, delta: [i64; 5]) -> sqlx::Result<()> {
    let changed = sqlx::query(UPDATE)
        .bind(community)
        .bind(delta[0])
        .bind(delta[1])
        .bind(delta[2])
        .bind(delta[3])
        .bind(delta[4])
        .execute(conn)
        .await?
        .rows_affected();
    if changed == 0 {
        return Err(sqlx::Error::RowNotFound);
    }
    Ok(())
}

async fn local_delta(
    conn: &mut PgConnection,
    community: Uuid,
    kind: &str,
    key: &str,
    occurred_at: NaiveDateTime,
    delta: [i64; 5],
) -> sqlx::Result<()> {
    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO social.community_ranking_events(kind,event_key,community_id,occurred_at)
         SELECT $1,$2,id,$4 FROM social.communities WHERE id=$3 AND origin='local'
         ON CONFLICT DO NOTHING RETURNING community_id",
    )
    .bind(kind)
    .bind(key)
    .bind(community)
    .bind(occurred_at)
    .fetch_optional(&mut *conn)
    .await?;
    if inserted.is_some() {
        add(conn, community, delta).await?;
    }
    Ok(())
}

/// Runs in the photo finalizer's transaction. All communities attached to the
/// place receive one contribution, including inactive communities as upstream.
pub async fn record_photo(
    conn: &mut PgConnection,
    key: &str,
    place: &str,
    occurred_at: NaiveDateTime,
) -> sqlx::Result<()> {
    sqlx::query("WITH receipts AS (
        INSERT INTO social.community_ranking_events(kind,event_key,community_id,occurred_at)
        SELECT 'photo',$1,p.community_id,$3 FROM social.community_places p
        JOIN social.communities c ON c.id=p.community_id AND c.origin='local' WHERE p.place_id=$2
        ON CONFLICT DO NOTHING RETURNING community_id)
        UPDATE social.communities c SET ranking_photos_count=LEAST(c.ranking_photos_count::bigint+1,2147483647)::int4
        FROM receipts r WHERE c.id=r.community_id")
        .bind(key).bind(place).bind(occurred_at).execute(conn).await?;
    Ok(())
}

pub(crate) async fn record_stream(
    conn: &mut PgConnection,
    community: Uuid,
    key: &str,
    event: &Value,
    occurred_at: NaiveDateTime,
) -> sqlx::Result<()> {
    let participants = event["metadata"]["totalParticipants"]
        .as_f64()
        .unwrap_or(0.0);
    if participants > 1.0 {
        local_delta(
            conn,
            community,
            "stream",
            key,
            occurred_at,
            [0, 0, 1, 0, contribution(Some(participants), 500)],
        )
        .await?;
    }
    Ok(())
}

/// Called by the shared event ingress after it has decoded the SNS/SQS event.
/// Source event/stream database failures are returned; photo lookup and each
/// photo counter failure are logged and acknowledged, as the source handler does.
/// The ingress owns acknowledgement (the pinned queue consumer deletes messages
/// even after a handler error); a returned error does not promise queue retries.
pub async fn apply_external_event(pool: &PgPool, event: &Value) -> sqlx::Result<bool> {
    let metadata = &event["metadata"];
    let (kind, subtype) = (event["type"].as_str(), event["subType"].as_str());
    if (kind, subtype) == (Some("camera"), Some("photo-taken")) {
        let Some(place) = metadata["placeId"].as_str().filter(|s| !s.is_empty()) else {
            return Ok(true);
        };
        let communities: Vec<Uuid> = match sqlx::query_scalar(
            "SELECT DISTINCT community_id FROM social.community_places WHERE place_id=$1",
        )
        .bind(place)
        .fetch_all(pool)
        .await
        {
            Ok(ids) => ids,
            Err(error) => {
                tracing::error!(%error, "photo ranking lookup failed");
                return Ok(true);
            }
        };
        for community in communities {
            let result = async {
                let mut tx = pool.begin().await?;
                add(&mut tx, community, [0, 1, 0, 0, 0]).await?;
                tx.commit().await
            }
            .await;
            if let Err(error) = result {
                tracing::error!(%error, %community, "photo ranking update failed");
            }
        }
        return Ok(true);
    }
    let delta = match (kind, subtype) {
        (Some("event"), Some("event-ended")) => [
            1,
            0,
            0,
            contribution(metadata["totalAttendees"].as_f64(), 1000),
            0,
        ],
        (Some("streaming"), Some("community-streaming-ended")) => {
            let participants = metadata["totalParticipants"].as_f64().unwrap_or(0.0);
            if participants <= 1.0 {
                return Ok(true);
            }
            [0, 0, 1, 0, contribution(Some(participants), 500)]
        }
        _ => return Ok(false),
    };
    let Some(community) = metadata["communityId"].as_str().filter(|s| !s.is_empty()) else {
        return Ok(true);
    };
    let community = Uuid::parse_str(community)
        .map_err(|_| sqlx::Error::Protocol("invalid community UUID".into()))?;
    let mut tx = pool.begin().await?;
    add(&mut tx, community, delta).await?;
    tx.commit().await?;
    Ok(true)
}

const METRICS_SQL: &str = "SELECT c.ranking_events_count::bigint AS events_count,
    c.ranking_photos_count::bigint AS photos_count,c.ranking_streams_count::bigint AS streams_count,
    (SELECT count(*) FROM social.community_places p WHERE p.community_id=c.id) AS places_count,
    (SELECT count(*) FROM social.community_posts p WHERE p.community_id=c.id) AS posts_count,
    (SELECT count(*) FROM social.community_members m WHERE m.community_id=c.id AND m.joined_at >= $2 - interval '7 days') AS new_members_count,
    c.ranking_events_total_attendees::bigint AS events_total_attendees,
    c.ranking_streams_total_participants::bigint AS streams_total_participants,
    (CASE WHEN EXISTS (SELECT 1 FROM social.community_thumbnails t WHERE t.community_id=c.id) THEN 1 ELSE 0 END)::int8 AS has_thumbnail,
    (CASE WHEN trim(c.description)<>'' THEN 1 ELSE 0 END)::int8 AS has_description,($2::date-c.created_at::date) AS age_in_days
    FROM social.communities c WHERE c.id=ANY($1)";

#[derive(sqlx::FromRow)]
struct RankedMetrics {
    id: Uuid,
    #[sqlx(flatten)]
    metrics: Metrics,
}

pub async fn recalculate(pool: &PgPool, now: NaiveDateTime) -> anyhow::Result<u64> {
    let mut after = Uuid::nil();
    let mut total = 0;
    loop {
        let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM social.communities WHERE origin='local' AND active AND id>$1 ORDER BY id LIMIT 100")
            .bind(after).fetch_all(pool).await?;
        if ids.is_empty() {
            return Ok(total);
        }
        recalculate_ids(pool, &ids, now).await?;
        total += ids.len() as u64;
        after = *ids.last().unwrap();
    }
}

pub(crate) async fn recalculate_ids(
    pool: &PgPool,
    ids: &[Uuid],
    now: NaiveDateTime,
) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL statement_timeout='2s'")
        .execute(&mut *tx)
        .await?;
    let query = METRICS_SQL.replacen("SELECT ", "SELECT c.id,", 1);
    let rows: Vec<RankedMetrics> = sqlx::query_as(sqlx::AssertSqlSafe(query))
        .bind(ids)
        .bind(now)
        .fetch_all(&mut *tx)
        .await?;
    let updates: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let scores: Vec<f32> = rows.iter().map(|row| score(&row.metrics) as f32).collect();
    sqlx::query("UPDATE social.communities c SET ranking_score=v.score,updated_at=$3
        FROM UNNEST($1::uuid[],$2::real[]) v(id,score) WHERE c.id=v.id AND c.origin='local' AND c.active")
        .bind(updates).bind(scores).bind(now).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

/// The events producer counts event-attendee rows (events 6a5f33a1,
/// Event/model.ts#getEventsEndingInRangeWithAttendeeCount). Local going rows are
/// authoritative here; a mirrored attendee preview never enters this count.
pub async fn scan_ended(pool: &PgPool, now: NaiveDateTime) -> anyhow::Result<u64> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL statement_timeout='2s'")
        .execute(&mut *tx)
        .await?;
    let rows: Vec<(Uuid,String,NaiveDateTime,i64)> = sqlx::query_as("SELECT c.id,e.id||':'||d.starts_at::text,
        d.starts_at+e.duration_ms*interval '1 millisecond' AS ends_at,
        (SELECT count(*) FROM explore.event_attendance_local a WHERE a.event_id=e.id AND a.action='going')
        FROM explore.event e JOIN social.communities c ON c.id=CASE WHEN pg_input_is_valid(e.community_id,'uuid') THEN e.community_id::uuid END
        CROSS JOIN LATERAL (SELECT CASE WHEN pg_input_is_valid(value,'timestamp with time zone')
            THEN value::timestamptz AT TIME ZONE 'UTC' END AS starts_at
            FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(e.raw->'recurrent_dates')='array'
                AND e.raw->'recurrent_dates'<>'[]'::jsonb THEN e.raw->'recurrent_dates' ELSE jsonb_build_array(e.raw->'start_at') END)) d
        WHERE e.origin='local' AND c.origin='local' AND e.approved AND e.duration_ms>=0
        AND e.finish_at>=c.ranking_tracking_since AND e.start_at<=$1
        AND COALESCE(e.raw->>'rejected','false')<>'true'
        AND COALESCE(e.raw->>'deleted_by_user','false')<>'true' AND COALESCE(e.raw->>'deleted_by_admin','false')<>'true'
        AND d.starts_at+e.duration_ms*interval '1 millisecond'>=c.ranking_tracking_since
        AND d.starts_at+e.duration_ms*interval '1 millisecond'<$1
        AND NOT EXISTS(SELECT 1 FROM social.community_ranking_events r WHERE r.kind='event'
            AND r.event_key=e.id||':'||d.starts_at::text AND r.community_id=c.id)
        ORDER BY ends_at,e.id,d.starts_at LIMIT 100")
        .bind(now).fetch_all(&mut *tx).await?;
    for (community, key, end, attendees) in &rows {
        local_delta(
            &mut tx,
            *community,
            "event",
            key,
            *end,
            [1, 0, 0, (*attendees).min(1000), 0],
        )
        .await?;
    }
    tx.commit().await?;
    Ok(rows.len() as u64)
}

pub fn spawn_jobs(state: &Arc<super::State>, jobs: &crate::jobs::Jobs) {
    let pool = state.pool.clone();
    jobs.spawn_periodic_leased(
        "community-ranking-events",
        crate::jobs::PeriodicCfg::every(Duration::from_secs(60)),
        move |ctx| {
            let pool = pool.clone();
            async move {
                let count = scan_ended(&pool, Utc::now().naive_utc()).await?;
                ctx.progress(format!("{count} local event occurrences ended"));
                Ok(())
            }
        },
    );
    let pool = state.pool.clone();
    jobs.spawn_leased("community-ranking", move |ctx| { let pool = pool.clone(); async move {
        tokio::select! { _=ctx.cancel.cancelled()=>return Ok(()), _=tokio::time::sleep(Duration::from_secs(1800))=>{} }
        loop {
            match recalculate(&pool,Utc::now().naive_utc()).await {
                Ok(count)=>ctx.progress(format!("{count} community scores calculated")),
                Err(error)=>{ctx.error(error.to_string()); tracing::error!(%error,"community ranking calculation failed");}
            }
            ctx.pass_done();
            tokio::select! { _=ctx.cancel.cancelled()=>return Ok(()), _=tokio::time::sleep(Duration::from_secs(86400))=>{} }
        }
    }});
}
