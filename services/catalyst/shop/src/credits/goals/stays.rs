// from: credits-server@d9c6a261fb7acf3beb926e10f03d1aed33e9771b:src/logic/goals-handlers/visit-event-and-stay-handler.ts
//! Credit five minutes of verified presence at a distinct event. Timers
//! survive a restart. A missing leave by itself is never proof of a stay.
use super::{rooms::Fact, *};
use crate::credits::dto::GoalEventStay;

pub(super) const GOAL: &str = "visit-event-and-stay";
const REQUIRED_MS: i64 = 300_000;
const BATCH_SIZE: i64 = 100;

/// Observe this exact session in a complete LiveKit room roster.
pub(super) type Observe = dyn Fn(&str, &str, &str) -> Option<NaiveDateTime> + Send + Sync;

#[derive(sqlx::FromRow)]
struct Visit {
    address: String,
    room: String,
    participant_sid: String,
    season_id: i32,
    week_id: i32,
    event_id: String,
    joined_at: NaiveDateTime,
    due_at: NaiveDateTime,
    occurrence_finish: NaiveDateTime,
}

const VISIT_COLUMNS: &str = "address, room, participant_sid, season_id, week_id, event_id, joined_at, due_at, occurrence_finish";

pub(super) async fn metadata(
    conn: &mut PgConnection,
    address: &str,
    season: i32,
    week: i32,
) -> Result<Vec<GoalEventStay>, ApiError> {
    // Distinct participant sessions can overlap during reconnects. Correlated
    // room proofs describe one wallet's elapsed time, so union the ranges.
    Ok(sqlx::query_as(
        "WITH events AS (
           SELECT event_id, min(joined_at) AS first_visit,
                  max(joined_at) FILTER (WHERE check_at IS NOT NULL) AS started,
                  range_agg(tsrange(joined_at, confirmed_until)) AS ranges
           FROM credits.goal_event_visits
           WHERE address=$1 AND season_id=$2 AND week_id=$3
           GROUP BY event_id
         )
         SELECT event_id AS id,
           LEAST($4, COALESCE((SELECT FLOOR(SUM(EXTRACT(EPOCH FROM upper(r) - lower(r)) * 1000))
                               FROM unnest(ranges) AS r), 0)::bigint) AS total_time_spent,
           (EXTRACT(EPOCH FROM started) * 1000)::bigint AS started_at
         FROM events ORDER BY first_visit, event_id",
    )
    .bind(address)
    .bind(season)
    .bind(week)
    .bind(REQUIRED_MS)
    .fetch_all(conn)
    .await?)
}

pub(super) async fn join(
    state: &SharedState,
    conn: &mut PgConnection,
    season: &Season,
    week: i32,
    fact: &mut Fact,
    now: NaiveDateTime,
    mirror: Option<&str>,
) -> Result<bool, ApiError> {
    let assignment = Assignment {
        address: &fact.address,
        season,
        week,
        goal: GOAL,
    };
    let Some(progress) = assignment.progress(conn).await? else {
        return Ok(true);
    };
    if progress.completed_steps == progress.total_steps {
        return Ok(true);
    }
    pin(conn, fact, mirror).await?;
    if !fact.stay_resolved {
        return Ok(false);
    }
    let (Some(event_id), Some(start), Some(finish)) =
        (&fact.stay_event_id, fact.stay_start, fact.stay_finish)
    else {
        return Ok(true);
    };
    let spent = metadata(conn, &fact.address, season.id, week)
        .await?
        .into_iter()
        .find(|visit| &visit.id == event_id)
        .map_or(0, |visit| visit.total_time_spent);
    if spent >= REQUIRED_MS {
        return Ok(true);
    }

    let due_at = (fact.occurred_at + TimeDelta::milliseconds(REQUIRED_MS - spent))
        .min(finish)
        .min(season.week(fact.occurred_at).2);
    let inserted = sqlx::query(
        "INSERT INTO credits.goal_event_visits
           (address, room, participant_sid, season_id, week_id, event_id,
            joined_at, occurrence_start, occurrence_finish, confirmed_until, due_at, check_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$7,$10,$10)
         ON CONFLICT (address, room, participant_sid) DO NOTHING",
    )
    .bind(&fact.address)
    .bind(&fact.room)
    .bind(&fact.participant_sid)
    .bind(season.id)
    .bind(week)
    .bind(event_id)
    .bind(fact.occurred_at)
    .bind(start)
    .bind(finish)
    .bind(due_at)
    .execute(&mut *conn)
    .await?
    .rows_affected()
        > 0;
    if !inserted {
        return Ok(true);
    }

    // A leave can arrive before its join, including on a different replica.
    if let Some(left_at) = observed_leave(conn, fact).await? {
        let visit = Visit {
            address: fact.address.clone(),
            room: fact.room.clone(),
            participant_sid: fact.participant_sid.clone(),
            season_id: season.id,
            week_id: week,
            event_id: event_id.clone(),
            joined_at: fact.occurred_at,
            due_at,
            occurrence_finish: finish,
        };
        confirm(state, conn, &visit, left_at, true, now).await?;
    }
    Ok(true)
}

