// from: credits-server@d9c6a261fb7acf3beb926e10f03d1aed33e9771b:src/logic/goals-handlers/log-in-handler.ts
use super::{
    ApiError, Assignment, NaiveDateTime, PgConnection, SharedState, current, initialize, policy,
    wallet_lock, wallet_refusal,
};
use chrono::NaiveDate;

pub(super) const GOAL: &str = "log-in";

#[derive(sqlx::FromRow)]
pub(super) struct EnrollmentDay {
    address: String,
    season_id: i32,
    week_id: i32,
    day: NaiveDate,
}

/// Remember the date with enrollment. Flags and signing are evaluated only
/// after that transaction has committed.
pub(super) async fn enqueue_enrollment(
    conn: &mut PgConnection,
    address: &str,
    now: NaiveDateTime,
) -> Result<Option<EnrollmentDay>, ApiError> {
    let Some(season) = current(conn, now).await? else {
        return Ok(None);
    };
    let (week, _, end) = season.week(now);
    if now >= end {
        return Ok(None);
    }
    Ok(sqlx::query_as(
        "INSERT INTO credits.goal_login_days (address, season_id, week_id, day, pending_at)
         SELECT $4,$1,$3,$5,$6
         WHERE EXISTS (
           SELECT 1 FROM credits.season_goals
           WHERE season_id=$1 AND goal_id=$2 AND $3 BETWEEN start_week AND end_week
         ) OR EXISTS (
           SELECT 1 FROM credits.goal_progress
           WHERE address=$4 AND season_id=$1 AND week_id=$3 AND goal_id=$2
         )
         ON CONFLICT DO NOTHING RETURNING address, season_id, week_id, day",
    )
    .bind(season.id)
    .bind(GOAL)
    .bind(week)
    .bind(address)
    .bind(now.date())
    .bind(now)
    .fetch_optional(conn)
    .await?)
}

async fn apply_enrollment(
    state: &SharedState,
    day: &EnrollmentDay,
    clock: &impl Fn() -> NaiveDateTime,
) -> Result<bool, ApiError> {
    let mut tx = state.credits.pool.begin().await?;
    wallet_lock(&mut tx, &day.address).await?;
    let now = clock();
    let pending: bool = sqlx::query_scalar(
        "SELECT EXISTS (
           SELECT 1 FROM credits.goal_login_days
           WHERE address=$1 AND season_id=$2 AND week_id=$3 AND day=$4 AND pending_at <= $5
         )",
    )
    .bind(&day.address)
    .bind(day.season_id)
    .bind(day.week_id)
    .bind(day.day)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;
    if !pending {
        return Ok(false);
    }

    if let Some(season) = current(&mut tx, now).await? {
        let (week, _, end) = season.week(now);
        if season.id == day.season_id
            && week == day.week_id
            && now < end
            && wallet_refusal(&mut tx, &day.address).await?.is_none()
            && policy(state, &mut tx, &day.address).await?.is_none()
        {
            initialize(&mut tx, &day.address, season.id, week).await?;
            let assignment = Assignment {
                address: &day.address,
                season: &season,
                week,
                goal: GOAL,
            };
            record(&mut tx, state, assignment, day.day, now).await?;
        }
    }
    // The day is either counted (pending_at cleared) or no longer eligible.
    sqlx::query(
        "DELETE FROM credits.goal_login_days
         WHERE address=$1 AND season_id=$2 AND week_id=$3 AND day=$4 AND pending_at IS NOT NULL",
    )
    .bind(&day.address)
    .bind(day.season_id)
    .bind(day.week_id)
    .bind(day.day)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

/// Best-effort immediate progression with durable retries for an unavailable
/// flag document, signer or grant transaction.
pub(super) async fn advance_enrollment(
    state: &SharedState,
    day: &EnrollmentDay,
    clock: &impl Fn() -> NaiveDateTime,
) -> bool {
    match apply_enrollment(state, day, clock).await {
        Ok(applied) => applied,
        Err(error) => {
            tracing::warn!(%error, "enrollment login remains pending");
            if let Err(backoff) = sqlx::query(
                "UPDATE credits.goal_login_days SET pending_at=$5 + interval '30 seconds'
                 WHERE address=$1 AND season_id=$2 AND week_id=$3 AND day=$4 AND pending_at <= $5",
            )
            .bind(&day.address)
            .bind(day.season_id)
            .bind(day.week_id)
            .bind(day.day)
            .bind(clock())
            .execute(&state.credits.pool)
            .await
            {
                tracing::warn!(error=%backoff, "could not back off an enrollment login");
            }
            false
        }
    }
}

pub(super) async fn process_pending(
    state: &SharedState,
    clock: &impl Fn() -> NaiveDateTime,
) -> Result<u64, ApiError> {
    let pending: Vec<EnrollmentDay> = sqlx::query_as(
        "SELECT address,season_id,week_id,day FROM credits.goal_login_days
         WHERE pending_at <= $1 ORDER BY pending_at,address,season_id,week_id,day LIMIT 100",
    )
    .bind(clock())
    .fetch_all(&state.credits.pool)
    .await?;
    let mut processed = 0;
    for day in pending {
        processed += u64::from(advance_enrollment(state, &day, clock).await);
    }
    Ok(processed)
}

/// Enrolment and room joins share the same UTC date key and wallet lock.
pub(super) async fn record(
    conn: &mut PgConnection,
    state: &SharedState,
    assignment: Assignment<'_>,
    day: NaiveDate,
    now: NaiveDateTime,
) -> Result<(), ApiError> {
    let Some(progress) = assignment.progress(conn).await? else {
        return Ok(());
    };
    if progress.completed_steps == progress.total_steps {
        return Ok(());
    }
    let new_day = sqlx::query(
        "INSERT INTO credits.goal_login_days AS d (address, season_id, week_id, day)
         VALUES ($1,$2,$3,$4)
         ON CONFLICT (address,season_id,week_id,day) DO UPDATE SET pending_at=NULL
         WHERE d.pending_at IS NOT NULL",
    )
    .bind(assignment.address)
    .bind(assignment.season.id)
    .bind(assignment.week)
    .bind(day)
    .execute(&mut *conn)
    .await?
    .rows_affected()
        > 0;
    if new_day {
        assignment
            .advance(conn, state, &progress, progress.completed_steps + 1, now)
            .await?;
    }
    Ok(())
}
