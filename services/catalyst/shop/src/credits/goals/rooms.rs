//! The verified scene and world room events that drive seasonal goals.
//! A failed goal transaction leaves the event queued for retry.
use super::{login, stays, *};
use crate::shared::identity::Address as WalletAddress;

const VISIT_LOCATION: &str = "visit-location";
const BATCH_SIZE: i64 = 100;

/// A participant fact from the verified LiveKit webhook.
pub struct RoomEvent<'a> {
    pub event_id: &'a str,
    pub identity: &'a str,
    pub room: &'a str,
    pub participant_sid: &'a str,
    pub joined: bool,
    pub world: bool,
    pub realm: &'a str,
    pub scene_id: Option<&'a str>,
    pub event_time_ms: u64,
}

/// Enqueue only goal input for an enrolled wallet in a configured season,
/// including a late leave for a visit that has already started.
pub async fn store(
    state: &SharedState,
    event: RoomEvent<'_>,
    mirror: Option<&str>,
) -> sqlx::Result<()> {
    let Ok(address) = WalletAddress::parse_untrimmed(event.identity) else {
        return Ok(());
    };
    let Some(occurred_at) = i64::try_from(event.event_time_ms)
        .ok()
        .filter(|&at| at > 0)
        .and_then(chrono::DateTime::from_timestamp_millis)
        .map(|at| at.naive_utc())
    else {
        return Ok(());
    };
    if event.event_id.is_empty()
        || event.event_id.len() > 128
        || event.participant_sid.is_empty()
        || event.participant_sid.len() > 128
        || event.room.is_empty()
        || event.room.len() > 512
        || event.realm.is_empty()
        || event.realm.len() > 512
        || (!event.world && event.scene_id.is_none_or(str::is_empty))
        || event.scene_id.is_some_and(|scene| scene.len() > 512)
    {
        return Ok(());
    }

    let mut tx = state.credits.pool.begin().await?;
    let fact: Option<Fact> = sqlx::query_as(
        "INSERT INTO credits.goal_room_events
             (event_id, address, room, participant_sid, joined, world, realm, scene_id, occurred_at, ready_at)
         SELECT $1, $2, $3, $4, $5, $6, $7, $8, $9, $9
         WHERE $9 >= timezone('UTC', now()) - interval '30 days'
           AND $9 <= timezone('UTC', now()) + interval '1 minute'
           AND (
            EXISTS (
              SELECT 1 FROM credits.wallets w
              JOIN credits.seasons s ON $9 BETWEEN s.start_date AND
                    CASE WHEN $5 THEN s.end_date
                         ELSE s.end_date + s.grace_days * interval '1 day' END
              JOIN credits.season_goals g ON g.season_id=s.id
                AND CASE WHEN $5 THEN g.goal_id <> 'take-photo'
                         ELSE g.goal_id = 'visit-event-and-stay' END
              WHERE w.address=$2 AND w.has_started_program
                AND NOT EXISTS (SELECT 1 FROM credits.program_opt_outs o WHERE o.address=$2)
            ) OR (NOT $5 AND (
              EXISTS (
                SELECT 1 FROM credits.goal_event_visits v
                WHERE v.address=$2 AND v.room=$3 AND v.participant_sid=$4
              ) OR EXISTS (
                SELECT 1 FROM credits.goal_room_events j
                WHERE j.address=$2 AND j.room=$3 AND j.participant_sid=$4 AND j.joined
                  AND j.occurred_at <= $9
              )
            ))
           )
         ON CONFLICT (event_id) DO NOTHING
         RETURNING event_id, address, room, participant_sid, joined, world, realm, scene_id,
                   occurred_at, stay_resolved, stay_event_id, stay_start, stay_finish",
    )
    .bind(event.event_id)
    .bind(address.as_str())
    .bind(event.room)
    .bind(event.participant_sid)
    .bind(event.joined)
    .bind(event.world)
    .bind(event.realm)
    .bind(event.scene_id)
    .bind(occurred_at)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(mut fact) = fact.filter(|fact| fact.joined) {
        let has_stay_goal: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM credits.seasons s JOIN credits.season_goals g ON g.season_id=s.id
                WHERE $1 BETWEEN s.start_date AND s.end_date AND g.goal_id=$2
             )",
        )
        .bind(fact.occurred_at)
        .bind(stays::GOAL)
        .fetch_one(&mut *tx)
        .await?;
        if has_stay_goal {
            stays::pin(&mut tx, &mut fact, mirror).await?;
        }
    }
    tx.commit().await
}

// from: credits-server@d9c6a261fb7acf3beb926e10f03d1aed33e9771b:src/logic/goals-handlers/visit-location-handler/utils.ts
pub(super) fn weekly_world(week: i32) -> Option<&'static str> {
    match week {
        1 => Some("bloomgarden.dcl.eth"),
        2 => Some("fastlane.dcl.eth"),
        3 => Some("fortunes.dcl.eth"),
        4 => Some("jaykaying.dcl.eth"),
        5 => Some("cleantheclub.dcl.eth"),
        _ => None,
    }
}

#[derive(sqlx::FromRow)]
pub(super) struct Fact {
    pub(super) event_id: String,
    pub(super) address: String,
    pub(super) room: String,
    pub(super) participant_sid: String,
    joined: bool,
    pub(super) world: bool,
    pub(super) realm: String,
    pub(super) scene_id: Option<String>,
    pub(super) occurred_at: NaiveDateTime,
    pub(super) stay_resolved: bool,
    pub(super) stay_event_id: Option<String>,
    pub(super) stay_start: Option<NaiveDateTime>,
    pub(super) stay_finish: Option<NaiveDateTime>,
}

