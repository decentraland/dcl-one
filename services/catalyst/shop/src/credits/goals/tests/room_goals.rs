use super::*;

const WEEKLY_WORLD: &str = "bloomgarden.dcl.eth";
const EVENT_WORLD: &str = "partylocation.dcl.eth";

async fn seed_room_goals(pool: &sqlx::PgPool) -> NaiveDateTime {
    seed(pool, 2, "1000000000000000000", "100000000000000000000").await;
    let now = crate::credits::store::now();
    let preceding_monday =
        now.date() - TimeDelta::days(14 + i64::from(now.weekday().num_days_from_monday()));
    let start = preceding_monday.and_hms_opt(0, 0, 0).unwrap();
    sqlx::query("UPDATE credits.seasons SET start_date=$1 WHERE id=1")
        .bind(start)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO credits.season_goals
            (season_id, goal_id, title, thumbnail, total_steps, reward_wei, start_week, end_week)
         VALUES (1,'log-in','Log in','login.png',2,1000000000000000000,1,20),
                (1,'visit-location','Visit','visit.png',1,2000000000000000000,1,20),
                (1,'visit-event-and-stay','Stay','stay.png',2,3000000000000000000,1,20)",
    )
    .execute(pool)
    .await
    .unwrap();
    start + TimeDelta::hours(10)
}

fn world<'a>(
    event_id: &'a str,
    sid: &'a str,
    realm: &'a str,
    at: NaiveDateTime,
) -> rooms::RoomEvent<'a> {
    rooms::RoomEvent {
        event_id,
        identity: OWNER,
        room: realm,
        participant_sid: sid,
        joined: true,
        world: true,
        realm,
        scene_id: None,
        event_time_ms: at.and_utc().timestamp_millis() as u64,
    }
}

async fn enqueue(state: &SharedState, event: rooms::RoomEvent<'_>) {
    rooms::store(state, event, None).await.unwrap();
}

