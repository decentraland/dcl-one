//! The one revocation entry point: a permission or block edit made outside
//! the gatekeeper (worlds' core maps its `Revocation` onto
//! [`RoomRevocation`]) evicts the affected participants from the running
//! LiveKit rooms through [`CommsState::revoke`], detached from the caller (an
//! SFU outage is logged under `comms-revocation`, never surfaced to the
//! edit). Untested against a real SFU until one is dialled; the dev
//! placeholders skip it.

use std::sync::Arc;

use crate::Address;
use crate::world_identity::WorldName;

use super::CommsState;
use super::admission::scene::{kick, live_world_rooms, readmit_world_rooms_in_inventory};

const WORLD_BATCH: usize = 16;

/// What an edit revoked, in the rooms' terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoomRevocation {
    /// One wallet lost its admission to the world: it leaves the world's
    /// live rooms.
    WorldAddress { world: WorldName, address: Address },
    /// The world's access setting changed: every participant it no longer
    /// admits leaves the world's live rooms (the owner stays).
    WorldAccess { world: WorldName },
    /// Re-evaluate current policy even when no durable intent could be stored.
    /// No cutoff is invented; existing shared-secret admissions remain valid.
    WorldAccessRecheck { world: WorldName },
    /// The wallet is blocked everywhere: it leaves every live room.
    Everywhere { address: Address },
}

impl CommsState {
    /// Enforces `revocation` in the running LiveKit rooms on a spawned task;
    /// returns at once. LiveKit not configured evicts nothing.
    pub fn revoke(self: &Arc<Self>, revocation: RoomRevocation) {
        if !self.livekit.configured {
            tracing::debug!(
                ?revocation,
                "LiveKit not configured; no participant evicted"
            );
            return;
        }
        let state = self.clone();
        tokio::spawn(async move {
            match &revocation {
                RoomRevocation::WorldAddress { world, address } => {
                    let rooms = match live_world_rooms(&state, world).await {
                        Ok(rooms) => rooms,
                        Err(error) => {
                            tracing::warn!(%error, %world, "revocation inventory incomplete; no completion claimed");
                            return;
                        }
                    };
                    kick(&state, &rooms, address).await;
                }
                RoomRevocation::WorldAccess { world } => {
                    // The worlds edit already committed its unique revision and
                    // cutoff. A delayed advisory wakeup must not enqueue another
                    // revision/cutoff and revoke sessions admitted since the edit.
                    let rooms = match state.room_service().list_rooms().await {
                        Ok(rooms) => rooms,
                        Err(error) => {
                            tracing::warn!(%error,%world,"world access inventory unavailable; durable intent retained");
                            return;
                        }
                    };
                    if let Err(error) =
                        retry_access_rechecks(&state, Some(world.key()), &rooms).await
                    {
                        tracing::warn!(%error,%world,"world access enforcement retained for maintenance retry");
                    }
                }

                RoomRevocation::WorldAccessRecheck { world } => {
                    let rooms = match state.room_service().list_rooms().await {
                        Ok(rooms) => rooms,
                        Err(error) => {
                            tracing::warn!(%error, %world, "advisory world access inventory unavailable; no durable intent accepted");
                            return;
                        }
                    };
                    if let Err(error) =
                        readmit_world_rooms_in_inventory(&state, world, None, &rooms).await
                    {
                        tracing::warn!(%error, %world, "advisory world access recheck incomplete; no durable intent accepted");
                    }
                }

                RoomRevocation::Everywhere { address } => {
                    if let Err(error) = super::removal::operator_block(&state.pool, address).await {
                        tracing::error!(%error,%address,"operator-block removal intent was not accepted; worlds write is already committed");
                        return;
                    }
                }
            }
            tracing::info!(
                job = "comms-revocation",
                ?revocation,
                "LiveKit revocation dispatched"
            );
        });
    }
}