async fn observed_leave(
    conn: &mut PgConnection,
    fact: &Fact,
) -> Result<Option<NaiveDateTime>, ApiError> {
    Ok(sqlx::query_scalar(
        "SELECT occurred_at FROM credits.goal_room_events
         WHERE address=$1 AND room=$2 AND participant_sid=$3 AND NOT joined AND occurred_at >= $4
         ORDER BY occurred_at LIMIT 1",
    )
    .bind(&fact.address)
    .bind(&fact.room)
    .bind(&fact.participant_sid)
    .bind(fact.occurred_at)
    .fetch_optional(conn)
    .await?)
}

pub(super) async fn leave(
    state: &SharedState,
    conn: &mut PgConnection,
    fact: &Fact,
    now: NaiveDateTime,
) -> Result<(), ApiError> {
    let sql = format!(
        "SELECT {VISIT_COLUMNS} FROM credits.goal_event_visits
         WHERE address=$1 AND room=$2 AND participant_sid=$3 AND joined_at <= $4"
    );
    let visit: Option<Visit> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(&fact.address)
        .bind(&fact.room)
        .bind(&fact.participant_sid)
        .bind(fact.occurred_at)
        .fetch_optional(&mut *conn)
        .await?;
    if let Some(visit) = visit {
        confirm(state, conn, &visit, fact.occurred_at, true, now).await?;
    }
    Ok(())
}

async fn confirm(
    state: &SharedState,
    conn: &mut PgConnection,
    visit: &Visit,
    verified_until: NaiveDateTime,
    left: bool,
    now: NaiveDateTime,
) -> Result<(), ApiError> {
    let Some(season) = current(conn, now).await? else {
        return schedule(conn, visit, None).await;
    };
    let (week, _, week_finish) = season.week(now);
    if season.id != visit.season_id || week != visit.week_id || now >= week_finish {
        // Claims only select the current season and week. Never sign a
        // credit for an assignment that can no longer be claimed.
        return schedule(conn, visit, None).await;
    }
    let assignment = Assignment {
        address: &visit.address,
        season: &season,
        week: visit.week_id,
        goal: GOAL,
    };
    let eligible = wallet_refusal(conn, &visit.address).await?.is_none()
        && now < season.grant_expires_at()?
        && policy(state, conn, &visit.address).await?.is_none();
    let progress = assignment.progress(conn).await?;
    let Some(progress) = progress.filter(|p| eligible && p.completed_steps < p.total_steps) else {
        return schedule(conn, visit, None).await;
    };

    let session_limit = visit.joined_at + TimeDelta::milliseconds(REQUIRED_MS);
    let credit_finish = visit.occurrence_finish.min(week_finish);
    let until = verified_until
        .min(session_limit)
        .min(credit_finish)
        .min(now);
    sqlx::query(
        "UPDATE credits.goal_event_visits SET confirmed_until=GREATEST(confirmed_until,$4)
         WHERE address=$1 AND room=$2 AND participant_sid=$3",
    )
    .bind(&visit.address)
    .bind(&visit.room)
    .bind(&visit.participant_sid)
    .bind(until)
    .execute(&mut *conn)
    .await?;
    let visits = metadata(conn, &visit.address, visit.season_id, visit.week_id).await?;
    let steps = visits
        .iter()
        .filter(|v| v.total_time_spent >= REQUIRED_MS)
        .count();
    let steps = i32::try_from(steps)
        .unwrap_or(i32::MAX)
        .min(progress.total_steps);
    if steps > progress.completed_steps {
        assignment
            .advance(conn, state, &progress, steps, now)
            .await?;
    }
    let event_complete = visits
        .iter()
        .any(|v| v.id == visit.event_id && v.total_time_spent >= REQUIRED_MS);
    let next_check =
        (!left && !event_complete && until < credit_finish).then(|| now + TimeDelta::seconds(30));
    schedule(conn, visit, next_check).await
}