async fn room_steps(pool: &sqlx::PgPool, goal: &str) -> i32 {
    sqlx::query_scalar(
        "SELECT completed_steps FROM credits.goal_progress
         WHERE season_id=1 AND week_id=1 AND address=$1 AND goal_id=$2",
    )
    .bind(OWNER)
    .bind(goal)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn notice_ids(pool: &sqlx::PgPool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT metadata->>'goalId' FROM social.notifications
         WHERE type='credits_goal_completed' ORDER BY metadata->>'goalId'",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn joins_deduplicate_utc_dates_and_only_the_weekly_world_completes_location() {
    let Some(db) = OwnDatabase::create("room_goal_login_location").await else {
        return;
    };
    let first = seed_room_goals(&db.pool).await;
    seed_scene(&db.pool, first).await;
    let now = first + TimeDelta::days(2);
    let state = fixture_state(db.pool.clone(), true);
    enqueue(&state, world("first", "sid1", "wrong.dcl.eth", first)).await;
    enqueue(&state, world("first", "sid1", WEEKLY_WORLD, first)).await;
    enqueue(
        &state,
        world(
            "another-join",
            "sid2",
            "wrong.dcl.eth",
            first + TimeDelta::minutes(1),
        ),
    )
    .await;
    let mut scene = world(
        "spoofed-realm",
        "sid3",
        WEEKLY_WORLD,
        first + TimeDelta::minutes(2),
    );
    scene.world = false;
    scene.scene_id = Some("active-scene");
    enqueue(&state, scene).await;
    let mut server = world(
        "server",
        "sid4",
        WEEKLY_WORLD,
        first + TimeDelta::minutes(3),
    );
    server.identity = "authoritative-server";
    enqueue(&state, server).await;
    assert_eq!(rooms::process_batch(&state, || now, None).await.unwrap(), 3);
    assert_eq!(room_steps(&db.pool, "log-in").await, 1);
    assert_eq!(room_steps(&db.pool, "visit-location").await, 0);

    let scheduled_world = WEEKLY_WORLD.to_ascii_uppercase();
    enqueue(
        &state,
        world(
            "next-date",
            "sid5",
            &scheduled_world,
            first + TimeDelta::days(1),
        ),
    )
    .await;
    assert_eq!(rooms::process_batch(&state, || now, None).await.unwrap(), 1);
    assert_eq!(room_steps(&db.pool, "log-in").await, 2);
    assert_eq!(room_steps(&db.pool, "visit-location").await, 1);
    assert_eq!(notice_ids(&db.pool).await, vec!["log-in", "visit-location"]);
    assert_eq!(
        count(&db.pool, "SELECT count(*) FROM credits.goal_grants").await,
        2
    );
    assert_eq!(rooms::process_batch(&state, || now, None).await.unwrap(), 0);
}

async fn seed_scene(pool: &sqlx::PgPool, start: NaiveDateTime) {
    sqlx::query(
        "INSERT INTO content.deployments
           (deployer_address,version,entity_type,entity_id,entity_timestamp,
            entity_pointers,local_timestamp,auth_chain)
         VALUES ($1,'v3','scene','active-scene',$2,ARRAY['7,-9','8,-9'],$2,'[]')",
    )
    .bind(OWNER)
    .bind(start - TimeDelta::days(7))
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_events(pool: &sqlx::PgPool, start: NaiveDateTime) {
    seed_scene(pool, start).await;
    let earlier = start - TimeDelta::days(2);
    sqlx::query(
        "INSERT INTO explore.event
            (id,name,start_at,finish_at,next_start_at,next_finish_at,duration_ms,recurrent,
             approved,coordinates_x,coordinates_y,raw,origin)
         VALUES
           ('world','World event',$1,$2,$1,$2,3600000,false,true,NULL,NULL,$3,'local'),
           ('denied','Unapproved event',$1,$2,$1,$2,3600000,false,false,NULL,NULL,$3,'upstream'),
           ('scene','Scene series',$4,$5,$4,$6,3600000,true,true,8,-9,$7,'local')",
    )
    .bind(start)
    .bind(start + TimeDelta::hours(1))
    .bind(json!({"world":true,"server":EVENT_WORLD}))
    .bind(earlier)
    .bind(start + TimeDelta::days(2) + TimeDelta::hours(1))
    .bind(earlier + TimeDelta::hours(1))
    .bind(json!({"recurrent_dates":[start.and_utc(), (start + TimeDelta::days(2)).and_utc()]}))
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn short_visits_accumulate_by_event_and_only_a_verified_session_can_fire_the_timer() {
    let Some(db) = OwnDatabase::create("room_goal_event_stay").await else {
        return;
    };
    let first = seed_room_goals(&db.pool).await;
    seed_events(&db.pool, first).await;
    let now = first + TimeDelta::minutes(12);
    let state = fixture_state(db.pool.clone(), true);

    // The leave can commit before the join. Pair their source timestamps.
    let left = first + TimeDelta::seconds(100);
    let mut leave = world("left-early", "sid1", EVENT_WORLD, left);
    leave.joined = false;
    enqueue(&state, leave).await;
    rooms::process_batch(&state, || now, None).await.unwrap();
    enqueue(&state, world("arrived-earlier", "sid1", EVENT_WORLD, first)).await;
    let overlap = first + TimeDelta::seconds(50);
    enqueue(&state, world("overlapping", "sid2", EVENT_WORLD, overlap)).await;
    let mut overlap_leave = world("overlap-left", "sid2", EVENT_WORLD, left);
    overlap_leave.joined = false;
    enqueue(&state, overlap_leave).await;
    rooms::process_batch(&state, || now, None).await.unwrap();
    let mut conn = db.pool.acquire().await.unwrap();
    let metadata = stays::metadata(&mut conn, OWNER, 1, 1).await.unwrap();
    assert_eq!(
        (metadata[0].id.as_str(), metadata[0].total_time_spent),
        ("world", 100_000)
    );
    assert_eq!(metadata[0].started_at, None);
    assert_eq!(room_steps(&db.pool, "visit-event-and-stay").await, 0);

    let rejoin = first + TimeDelta::seconds(200);
    enqueue(&state, world("rejoin", "sid3", EVENT_WORLD, rejoin)).await;
    rooms::process_batch(&state, || now, None).await.unwrap();
    // A restart loses in-memory state and roster observations, not the timer.
    let restarted = fixture_state(db.pool.clone(), true);
    let due = rejoin + TimeDelta::seconds(200);
    stays::process_due(&restarted, || due, &|_, _, _| None)
        .await
        .unwrap();
    assert_eq!(room_steps(&db.pool, "visit-event-and-stay").await, 0);
    stays::process_due(
        &restarted,
        || due + TimeDelta::seconds(30),
        &move |room, sid, wallet| {
            (room == EVENT_WORLD && sid == "sid3" && wallet == OWNER).then_some(due)
        },
    )
    .await
    .unwrap();
    assert_eq!(room_steps(&db.pool, "visit-event-and-stay").await, 1);

    // The scene's stored next occurrence already expired. Its actual recurring
    // occurrence qualifies; a join in the gap beforehand does not.
    for (id, sid, at) in [
        ("scene-in-gap", "sid4", first - TimeDelta::minutes(4)),
        ("scene-in-occurrence", "sid5", first + TimeDelta::minutes(5)),
    ] {
        let mut scene = world(id, sid, "main", at);
        scene.world = false;
        scene.scene_id = Some("active-scene");
        enqueue(&restarted, scene).await;
    }
    rooms::process_batch(&restarted, || now, None)
        .await
        .unwrap();
    stays::process_due(&restarted, || now, &move |room, sid, wallet| {
        (room == "main" && sid == "sid5" && wallet == OWNER)
            .then_some(first + TimeDelta::minutes(10))
    })
    .await
    .unwrap();
    assert_eq!(room_steps(&db.pool, "visit-event-and-stay").await, 2);
    assert_eq!(notice_ids(&db.pool).await, vec!["visit-event-and-stay"]);
    assert_eq!(
        count(&db.pool, "SELECT count(*) FROM credits.goal_grants").await,
        1
    );
    assert_eq!(
        count(&db.pool, "SELECT count(*) FROM credits.goal_event_visits").await,
        4
    );
    assert_eq!(
        stays::process_due(&restarted, || now, &move |_, _, _| Some(now))
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn enrollment_succeeds_and_retries_the_same_date_through_flags_and_signer_outages() {
    let Some(db) = OwnDatabase::create("room_goal_enrollment").await else {
        return;
    };
    let now = seed_room_goals(&db.pool).await;
    sqlx::query(
        "UPDATE credits.season_goals SET total_steps=1 WHERE season_id=1 AND goal_id='log-in'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query("UPDATE social.subscriptions SET email='login@example.org' WHERE address=$1")
        .bind(OWNER)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE credits.wallets SET has_started_program=false WHERE address=$1")
        .bind(OWNER)
        .execute(&db.pool)
        .await
        .unwrap();

    let unsigned_state = fixture_state(db.pool.clone(), false);
    unsigned_state.goal_flags.set_documents(None, None);
    reads::enroll_at(&unsigned_state, OWNER, now).await.unwrap();
    assert_eq!(
        status(&unsigned_state, OWNER).await.unwrap()["data"]["status"],
        "enrolled"
    );
    assert_eq!(
        count(
            &db.pool,
            "SELECT count(*) FROM credits.goal_login_days WHERE pending_at IS NOT NULL"
        )
        .await,
        1
    );
    assert_eq!(
        count(&db.pool, "SELECT count(*) FROM credits.goal_grants").await,
        0
    );

    let before = reads::progress_at(&unsigned_state, OWNER, now)
        .await
        .unwrap();
    let login = before.goals.iter().find(|g| g.id == "log-in").unwrap();
    assert_eq!(
        serde_json::to_value(&login.progress.metadata).unwrap(),
        json!({"dates":[]})
    );
    assert_eq!(login.progress.completed_steps, 0);
    unsigned_state.goal_flags.set_documents(
        Some(json!({"flags":{"dapps-credits-server":true},"variants":{}})),
        Some(json!({"flags":{},"variants":{}})),
    );
    let after_flags = now + TimeDelta::seconds(30);
    assert_eq!(
        login::process_pending(&unsigned_state, &|| after_flags)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        count(
            &db.pool,
            "SELECT count(*) FROM credits.goal_login_days WHERE pending_at IS NOT NULL"
        )
        .await,
        1,
        "the missing signer still leaves enrollment intact and the original day queued"
    );
    assert!(notice_ids(&db.pool).await.is_empty());

    let state = fixture_state(db.pool.clone(), true);
    let recovered = now + TimeDelta::seconds(60);
    assert_eq!(
        login::process_pending(&state, &|| recovered - TimeDelta::seconds(1))
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        login::process_pending(&state, &|| recovered).await.unwrap(),
        1
    );
    reads::enroll_at(&state, OWNER, recovered).await.unwrap();
    enqueue(&state, world("same-date", "sid1", "any.dcl.eth", recovered)).await;
    rooms::process_batch(&state, || recovered, None)
        .await
        .unwrap();
    let report = reads::progress_at(&state, OWNER, recovered).await.unwrap();
    let login = report.goals.iter().find(|g| g.id == "log-in").unwrap();
    assert!(login.is_completed);
    assert_eq!(login.progress.completed_steps, 1);
    assert_eq!(
        serde_json::to_value(&login.progress.metadata).unwrap(),
        json!({"dates":[now.date().to_string()]})
    );
    let location = report
        .goals
        .iter()
        .find(|g| g.id == "visit-location")
        .unwrap();
    assert_eq!(location.title, "Explore The Scene At bloomgarden.dcl.eth");
    assert_eq!(
        serde_json::to_value(&location.progress.metadata).unwrap(),
        json!({})
    );
    assert_eq!(notice_ids(&db.pool).await, vec!["log-in"]);
    assert_eq!(
        count(&db.pool, "SELECT count(*) FROM credits.goal_grants").await,
        1
    );
}

#[tokio::test]
async fn leave_and_roster_only_count_time_inside_the_pinned_occurrence() {
    let Some(db) = OwnDatabase::create("room_goal_occurrence_bounds").await else {
        return;
    };
    let first = seed_room_goals(&db.pool).await;
    seed_events(&db.pool, first).await;
    let now = first + TimeDelta::hours(2);
    let state = fixture_state(db.pool.clone(), true);
    let join = first + TimeDelta::minutes(59);
    let observed = join + TimeDelta::minutes(6);
    let mut leave = world("world-leave", "sid1", EVENT_WORLD, observed);
    leave.joined = false;
    enqueue(&state, world("world-join", "sid1", EVENT_WORLD, join)).await;
    enqueue(&state, leave).await;
    let mut scene = world("scene-join", "sid2", "main", join);
    scene.world = false;
    scene.scene_id = Some("active-scene");
    enqueue(&state, scene).await;

    // The room facts retain their initial event evidence after both an edit
    // and content cleanup while the goal worker is behind.
    sqlx::query("UPDATE explore.event SET approved=false WHERE id IN ('world','scene')")
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM content.deployments WHERE entity_id='active-scene'")
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(rooms::process_batch(&state, || now, None).await.unwrap(), 3);
    stays::process_due(&state, || now, &move |room, sid, wallet| {
        (room == "main" && sid == "sid2" && wallet == OWNER).then_some(observed)
    })
    .await
    .unwrap();
    let mut conn = db.pool.acquire().await.unwrap();
    let visits = stays::metadata(&mut conn, OWNER, 1, 1).await.unwrap();
    assert_eq!(visits.len(), 2);
    assert!(
        visits
            .iter()
            .all(|visit| visit.total_time_spent == 60_000 && visit.started_at.is_none())
    );
    assert_eq!(room_steps(&db.pool, "visit-event-and-stay").await, 0);
    assert!(notice_ids(&db.pool).await.is_empty());
}

#[tokio::test]
async fn unknown_venue_remains_retryable_and_does_not_hold_login() {
    let Some(db) = OwnDatabase::create("room_goal_projection_gap").await else {
        return;
    };
    let first = seed_room_goals(&db.pool).await;
    let now = first + TimeDelta::minutes(15);
    let state = fixture_state(db.pool.clone(), true);
    let mirror = "https://events.example.org";
    let mut scene = world("scene-unknown", "sid1", "main", first);
    scene.world = false;
    scene.scene_id = Some("active-scene");
    rooms::store(&state, scene, Some(mirror)).await.unwrap();
    rooms::store(
        &state,
        world("empty-world", "sid2", "wrong.dcl.eth", first),
        Some(mirror),
    )
    .await
    .unwrap();
    assert_eq!(
        rooms::process_batch(&state, || now, Some(mirror))
            .await
            .unwrap(),
        0
    );
    assert_eq!(room_steps(&db.pool, "log-in").await, 1);
    assert_eq!(
        count(
            &db.pool,
            "SELECT count(*) FROM credits.goal_room_events WHERE processed_at IS NULL"
        )
        .await,
        2
    );

    seed_events(&db.pool, first).await;
    // A mirror pass whose scan started before the join cannot establish an
    // empty venue at that time, even if it finished later.
    sqlx::query(
        "INSERT INTO explore.mirror_state
           (mirror, source_url, last_pass_complete, pass_started_at, last_complete_pass_at)
           VALUES ('events',$1,true,$2,$3)",
    )
    .bind(mirror)
    .bind(first - TimeDelta::minutes(1))
    .bind(now)
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        rooms::process_batch(&state, || now + TimeDelta::seconds(30), Some(mirror))
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        count(&db.pool, "SELECT count(*) FROM credits.goal_event_visits").await,
        1
    );
    sqlx::query("UPDATE explore.mirror_state SET pass_started_at=$1 WHERE mirror='events'")
        .bind(first + TimeDelta::minutes(1))
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        rooms::process_batch(&state, || now + TimeDelta::seconds(60), Some(mirror))
            .await
            .unwrap(),
        1
    );
    assert_eq!(room_steps(&db.pool, "log-in").await, 1);
    assert_eq!(
        count(
            &db.pool,
            "SELECT count(*) FROM credits.goal_room_events WHERE processed_at IS NULL"
        )
        .await,
        0
    );
}

#[tokio::test]
async fn stays_cannot_complete_after_the_season_or_week_closes() {
    for end_season in [true, false] {
        let Some(db) = OwnDatabase::create("room_goal_rollover").await else {
            return;
        };
        let first = seed_room_goals(&db.pool).await;
        let end = (first.date() + TimeDelta::days(7))
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let join = end - TimeDelta::minutes(2);
        if end_season {
            sqlx::query("UPDATE credits.seasons SET end_date=$1 WHERE id=1")
                .bind(end)
                .execute(&db.pool)
                .await
                .unwrap();
        }
        sqlx::query(
            "UPDATE credits.season_goals SET total_steps=1 WHERE goal_id='visit-event-and-stay'",
        )
        .execute(&db.pool)
        .await
        .unwrap();
        seed_events(&db.pool, join).await;
        let state = fixture_state(db.pool.clone(), true);
        enqueue(&state, world("opened", "sid1", EVENT_WORLD, join)).await;
        assert_eq!(
            rooms::process_batch(&state, || join, None).await.unwrap(),
            1
        );
        let due: NaiveDateTime = sqlx::query_scalar("SELECT due_at FROM credits.goal_event_visits")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(due, end, "the timer is bounded by the assignment");

        let observed = join + TimeDelta::minutes(5);
        let mut leave = world("leave-after-window", "sid2", EVENT_WORLD, observed);
        leave.joined = false;
        enqueue(&state, leave).await;
        assert_eq!(
            rooms::process_batch(&state, || observed, None)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            count(
                &db.pool,
                "SELECT count(*) FROM credits.goal_room_events WHERE NOT joined"
            )
            .await,
            1,
            "a leave can be persisted ahead of its queued join even after season end"
        );
        enqueue(&state, world("earlier-join", "sid2", EVENT_WORLD, join)).await;
        assert_eq!(
            rooms::process_batch(&state, || observed, None)
                .await
                .unwrap(),
            1
        );
        stays::process_due(&state, || observed, &move |_, _, _| Some(observed))
            .await
            .unwrap();
        assert_eq!(room_steps(&db.pool, "visit-event-and-stay").await, 0);
        assert!(notice_ids(&db.pool).await.is_empty());
        assert_eq!(
            count(&db.pool, "SELECT count(*) FROM credits.goal_grants").await,
            0
        );
    }
}

#[tokio::test]
async fn a_stale_event_from_the_previous_mirror_cannot_authorize_a_stay() {
    let Some(db) = OwnDatabase::create("room_goal_mirror_switch").await else {
        return;
    };
    let first = seed_room_goals(&db.pool).await;
    seed_events(&db.pool, first).await;
    let mirror_a = "https://events-a.example.org";
    let mirror_b = "https://events-b.example.org";
    let now = first + TimeDelta::minutes(20);
    sqlx::query("UPDATE explore.event SET origin='upstream', fetched_at=$1 WHERE id='world'")
        .bind(first - TimeDelta::minutes(2))
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO explore.mirror_state
           (mirror, source_url, last_pass_complete, pass_started_at, last_complete_pass_at)
           VALUES ('events',$1,true,$2,$3)",
    )
    .bind(mirror_a)
    .bind(first - TimeDelta::minutes(3))
    .bind(first - TimeDelta::minutes(1))
    .execute(&db.pool)
    .await
    .unwrap();
    let state = fixture_state(db.pool.clone(), true);
    rooms::store(
        &state,
        world("stale-world", "sid1", EVENT_WORLD, first),
        Some(mirror_b),
    )
    .await
    .unwrap();
    assert_eq!(
        rooms::process_batch(&state, || now, Some(mirror_b))
            .await
            .unwrap(),
        0
    );
    assert_eq!(room_steps(&db.pool, "log-in").await, 1);
    assert_eq!(
        count(&db.pool, "SELECT count(*) FROM credits.goal_event_visits").await,
        0
    );

    // A completed pass on B cannot prove any omitted row from A, even if it
    // is retained by Explore's sweep protection until the next confirmation.
    let pass = first + TimeDelta::minutes(1);
    sqlx::query("UPDATE explore.mirror_state SET source_url=$1,pass_started_at=$2,last_complete_pass_at=$3 WHERE mirror='events'")
        .bind(mirror_b).bind(pass).bind(now).execute(&db.pool).await.unwrap();
    let processed = rooms::process_batch(&state, || now + TimeDelta::seconds(30), Some(mirror_b))
        .await
        .unwrap();
    assert_eq!(processed, 1);
    sqlx::query("UPDATE explore.event SET raw=raw || jsonb_build_object('deleted_by_admin',true) WHERE id='world'")
        .execute(&db.pool).await.unwrap();
    assert_eq!(
        count(&db.pool, "SELECT count(*) FROM credits.goal_event_visits").await,
        0
    );
    assert_eq!(
        stays::process_due(&state, || now, &move |_, _, _| Some(now))
            .await
            .unwrap(),
        0
    );
    assert!(notice_ids(&db.pool).await.is_empty());
    assert_eq!(room_steps(&db.pool, "visit-event-and-stay").await, 0);

    // A subsequent fact can use a row that B has actually observed in the
    // published pass. It cannot revive the earlier rejected session.
    let joined = first + TimeDelta::seconds(45);
    sqlx::query(
        "UPDATE explore.event SET raw=raw-'deleted_by_admin',fetched_at=$1 WHERE id='world'",
    )
    .bind(pass + TimeDelta::seconds(1))
    .execute(&db.pool)
    .await
    .unwrap();
    rooms::store(
        &state,
        world("proved-world", "sid2", EVENT_WORLD, joined),
        Some(mirror_b),
    )
    .await
    .unwrap();
    assert_eq!(
        rooms::process_batch(&state, || now, Some(mirror_b))
            .await
            .unwrap(),
        1
    );
    let session: String =
        sqlx::query_scalar("SELECT participant_sid FROM credits.goal_event_visits")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(session, "sid2");
}

#[tokio::test]
async fn a_progress_read_keeps_balance_and_goal_status_in_one_snapshot_during_claim() {
    let Some(db) = OwnDatabase::create("room_goal_progress_snapshot").await else {
        return;
    };
    let first = seed_room_goals(&db.pool).await;
    sqlx::query("UPDATE credits.season_goals SET total_steps=1 WHERE goal_id='log-in'")
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE social.subscriptions SET email='login@example.org' WHERE address=$1")
        .bind(OWNER)
        .execute(&db.pool)
        .await
        .unwrap();
    let state = fixture_state(db.pool.clone(), true);
    reads::enroll_at(&state, OWNER, first).await.unwrap();

    let mut claim = db.pool.begin().await.unwrap();
    wallet_lock(&mut claim, OWNER).await.unwrap();
    sqlx::query("UPDATE credits.goal_grants SET claimed_at=$1 WHERE address=$2")
        .bind(first)
        .bind(OWNER)
        .execute(&mut *claim)
        .await
        .unwrap();
    let before = tokio::time::timeout(
        Duration::from_secs(5),
        reads::progress_at(&state, OWNER, first),
    )
    .await
    .expect("the progress snapshot does not wait for the wallet lock")
    .unwrap();
    assert!(
        !before
            .goals
            .iter()
            .find(|g| g.id == "log-in")
            .unwrap()
            .is_claimed
    );
    assert_eq!(before.credits.available, 0.0);

    claim.commit().await.unwrap();
    let report = reads::progress_at(&state, OWNER, first).await.unwrap();
    assert!(
        report
            .goals
            .iter()
            .find(|g| g.id == "log-in")
            .unwrap()
            .is_claimed
    );
    assert_eq!(report.credits.available, 1.0);
}

#[tokio::test]
async fn opting_out_after_a_join_prevents_a_due_timer_from_granting() {
    let Some(db) = OwnDatabase::create("room_goal_active_opt_out").await else {
        return;
    };
    let first = seed_room_goals(&db.pool).await;
    seed_events(&db.pool, first).await;
    sqlx::query(
        "UPDATE credits.season_goals SET total_steps=1 WHERE goal_id='visit-event-and-stay'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let state = fixture_state(db.pool.clone(), true);
    enqueue(&state, world("arrival", "sid1", EVENT_WORLD, first)).await;
    assert_eq!(
        rooms::process_batch(&state, || first, None).await.unwrap(),
        1
    );
    opt_out(&state, OWNER).await.unwrap();

    let checked_at = first + TimeDelta::minutes(5);
    stays::process_due(&state, || checked_at, &move |room, sid, wallet| {
        (room == EVENT_WORLD && sid == "sid1" && wallet == OWNER).then_some(checked_at)
    })
    .await
    .unwrap();
    assert_eq!(room_steps(&db.pool, "visit-event-and-stay").await, 0);
    assert_eq!(
        count(&db.pool, "SELECT count(*) FROM credits.goal_grants").await,
        0
    );
    assert!(notice_ids(&db.pool).await.is_empty());
}