/// Rotate due work before external calls; failed/undecided participants retain
/// their obligation across restarts. A newer edit cannot be cleared by this pass.
async fn retry_access_rechecks(
    state: &CommsState,
    only_world: Option<&str>,
    rooms: &[String],
) -> anyhow::Result<()> {
    if !state.livekit.configured {
        return Ok(());
    }
    let rows = claim_rechecks(&state.pool, only_world).await?;
    for (raw, revision, changed_at, claimed_schedule) in rows {
        let Ok(world) = WorldName::parse(&raw) else {
            continue;
        };
        match readmit_world_rooms_in_inventory(state, &world, Some(changed_at), rooms).await {
            // Retain the cutoff for live sweeps and reconnects. A newer revision
            // or same-revision community wakeup must not be completed by this pass.
            Ok(()) => match complete_recheck(&state.pool, &raw, revision, claimed_schedule).await {
                Ok(true) => {}
                Ok(false) => tracing::debug!(%world, %revision, %claimed_schedule,
                        "world access completion skipped; claim changed or expired"),
                Err(error) => {
                    tracing::warn!(%error,%world,"world access completion failed; recheck retained");
                }
            },
            Err(error) => tracing::warn!(%error,%world,"world access recheck retained"),
        }
    }
    Ok(())
}

type ClaimedRecheck = (
    String,
    uuid::Uuid,
    chrono::NaiveDateTime,
    chrono::NaiveDateTime,
);

async fn claim_rechecks(
    pool: &sqlx::PgPool,
    only_world: Option<&str>,
) -> Result<Vec<ClaimedRecheck>, sqlx::Error> {
    sqlx::query_as("WITH due AS (SELECT world_name FROM social.world_access_rechecks WHERE next_attempt_at<=timezone('UTC',now()) AND ($1::text IS NULL OR world_name=$1) ORDER BY next_attempt_at,world_name LIMIT $2 FOR UPDATE SKIP LOCKED) UPDATE social.world_access_rechecks q SET next_attempt_at=timezone('UTC',now())+interval '1 minute' FROM due WHERE q.world_name=due.world_name RETURNING q.world_name,q.revision,q.changed_at,q.next_attempt_at")
        .bind(only_world).bind(WORLD_BATCH as i64).fetch_all(pool).await
}

async fn complete_recheck(
    pool: &sqlx::PgPool,
    world: &str,
    revision: uuid::Uuid,
    claimed_schedule: chrono::NaiveDateTime,
) -> Result<bool, sqlx::Error> {
    // Lock first: a clock predicate on a plain UPDATE can be evaluated before
    // waiting for another holder. Even an unchanged row must still have a live
    // lease when the lock is acquired, including after a community LEAST wakeup.
    Ok(sqlx::query("WITH claimed AS MATERIALIZED (SELECT world_name,next_attempt_at FROM social.world_access_rechecks WHERE world_name=$1 AND revision=$2 AND next_attempt_at=$3 FOR UPDATE) UPDATE social.world_access_rechecks q SET next_attempt_at='infinity' FROM claimed WHERE q.world_name=claimed.world_name AND claimed.next_attempt_at>timezone('UTC',clock_timestamp())")
        .bind(world).bind(revision).bind(claimed_schedule).execute(pool).await?.rows_affected() == 1)
}

/// Recheck live worlds even when an operator/name-deny change did not emit a
/// revocation, and retry undecided allow-list reads on the next maintenance tick.
/// Existing shared-secret admissions are preserved unless an access-change
/// revocation explicitly requires them to mint again.
pub(crate) async fn sweep(state: &CommsState) -> anyhow::Result<()> {
    // Retain durable obligations while disabled, without making doomed SFU
    // calls or warning every tick. Enabling LiveKit resumes those obligations.
    if !state.livekit.configured {
        return Ok(());
    }
    let rooms = state.room_service().list_rooms().await?;
    retry_access_rechecks(state, None, &rooms).await?;
    let live_worlds = world_batch(state, &rooms);
    let names: Vec<&str> = live_worlds.iter().map(|world| world.key()).collect();
    let cutoffs: Vec<(String, chrono::NaiveDateTime)> = sqlx::query_as(
        "SELECT world_name,changed_at FROM social.world_access_rechecks WHERE world_name=ANY($1::text[])")
        .bind(&names).fetch_all(&state.pool).await?;
    // Live worlds do not wait behind inactive historical cutoffs in the due queue.
    for world in live_worlds {
        let cutoff = cutoffs
            .iter()
            .find(|(name, _)| name == world.key())
            .map(|(_, cutoff)| *cutoff);
        if let Err(error) = readmit_world_rooms_in_inventory(state, &world, cutoff, &rooms).await {
            tracing::warn!(%error,%world,"world access maintenance retained for next tick");
        }
    }
    Ok(())
}