async fn schedule(
    conn: &mut PgConnection,
    visit: &Visit,
    check_at: Option<NaiveDateTime>,
) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE credits.goal_event_visits SET check_at=$4
         WHERE address=$1 AND room=$2 AND participant_sid=$3",
    )
    .bind(&visit.address)
    .bind(&visit.room)
    .bind(&visit.participant_sid)
    .bind(check_at)
    .execute(conn)
    .await?;
    Ok(())
}

async fn check(
    state: &SharedState,
    candidate: Visit,
    clock: &impl Fn() -> NaiveDateTime,
    observe: &Observe,
) -> Result<(), ApiError> {
    let mut tx = state.credits.pool.begin().await?;
    wallet_lock(&mut tx, &candidate.address).await?;
    let now = clock();
    let sql = format!(
        "SELECT {VISIT_COLUMNS} FROM credits.goal_event_visits
         WHERE address=$1 AND room=$2 AND participant_sid=$3 AND check_at <= $4"
    );
    let visit: Option<Visit> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(&candidate.address)
        .bind(&candidate.room)
        .bind(&candidate.participant_sid)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await?;
    if let Some(visit) = visit {
        if let Some(at) = observe(&visit.room, &visit.participant_sid, &visit.address)
            .filter(|at| *at >= visit.due_at)
        {
            confirm(state, &mut tx, &visit, at, false, now).await?;
        } else {
            // Give a delayed leave time to arrive. After an hour, only a
            // verified leave can finish this session.
            let retry =
                (now < visit.due_at + TimeDelta::hours(1)).then(|| now + TimeDelta::seconds(30));
            schedule(&mut tx, &visit, retry).await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

pub(super) async fn process_due(
    state: &SharedState,
    clock: impl Fn() -> NaiveDateTime,
    observe: &Observe,
) -> Result<usize, ApiError> {
    let sql = format!(
        "SELECT {VISIT_COLUMNS} FROM credits.goal_event_visits
         WHERE check_at <= $1 ORDER BY check_at LIMIT $2"
    );
    let candidates: Vec<Visit> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(clock())
        .bind(BATCH_SIZE)
        .fetch_all(&state.credits.pool)
        .await?;
    let mut processed = 0;
    let mut first_error = None;
    for visit in candidates {
        let address = visit.address.clone();
        let room = visit.room.clone();
        let sid = visit.participant_sid.clone();
        match check(state, visit, &clock, observe).await {
            Ok(()) => processed += 1,
            Err(error) => {
                tracing::warn!(%error, "event stay timer remains retryable");
                sqlx::query(
                    "UPDATE credits.goal_event_visits SET check_at=$4 + interval '30 seconds'
                     WHERE address=$1 AND room=$2 AND participant_sid=$3 AND check_at <= $4",
                )
                .bind(address)
                .bind(room)
                .bind(sid)
                .bind(clock())
                .execute(&state.credits.pool)
                .await?;
                first_error.get_or_insert(error);
            }
        }
    }
    if let Some(error) = first_error {
        return Err(error);
    }
    Ok(processed)
}

#[derive(sqlx::FromRow)]
struct VenueEvidence {
    scene_known: bool,
    mirror_covers_join: bool,
    id: Option<String>,
    starts_at: Option<NaiveDateTime>,
    finishes_at: Option<NaiveDateTime>,
}

/// Pin the occurrence before the webhook is acknowledged. Once pinned, a
/// redeploy or schedule edit cannot change a queued join's meaning. An
/// unsynced scene or a mirror that has yet to cover the join remains retryable.
pub(super) async fn pin(
    conn: &mut PgConnection,
    fact: &mut Fact,
    mirror: Option<&str>,
) -> sqlx::Result<()> {
    if fact.stay_resolved {
        return Ok(());
    }
    let evidence: VenueEvidence = sqlx::query_as(include_str!("stay_event.sql"))
        .bind(fact.world)
        .bind(&fact.realm)
        .bind(&fact.scene_id)
        .bind(fact.occurred_at)
        .bind(mirror)
        .fetch_one(&mut *conn)
        .await?;
    let resolved = evidence.scene_known && (evidence.id.is_some() || evidence.mirror_covers_join);
    if resolved {
        sqlx::query(
            "UPDATE credits.goal_room_events
             SET stay_resolved=true, stay_event_id=$2, stay_start=$3, stay_finish=$4
             WHERE event_id=$1",
        )
        .bind(&fact.event_id)
        .bind(&evidence.id)
        .bind(evidence.starts_at)
        .bind(evidence.finishes_at)
        .execute(conn)
        .await?;
        fact.stay_resolved = true;
        fact.stay_event_id = evidence.id;
        fact.stay_start = evidence.starts_at;
        fact.stay_finish = evidence.finishes_at;
    }
    Ok(())
}
