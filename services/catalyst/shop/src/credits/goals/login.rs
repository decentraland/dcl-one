// from: credits-server@d9c6a261fb7acf3beb926e10f03d1aed33e9771b:src/logic/goals-handlers/log-in-handler.ts
use super::{
    ApiError, Assignment, NaiveDateTime, PgConnection, SharedState, current, initialize, policy,
    wallet_refusal,
};

pub(super) const GOAL: &str = "log-in";

/// Called in the signed enrollment transaction after restoring eligibility.
pub(super) async fn record_enrollment(
    conn: &mut PgConnection,
    state: &SharedState,
    address: &str,
    now: NaiveDateTime,
) -> Result<(), ApiError> {
    let Some(season) = current(conn, now).await? else {
        return Ok(());
    };
    let week = season.week(now).0;
    let assigned: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1 FROM credits.season_goals
           WHERE season_id=$1 AND goal_id=$2 AND $3 BETWEEN start_week AND end_week
         ) OR EXISTS(
           SELECT 1 FROM credits.goal_progress
           WHERE address=$4 AND season_id=$1 AND week_id=$3 AND goal_id=$2
         )",
    )
    .bind(season.id)
    .bind(GOAL)
    .bind(week)
    .bind(address)
    .fetch_one(&mut *conn)
    .await?;
    if !assigned || wallet_refusal(conn, address).await?.is_some() {
        return Ok(());
    }
    initialize(conn, address, season.id, week).await?;
    let assignment = Assignment {
        address,
        season: &season,
        week,
        goal: GOAL,
    };
    if policy(state, conn, address).await?.is_none() {
        record(conn, state, assignment, now, now).await?;
    }
    Ok(())
}

/// Enrolment and room joins share the same UTC date key and wallet lock.
pub(super) async fn record(
    conn: &mut PgConnection,
    state: &SharedState,
    assignment: Assignment<'_>,
    occurred_at: NaiveDateTime,
    now: NaiveDateTime,
) -> Result<(), ApiError> {
    let Some(progress) = assignment.progress(conn).await? else {
        return Ok(());
    };
    if progress.completed_steps == progress.total_steps {
        return Ok(());
    }
    let new_day = sqlx::query(
        "INSERT INTO credits.goal_login_days (address, season_id, week_id, day)
         VALUES ($1,$2,$3,$4) ON CONFLICT DO NOTHING",
    )
    .bind(assignment.address)
    .bind(assignment.season.id)
    .bind(assignment.week)
    .bind(occurred_at.date())
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