fn world_batch(state: &CommsState, rooms: &[String]) -> Vec<WorldName> {
    let mut worlds = std::collections::BTreeMap::new();
    for room in rooms {
        if let Some((world, _)) = crate::rooms::decode_world_scene_room(room) {
            worlds.insert(world.key().to_owned(), world);
        } else if let Some(raw) = room.strip_prefix(crate::rooms::WORLD_ROOM_PREFIX)
            && let Ok(world) = WorldName::parse(raw)
        {
            worlds.insert(world.key().to_owned(), world);
        }
    }
    let after = state.world_access_cursor.lock().unwrap().clone();
    let mut selected: Vec<_> = worlds
        .values()
        .filter(|world| after.as_deref().is_none_or(|after| world.key() > after))
        .take(WORLD_BATCH)
        .cloned()
        .collect();
    if selected.len() < WORLD_BATCH && after.is_some() {
        let remaining = WORLD_BATCH - selected.len();
        selected.extend(
            worlds
                .values()
                .filter(|world| world.key() <= after.as_deref().unwrap())
                .take(remaining)
                .cloned(),
        );
    }
    *state.world_access_cursor.lock().unwrap() =
        selected.last().map(|world| world.key().to_owned());
    selected
}

#[cfg(test)]
mod completion_tests {
    use super::*;

    async fn queue(pool: &sqlx::PgPool, world: &str) -> uuid::Uuid {
        let revision = uuid::Uuid::new_v4();
        sqlx::query("INSERT INTO social.world_access_rechecks(world_name,revision) VALUES($1,$2)")
            .bind(world)
            .bind(revision)
            .execute(pool)
            .await
            .unwrap();
        revision
    }

    async fn wake(pool: &sqlx::PgPool, world: &str) {
        // Same-revision community enqueue, including after a completed revision.
        sqlx::query("UPDATE social.world_access_rechecks SET next_attempt_at=LEAST(next_attempt_at,timezone('UTC',clock_timestamp())) WHERE world_name=$1")
            .bind(world).execute(pool).await.unwrap();
    }