async fn apply(
    state: &SharedState,
    conn: &mut PgConnection,
    event: &mut Fact,
    now: NaiveDateTime,
    mirror: Option<&str>,
) -> Result<bool, ApiError> {
    if !event.joined {
        stays::leave(state, conn, event, now).await?;
        return Ok(true);
    }
    let Some(season) = current(conn, now).await? else {
        return Ok(true);
    };
    let (week, week_start, week_finish) = season.week(now);
    if event.occurred_at < week_start
        || now >= week_finish
        || wallet_refusal(conn, &event.address).await?.is_some()
        || policy(state, conn, &event.address).await?.is_some()
    {
        return Ok(true);
    }

    initialize(conn, &event.address, season.id, week).await?;
    let mut assignment = Assignment {
        address: &event.address,
        season: &season,
        week,
        goal: login::GOAL,
    };
    login::record(conn, state, assignment, event.occurred_at.date(), now).await?;
    if event.world
        && weekly_world(week).is_some_and(|world| world.eq_ignore_ascii_case(&event.realm))
    {
        assignment.goal = VISIT_LOCATION;
        if let Some(p) = assignment.progress(conn).await?
            && p.completed_steps < p.total_steps
        {
            assignment
                .advance(conn, state, &p, p.total_steps, now)
                .await?;
        }
    }
    stays::join(state, conn, &season, week, event, now, mirror).await
}

async fn process_event(
    state: &SharedState,
    id: &str,
    clock: &impl Fn() -> NaiveDateTime,
    mirror: Option<&str>,
) -> Result<bool, ApiError> {
    let mut tx = state.credits.pool.begin().await?;
    let Some(mut event) = sqlx::query_as::<_, Fact>(
        "SELECT event_id, address, room, participant_sid, joined, world, realm, scene_id,
                occurred_at, stay_resolved, stay_event_id, stay_start, stay_finish
         FROM credits.goal_room_events WHERE event_id=$1 AND processed_at IS NULL FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Ok(false);
    };
    wallet_lock(&mut tx, &event.address).await?;
    let now = clock();
    let applied = apply(state, &mut tx, &mut event, now, mirror).await?;
    sqlx::query(
        "UPDATE credits.goal_room_events
         SET processed_at=CASE WHEN $3 THEN $2 ELSE NULL END, ready_at=$2 + interval '30 seconds'
         WHERE event_id=$1",
    )
    .bind(id)
    .bind(now)
    .bind(applied)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(applied)
}

pub(super) async fn process_batch(
    state: &SharedState,
    clock: impl Fn() -> NaiveDateTime,
    mirror: Option<&str>,
) -> Result<u64, ApiError> {
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT event_id FROM credits.goal_room_events
         WHERE processed_at IS NULL AND ready_at <= $1
         ORDER BY ready_at, occurred_at, event_id LIMIT $2",
    )
    .bind(clock())
    .bind(BATCH_SIZE)
    .fetch_all(&state.credits.pool)
    .await?;
    let mut processed = 0;
    let mut first_error = None;
    for id in ids {
        match process_event(state, &id, &clock, mirror).await {
            Ok(applied) => processed += u64::from(applied),
            Err(error) => {
                tracing::warn!(event_id = id, %error, "room goal event remains retryable");
                // Back off this fact so that an unavailable dependency cannot
                // keep the front of the queue ahead of all other room events.
                sqlx::query(
                    "UPDATE credits.goal_room_events SET ready_at=$2 + interval '30 seconds'
                     WHERE event_id=$1 AND processed_at IS NULL",
                )
                .bind(&id)
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

pub(super) async fn prune(state: &SharedState) -> Result<(), ApiError> {
    sqlx::query(
        "DELETE FROM credits.goal_room_events WHERE event_id IN (
           SELECT event_id FROM credits.goal_room_events
           WHERE occurred_at < timezone('UTC', now()) - interval '30 days'
           ORDER BY occurred_at LIMIT 1000
         )",
    )
    .execute(&state.credits.pool)
    .await?;
    Ok(())
}

pub fn spawn<F>(state: &SharedState, jobs: &Jobs, mirror: Option<String>, observe: F)
where
    F: Fn(&str, &str, &str) -> Option<NaiveDateTime> + Send + Sync + 'static,
{
    let goal_state = Arc::clone(state);
    let observe = Arc::new(observe);
    let mirror = Arc::new(mirror);
    jobs.spawn_periodic_leased(
        "credits-room-goals",
        PeriodicCfg::every(Duration::from_secs(5)),
        move |ctx| {
            let state = goal_state.clone();
            let observe = observe.clone();
            let mirror = mirror.clone();
            async move {
                let clock = crate::credits::store::now;
                // Run due timers even if an unrelated fact remains retryable.
                let enrollments = login::process_pending(&state, &clock).await;
                let facts = process_batch(&state, clock, mirror.as_deref()).await;
                let timers = stays::process_due(&state, clock, observe.as_ref()).await;
                let enrollments = enrollments?;
                let facts = facts?;
                let timers = timers?;
                ctx.progress(format!(
                    "{enrollments} enrollments applied, {facts} room events applied, {timers} stay timers checked"
                ));
                Ok(())
            }
        },
    );
    let state = Arc::clone(state);
    jobs.spawn_periodic_leased(
        "credits-room-goal-retention",
        PeriodicCfg::every(Duration::from_secs(60)),
        move |_| {
            let state = state.clone();
            async move { Ok(prune(&state).await?) }
        },
    );
}