    #[tokio::test]
    async fn completion_retains_cutoff_and_a_community_wakeup_reopens_it() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let world = format!("complete-{}.dcl.eth", uuid::Uuid::new_v4());
        queue(&pool, &world).await;
        let rows = claim_rechecks(&pool, Some(&world)).await.unwrap();
        let (name, revision, cutoff, schedule) = &rows[0];
        assert!(
            complete_recheck(&pool, name, *revision, *schedule)
                .await
                .unwrap()
        );
        assert!(
            claim_rechecks(&pool, Some(&world))
                .await
                .unwrap()
                .is_empty()
        );
        // This is the live-sweep read; it never decodes the infinity schedule.
        let retained: Vec<(String, chrono::NaiveDateTime)> = sqlx::query_as("SELECT world_name,changed_at FROM social.world_access_rechecks WHERE world_name=ANY($1::text[])")
            .bind(vec![world.clone()]).fetch_all(&pool).await.unwrap();
        assert_eq!(retained, vec![(world.clone(), *cutoff)]);
        wake(&pool, &world).await;
        let next = claim_rechecks(&pool, Some(&world)).await.unwrap();
        assert_eq!(next.len(), 1);
        assert_eq!((next[0].1, next[0].2), (*revision, *cutoff));
    }

    #[tokio::test]
    async fn stale_completion_cannot_consume_a_community_wakeup_or_new_claim() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let world = format!("requeue-{}.dcl.eth", uuid::Uuid::new_v4());
        queue(&pool, &world).await;
        let old = claim_rechecks(&pool, Some(&world)).await.unwrap().remove(0);
        wake(&pool, &world).await;
        assert!(!complete_recheck(&pool, &world, old.1, old.3).await.unwrap());
        let newer = claim_rechecks(&pool, Some(&world)).await.unwrap().remove(0);
        assert!(!complete_recheck(&pool, &world, old.1, old.3).await.unwrap());
        assert!(
            complete_recheck(&pool, &world, newer.1, newer.3)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn expired_unchanged_claim_survives_a_community_wakeup() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let world = format!("expired-{}.dcl.eth", uuid::Uuid::new_v4());
        let revision = queue(&pool, &world).await;
        // Capture a short lease, then let it expire without changing its schedule.
        let schedule: chrono::NaiveDateTime = sqlx::query_scalar("UPDATE social.world_access_rechecks SET next_attempt_at=timezone('UTC',clock_timestamp())+interval '50 milliseconds' WHERE world_name=$1 RETURNING next_attempt_at")
            .bind(&world).fetch_one(&pool).await.unwrap();
        sqlx::query("SELECT pg_sleep(GREATEST(0,extract(epoch FROM ($1::timestamp-timezone('UTC',clock_timestamp())))) + 0.01)")
            .bind(schedule).execute(&pool).await.unwrap();
        wake(&pool, &world).await;
        let unchanged: chrono::NaiveDateTime = sqlx::query_scalar(
            "SELECT next_attempt_at FROM social.world_access_rechecks WHERE world_name=$1",
        )
        .bind(&world)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(schedule, unchanged, "LEAST does not move an expired lease");
        assert!(
            !complete_recheck(&pool, &world, revision, schedule)
                .await
                .unwrap()
        );
        assert_eq!(claim_rechecks(&pool, Some(&world)).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn completion_checks_lease_after_waiting_for_an_unchanged_locked_row() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let world = format!("locked-{}.dcl.eth", uuid::Uuid::new_v4());
        let revision = queue(&pool, &world).await;
        let schedule: chrono::NaiveDateTime = sqlx::query_scalar("UPDATE social.world_access_rechecks SET next_attempt_at=timezone('UTC',clock_timestamp())+interval '10 seconds' WHERE world_name=$1 RETURNING next_attempt_at")
            .bind(&world).fetch_one(&pool).await.unwrap();
        let mut blocker = pool.begin().await.unwrap();
        sqlx::query(
            "SELECT world_name FROM social.world_access_rechecks WHERE world_name=$1 FOR UPDATE",
        )
        .bind(&world)
        .execute(&mut *blocker)
        .await
        .unwrap();
        let completion = tokio::spawn({
            let pool = pool.clone();
            let world = world.clone();
            async move { complete_recheck(&pool, &world, revision, schedule).await }
        });
        // Observe the actual lock wait, then keep the unchanged row locked until
        // PostgreSQL's clock says the exact claimed schedule has expired.
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let waiting: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE 'WITH claimed AS MATERIALIZED%')")
                    .fetch_one(&pool).await.unwrap();
                if waiting {
                    let live: bool = sqlx::query_scalar("SELECT $1::timestamp > timezone('UTC',clock_timestamp())")
                        .bind(schedule).fetch_one(&pool).await.unwrap();
                    assert!(live, "completion must start waiting before the lease expires");
                    break;
                }
                tokio::task::yield_now().await;
            }
        }).await.unwrap();
        sqlx::query("SELECT pg_sleep(GREATEST(0,extract(epoch FROM ($1::timestamp-timezone('UTC',clock_timestamp())))) + 0.05)")
            .bind(schedule).execute(&mut *blocker).await.unwrap();
        blocker.commit().await.unwrap();
        assert!(!completion.await.unwrap().unwrap());
        assert_eq!(claim_rechecks(&pool, Some(&world)).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn new_edit_and_expired_claim_each_survive_old_completion() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let world = format!("edit-{}.dcl.eth", uuid::Uuid::new_v4());
        queue(&pool, &world).await;
        let old = claim_rechecks(&pool, Some(&world)).await.unwrap().remove(0);
        let revision = uuid::Uuid::new_v4();
        sqlx::query("UPDATE social.world_access_rechecks SET revision=$2,next_attempt_at=timezone('UTC',now()) WHERE world_name=$1")
            .bind(&world).bind(revision).execute(&pool).await.unwrap();
        assert!(!complete_recheck(&pool, &world, old.1, old.3).await.unwrap());
        let claimed = claim_rechecks(&pool, Some(&world)).await.unwrap().remove(0);
        // No completion on failed enforcement; after the lease expires a restart can retry.
        sqlx::query("UPDATE social.world_access_rechecks SET next_attempt_at=timezone('UTC',now())-interval '1 second' WHERE world_name=$1")
            .bind(&world).execute(&pool).await.unwrap();
        let restarted = claim_rechecks(&pool, Some(&world)).await.unwrap().remove(0);
        assert_eq!(restarted.1, revision);
        assert!(
            !complete_recheck(&pool, &world, claimed.1, claimed.3)
                .await
                .unwrap()
        );
        assert!(
            complete_recheck(&pool, &world, restarted.1, restarted.3)
                .await
                .unwrap()
        );
    }
}

#[cfg(test)]
mod recheck_regressions {
    use super::*;
    use crate::AuthorityNotEstablished;
    use crate::interfaces::test_support::{StubDirectory, StubPolicy, StubScenes};
    use std::sync::atomic::{AtomicBool, Ordering};

    #[tokio::test]
    async fn sweep_retries_failed_policy_and_continues_after_completion_failure() {
        let Some(base) = crate::db::scratch::pool(2).await else {
            return;
        };
        // The trigger belongs to a private database, never the shared scratch
        // schema used by concurrently running tests.
        let name = format!(
            "catalyst_test_lib_{}_{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        );
        let admin = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_with(base.connect_options().as_ref().clone().database("postgres"))
            .await
            .unwrap();
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&admin)
            .await
            .unwrap();
        use futures::FutureExt;
        let mut server = None;
        let outcome = std::panic::AssertUnwindSafe(async {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(4)
                .connect_with(base.connect_options().as_ref().clone().database(&name))
                .await
                .unwrap();
            crate::db::apply_schema(&pool).await.unwrap();
            sqlx::query("INSERT INTO social.world_access_rechecks(world_name,next_attempt_at) VALUES ('a-fault.dcl.eth',timestamp '2000-01-01'),('b-later.dcl.eth',timestamp '2000-01-02')")
                .execute(&pool).await.unwrap();
            sqlx::raw_sql("CREATE FUNCTION social.fail_test_completion() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.world_name='a-fault.dcl.eth' AND NEW.next_attempt_at='infinity'::timestamp THEN RAISE EXCEPTION 'injected completion failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER fail_test_completion BEFORE UPDATE ON social.world_access_rechecks FOR EACH ROW EXECUTE FUNCTION social.fail_test_completion();")
                .execute(&pool).await.unwrap();
            let calls = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
            let app = axum::Router::new().route("/twirp/livekit.RoomService/{method}", axum::routing::post({
                let calls = calls.clone();
                move |axum::extract::Path(method): axum::extract::Path<String>, axum::Json(body): axum::Json<serde_json::Value>| {
                    let calls = calls.clone();
                    async move {
                        if method == "ListRooms" {
                            return axum::Json(serde_json::json!({"rooms":[{"name":"world-a-fault.dcl.eth"},{"name":"world-b-later.dcl.eth"},{"name":"world-c-live.dcl.eth"}]}));
                        }
                        assert_eq!(method, "ListParticipants");
                        calls.lock().unwrap().push(body["room"].as_str().unwrap().to_owned());
                        axum::Json(serde_json::json!({"participants":[]}))
                    }
                }
            }));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            server = Some(tokio::spawn(async move {
                axum::serve(listener, app).await.unwrap();
            }));
            let mut state = CommsState::new(
                pool.clone(),
                crate::http::Outbound::for_tests(),
                super::super::Settings::for_tests(),
                None,
                None,
                Arc::new(StubPolicy {
                    access_type: Err(AuthorityNotEstablished::store_unavailable(
                        "worlds.worlds",
                        &sqlx::Error::PoolTimedOut,
                    )),
                    ..Default::default()
                }),
                Arc::new(StubScenes::default()),
                Arc::new(crate::presence::Presence::new()),
                Arc::new(StubDirectory::default()),
            );
            state.livekit.configured = true;
            state.livekit.api_url = url;
            sweep(&state).await.unwrap();
            let pending: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM social.world_access_rechecks WHERE isfinite(next_attempt_at)",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(pending, 2, "failed policy must retain retryable schedules");
            assert!(calls.lock().unwrap().is_empty());
            // Retry the actual durable obligations on the next maintenance tick.
            sqlx::query(
                "UPDATE social.world_access_rechecks SET next_attempt_at=timestamp '2000-01-01'",
            )
            .execute(&pool)
            .await
            .unwrap();
            state.policy = Arc::new(StubPolicy::default());
            sweep(&state).await.unwrap();
            let completed: Vec<(String, bool)> = sqlx::query_as("SELECT world_name,NOT isfinite(next_attempt_at) FROM social.world_access_rechecks ORDER BY world_name")
                .fetch_all(&pool).await.unwrap();
            assert_eq!(
                completed,
                vec![
                    ("a-fault.dcl.eth".into(), false),
                    ("b-later.dcl.eth".into(), true)
                ]
            );
            {
                let calls = calls.lock().unwrap();
                assert_eq!(
                    calls
                        .iter()
                        .filter(|room| *room == "world-a-fault.dcl.eth")
                        .count(),
                    2
                );
                assert_eq!(
                    calls
                        .iter()
                        .filter(|room| *room == "world-b-later.dcl.eth")
                        .count(),
                    2,
                    "later claimed world and its live sweep both run"
                );
                assert_eq!(
                    calls
                        .iter()
                        .filter(|room| *room == "world-c-live.dcl.eth")
                        .count(),
                    1,
                    "live-only world runs in the same tick"
                );
            }
            drop(state);
            pool.close().await;
        })
        .catch_unwind()
        .await;
        if let Some(server) = server {
            server.abort();
            let _ = server.await;
        }
        // Close this private database's sessions even after setup or assertions panic.
        let cleanup = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{name}\" WITH (FORCE)"
        )))
        .execute(&admin)
        .await;
        admin.close().await;
        if let Err(panic) = outcome {
            if let Err(error) = &cleanup {
                eprintln!("cleaning up private recheck database {name}: {error}");
            }
            std::panic::resume_unwind(panic);
        }
        cleanup.unwrap();
    }

    #[tokio::test]
    async fn undecided_access_rechecks_persist_and_a_newer_edit_survives_an_older_pass() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let world = format!("recheck-{}.dcl.eth", uuid::Uuid::new_v4());
        sqlx::query("INSERT INTO social.world_access_rechecks(world_name) VALUES($1)")
            .bind(&world)
            .execute(&pool)
            .await
            .unwrap();
        let newer = Arc::new(AtomicBool::new(false));
        let fixture_pool = pool.clone();
        let fixture_world = world.clone();
        let fixture_newer = newer.clone();
        let app=axum::Router::new().route("/twirp/livekit.RoomService/{method}",axum::routing::post(move |axum::extract::Path(method): axum::extract::Path<String>| {
            let (pool,world,newer)=(fixture_pool.clone(),fixture_world.clone(),fixture_newer.clone());
            async move {
                if method == "ListRooms" {
                    return axum::Json(serde_json::json!({"rooms":[{"name":format!("world-{world}")}]}));
                }
                // ListParticipants runs after the retry captured its revision.
                if newer.swap(false,Ordering::SeqCst) {
                    sqlx::query("UPDATE social.world_access_rechecks SET revision=gen_random_uuid() WHERE world_name=$1").bind(world).execute(&pool).await.unwrap();
                }
                axum::Json(serde_json::json!({"participants":[]}))
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let mut state = CommsState::new(
            pool.clone(),
            crate::http::Outbound::for_tests(),
            super::super::Settings::for_tests(),
            None,
            None,
            Arc::new(StubPolicy {
                protected: Err(AuthorityNotEstablished::store_unavailable(
                    "worlds.worlds",
                    &sqlx::Error::PoolTimedOut,
                )),
                ..Default::default()
            }),
            Arc::new(StubScenes::default()),
            Arc::new(crate::presence::Presence::new()),
            Arc::new(StubDirectory::default()),
        );
        state.livekit.configured = true;
        state.livekit.api_url = url;
        let rooms = state.room_service().list_rooms().await.unwrap();
        retry_access_rechecks(&state, Some(&world), &rooms)
            .await
            .unwrap();
        let initial = sqlx::query_scalar::<_, uuid::Uuid>(
            "SELECT revision FROM social.world_access_rechecks WHERE world_name=$1",
        )
        .bind(&world)
        .fetch_one(&pool)
        .await
        .unwrap();
        state.policy = Arc::new(StubPolicy::default());
        newer.store(true, Ordering::SeqCst);
        for retained in [true, true] {
            sqlx::query("UPDATE social.world_access_rechecks SET next_attempt_at=timezone('UTC',now()) WHERE world_name=$1").bind(&world).execute(&pool).await.unwrap();
            let rooms = state.room_service().list_rooms().await.unwrap();
            retry_access_rechecks(&state, Some(&world), &rooms)
                .await
                .unwrap();
            let revision = sqlx::query_scalar::<_, uuid::Uuid>(
                "SELECT revision FROM social.world_access_rechecks WHERE world_name=$1",
            )
            .bind(&world)
            .fetch_optional(&pool)
            .await
            .unwrap();
            assert_eq!(revision.is_some(), retained);
            if let Some(revision) = revision {
                assert_ne!(revision, initial);
            }
        }
        sqlx::query("DELETE FROM social.world_access_rechecks WHERE world_name=$1")
            .bind(&world)
            .execute(&pool)
            .await
            .unwrap();
        // ABA: an old completion cannot remove a newly reinserted obligation.
        let old = sqlx::query_scalar::<_, uuid::Uuid>(
            "INSERT INTO social.world_access_rechecks(world_name) VALUES($1) RETURNING revision",
        )
        .bind(&world)
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query("DELETE FROM social.world_access_rechecks WHERE world_name=$1")
            .bind(&world)
            .execute(&pool)
            .await
            .unwrap();
        let fresh = sqlx::query_scalar::<_, uuid::Uuid>(
            "INSERT INTO social.world_access_rechecks(world_name) VALUES($1) RETURNING revision",
        )
        .bind(&world)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_ne!(old, fresh);
        let deleted = sqlx::query(
            "DELETE FROM social.world_access_rechecks WHERE world_name=$1 AND revision=$2",
        )
        .bind(&world)
        .bind(old)
        .execute(&pool)
        .await
        .unwrap()
        .rows_affected();
        assert_eq!(deleted, 0);
        server.abort();
    }
}

#[cfg(test)]
mod roster_regressions {
    use super::*;
    use crate::interfaces::test_support::{StubDirectory, StubPolicy, StubScenes};
    #[tokio::test]
    async fn disabled_livekit_retains_work_and_rotation_is_bounded() {
        let mut state = CommsState::new(
            sqlx::PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap(),
            crate::http::Outbound::for_tests(),
            super::super::Settings::for_tests(),
            None,
            None,
            Arc::new(StubPolicy::default()),
            Arc::new(StubScenes::default()),
            Arc::new(crate::presence::Presence::new()),
            Arc::new(StubDirectory::default()),
        );
        state.livekit.configured = false;
        let rooms: Vec<_> = (0..WORLD_BATCH + 4)
            .map(|n| format!("world-world{n:02}.dcl.eth"))
            .collect();
        let first = world_batch(&state, &rooms);
        let second = world_batch(&state, &rooms);
        assert_eq!(first.len(), WORLD_BATCH);
        assert_eq!(second.len(), WORLD_BATCH);
        assert!(second.iter().take(4).all(|world| !first.contains(world)));
        sweep(&state).await.unwrap();
        retry_access_rechecks(&state, None, &rooms).await.unwrap();
    }

    #[tokio::test]
    async fn a_roster_fault_is_undecided_and_cannot_complete_a_secret_change() {
        let fixture=axum::Router::new()
            .route("/twirp/livekit.RoomService/ListRooms",axum::routing::post(||async{axum::Json(serde_json::json!({"rooms":[{"name":"w1:d:mixed.dcl.ethSceneA","sid":"RM_a","metadata":"{}"}]}))}))
            .route("/twirp/livekit.RoomService/ListParticipants",axum::routing::post(||async{axum::http::StatusCode::SERVICE_UNAVAILABLE}));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, fixture).await.unwrap();
        });
        let mut state = CommsState::new(
            sqlx::PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap(),
            crate::http::Outbound::for_tests(),
            super::super::Settings::for_tests(),
            None,
            None,
            Arc::new(StubPolicy::default()),
            Arc::new(StubScenes::default()),
            Arc::new(crate::presence::Presence::new()),
            Arc::new(StubDirectory::default()),
        );
        state.livekit.configured = true;
        state.livekit.api_url = url;
        assert!(
            readmit_world_rooms_in_inventory(
                &state,
                &WorldName::parse("mixed.dcl.eth").unwrap(),
                Some(chrono::Utc::now().naive_utc()),
                &["w1:d:mixed.dcl.ethSceneA".into()]
            )
            .await
            .is_err()
        );
        server.abort();
    }

    /// A failed access-setting read retains the pass before any roster call.
    /// Nobody is evicted on an undecided setting.
    #[tokio::test]
    async fn a_failed_access_read_retains_the_pass_and_evicts_nobody() {
        let calls = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let fixture = axum::Router::new().route(
            "/twirp/livekit.RoomService/{method}",
            axum::routing::post({
                let calls = calls.clone();
                move |axum::extract::Path(method): axum::extract::Path<String>| {
                    let calls = calls.clone();
                    async move {
                        calls.lock().unwrap().push(method.clone());
                        axum::Json(match method.as_str() {
                            "ListRooms" => serde_json::json!({"rooms":[{"name":"world-faulted.dcl.eth"}]}),
                            "ListParticipants" => serde_json::json!({"participants":[{"identity":format!("0x{:040x}", 1),"sid":"PA_1"}]}),
                            _ => serde_json::json!({}),
                        })
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, fixture).await.unwrap();
        });
        let mut state = CommsState::new(
            sqlx::PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap(),
            crate::http::Outbound::for_tests(),
            super::super::Settings::for_tests(),
            None,
            None,
            Arc::new(StubPolicy {
                access_type: Err(crate::AuthorityNotEstablished::store_unavailable(
                    "worlds.worlds",
                    &sqlx::Error::PoolTimedOut,
                )),
                ..Default::default()
            }),
            Arc::new(StubScenes::default()),
            Arc::new(crate::presence::Presence::new()),
            Arc::new(StubDirectory::default()),
        );
        state.livekit.configured = true;
        state.livekit.api_url = url;
        let world = WorldName::parse("faulted.dcl.eth").unwrap();
        let inventory = state.room_service().list_rooms().await.unwrap();
        let outcome = readmit_world_rooms_in_inventory(&state, &world, None, &inventory).await;
        assert!(outcome.is_err(), "the pass is retained: {outcome:?}");
        assert_eq!(
            calls
                .lock()
                .unwrap()
                .iter()
                .filter(|method| *method != "ListRooms")
                .count(),
            0,
            "no roster read and no eviction on an undecided access setting"
        );
        server.abort();
    }
}
