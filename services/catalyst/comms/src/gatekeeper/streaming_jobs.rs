//! Durable, bounded streaming revocation and notification work. SQL transactions
//! only claim/commit facts; LiveKit and the offered notification interface run outside them.
use std::sync::Arc;
use std::time::Duration;

use chrono::NaiveDateTime;
use sqlx::PgPool;
use uuid::Uuid;

use super::super::{CommsState, errors::ApiError};
use crate::Address;
use crate::gatekeeper::removed_places::{PlaceContext, RemovedPlaces};
use crate::interfaces::{StreamingNotification, StreamingNotificationKind};
use crate::jobs::{Jobs, PeriodicCfg};

const BATCH: i64 = 20;
const RECONCILE_BATCH: i64 = 20;

fn reconcile_bounds(now: NaiveDateTime) -> (NaiveDateTime, NaiveDateTime) {
    (
        now - chrono::Duration::minutes(2),
        now - chrono::Duration::seconds(60),
    )
}

/// Each workflow has its own fleet lease and full execution budget. Slow owner
/// preparation in enforcement cannot consume ingress cleanup's scheduling slot.
pub fn spawn_jobs(state: &Arc<CommsState>, jobs: &Jobs) {
    for (name, interval, workflow) in [
        ("comms-streaming-ttl", 60, Workflow::Streaming),
        ("comms-streaming-key-expiry", 600, Workflow::Keys),
        ("comms-streaming-key-recheck", 600, Workflow::Recheck),
        ("comms-streaming-cleanup", 60, Workflow::Cleanup),
    ] {
        let (state, cursor) = (state.clone(), Arc::new(std::sync::Mutex::default()));
        jobs.spawn_periodic_leased(
            name,
            PeriodicCfg::every(Duration::from_secs(interval))
                .with_error_backoff(Duration::from_secs(600)),
            move |ctx| {
                let (state, cursor) = (state.clone(), cursor.clone());
                async move {
                    if !state.livekit.configured {
                        return Ok(());
                    }
                    run_budgeted(&ctx.cancel, async {
                        match workflow {
                            Workflow::Streaming => {
                                run_streaming_workflows(expire_streams(&state), reconcile(&state))
                                    .await
                            }
                            Workflow::Keys => expire_keys(&state).await,
                            // A minter can lose the place with no event here: a
                            // LAND or NAME transfer, a world permission edit.
                            Workflow::Recheck => {
                                let page = super::cast::Keys::After(&cursor);
                                super::cast::recheck_keys(&state, page).await;
                                Ok(())
                            }
                            Workflow::Cleanup => drain_cleanup(&state).await,
                        }
                    })
                    .await?;
                    ctx.progress("bounded streaming lifecycle pass complete");
                    Ok(())
                }
            },
        );
    }
    // Stale keys at once: the writes PostgreSQL announces. The recheck job
    // above stays for the notifications no session received.
    let state = state.clone();
    jobs.spawn_leased("comms-streaming-key-listener", move |ctx| {
        let state = state.clone();
        async move {
            if state.livekit.configured {
                let state = &state;
                let check =
                    |wake: String| async move { super::cast::recheck_notified(state, &wake).await };
                tokio::select! {
                    _ = ctx.cancel.cancelled() => {}
                    _ = super::cast::listen(&state.pool, check) => {}
                }
            }
            Ok(())
        }
    });
}

#[derive(Clone, Copy)]
enum Workflow {
    Streaming,
    Keys,
    Recheck,
    Cleanup,
}

/// Poll both TTL responsibilities together; owner preparation must not postpone
/// stale-state reads. Join also runs reconciliation when enforcement returns an error.
async fn run_streaming_workflows<E, R>(enforcement: E, reconciliation: R) -> Result<(), ApiError>
where
    E: std::future::Future<Output = Result<(), ApiError>>,
    R: std::future::Future<Output = Result<(), ApiError>>,
{
    let (enforcement, reconciliation) = tokio::join!(enforcement, reconciliation);
    enforcement?;
    reconciliation?;
    Ok(())
}

async fn run_budgeted<F>(
    cancel: &tokio_util::sync::CancellationToken,
    work: F,
) -> anyhow::Result<()>
where
    F: std::future::Future<Output = Result<(), ApiError>>,
{
    tokio::select! {
        _=cancel.cancelled()=>Ok(()),
        result=tokio::time::timeout(Duration::from_secs(60),work)=>{
            result.map_err(|_|anyhow::anyhow!("streaming lifecycle pass exceeded 60 seconds"))??;
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CleanupCommand {
    IdleReplacement,
    Reset,
    Revoke,
    KeyExpiry,
    DurationExpiry,
    /// The place left the catalogue: the removal tells the owner.
    PlaceRemoval,
}

impl CleanupCommand {
    fn notification(self) -> Option<StreamingNotificationKind> {
        match self {
            Self::IdleReplacement | Self::PlaceRemoval => None,
            Self::Reset => Some(StreamingNotificationKind::KeyReset),
            Self::Revoke => Some(StreamingNotificationKind::KeyRevoke),
            Self::KeyExpiry => Some(StreamingNotificationKind::KeyExpired),
            Self::DurationExpiry => Some(StreamingNotificationKind::TimeExceeded),
        }
    }

    /// Key expiry's own predicate spares only a broadcast live since before the deadline.
    fn permits_running(self) -> bool {
        self != Self::IdleReplacement
    }
}

#[derive(Clone, Copy)]
enum Expiry {
    Key,
    Duration,
}

impl Expiry {
    fn command(self) -> CleanupCommand {
        match self {
            Self::Key => CleanupCommand::KeyExpiry,
            Self::Duration => CleanupCommand::DurationExpiry,
        }
    }
}

/// Capture the recipient and metadata before a destructive transition, outside SQL locks.
pub(super) async fn enqueue(
    state: &CommsState,
    id: Uuid,
    command: CleanupCommand,
) -> Result<bool, ApiError> {
    match prepare(state, id, command).await? {
        Some(prepared) => Ok(prepared.record(&state.pool).await?),
        None => Ok(false),
    }
}

/// A cleanup intent with its notification context resolved, recorded by the
/// caller's executor: a replacement records it in the transaction that stores
/// the new credential.
pub(super) struct Prepared {
    id: Uuid,
    command: CleanupCommand,
    version: i64,
    event: Option<StreamingNotification>,
    context: Option<serde_json::Value>,
    recipients: Vec<Address>,
    recipients_resolved: bool,
}

impl Prepared {
    /// The credential's `streaming_state_version` this intent is conditional on.
    pub(super) fn version(&self) -> i64 {
        self.version
    }

    /// Deactivates the credential and records its cleanup; false when it
    /// changed since `prepare` (or is gone).
    pub(super) async fn record<'e, E: sqlx::PgExecutor<'e>>(
        &self,
        executor: E,
    ) -> Result<bool, sqlx::Error> {
        enqueue_prepared(
            executor,
            self.id,
            self.command,
            self.version,
            self.event.as_ref(),
            self.context.as_ref(),
            Some((&self.recipients, self.recipients_resolved)),
        )
        .await
    }
}

/// `None` when the credential is no longer active.
pub(super) async fn prepare(
    state: &CommsState,
    id: Uuid,
    command: CleanupCommand,
) -> Result<Option<Prepared>, ApiError> {
    let snapshot: Option<(String,i64,Option<String>,Option<String>)> = sqlx::query_as("SELECT place_id,streaming_state_version,admin_place_key,room_id FROM social.scene_stream_access WHERE id=$1 AND active")
        .bind(id).fetch_optional(&state.pool).await?;
    let Some((place_id, version, mut admin_place_key, room)) = snapshot else {
        return Ok(None);
    };
    let at = chrono::Utc::now().naive_utc();
    let mut context = None;
    let mut recipients = Vec::new();
    let mut recipients_resolved = false;
    let mut binding_pending = admin_place_key.is_none()
        && room
            .as_deref()
            .and_then(crate::rooms::decode_world_scene_room)
            .is_some();
    let mut binding_unresolvable = false;
    let mut admin_grants: Option<Vec<(String, String)>> = None;
    let mut grant_candidates: Option<GrantCandidates> = None;
    let event = if let Some(kind) = command.notification() {
        let result = tokio::time::timeout(Duration::from_secs(5), async {
            // Freeze the admins with their granters alongside the place context
            // and binding lookups, so neither waits on the other: a re-grant
            // before a retry overwrites both on the row. A failed freeze leaves
            // the retry to capture them, as before.
            let saved_key = admin_place_key.clone();
            let freeze = async {
                match scene_grant_candidates(&state.pool, saved_key.as_deref(), room.as_deref(), at).await {
                    Ok(candidates) => grant_candidates = candidates,
                    Err(error) => tracing::warn!(access_id=%id,%error,"stream notification admins not frozen; the retry captures them"),
                }
            };
            let capture = async {
                // Keep the original context even if binding recovery fails or times out.
                context = key_context(state, &place_id, room.as_deref()).await?;
                if binding_pending {
                    admin_place_key = notification_admin_key(&state.pool, id, room.as_deref()).await?;
                    binding_unresolvable = admin_place_key.is_none();
                    binding_pending = false;
                }
                Ok::<_, ApiError>(())
            };
            let ((), captured) = tokio::join!(freeze, capture);
            captured?;
            let context = context
                .as_ref()
                .ok_or_else(|| ApiError::http(503, "stream notification context unavailable"))?;
            if let Some(key) = admin_place_key.as_deref().filter(|_| context.world) {
                admin_grants = Some(match grant_candidates.as_ref().and_then(|c| c.get(key)) {
                    Some(frozen) => frozen.clone(),
                    None => scene_grants(&state.pool, key, at).await?,
                });
            }
            let owner = super::notifications::owner(state, context).await?;
            recipients = key_recipients(
                state,
                context,
                owner.clone(),
                admin_place_key.as_deref(),
                binding_unresolvable,
                room.as_deref(),
                at,
                admin_grants.as_deref().unwrap_or_default(),
            )
            .await?;
            recipients_resolved = true;
            Ok::<_, ApiError>(super::notifications::notification(
                context,
                owner,
                &format!("{}-{id}", kind.as_str()),
                kind,
                at,
            ))
        })
        .await;
        match result {
            Ok(Ok(event)) => Some(event),
            failure => {
                tracing::warn!(access_id=%id,error=?failure,"stream notification preparation pending; cleanup will proceed");
                None
            }
        }
    } else {
        None
    };
    let mut evidence = context
        .map(serde_json::to_value)
        .transpose()
        .map_err(|_| ApiError::internal("stream context serialization failed"))?
        .unwrap_or_else(|| serde_json::json!({}));
    evidence["admin_place_key"] = serde_json::json!(admin_place_key);
    evidence["admin_binding_pending"] = serde_json::json!(binding_pending);
    evidence["admin_binding_unresolvable"] = serde_json::json!(binding_unresolvable);
    evidence["binding_room"] = serde_json::json!(room);
    evidence["notification_at"] = serde_json::json!(at);
    if let Some(grants) = admin_grants {
        evidence["admin_grants"] = serde_json::json!(grants);
    } else if let Some(candidates) = grant_candidates {
        // The scene or its place context is not settled yet: the retry keeps
        // the pairs of the scene place it settles on.
        evidence["admin_grant_candidates"] = serde_json::json!(candidates);
    }
    let context = Some(evidence);
    Ok(Some(Prepared {
        id,
        command,
        version,
        event,
        context,
        recipients,
        recipients_resolved,
    }))
}

/// The notice context of a key under `place_id` streaming into `room`. A
/// world scene room's key is stored under its scene's place, which is no
/// catalogue row: its world tells the owner and the location.
async fn key_context(
    state: &CommsState,
    place_id: &str,
    room: Option<&str>,
) -> Result<Option<PlaceContext>, ApiError> {
    let feed = RemovedPlaces::new(state.pool.clone());
    match room.and_then(crate::rooms::decode_world_scene_room) {
        Some((world, _)) => {
            let room = room.unwrap_or_default();
            Ok(Some(
                world_key_context(state, &feed, place_id, &world, room).await?,
            ))
        }
        None => Ok(feed.place(place_id).await?),
    }
}

/// The notice context of a key streaming into a scene room of `world`: the
/// world's catalogue place, else the world alone, under the key's place.
async fn world_key_context(
    state: &CommsState,
    feed: &RemovedPlaces,
    place_id: &str,
    world: &crate::world_identity::WorldName,
    room: &str,
) -> Result<PlaceContext, ApiError> {
    let catalogued = feed.world(world.key()).await?;
    let mut context = PlaceContext {
        place_id: place_id.to_owned(),
        ..catalogued.unwrap_or_else(|| PlaceContext {
            place_id: String::new(),
            owner_address: None,
            world: true,
            world_name: Some(world.key().to_owned()),
            base_position: String::new(),
            positions: serde_json::json!([]),
        })
    };
    // The scene's own footprint, so the holders of its parcels are told; the
    // room names the scene, not its parcels, so the resolver reads them. A
    // resolver that cannot be read fails the preparation, and the job tries
    // the key again later (fail closed: never a notice with fewer holders).
    // A room whose scene is gone keeps the catalogue row's footprint, or none
    // (`positions = []`) without a catalogue row: parcel grantees are then
    // not told.
    let request = crate::rooms::RoomRequest::NamedWorldScene { room };
    match state.scenes.resolve(request).await {
        Ok(resolved) => {
            if let Some(deployment) = resolved.deployment {
                context.base_position = deployment.base.canonical();
                context.positions = serde_json::json!(
                    deployment
                        .parcels
                        .iter()
                        .map(crate::world_identity::Parcel::canonical)
                        .collect::<Vec<_>>()
                );
            }
        }
        Err(crate::rooms::ResolveError::Db(error)) => {
            return Err(super::scene_identity_unavailable(&error));
        }
        Err(_) => {}
    }
    Ok(context)
}

/// Everyone a key notice reaches (`notifications::recipients`), sorted. The
/// saved or recovered scene place decides its scene admins, so they survive
/// the scene's retirement; a definitively unresolvable binding tells none;
/// with neither, the room's live scene does.
#[allow(clippy::too_many_arguments)]
async fn key_recipients(
    state: &CommsState,
    context: &PlaceContext,
    owner: Address,
    admin_key: Option<&str>,
    binding_unresolvable: bool,
    room: Option<&str>,
    at: NaiveDateTime,
    grants: &[(String, String)],
) -> Result<Vec<Address>, ApiError> {
    let scope = if admin_key.is_some() || binding_unresolvable {
        super::notifications::Scope::KeyScene {
            place: admin_key,
            at,
            grants,
        }
    } else {
        super::notifications::Scope::Key { room }
    };
    let recipients = super::notifications::recipients(state, context, owner, scope).await?;
    Ok(recipients.into_iter().collect())
}

/// The [admin, granter] pairs of the grants on scene place `key` that stood at
/// `at`, normalized as the recipient rule reads them.
async fn scene_grants(
    pool: &PgPool,
    key: &str,
    at: NaiveDateTime,
) -> Result<Vec<(String, String)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT DISTINCT lower(btrim(admin_address)), lower(btrim(added_by)) FROM social.scene_admins \
         WHERE place_id = $1 AND added_at <= $2 AND (active OR removed_at >= $2) ORDER BY 1, 2",
    )
    .bind(key)
    .bind(at)
    .fetch_all(pool)
    .await
}

/// Frozen [admin, granter] pairs by scene place key.
type GrantCandidates = std::collections::BTreeMap<String, Vec<(String, String)>>;

/// The grant pairs (`scene_grants`) that stood at `at` on each scene place a
/// key notice may settle on: the credential's saved key, or else the bindings
/// `notification_admin_key` weighs for its room's World scene. Every key is listed,
/// empty when it had no grants. `None` when there is neither.
async fn scene_grant_candidates(
    pool: &PgPool,
    key: Option<&str>,
    room: Option<&str>,
    at: NaiveDateTime,
) -> Result<Option<GrantCandidates>, sqlx::Error> {
    let binding = room.and_then(crate::rooms::decode_world_scene_room);
    if key.is_none() && binding.is_none() {
        return Ok(None);
    }
    let (world, scene) = binding
        .as_ref()
        .map(|(world, scene)| (Some(world.key()), Some(scene.as_str())))
        .unwrap_or((None, None));
    let rows: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "WITH keys AS (SELECT $1::text AS key WHERE $1::text IS NOT NULL \
           UNION (SELECT identity::text FROM explore.world_scene_binding WHERE $1::text IS NULL AND world_name=$2 AND entity_id=$3 LIMIT 2) \
           UNION SELECT identity::text FROM explore.world_scene_place WHERE $1::text IS NULL AND world_name=$2 AND entity_id=$3 AND active) \
         SELECT k.key, lower(btrim(a.admin_address)), lower(btrim(a.added_by)) FROM keys k \
         LEFT JOIN social.scene_admins a ON a.place_id = k.key AND a.added_at <= $4 AND (a.active OR a.removed_at >= $4) \
         ORDER BY 1, 2, 3",
    )
    .bind(key)
    .bind(world)
    .bind(scene)
    .bind(at)
    .fetch_all(pool)
    .await?;
    let mut candidates = GrantCandidates::new();
    for (key, admin, granter) in rows {
        let pairs = candidates.entry(key).or_default();
        if let (Some(admin), Some(granter)) = (admin, granter)
            && pairs.last() != Some(&(admin.clone(), granter.clone()))
        {
            pairs.push((admin, granter));
        }
    }
    Ok(Some(candidates))
}

/// Runtime notification lookup takes no publication locks and never writes a credential.
/// Displaced bindings survive retirement. `Ok(None)` means no unique exact binding;
/// SQL failures remain errors. A key already saved on the credential always wins.
async fn notification_admin_key(
    pool: &PgPool,
    id: Uuid,
    room: Option<&str>,
) -> Result<Option<String>, sqlx::Error> {
    let binding = room.and_then(crate::rooms::decode_world_scene_room);
    let (world, scene) = binding
        .as_ref()
        .map(|(world, scene)| (Some(world.key()), Some(scene.as_str())))
        .unwrap_or((None, None));
    sqlx::query_scalar("WITH historical AS (SELECT identity FROM explore.world_scene_binding WHERE world_name=$2 AND entity_id=$3 LIMIT 2), \
        bindings AS (SELECT identity FROM historical UNION SELECT identity FROM explore.world_scene_place WHERE world_name=$2 AND entity_id=$3 AND active) \
        SELECT COALESCE(admin_place_key,(SELECT min(identity::text) FROM bindings HAVING count(*)=1)) FROM social.scene_stream_access WHERE id=$1")
        .bind(id).bind(world).bind(scene).fetch_optional(pool).await.map(Option::flatten)
}

fn notification_kind(kind: &str) -> Result<StreamingNotificationKind, ApiError> {
    match kind {
        "streaming_key_reset" => Ok(StreamingNotificationKind::KeyReset),
        "streaming_key_revoke" => Ok(StreamingNotificationKind::KeyRevoke),
        "streaming_key_expired" => Ok(StreamingNotificationKind::KeyExpired),
        "streaming_time_exceeded" => Ok(StreamingNotificationKind::TimeExceeded),
        _ => Err(ApiError::internal("unknown streaming notification kind")),
    }
}

pub(super) async fn enqueue_prepared<'e, E: sqlx::PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    command: CleanupCommand,
    version: i64,
    event: Option<&StreamingNotification>,
    context: Option<&serde_json::Value>,
    snapshot: Option<(&[Address], bool)>,
) -> Result<bool, sqlx::Error> {
    let (recipients, recipients_resolved) = snapshot.unwrap_or((&[], false));
    let kind = command.notification().map(|kind| kind.as_str());
    let force = command.permits_running();
    // The predicate rechecks expiry and the webhook version after the snapshot lookup.
    let changed:Option<Uuid> = sqlx::query_scalar(
        "WITH removed AS (UPDATE social.scene_stream_access SET active=false,streaming=false \
         WHERE id=$1 AND active AND ($3 OR NOT streaming) AND streaming_state_version=$4 \
         AND ($2::text IS DISTINCT FROM 'streaming_key_expired' OR (COALESCE(expiration_time,created_at+interval '4 days')<=timezone('UTC',now()) \
           AND (NOT streaming OR streaming_start_time IS NULL OR streaming_start_time>COALESCE(expiration_time,created_at+interval '4 days')))) \
         AND ($2::text IS DISTINCT FROM 'streaming_time_exceeded' OR (streaming AND streaming_start_time<=timezone('UTC',now())-interval '4 hours')) \
         RETURNING id,place_id,ingress_id) \
         INSERT INTO social.scene_stream_cleanup(access_id,place_id,ingress_id,notification_kind,occurred_at,due_at,recipient,metadata,source_context,recipients,recipients_resolved) \
         SELECT id,place_id,ingress_id,$2,COALESCE($5,($8->>'notification_at')::timestamp,timezone('UTC',now())),timezone('UTC',now()),$6,$7,$8,$9,$10 FROM removed \
         ON CONFLICT(access_id) DO NOTHING RETURNING access_id")
        .bind(id).bind(kind).bind(force).bind(version).bind(event.map(|e| e.occurred_at))
        .bind(event.map(|e| &e.recipient)).bind(event.map(|e| &e.metadata)).bind(context)
        .bind(recipients).bind(recipients_resolved).fetch_optional(executor).await?;
    Ok(changed.is_some())
}

async fn expire_streams(state: &CommsState) -> Result<(), ApiError> {
    expire(state, Expiry::Duration).await
}
pub(super) async fn expire_keys(state: &CommsState) -> Result<(), ApiError> {
    expire(state, Expiry::Key).await
}

async fn expire(state: &CommsState, expiry: Expiry) -> Result<(), ApiError> {
    let query = if matches!(expiry, Expiry::Duration) {
        "SELECT id FROM social.scene_stream_access WHERE active AND streaming AND streaming_start_time<=timezone('UTC',now())-interval '4 hours' AND (cleanup_retry_at IS NULL OR cleanup_retry_at<=timezone('UTC',now())) ORDER BY streaming_start_time,id LIMIT $1"
    } else {
        "SELECT id FROM social.scene_stream_access WHERE active AND (NOT streaming OR streaming_start_time IS NULL OR streaming_start_time>COALESCE(expiration_time,created_at+interval '4 days')) AND COALESCE(expiration_time,created_at+interval '4 days')<=timezone('UTC',now()) AND (cleanup_retry_at IS NULL OR cleanup_retry_at<=timezone('UTC',now())) ORDER BY COALESCE(expiration_time,created_at+interval '4 days'),id LIMIT $1"
    };
    let ids: Vec<Uuid> = sqlx::query_scalar(query)
        .bind(BATCH)
        .fetch_all(&state.pool)
        .await?;
    for id in ids {
        if let Err(error) = enqueue(state, id, expiry.command()).await {
            // Historic rows with no resolvable owner remain explicit retry work; don't
            // deactivate them and lose the only remaining source of notification context.
            sqlx::query("UPDATE social.scene_stream_access SET cleanup_retry_at=timezone('UTC',now())+interval '10 minutes' WHERE id=$1 AND active")
                .bind(id).execute(&state.pool).await?;
            tracing::warn!(access_id=%id,error=%error,"stream lifecycle snapshot failed; retained for retry");
        }
    }
    Ok(())
}

#[derive(sqlx::FromRow)]
struct Cleanup {
    access_id: Uuid,
    place_id: String,
    ingress_id: Option<String>,
    notification_kind: Option<String>,
    occurred_at: NaiveDateTime,
    ingress_deleted: bool,
    recipient: Option<Address>,
    metadata: Option<serde_json::Value>,
    source_context: Option<serde_json::Value>,
    recipients: Vec<Address>,
    recipients_resolved: bool,
    recipient_cursor: i32,
    notification_started: bool,
}

pub(super) async fn drain_cleanup(state: &CommsState) -> Result<(), ApiError> {
    cleanup(state, None).await
}

pub(super) async fn cleanup_access(state: &CommsState, id: Uuid) -> Result<(), ApiError> {
    cleanup(state, Some(id)).await
}

async fn cleanup(state: &CommsState, target: Option<Uuid>) -> Result<(), ApiError> {
    for _ in 0..BATCH {
        let token = Uuid::new_v4();
        // Five minutes covers one 5s admin call and the local notification lookup/write.
        let row: Option<Cleanup> = sqlx::query_as(
            "UPDATE social.scene_stream_cleanup SET claim_token=$1,claim_until=timezone('UTC',now())+interval '5 minutes' \
             WHERE access_id=(SELECT access_id FROM social.scene_stream_cleanup WHERE due_at<=timezone('UTC',now()) \
             AND (claim_until IS NULL OR claim_until<=timezone('UTC',now())) AND ($2::uuid IS NULL OR access_id=$2) ORDER BY due_at,access_id LIMIT 1 FOR UPDATE SKIP LOCKED) \
             RETURNING access_id,place_id,ingress_id,notification_kind,occurred_at,ingress_deleted,recipient,metadata,source_context,recipients,recipients_resolved,recipient_cursor,notification_started")
            .bind(token).bind(target).fetch_optional(&state.pool).await?;
        let Some(row) = row else {
            if let Some(id) = target {
                let pending: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM social.scene_stream_cleanup WHERE access_id=$1)",
                )
                .bind(id)
                .fetch_one(&state.pool)
                .await?;
                if pending {
                    return Err(ApiError::internal("stream cleanup is pending retry"));
                }
            }
            break;
        };
        match deliver(state, &row, token).await {
            Ok(()) => {
                sqlx::query(
                    "DELETE FROM social.scene_stream_cleanup WHERE access_id=$1 AND claim_token=$2",
                )
                .bind(row.access_id)
                .bind(token)
                .execute(&state.pool)
                .await?;
            }
            Err(error) => {
                // No exhaustion discard: retries stay durable, delay is bounded at ten minutes.
                sqlx::query("UPDATE social.scene_stream_cleanup SET attempts=LEAST(attempts+1,30), \
                    due_at=timezone('UTC',now())+make_interval(secs=>LEAST(600,5*power(2,LEAST(attempts,7)))::double precision), \
                    claim_token=NULL,claim_until=NULL WHERE access_id=$1 AND claim_token=$2")
                    .bind(row.access_id).bind(token).execute(&state.pool).await?;
                tracing::warn!(access_id=%row.access_id,error=%error,"stream cleanup retained for retry");
                if target.is_some() {
                    return Err(error);
                }
            }
        }
    }
    Ok(())
}

/// The Cast publishers the cleaned-up credential admitted leave its room
/// (`cast::evict_revoked_publishers`); a credential that replaced it in the
/// same room keeps its own publishers.
async fn evict_publishers(state: &CommsState, row: &Cleanup) -> Result<(), ApiError> {
    let room: Option<String> =
        sqlx::query_scalar("SELECT room_id FROM social.scene_stream_access WHERE id = $1")
            .bind(row.access_id)
            .fetch_optional(&state.pool)
            .await?
            .flatten();
    let Some(room) = room.filter(|r| !r.is_empty()) else {
        return Ok(());
    };
    // The room's own successor, under whatever place: a world scene's key
    // can replace one stored under the world's catalogue place.
    let successor: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM social.scene_stream_access \
         WHERE active AND id <> $1 AND room_id = $2 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(row.access_id)
    .bind(&room)
    .fetch_optional(&state.pool)
    .await?;
    super::cast::evict_revoked_publishers(state, &room, successor).await
}

/// How long a reset intent waits for its replacement credential before it
/// is delivered as a revocation.
const RESET_REPLACEMENT_GRACE: chrono::Duration = chrono::Duration::hours(1);

/// Revokes first: the RTMP ingress is deleted (and `ingress_deleted`
/// recorded) before anything else, so a room-service failure while evicting
/// the WebRTC publishers never leaves the revoked key accepting RTMP. Eviction
/// and the notification follow, each also when the step before it failed (the
/// key is refused since its intent was stored); a failure retries the row,
/// which skips the ingress already deleted and tells the owner once.
async fn deliver(state: &CommsState, row: &Cleanup, token: Uuid) -> Result<(), ApiError> {
    // Each failure is logged here: the caller reports one of them.
    let failed = |error: &ApiError| tracing::warn!(access_id = %row.access_id, %error, "stream cleanup step failed");
    let ingress = delete_ingress(state, row, token).await.inspect_err(failed);
    let evicted = evict_publishers(state, row).await.inspect_err(failed);
    notify(state, row, token).await.and(ingress).and(evicted)
}

async fn delete_ingress(state: &CommsState, row: &Cleanup, token: Uuid) -> Result<(), ApiError> {
    if !row.ingress_deleted {
        if let Some(id) = row.ingress_id.as_deref().filter(|id| !id.is_empty()) {
            // LiveKit answers when the ingress worker stopped the push.
            tokio::time::timeout(Duration::from_secs(5), state.ingress().delete_ingress(id))
                .await
                .map_err(|_| ApiError::internal("stream cleanup ingress timed out"))?
                .map_err(|e| ApiError::internal(format!("stream cleanup ingress: {e}")))?;
        }
        let changed = sqlx::query("UPDATE social.scene_stream_cleanup SET ingress_deleted=true WHERE access_id=$1 AND claim_token=$2")
            .bind(row.access_id).bind(token).execute(&state.pool).await?;
        if changed.rows_affected() == 0 {
            return Err(ApiError::internal("stream cleanup lease lost"));
        }
    }
    Ok(())
}

/// Whether credential `access_id`, stored under `place_id`, has an active
/// replacement: in a world scene room, the key streaming into that room
/// (also a world scene's key replacing one stored under the world's
/// catalogue place); else one under the same place. Another scene's key
/// under a colliding place id is no replacement.
pub(super) async fn replaced(
    pool: &PgPool,
    place_id: &str,
    access_id: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "WITH o AS (SELECT room_id FROM social.scene_stream_access WHERE id = $2 AND starts_with(room_id, $3)) \
         SELECT EXISTS(SELECT 1 FROM social.scene_stream_access n WHERE n.active AND n.id <> $2 AND CASE \
             WHEN EXISTS (SELECT 1 FROM o) THEN n.room_id = (SELECT room_id FROM o) \
             ELSE n.place_id = $1 END)",
    )
    .bind(place_id)
    .bind(access_id)
    .bind(crate::rooms::SCENE_ROOM_PREFIX)
    .fetch_one(pool)
    .await
}

async fn notify(state: &CommsState, row: &Cleanup, token: Uuid) -> Result<(), ApiError> {
    let Some(kind_str) = row.notification_kind.as_deref() else {
        return Ok(());
    };
    let kind = notification_kind(kind_str)?;
    if kind == StreamingNotificationKind::KeyExpired || kind == StreamingNotificationKind::KeyReset
    {
        let replacement = replaced(&state.pool, &row.place_id, row.access_id).await?;
        if kind == StreamingNotificationKind::KeyExpired && replacement && !row.notification_started
        {
            return Ok(());
        }
        // Once delivery started the saved reset is finished as it is, never
        // converted: recipients already told of the reset are not told again.
        if kind == StreamingNotificationKind::KeyReset && !replacement && !row.notification_started
        {
            // A replacement commits with its reset intent (`replace_access`), so one still
            // missing after the grace period never will (an intent from before that, or the
            // replacement revoked in turn): the reset is delivered as the revocation it is.
            let now = chrono::Utc::now().naive_utc();
            if now - row.occurred_at > RESET_REPLACEMENT_GRACE {
                sqlx::query("UPDATE social.scene_stream_cleanup SET notification_kind='streaming_key_revoke', metadata=NULL, recipient_cursor=0, notification_started=false \
                    WHERE access_id=$1 AND claim_token=$2 AND notification_kind='streaming_key_reset'")
                    .bind(row.access_id).bind(token).execute(&state.pool).await?;
                return Err(ApiError::internal(
                    "stream reset without a replacement: delivered as a revocation on retry",
                ));
            }
            return Err(ApiError::internal(
                "stream reset awaiting replacement credential",
            ));
        }
    }
    let key = format!("{kind_str}-{}", row.access_id);
    let mut recipients = row.recipients.clone();
    let mut metadata = row.metadata.clone();
    if !row.recipients_resolved || metadata.is_none() {
        let mut saved_context = row
            .source_context
            .clone()
            .unwrap_or_else(|| serde_json::json!({}));
        let context = if let Some(context) = row
            .source_context
            .as_ref()
            .filter(|value| value.get("place_id").is_some())
        {
            serde_json::from_value::<PlaceContext>(context.clone())
                .map_err(|_| ApiError::internal("stream context snapshot invalid"))?
        } else {
            let saved_room = row
                .source_context
                .as_ref()
                .and_then(|value| value.get("binding_room"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
            let room = match saved_room {
                Some(room) => Some(room),
                None => {
                    sqlx::query_scalar("SELECT room_id FROM social.scene_stream_access WHERE id=$1")
                        .bind(row.access_id)
                        .fetch_optional(&state.pool)
                        .await?
                        .flatten()
                }
            };
            key_context(state, &row.place_id, room.as_deref())
                .await?
                .ok_or_else(|| ApiError::http(503, "stream notification context unavailable"))?
        };
        let fields = serde_json::to_value(&context)
            .map_err(|_| ApiError::internal("stream context serialization failed"))?;
        saved_context
            .as_object_mut()
            .ok_or_else(|| ApiError::internal("stream context snapshot invalid"))?
            .extend(
                fields
                    .as_object()
                    .expect("place context is an object")
                    .clone(),
            );
        if !row.recipients_resolved {
            // Persist newly captured context before binding recovery can fail. Never
            // refresh an existing snapshot from another catalogue lifetime.
            let changed = sqlx::query("UPDATE social.scene_stream_cleanup SET source_context=$3 WHERE access_id=$1 AND claim_token=$2 AND NOT recipients_resolved")
                .bind(row.access_id).bind(token).bind(&saved_context).execute(&state.pool).await?;
            if changed.rows_affected() == 0 {
                return Err(ApiError::internal("stream cleanup lease lost"));
            }
            // Expand scalar-era intents; a saved owner is not a complete set.
            if let Some(recipient) = &row.recipient {
                recipients.push(recipient.clone());
            }
            let live_room: Option<String> =
                sqlx::query_scalar("SELECT room_id FROM social.scene_stream_access WHERE id=$1")
                    .bind(row.access_id)
                    .fetch_optional(&state.pool)
                    .await?
                    .flatten();
            let room = row
                .source_context
                .as_ref()
                .and_then(|value| value.get("binding_room"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .or(live_room);
            let mut admin_key = row
                .source_context
                .as_ref()
                .and_then(|value| value.get("admin_place_key"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
            let mut binding_unresolvable = admin_key.is_none()
                && saved_context
                    .get("admin_binding_unresolvable")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true);
            if admin_key.is_none() && !binding_unresolvable {
                admin_key = tokio::time::timeout(
                    Duration::from_secs(5),
                    notification_admin_key(&state.pool, row.access_id, room.as_deref()),
                )
                .await
                .map_err(|_| ApiError::internal("stream notification binding timed out"))??;
                binding_unresolvable = admin_key.is_none()
                    && (room
                        .as_deref()
                        .and_then(crate::rooms::decode_world_scene_room)
                        .is_some()
                        || saved_context
                            .get("admin_binding_pending")
                            .and_then(serde_json::Value::as_bool)
                            == Some(true));
            }
            saved_context["admin_place_key"] = serde_json::json!(admin_key);
            saved_context["admin_binding_pending"] = serde_json::json!(false);
            saved_context["admin_binding_unresolvable"] = serde_json::json!(binding_unresolvable);
            // Pairs frozen at preparation win, those of the settled scene place
            // among the candidates next; otherwise freeze them now, once.
            if saved_context.get("admin_grants").is_none()
                && let Some(key) = admin_key.as_deref().filter(|_| context.world)
            {
                let frozen = saved_context
                    .get("admin_grant_candidates")
                    .and_then(|candidates| candidates.get(key))
                    .map(|pairs| super::notifications::grant_pairs(Some(pairs)));
                let grants = match frozen {
                    Some(grants) => grants,
                    None => scene_grants(&state.pool, key, row.occurred_at).await?,
                };
                saved_context["admin_grants"] = serde_json::json!(grants);
            }
            let grants = super::notifications::grant_pairs(saved_context.get("admin_grants"));
            // Save exact evidence before another authority lookup can fail or retire.
            let changed = sqlx::query("UPDATE social.scene_stream_cleanup SET source_context=$3 WHERE access_id=$1 AND claim_token=$2 AND NOT recipients_resolved")
                .bind(row.access_id).bind(token).bind(&saved_context).execute(&state.pool).await?;
            if changed.rows_affected() == 0 {
                return Err(ApiError::internal("stream cleanup lease lost"));
            }
            recipients.sort();
            recipients.dedup();
            let changed = sqlx::query("UPDATE social.scene_stream_cleanup SET recipients=$3 WHERE access_id=$1 AND claim_token=$2 AND NOT recipients_resolved")
                .bind(row.access_id).bind(token).bind(&recipients).execute(&state.pool).await?;
            if changed.rows_affected() == 0 {
                return Err(ApiError::internal("stream cleanup lease lost"));
            }
            let resolved = tokio::time::timeout(Duration::from_secs(5), async {
                let owner = match &row.recipient {
                    Some(owner) => owner.clone(),
                    None => super::notifications::owner(state, &context).await?,
                };
                key_recipients(
                    state,
                    &context,
                    owner,
                    admin_key.as_deref(),
                    binding_unresolvable,
                    room.as_deref(),
                    row.occurred_at,
                    &grants,
                )
                .await
            })
            .await
            .map_err(|_| ApiError::internal("stream notification authority timed out"))??;
            recipients.extend(resolved);
            recipients.sort();
            recipients.dedup();
        }
        let template = if let Some(metadata) = metadata {
            metadata
        } else {
            super::notifications::notification(
                &context,
                recipients.first().cloned().ok_or_else(|| {
                    ApiError::internal("stream notification recipients unavailable")
                })?,
                &key,
                kind,
                row.occurred_at,
            )
            .metadata
        };
        let changed = sqlx::query("UPDATE social.scene_stream_cleanup SET recipients=$3,recipients_resolved=true,metadata=$4,source_context=$5 WHERE access_id=$1 AND claim_token=$2 AND recipient_cursor=$6")
            .bind(row.access_id).bind(token).bind(&recipients).bind(&template).bind(&saved_context).bind(row.recipient_cursor)
            .execute(&state.pool).await?;
        if changed.rows_affected() == 0 {
            return Err(ApiError::internal("stream cleanup lease lost"));
        }
        metadata = Some(template);
    }
    let metadata =
        metadata.ok_or_else(|| ApiError::internal("stream notification metadata unavailable"))?;
    let cursor = usize::try_from(row.recipient_cursor)
        .map_err(|_| ApiError::internal("invalid stream notification cursor"))?;
    if cursor > recipients.len() {
        return Err(ApiError::internal("invalid stream notification cursor"));
    }
    // Commit the delivery decision before the first possible acceptance. A
    // replacement created after acceptance cannot suppress an unfinished suffix,
    // even if a crash prevented the first cursor checkpoint.
    if !row.notification_started {
        let changed=sqlx::query("UPDATE social.scene_stream_cleanup SET notification_started=true WHERE access_id=$1 AND claim_token=$2 AND recipient_cursor=$3")
            .bind(row.access_id).bind(token).bind(row.recipient_cursor).execute(&state.pool).await?;
        if changed.rows_affected() == 0 {
            return Err(ApiError::internal("stream cleanup lease lost"));
        }
    }
    for (index, recipient) in recipients.iter().enumerate().skip(cursor) {
        let mut metadata = metadata.clone();
        metadata["address"] = serde_json::json!(recipient.as_str());
        let event = StreamingNotification {
            event_key: key.clone(),
            recipient: recipient.clone(),
            kind,
            metadata,
            occurred_at: row.occurred_at,
        };
        tokio::time::timeout(
            Duration::from_secs(5),
            state.social.notify_streaming_notification(&event),
        )
        .await
        .map_err(|_| ApiError::internal("stream notification delivery timed out"))??;
        // A crash between acceptance and this checkpoint safely replays one
        // event/recipient identity; a successful prefix never starves its suffix.
        let changed=sqlx::query("UPDATE social.scene_stream_cleanup SET recipient_cursor=$4 WHERE access_id=$1 AND claim_token=$2 AND recipient_cursor=$3 AND recipients_resolved")
            .bind(row.access_id).bind(token).bind(index as i32).bind(index as i32+1).execute(&state.pool).await?;
        if changed.rows_affected() == 0 {
            return Err(ApiError::internal("stream cleanup lease lost"));
        }
    }
    Ok(())
}

#[derive(sqlx::FromRow)]
struct Snapshot {
    id: Uuid,
    ingress_id: String,
    streaming: bool,
    streaming_start_time: Option<NaiveDateTime>,
    streaming_state_version: i64,
    streaming_checked_at: Option<NaiveDateTime>,
}

impl Snapshot {
    fn eligible(&self, now: NaiveDateTime) -> bool {
        let (start, checked) = reconcile_bounds(now);
        (!self.streaming || self.streaming_start_time.is_some_and(|at| at <= start))
            && self.streaming_checked_at.is_none_or(|at| at <= checked)
    }
}

/// Makes the `streaming` flag of the active credentials agree with LiveKit:
/// a flag that is up with no stream comes down (a lost `ingress_ended`), and
/// one that is down while the ingress publishes goes up (a lost
/// `ingress_started`), so the four-hour cap and the renewal conflict hold
/// for that stream. The streaming credentials are read first, then the idle
/// ones, each kind with its own bound of 20 a pass and its own index.
pub(super) async fn reconcile(state: &CommsState) -> Result<(), ApiError> {
    for kind in ["streaming", "NOT streaming"] {
        reconcile_kind(state, kind).await?;
    }
    Ok(())
}

async fn reconcile_kind(state: &CommsState, kind: &str) -> Result<(), ApiError> {
    let now = chrono::Utc::now().naive_utc();
    let (started_before, checked_before) = reconcile_bounds(now);
    let rows: Vec<Snapshot> = sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT id,ingress_id,streaming,streaming_start_time,streaming_state_version,streaming_checked_at FROM social.scene_stream_access \
        WHERE active AND {kind} AND ingress_id IS NOT NULL AND ingress_id<>'' \
        AND (NOT streaming OR streaming_start_time<=$2) \
        AND (streaming_checked_at IS NULL OR streaming_checked_at<=$3) \
        ORDER BY streaming_checked_at NULLS FIRST,id LIMIT $1")))
        .bind(RECONCILE_BATCH).bind(started_before).bind(checked_before).fetch_all(&state.pool).await?;
    // At most 20 reads in groups of five; each group finishes before the next.
    for group in rows.chunks(5) {
        let results = futures::future::join_all(group.iter().map(|row| async {
            if !row.eligible(now) { return Ok(()); }
            let status = state.ingress().is_ingress_streaming(&row.ingress_id).await;
            if matches!(status, Ok(Some(false))) && row.streaming {
                clear_stale(&state.pool,row,now).await?;
            } else if matches!(status, Ok(Some(true))) && !row.streaming {
                raise(&state.pool,row.id,row.streaming_state_version).await?;
            } else if let Err(error) = status {
                tracing::warn!(access_id=%row.id,error=%error,"stream state reconciliation failed; flag preserved");
            }
            // Rotate the bounded read set even on transport errors to avoid starving later rows.
            sqlx::query("UPDATE social.scene_stream_access SET streaming_checked_at=timezone('UTC',now()) WHERE id=$1 AND streaming_state_version=$2")
                .bind(row.id).bind(row.streaming_state_version).execute(&state.pool).await?;
            Ok::<(),sqlx::Error>(())
        })).await;
        for result in results {
            result?;
        }
    }
    Ok(())
}

/// The flag of a credential whose ingress publishes goes up; the cap counts
/// from now. False when the row changed since it was read.
pub(super) async fn raise(pool: &PgPool, id: Uuid, version: i64) -> Result<bool, sqlx::Error> {
    let raised = sqlx::query("UPDATE social.scene_stream_access SET streaming=true,streaming_start_time=timezone('UTC',now()),streaming_state_version=streaming_state_version+1 \
        WHERE id=$1 AND active AND NOT streaming AND streaming_state_version=$2")
        .bind(id).bind(version).execute(pool).await?;
    Ok(raised.rows_affected() == 1)
}

async fn clear_stale(
    pool: &PgPool,
    row: &Snapshot,
    now: NaiveDateTime,
) -> Result<u64, sqlx::Error> {
    let (started_before, checked_before) = reconcile_bounds(now);
    let result=sqlx::query("UPDATE social.scene_stream_access SET streaming=false,streaming_start_time=NULL \
        WHERE id=$1 AND ingress_id=$2 AND active AND streaming AND streaming_start_time IS NOT DISTINCT FROM $3 AND streaming_state_version=$4 \
        AND streaming_start_time<=$5 AND (streaming_checked_at IS NULL OR streaming_checked_at<=$6)")
        .bind(row.id).bind(&row.ingress_id).bind(row.streaming_start_time).bind(row.streaming_state_version)
        .bind(started_before).bind(checked_before).execute(pool).await?;
    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A world scene key's notice names the world under the key's place,
    /// with the footprint of the scene of the key's room; a resolver that
    /// cannot be read fails it (503), never a notice with fewer holders.
    #[tokio::test]
    async fn a_world_key_notice_has_its_scene_footprint_and_fails_closed() {
        use crate::interfaces::test_support::{StubDirectory, StubPolicy, StubScenes};
        use crate::rooms::{ActiveDeployment, ResolveError, ResolvedRoom};
        use crate::world_identity::{Parcel, RealmId, SceneId, WorldName};
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let tag = Uuid::new_v4().simple().to_string();
        let world = WorldName::parse(&format!("ctx{}.dcl.eth", &tag[..10])).unwrap();
        let scene = SceneId::parse("SceneA").unwrap();
        let room = crate::rooms::worlds_service_scene_room(&world, &scene);
        let base = Parcel::parse("3,4").unwrap();
        let resolved = ResolvedRoom {
            room: room.clone(),
            scene_id: Some(scene.clone()),
            deployment: Some(ActiveDeployment {
                scene_id: scene,
                title: None,
                parcels: [base, Parcel::parse("3,5").unwrap()].into_iter().collect(),
                base,
                namespace: None,
                local: true,
            }),
            realm: RealmId::World(world.clone()),
        };
        let state = |room: Result<ResolvedRoom, ResolveError>| {
            CommsState::new(
                pool.clone(),
                crate::http::Outbound::for_tests(),
                crate::gatekeeper::Settings::for_tests(),
                None,
                None,
                Arc::new(StubPolicy::default()),
                Arc::new(StubScenes {
                    room,
                    scene: Ok(None),
                }),
                Arc::new(crate::presence::Presence::new()),
                Arc::new(StubDirectory::default()),
            )
        };
        let feed = RemovedPlaces::new(pool.clone());
        let context = world_key_context(&state(Ok(resolved)), &feed, "scene-place", &world, &room)
            .await
            .unwrap();
        assert_eq!(context.place_id, "scene-place");
        assert!(context.world);
        assert_eq!(context.world_name.as_deref(), Some(world.key()));
        assert_eq!(context.base_position, "3,4");
        let mut positions: Vec<String> = serde_json::from_value(context.positions).unwrap();
        positions.sort();
        assert_eq!(positions, ["3,4", "3,5"]);
        let unreadable = Err(ResolveError::Db(sqlx::Error::PoolTimedOut));
        let error = world_key_context(&state(unreadable), &feed, "scene-place", &world, &room)
            .await
            .unwrap_err();
        assert_eq!(error.code, 503);
    }

    #[tokio::test]
    async fn a_scalar_era_notice_reaches_the_stored_owner_and_every_admin_and_operator_once() {
        use crate::interfaces::test_support::{StubDirectory, StubPolicy, StubScenes};
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            return;
        };
        let (owner, approved, admin) = (
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "0xcccccccccccccccccccccccccccccccccccccccc",
        );
        let directory = Arc::new(StubDirectory::default());
        let state = CommsState::new(
            pool.clone(),
            crate::http::Outbound::for_tests(),
            crate::gatekeeper::Settings::for_tests(),
            None,
            None,
            Arc::new(StubPolicy {
                land: Ok(Some(Arc::new(Some(crate::access::LandOperators {
                    owner: owner.into(),
                    approved_for_all: vec![approved.into(), owner.into()],
                    ..Default::default()
                })))),
                ..Default::default()
            }),
            Arc::new(StubScenes::default()),
            Arc::new(crate::presence::Presence::new()),
            directory.clone(),
        );
        sqlx::query("INSERT INTO social.scene_admins (place_id, admin_address, added_by) VALUES ('-130,8', $1, $1)")
            .bind(admin).execute(&pool).await.unwrap();
        let context = PlaceContext {
            place_id: "catalogue-place".into(),
            owner_address: None,
            world: false,
            world_name: None,
            base_position: "-130,8".into(),
            positions: serde_json::json!(["-130,8"]),
        };
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO social.scene_stream_cleanup(access_id,place_id,notification_kind,occurred_at,due_at,recipient,metadata,source_context) \
            VALUES($1,'catalogue-place','streaming_key_revoke',timezone('UTC',now()),timezone('UTC',now()),$2,$3,$4)")
            .bind(id).bind(owner).bind(serde_json::json!({"title":"snapshot","address":owner}))
            .bind(serde_json::to_value(&context).unwrap()).execute(&pool).await.unwrap();

        cleanup_access(&state, id).await.unwrap();

        {
            let told = directory.streaming_notifications.lock().unwrap();
            let mut recipients: Vec<&str> = told.iter().map(|n| n.recipient.as_str()).collect();
            recipients.sort();
            assert_eq!(recipients, [owner, approved, admin]);
            for notice in told.iter() {
                assert_eq!(notice.event_key, format!("streaming_key_revoke-{id}"));
                assert_eq!(notice.metadata["title"], "snapshot");
                assert_eq!(notice.metadata["address"], notice.recipient.as_str());
            }
        }
        let pending: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM social.scene_stream_cleanup WHERE access_id=$1)",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!pending, "the delivered row is gone");
    }

    fn wallet(n: u8) -> Address {
        Address::parse(&format!("0x{n:040x}")).unwrap()
    }

    async fn fanout_fixture(
        world: bool,
    ) -> Option<(
        CommsState,
        Arc<crate::interfaces::test_support::StubDirectory>,
        String,
    )> {
        use crate::interfaces::test_support::{StubDirectory, StubPolicy, StubScenes};
        let pool = crate::db::scratch::isolated_pool(4).await?;
        let place = format!("stream-fanout-{}", Uuid::new_v4());
        let name = format!("{place}.dcl.eth");
        sqlx::query("INSERT INTO explore.place (id,base_position,raw) VALUES ($1,'10,20',$2)")
            .bind(&place).bind(serde_json::json!({"world":world,"world_name":name,"owner":wallet(1).as_str(),"positions":["10,20"]}))
            .execute(&pool).await.unwrap();
        let admin_key = if world {
            let identity = Uuid::new_v4();
            sqlx::query("INSERT INTO explore.world_scene_place (world_name,identity,entity_id,parcels) VALUES ($1,$2,'fanout-scene','{\"10,20\"}')")
                .bind(&name).bind(identity).execute(&pool).await.unwrap();
            let unrelated = Uuid::new_v4();
            sqlx::query("INSERT INTO explore.world_scene_place (world_name,identity,entity_id,parcels) VALUES ($1,$2,'unrelated-scene','{\"30,40\"}')")
                .bind(&name).bind(unrelated).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO social.scene_admins (place_id,admin_address,added_by) VALUES ($1,$2,$2)")
                .bind(unrelated.to_string()).bind(wallet(10)).execute(&pool).await.unwrap();
            identity.to_string()
        } else {
            "10,20".to_string()
        };
        for (key, admin, active) in [
            (admin_key.as_str(), wallet(2), true),
            (admin_key.as_str(), wallet(3), true),
            (admin_key.as_str(), wallet(7), false),
            (place.as_str(), wallet(8), true),
            ("unrelated-scene", wallet(9), true),
        ] {
            // Granted by the owner, who holds the world-wide authority.
            sqlx::query("INSERT INTO social.scene_admins (place_id,admin_address,added_by,active,removed_at) VALUES ($1,$2,$3,$4,CASE WHEN $4 THEN NULL ELSE timezone('UTC',now()) END)")
                .bind(key).bind(admin).bind(wallet(1)).bind(active).execute(&pool).await.unwrap();
        }
        let policy: Arc<dyn crate::interfaces::AccessPolicy> = if world {
            // A parcel-scoped grantee never widens its authority to the key:
            // the admin it granted is never told, saved list or not.
            sqlx::query("INSERT INTO social.scene_admins (place_id,admin_address,added_by) VALUES ($1,$2,$3)")
                .bind(&admin_key).bind(wallet(11)).bind(wallet(3)).execute(&pool).await.unwrap();
            Arc::new(super::super::notifications::test_support::WorldPolicy {
                owner: wallet(1),
                wide: Default::default(),
                protected: [wallet(1), wallet(3), wallet(4)].into_iter().collect(),
            })
        } else {
            Arc::new(StubPolicy {
                land: Ok(Some(Arc::new(Some(crate::access::LandOperators {
                    owner: wallet(1).to_string(),
                    operator: Some(wallet(3).to_string()),
                    update_operator: Some(wallet(4).to_string()),
                    update_managers: vec![wallet(5).to_string(), wallet(3).to_string()],
                    approved_for_all: vec![wallet(6).to_string()],
                })))),
                ..Default::default()
            })
        };
        let directory = Arc::new(StubDirectory::default());
        let state = CommsState::new(
            pool,
            crate::http::Outbound::for_tests(),
            crate::gatekeeper::Settings::for_tests(),
            None,
            None,
            policy,
            Arc::new(StubScenes::default()),
            Arc::new(crate::presence::Presence::new()),
            directory.clone(),
        );
        Some((state, directory, place))
    }

    async fn fanout_access(state: &CommsState, place: &str, command: CleanupCommand) -> Uuid {
        fanout_access_bound(state, place, command, None).await
    }

    /// `scene`: the world scene entity whose place the key saves.
    async fn fanout_access_bound(
        state: &CommsState,
        place: &str,
        command: CleanupCommand,
        scene: Option<&str>,
    ) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO social.scene_stream_access (id,place_id,streaming_url,streaming_key,generated_by,streaming,streaming_start_time,expiration_time,admin_place_key) VALUES ($1,$2,'rtmp://example.invalid/live','test-key','test',$3,timezone('UTC',now())-interval '5 hours',timezone('UTC',now())-interval '1 second',(SELECT identity::text FROM explore.world_scene_place WHERE entity_id=$4 AND world_name=$2||'.dcl.eth'))")
            .bind(id).bind(place).bind(command == CleanupCommand::DurationExpiry).bind(scene)
            .execute(&state.pool).await.unwrap();
        assert!(enqueue(state, id, command).await.unwrap());
        if command == CleanupCommand::Reset {
            sqlx::query("INSERT INTO social.scene_stream_access (place_id,streaming_url,streaming_key,generated_by) VALUES ($1,'rtmp://example.invalid/live','replacement','test')")
                .bind(place).execute(&state.pool).await.unwrap();
        }
        id
    }

    #[tokio::test]
    async fn streaming_fanout_failed_binding_recovers_after_retirement_before_first_retry() {
        let Some((state, directory, place)) = fanout_fixture(true).await else {
            return;
        };
        let name = format!("{place}.dcl.eth");
        let room = crate::rooms::worlds_service_scene_room(
            &crate::world_identity::WorldName::parse(&name).unwrap(),
            &crate::world_identity::SceneId::parse("fanout-scene").unwrap(),
        );
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO social.scene_stream_access(id,place_id,room_id,streaming_url,streaming_key,generated_by) VALUES ($1,$2,$3,'rtmp://example.invalid/live','test','test')")
            .bind(id).bind(&place).bind(&room).execute(&state.pool).await.unwrap();
        let mut locked = state.pool.begin().await.unwrap();
        sqlx::query("LOCK explore.world_scene_place IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *locked)
            .await
            .unwrap();
        let revoked = enqueue(&state, id, CleanupCommand::Revoke).await.unwrap();
        locked.rollback().await.unwrap();
        assert!(revoked);
        let facts: (bool, bool, bool) = sqlx::query_as("SELECT a.active,c.recipients_resolved,(c.source_context->>'admin_binding_pending')::boolean FROM social.scene_stream_access a JOIN social.scene_stream_cleanup c ON c.access_id=a.id WHERE a.id=$1")
            .bind(id).fetch_one(&state.pool).await.unwrap();
        assert_eq!(facts, (false, false, true));
        // Retire before any deferred binding recovery. The sibling remains live.
        sqlx::query("UPDATE explore.world_scene_place SET active=false,entity_id=NULL WHERE world_name=$1 AND entity_id='fanout-scene'")
            .bind(&name).execute(&state.pool).await.unwrap();
        sqlx::query("DELETE FROM explore.place WHERE id=$1")
            .bind(&place)
            .execute(&state.pool)
            .await
            .unwrap();
        *directory.fail_streaming_recipient.lock().unwrap() = Some(wallet(2));
        let _ = cleanup_access(&state, id).await;
        let progress: (bool, i32) = sqlx::query_as("SELECT recipients_resolved,recipient_cursor FROM social.scene_stream_cleanup WHERE access_id=$1")
            .bind(id).fetch_one(&state.pool).await.unwrap();
        assert_eq!(
            progress,
            (true, 1),
            "capture the retired binding before delivery"
        );
        sqlx::query("UPDATE social.scene_stream_cleanup SET due_at=timezone('UTC',now()) WHERE access_id=$1")
            .bind(id).execute(&state.pool).await.unwrap();
        let _ = cleanup_access(&state, id).await;
        let actual: std::collections::BTreeSet<_> = directory
            .streaming_notifications
            .lock()
            .unwrap()
            .iter()
            .map(|notice| notice.recipient.clone())
            .collect();
        assert_eq!(
            actual,
            (1..=4).map(wallet).collect(),
            "retired bound-scene admins must survive the first failed recovery; exclude sibling admins"
        );
        assert_eq!(
            directory
                .streaming_attempts
                .lock()
                .unwrap()
                .iter()
                .filter(|recipient| **recipient == wallet(1))
                .count(),
            1,
            "accepted notices must not be replayed after the saved cursor"
        );
    }

    #[tokio::test]
    async fn streaming_fanout_retry_saves_context_before_failed_binding_lookup() {
        let Some((state, directory, place)) = fanout_fixture(true).await else {
            return;
        };
        let name = format!("{place}.dcl.eth");
        let room = crate::rooms::worlds_service_scene_room(
            &crate::world_identity::WorldName::parse(&name).unwrap(),
            &crate::world_identity::SceneId::parse("fanout-scene").unwrap(),
        );
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO social.scene_stream_access(id,place_id,room_id,streaming_url,streaming_key,generated_by) VALUES ($1,$2,$3,'rtmp://example.invalid/live','test','test')")
            .bind(id).bind(&place).bind(&room).execute(&state.pool).await.unwrap();
        // Simulate an older cleanup intent without a place snapshot or saved key.
        assert!(enqueue(&state, id, CleanupCommand::Revoke).await.unwrap());
        sqlx::query("UPDATE social.scene_stream_access SET admin_place_key=NULL WHERE id=$1")
            .bind(id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE social.scene_stream_cleanup SET source_context=jsonb_build_object('binding_room',$2::text,'admin_binding_pending',true),recipients='{}',recipients_resolved=false,metadata=NULL WHERE access_id=$1")
            .bind(id).bind(&room).execute(&state.pool).await.unwrap();
        sqlx::query("ALTER TABLE explore.world_scene_binding RENAME TO test_unavailable_binding")
            .execute(&state.pool)
            .await
            .unwrap();
        assert!(cleanup_access(&state, id).await.is_err());
        sqlx::query("ALTER TABLE explore.test_unavailable_binding RENAME TO world_scene_binding")
            .execute(&state.pool)
            .await
            .unwrap();
        let saved: (Option<String>,bool) = sqlx::query_as("SELECT source_context->>'place_id',recipients_resolved FROM social.scene_stream_cleanup WHERE access_id=$1")
            .bind(id).fetch_one(&state.pool).await.unwrap();
        assert_eq!(
            saved,
            (Some(place.clone()), false),
            "checkpoint context even when binding SQL fails"
        );
        assert!(directory.streaming_notifications.lock().unwrap().is_empty());
        sqlx::query("UPDATE explore.world_scene_place SET active=false,entity_id=NULL WHERE world_name=$1 AND entity_id='fanout-scene'")
            .bind(&name).execute(&state.pool).await.unwrap();
        sqlx::query("DELETE FROM explore.place WHERE id=$1")
            .bind(&place)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE social.scene_stream_cleanup SET due_at=timezone('UTC',now()) WHERE access_id=$1")
            .bind(id).execute(&state.pool).await.unwrap();
        let _ = cleanup_access(&state, id).await;
        let actual: std::collections::BTreeSet<_> = directory
            .streaming_notifications
            .lock()
            .unwrap()
            .iter()
            .map(|notice| notice.recipient.clone())
            .collect();
        assert_eq!(actual, (1..=4).map(wallet).collect());
    }

    #[tokio::test]
    async fn streaming_fanout_ambiguous_retired_binding_never_selects_a_new_incarnation() {
        for deferred in [false, true] {
            unresolvable_binding_fanout(true, deferred).await;
        }
    }

    #[tokio::test]
    async fn streaming_fanout_missing_pre_upgrade_binding_notifies_known_managers() {
        for deferred in [false, true] {
            unresolvable_binding_fanout(false, deferred).await;
        }
    }

    async fn unresolvable_binding_fanout(ambiguous: bool, deferred: bool) {
        let Some((state, directory, place)) = fanout_fixture(true).await else {
            return;
        };
        let name = format!("{place}.dcl.eth");
        let room = crate::rooms::worlds_service_scene_room(
            &crate::world_identity::WorldName::parse(&name).unwrap(),
            &crate::world_identity::SceneId::parse("fanout-scene").unwrap(),
        );
        let original: String = sqlx::query_scalar("SELECT identity::text FROM explore.world_scene_place WHERE world_name=$1 AND entity_id='fanout-scene'")
            .bind(&name).fetch_one(&state.pool).await.unwrap();
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO social.scene_stream_access(id,place_id,room_id,streaming_url,streaming_key,generated_by) VALUES ($1,$2,$3,'rtmp://example.invalid/live','test','test')")
            .bind(id).bind(&place).bind(&room).execute(&state.pool).await.unwrap();
        sqlx::query("UPDATE explore.world_scene_place SET active=false,entity_id=NULL WHERE world_name=$1 AND entity_id='fanout-scene'")
            .bind(&name).execute(&state.pool).await.unwrap();
        assert_eq!(
            notification_admin_key(&state.pool, id, Some(&room))
                .await
                .unwrap(),
            Some(original.clone())
        );
        if ambiguous {
            let replacement = Uuid::new_v4();
            sqlx::query("INSERT INTO explore.world_scene_place(world_name,identity,entity_id,parcels) VALUES ($1,$2,'fanout-scene',ARRAY['10,20'])")
                .bind(&name).bind(replacement).execute(&state.pool).await.unwrap();
            sqlx::query("INSERT INTO social.scene_admins(place_id,admin_address,added_by,added_at) VALUES ($1,$2,$3,timezone('UTC',now())-interval '1 hour')")
                .bind(replacement.to_string()).bind(wallet(11)).bind(wallet(1)).execute(&state.pool).await.unwrap();
        } else {
            // This retirement predates installation of the binding history trigger.
            sqlx::query("DELETE FROM explore.world_scene_binding WHERE world_name=$1")
                .bind(&name)
                .execute(&state.pool)
                .await
                .unwrap();
        }
        assert_eq!(
            notification_admin_key(&state.pool, id, Some(&room))
                .await
                .unwrap(),
            None
        );
        if deferred {
            sqlx::query(
                "ALTER TABLE explore.world_scene_binding RENAME TO test_unavailable_binding",
            )
            .execute(&state.pool)
            .await
            .unwrap();
        }
        assert!(enqueue(&state, id, CleanupCommand::Revoke).await.unwrap());
        if deferred {
            sqlx::query(
                "ALTER TABLE explore.test_unavailable_binding RENAME TO world_scene_binding",
            )
            .execute(&state.pool)
            .await
            .unwrap();
        }
        let evidence: (bool,bool,bool) = sqlx::query_as("SELECT (source_context->>'admin_binding_pending')::boolean,COALESCE((source_context->>'admin_binding_unresolvable')::boolean,false),recipients_resolved FROM social.scene_stream_cleanup WHERE access_id=$1")
            .bind(id).fetch_one(&state.pool).await.unwrap();
        assert_eq!(evidence, (deferred, !deferred, !deferred));
        *directory.fail_streaming_recipient.lock().unwrap() = Some(wallet(3));
        let _ = cleanup_access(&state, id).await;
        let progress: (bool,i32) = sqlx::query_as("SELECT recipients_resolved,recipient_cursor FROM social.scene_stream_cleanup WHERE access_id=$1")
            .bind(id).fetch_one(&state.pool).await.unwrap();
        assert_eq!(progress, (true, 1));
        let binding: (bool,bool) = sqlx::query_as("SELECT (source_context->>'admin_binding_pending')::boolean,(source_context->>'admin_binding_unresolvable')::boolean FROM social.scene_stream_cleanup WHERE access_id=$1")
            .bind(id).fetch_one(&state.pool).await.unwrap();
        assert_eq!(binding, (false, true));
        sqlx::query("UPDATE social.scene_stream_cleanup SET due_at=timezone('UTC',now()) WHERE access_id=$1")
            .bind(id).execute(&state.pool).await.unwrap();
        let _ = cleanup_access(&state, id).await;
        let actual: std::collections::BTreeSet<_> = directory
            .streaming_notifications
            .lock()
            .unwrap()
            .iter()
            .map(|notice| notice.recipient.clone())
            .collect();
        assert_eq!(
            actual,
            [wallet(1), wallet(3), wallet(4)].into_iter().collect(),
            "no local admins from either ambiguous incarnation or sibling"
        );
        assert_eq!(
            directory
                .streaming_attempts
                .lock()
                .unwrap()
                .iter()
                .filter(|recipient| **recipient == wallet(1))
                .count(),
            1
        );
        sqlx::query("UPDATE social.scene_stream_access SET admin_place_key=$2 WHERE id=$1")
            .bind(id)
            .bind(&original)
            .execute(&state.pool)
            .await
            .unwrap();
        assert_eq!(
            notification_admin_key(&state.pool, id, Some(&room))
                .await
                .unwrap(),
            Some(original)
        );
    }

    #[tokio::test]
    async fn streaming_fanout_deferred_binding_recovery_reads_inactive_credentials_and_keeps_evidence()
     {
        let Some((mut state, directory, place)) = fanout_fixture(true).await else {
            return;
        };
        let name = format!("{place}.dcl.eth");
        let room = crate::rooms::worlds_service_scene_room(
            &crate::world_identity::WorldName::parse(&name).unwrap(),
            &crate::world_identity::SceneId::parse("fanout-scene").unwrap(),
        );
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO social.scene_stream_access(id,place_id,room_id,streaming_url,streaming_key,generated_by) VALUES ($1,$2,$3,'rtmp://example.invalid/live','test','test')")
            .bind(id).bind(&place).bind(&room).execute(&state.pool).await.unwrap();
        let mut locked = state.pool.begin().await.unwrap();
        sqlx::query("LOCK explore.world_scene_place IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *locked)
            .await
            .unwrap();
        let result = tokio::time::timeout(
            Duration::from_secs(7),
            enqueue(&state, id, CleanupCommand::Revoke),
        )
        .await;
        locked.rollback().await.unwrap();
        assert!(
            result
                .expect("failed binding lookup must not prevent revocation")
                .unwrap()
        );
        let facts:(bool,bool,bool)=sqlx::query_as("SELECT a.active,c.recipients_resolved,(c.source_context->>'admin_binding_pending')::boolean FROM social.scene_stream_access a JOIN social.scene_stream_cleanup c ON c.access_id=a.id WHERE a.id=$1")
            .bind(id).fetch_one(&state.pool).await.unwrap();
        assert_eq!(facts, (false, false, true));
        let policy = state.policy.clone();
        state.policy = Arc::new(crate::interfaces::test_support::StubPolicy {
            grantees: Err(crate::AuthorityNotEstablished::store_unavailable(
                "test",
                &sqlx::Error::PoolClosed,
            )),
            ..Default::default()
        });
        assert!(cleanup_access(&state, id).await.is_err());
        let evidence:(Option<String>,bool)=sqlx::query_as("SELECT source_context->>'admin_place_key',recipients_resolved FROM social.scene_stream_cleanup WHERE access_id=$1")
            .bind(id).fetch_one(&state.pool).await.unwrap();
        assert!(evidence.0.is_some());
        assert!(!evidence.1);
        assert!(directory.streaming_notifications.lock().unwrap().is_empty());
        sqlx::query("UPDATE explore.world_scene_place SET active=false,entity_id=NULL WHERE world_name=$1 AND entity_id='fanout-scene'")
            .bind(&name).execute(&state.pool).await.unwrap();
        state.policy = policy;
        sqlx::query("UPDATE social.scene_stream_cleanup SET due_at=timezone('UTC',now()) WHERE access_id=$1")
            .bind(id).execute(&state.pool).await.unwrap();
        let _ = cleanup_access(&state, id).await;
        let actual: std::collections::BTreeSet<_> = directory
            .streaming_notifications
            .lock()
            .unwrap()
            .iter()
            .map(|n| n.recipient.clone())
            .collect();
        assert_eq!(actual, (1..=4).map(wallet).collect());
    }

    #[tokio::test]
    async fn streaming_fanout_revocation_does_not_wait_for_publication_locks() {
        let Some((state, directory, place)) = fanout_fixture(true).await else {
            return;
        };
        let name = format!("{place}.dcl.eth");
        sqlx::query("INSERT INTO worlds.worlds(name) VALUES ($1)")
            .bind(&name)
            .execute(&state.pool)
            .await
            .unwrap();
        let room = crate::rooms::worlds_service_scene_room(
            &crate::world_identity::WorldName::parse(&name).unwrap(),
            &crate::world_identity::SceneId::parse("fanout-scene").unwrap(),
        );
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO social.scene_stream_access(id,place_id,room_id,streaming_url,streaming_key,generated_by) VALUES ($1,$2,$3,'rtmp://example.invalid/live','test','test')")
            .bind(id).bind(&place).bind(&room).execute(&state.pool).await.unwrap();
        let mut publication = state.pool.begin().await.unwrap();
        sqlx::query("SELECT name FROM worlds.worlds WHERE name=$1 FOR UPDATE")
            .bind(&name)
            .execute(&mut *publication)
            .await
            .unwrap();
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            enqueue(&state, id, CleanupCommand::Revoke),
        )
        .await;
        publication.rollback().await.unwrap();
        assert!(
            result
                .expect("notification binding must not wait for publication")
                .unwrap()
        );
        sqlx::query("UPDATE explore.world_scene_place SET active=false,entity_id=NULL WHERE world_name=$1 AND entity_id='fanout-scene'")
            .bind(&name).execute(&state.pool).await.unwrap();
        let _ = cleanup_access(&state, id).await;
        let actual: std::collections::BTreeSet<_> = directory
            .streaming_notifications
            .lock()
            .unwrap()
            .iter()
            .map(|notice| notice.recipient.clone())
            .collect();
        assert_eq!(actual, (1..=4).map(wallet).collect());
    }

    #[tokio::test]
    async fn streaming_fanout_failed_local_capture_commits_revocation_and_retries_retired_binding()
    {
        let Some((state, directory, place)) = fanout_fixture(true).await else {
            return;
        };
        let name = format!("{place}.dcl.eth");
        sqlx::query("INSERT INTO worlds.worlds(name) VALUES ($1)")
            .bind(&name)
            .execute(&state.pool)
            .await
            .unwrap();
        let room = crate::rooms::worlds_service_scene_room(
            &crate::world_identity::WorldName::parse(&name).unwrap(),
            &crate::world_identity::SceneId::parse("fanout-scene").unwrap(),
        );
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO social.scene_stream_access(id,place_id,room_id,streaming_url,streaming_key,generated_by) VALUES ($1,$2,$3,'rtmp://example.invalid/live','test','test')")
            .bind(id).bind(&place).bind(&room).execute(&state.pool).await.unwrap();
        let mut locked = state.pool.begin().await.unwrap();
        sqlx::query("LOCK social.scene_admins IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *locked)
            .await
            .unwrap();
        let result = tokio::time::timeout(
            Duration::from_secs(7),
            enqueue(&state, id, CleanupCommand::Revoke),
        )
        .await;
        locked.rollback().await.unwrap();
        assert!(
            result
                .expect("notification-only reads must have a deadline")
                .unwrap()
        );
        let facts: (bool, bool, Option<String>) = sqlx::query_as("SELECT a.active,c.recipients_resolved,c.source_context->>'admin_place_key' FROM social.scene_stream_access a JOIN social.scene_stream_cleanup c ON c.access_id=a.id WHERE a.id=$1")
            .bind(id).fetch_one(&state.pool).await.unwrap();
        assert!(!facts.0);
        assert!(!facts.1);
        assert!(
            facts.2.is_some(),
            "exact binding evidence must survive failed admin capture"
        );
        sqlx::query("UPDATE explore.world_scene_place SET active=false,entity_id=NULL WHERE world_name=$1 AND entity_id='fanout-scene'")
            .bind(&name).execute(&state.pool).await.unwrap();
        let _ = cleanup_access(&state, id).await;
        let actual: std::collections::BTreeSet<_> = directory
            .streaming_notifications
            .lock()
            .unwrap()
            .iter()
            .map(|notice| notice.recipient.clone())
            .collect();
        assert_eq!(
            actual,
            (1..=4).map(wallet).collect(),
            "retain bound admins and exclude sibling admins"
        );
    }

    #[tokio::test]
    async fn streaming_fanout_startup_backfill_skips_a_scene_with_a_conflicting_retained_binding() {
        let Some((state, _directory, place)) = fanout_fixture(true).await else {
            return;
        };
        let name = format!("{place}.dcl.eth");
        sqlx::query("INSERT INTO worlds.worlds(name) VALUES ($1)")
            .bind(&name)
            .execute(&state.pool)
            .await
            .unwrap();
        // An earlier identity of the same scene left a retained binding: the
        // live one is ambiguous, so the backfill must not make it authoritative.
        sqlx::query("INSERT INTO explore.world_scene_binding(world_name,entity_id,identity) VALUES ($1,'fanout-scene',$2)")
            .bind(&name).bind(Uuid::new_v4()).execute(&state.pool).await.unwrap();
        let room = crate::rooms::worlds_service_scene_room(
            &crate::world_identity::WorldName::parse(&name).unwrap(),
            &crate::world_identity::SceneId::parse("fanout-scene").unwrap(),
        );
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO social.scene_stream_access(id,place_id,streaming_url,streaming_key,generated_by,room_id) VALUES ($1,$2,'rtmp://example.invalid/live','test','test',$3)")
            .bind(id).bind(&place).bind(&room).execute(&state.pool).await.unwrap();
        super::super::backfill_admin_place_keys(&state.pool)
            .await
            .unwrap();
        let saved: Option<String> = sqlx::query_scalar(
            "SELECT admin_place_key FROM social.scene_stream_access WHERE id=$1",
        )
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert_eq!(saved, None);
    }

    #[tokio::test]
    async fn streaming_fanout_reset_delivered_before_cleanup_failure_is_never_converted() {
        let Some((state, directory, place)) = fanout_fixture(false).await else {
            return;
        };
        let id = fanout_access(&state, &place, CleanupCommand::Reset).await;
        // The reset reached everyone, but ingress deletion kept failing, the
        // replacement was revoked in turn, and the grace period passed.
        let told = directory.streaming_notifications.lock().unwrap().len();
        sqlx::query("UPDATE social.scene_stream_cleanup SET ingress_deleted=false, ingress_id='IN_failing', notification_started=true, recipient_cursor=cardinality(recipients), occurred_at=timezone('UTC',now())-interval '2 hours', due_at=timezone('UTC',now()) WHERE access_id=$1")
            .bind(id).execute(&state.pool).await.unwrap();
        sqlx::query("DELETE FROM social.scene_stream_access WHERE place_id=$1 AND id<>$2")
            .bind(&place)
            .bind(id)
            .execute(&state.pool)
            .await
            .unwrap();
        // Whether ingress deletion fails again or not, the reset is never
        // turned into a revocation of the whole saved list.
        let _ = cleanup_access(&state, id).await;
        let row: Option<(String, i32, bool)> = sqlx::query_as(
            "SELECT notification_kind, recipient_cursor, notification_started FROM social.scene_stream_cleanup WHERE access_id=$1",
        )
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .unwrap();
        if let Some((kind, cursor, started)) = row {
            assert_eq!(kind, "streaming_key_reset");
            assert!(started && cursor > 0, "the delivery position is kept");
        }
        let notices = directory.streaming_notifications.lock().unwrap();
        assert_eq!(notices.len(), told, "nobody is told again");
        assert!(
            notices
                .iter()
                .all(|n| n.kind != StreamingNotificationKind::KeyRevoke)
        );
    }

    #[tokio::test]
    async fn streaming_fanout_native_upgrade_backfills_before_retirement_with_sibling() {
        let Some((state, directory, place)) = fanout_fixture(true).await else {
            return;
        };
        let admin_key: String = sqlx::query_scalar(
            "SELECT identity::text FROM explore.world_scene_place WHERE entity_id='fanout-scene'",
        )
        .fetch_one(&state.pool)
        .await
        .unwrap();
        let name = format!("{place}.dcl.eth");
        sqlx::query("INSERT INTO worlds.worlds(name) VALUES ($1)")
            .bind(&name)
            .execute(&state.pool)
            .await
            .unwrap();
        for entity in ["fanout-scene", "unrelated-scene"] {
            sqlx::query("INSERT INTO worlds.world_scenes(world_name,entity_id,deployment_auth_chain,entity,deployer,parcels,size) VALUES ($1,$2,'[]','{}','owner',ARRAY['10,20'],0)")
                .bind(&name).bind(entity).execute(&state.pool).await.unwrap();
        }
        let room = crate::rooms::worlds_service_scene_room(
            &crate::world_identity::WorldName::parse(&name).unwrap(),
            &crate::world_identity::SceneId::parse("fanout-scene").unwrap(),
        );
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO social.scene_stream_access(id,place_id,streaming_url,streaming_key,generated_by,room_id) VALUES ($1,$2,'rtmp://example.invalid/live','test','test',$3)")
            .bind(id).bind(&place).bind(&room).execute(&state.pool).await.unwrap();
        super::super::backfill_admin_place_keys(&state.pool)
            .await
            .unwrap();
        let saved: Option<String> = sqlx::query_scalar(
            "SELECT admin_place_key FROM social.scene_stream_access WHERE id=$1",
        )
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert_eq!(saved.as_deref(), Some(admin_key.as_str()));
        sqlx::query(
            "DELETE FROM worlds.world_scenes WHERE world_name=$1 AND entity_id='fanout-scene'",
        )
        .bind(&name)
        .execute(&state.pool)
        .await
        .unwrap();
        sqlx::query("UPDATE explore.world_scene_place SET active=false,entity_id=NULL WHERE identity::text=$1").bind(&admin_key).execute(&state.pool).await.unwrap();
        sqlx::query("INSERT INTO social.scene_admins(place_id,admin_address,added_by) VALUES ($1,'invalid-historical-admin','operator')")
            .bind(&admin_key).execute(&state.pool).await.unwrap();
        assert!(enqueue(&state, id, CleanupCommand::Revoke).await.unwrap());
        // A real room keeps independent LiveKit eviction pending in this
        // database-only fixture; notification delivery must still complete.
        assert!(
            cleanup_access(&state, id)
                .await
                .unwrap_err()
                .message
                .contains("cast eviction")
        );
        let notices = directory.streaming_notifications.lock().unwrap();
        assert_eq!(
            notices
                .iter()
                .map(|notice| notice.recipient.clone())
                .collect::<std::collections::BTreeSet<_>>(),
            (1..=4).map(wallet).collect()
        );
    }

    #[tokio::test]
    async fn streaming_fanout_every_key_notice_reaches_all_genesis_managers() {
        all_key_notice_recipients(false).await;
    }

    #[tokio::test]
    async fn streaming_fanout_every_key_notice_reaches_world_managers() {
        all_key_notice_recipients(true).await;
    }

    async fn all_key_notice_recipients(world: bool) {
        let Some((state, directory, place)) = fanout_fixture(world).await else {
            return;
        };
        let expected: std::collections::BTreeSet<_> =
            (1..=if world { 4 } else { 6 }).map(wallet).collect();
        for command in [
            CleanupCommand::Reset,
            CleanupCommand::Revoke,
            CleanupCommand::KeyExpiry,
            CleanupCommand::DurationExpiry,
        ] {
            // A world key names its scene; a key without one tells no scene admin.
            let key = world.then(|| "fanout-scene");
            let id = fanout_access_bound(&state, &place, command, key).await;
            cleanup_access(&state, id).await.unwrap();
            {
                let notices = directory.streaming_notifications.lock().unwrap();
                let actual: std::collections::BTreeSet<_> =
                    notices.iter().map(|n| n.recipient.clone()).collect();
                assert_eq!(
                    actual, expected,
                    "{command:?}: every affected manager must receive the notice"
                );
                assert_eq!(
                    notices.len(),
                    expected.len(),
                    "overlapping manager roles must be deduplicated"
                );
                for notice in notices.iter() {
                    assert_eq!(notice.kind, command.notification().unwrap());
                    assert_eq!(notice.metadata["address"], notice.recipient.as_str());
                }
            }
            directory.streaming_notifications.lock().unwrap().clear();
            sqlx::query("DELETE FROM social.scene_stream_access WHERE place_id=$1")
                .bind(&place)
                .execute(&state.pool)
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    async fn streaming_fanout_deferred_key_notice_keeps_admins_granted_at_the_event() {
        let Some((state, directory, place)) = fanout_fixture(true).await else {
            return;
        };
        let id =
            fanout_access_bound(&state, &place, CleanupCommand::Revoke, Some("fanout-scene")).await;
        let saved: serde_json::Value = sqlx::query_scalar(
            "SELECT source_context->'admin_grants' FROM social.scene_stream_cleanup WHERE access_id=$1",
        )
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
        let original = serde_json::json!([wallet(2).as_str(), wallet(1).as_str()]);
        assert!(
            saved
                .as_array()
                .is_some_and(|pairs| pairs.contains(&original)),
            "preparation froze each admin with its granter: {saved}"
        );
        // As if the authority lookup had failed: the list is resolved later,
        // after a wallet without authority re-granted wallet(2).
        sqlx::query("UPDATE social.scene_stream_cleanup SET recipients='{}',recipients_resolved=false WHERE access_id=$1")
            .bind(id).execute(&state.pool).await.unwrap();
        sqlx::query("UPDATE social.scene_admins SET added_by=$2,added_at=timezone('UTC',now())+interval '1 second',active=true,removed_at=NULL WHERE admin_address=$1")
            .bind(wallet(2)).bind(wallet(12)).execute(&state.pool).await.unwrap();
        cleanup_access(&state, id).await.unwrap();
        let actual: std::collections::BTreeSet<_> = directory
            .streaming_notifications
            .lock()
            .unwrap()
            .iter()
            .map(|notice| notice.recipient.clone())
            .collect();
        assert_eq!(
            actual,
            (1..=4).map(wallet).collect(),
            "the admin granted by the owner at the event is still told"
        );
    }

    #[tokio::test]
    async fn streaming_fanout_unprepared_key_notice_keeps_admins_granted_at_the_event() {
        unprepared_regrant(true).await;
    }

    #[tokio::test]
    async fn streaming_fanout_unbound_key_notice_keeps_admins_granted_at_the_event() {
        unprepared_regrant(false).await;
    }

    /// Preparation cannot read the place context, so the recipients are
    /// resolved on a retry, after a wallet without authority re-granted
    /// wallet(2). `saved_key`: the credential already names its scene place;
    /// otherwise the retry recovers it from the room.
    async fn unprepared_regrant(saved_key: bool) {
        let Some((state, directory, place)) = fanout_fixture(true).await else {
            return;
        };
        let name = format!("{place}.dcl.eth");
        sqlx::query("INSERT INTO worlds.worlds(name) VALUES ($1)")
            .bind(&name)
            .execute(&state.pool)
            .await
            .unwrap();
        let room = crate::rooms::worlds_service_scene_room(
            &crate::world_identity::WorldName::parse(&name).unwrap(),
            &crate::world_identity::SceneId::parse("fanout-scene").unwrap(),
        );
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO social.scene_stream_access(id,place_id,room_id,streaming_url,streaming_key,generated_by,admin_place_key) VALUES ($1,$2,$3,'rtmp://example.invalid/live','test','test',CASE WHEN $4 THEN (SELECT identity::text FROM explore.world_scene_place WHERE world_name=$5 AND entity_id='fanout-scene') END)")
            .bind(id).bind(&place).bind(&room).bind(saved_key).bind(&name).execute(&state.pool).await.unwrap();
        let mut locked = state.pool.begin().await.unwrap();
        sqlx::query("LOCK explore.place IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *locked)
            .await
            .unwrap();
        let result = tokio::time::timeout(
            Duration::from_secs(12),
            enqueue(&state, id, CleanupCommand::Revoke),
        )
        .await;
        locked.rollback().await.unwrap();
        assert!(result.expect("preparation has a deadline").unwrap());
        let resolved: bool = sqlx::query_scalar(
            "SELECT recipients_resolved FROM social.scene_stream_cleanup WHERE access_id=$1",
        )
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert!(!resolved, "the place context was unavailable");
        sqlx::query("UPDATE social.scene_admins SET added_by=$2,added_at=timezone('UTC',now())+interval '1 second',active=true,removed_at=NULL WHERE admin_address=$1")
            .bind(wallet(2)).bind(wallet(12)).execute(&state.pool).await.unwrap();
        let _ = cleanup_access(&state, id).await;
        let actual: std::collections::BTreeSet<_> = directory
            .streaming_notifications
            .lock()
            .unwrap()
            .iter()
            .map(|notice| notice.recipient.clone())
            .collect();
        assert_eq!(
            actual,
            (1..=4).map(wallet).collect(),
            "the admin granted by the owner at the event is still told"
        );
    }

    #[tokio::test]
    async fn streaming_fanout_partial_delivery_keeps_original_set() {
        let Some((mut state, directory, place)) = fanout_fixture(false).await else {
            return;
        };
        let id = fanout_access(&state, &place, CleanupCommand::Revoke).await;
        *directory.fail_streaming_recipient.lock().unwrap() = Some(wallet(2));
        assert!(
            cleanup_access(&state, id).await.is_err(),
            "the second recipient must be attempted"
        );
        assert_eq!(directory.streaming_notifications.lock().unwrap().len(), 1);
        sqlx::query("UPDATE social.scene_admins SET active=false,removed_at=timezone('UTC',now()) WHERE place_id='10,20'")
            .execute(&state.pool).await.unwrap();
        state.policy = Arc::new(crate::interfaces::test_support::StubPolicy::default());
        sqlx::query("UPDATE social.scene_stream_cleanup SET due_at=timezone('UTC',now()) WHERE access_id=$1")
            .bind(id).execute(&state.pool).await.unwrap();
        cleanup_access(&state, id).await.unwrap();
        {
            let notices = directory.streaming_notifications.lock().unwrap();
            assert_eq!(
                notices.len(),
                6,
                "a retry must retain admins and external manager addresses from preparation"
            );
            assert_eq!(
                notices
                    .iter()
                    .map(|n| n.recipient.clone())
                    .collect::<std::collections::BTreeSet<_>>(),
                (1..=6).map(wallet).collect()
            );
        }
        let pending: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM social.scene_stream_cleanup WHERE access_id=$1)",
        )
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert!(
            !pending,
            "the intent is acknowledged only after the complete fanout"
        );
        assert_eq!(
            *directory.streaming_attempts.lock().unwrap(),
            [1, 2, 2, 3, 4, 5, 6].map(wallet),
            "a successful prefix is checkpointed, rather than replayed on every pass"
        );
    }

    #[tokio::test]
    async fn streaming_fanout_partial_expiry_completes_after_key_regeneration() {
        let Some((state, directory, place)) = fanout_fixture(false).await else {
            return;
        };
        let id = fanout_access(&state, &place, CleanupCommand::KeyExpiry).await;
        *directory.fail_streaming_recipient.lock().unwrap() = Some(wallet(2));
        assert!(cleanup_access(&state, id).await.is_err());
        sqlx::query("INSERT INTO social.scene_stream_access(place_id,streaming_url,streaming_key,generated_by) VALUES ($1,'rtmp://example.invalid/live','replacement','test')")
            .bind(&place).execute(&state.pool).await.unwrap();
        sqlx::query("UPDATE social.scene_stream_cleanup SET due_at=timezone('UTC',now()) WHERE access_id=$1").bind(id).execute(&state.pool).await.unwrap();
        cleanup_access(&state, id).await.unwrap();
        let notices = directory.streaming_notifications.lock().unwrap();
        assert_eq!(notices.len(), 6);
        assert!(
            notices
                .iter()
                .all(|notice| notice.kind == StreamingNotificationKind::KeyExpired)
        );
        assert_eq!(
            notices
                .iter()
                .map(|notice| notice.recipient.clone())
                .collect::<std::collections::BTreeSet<_>>(),
            (1..=6).map(wallet).collect()
        );
    }

    #[tokio::test]
    async fn streaming_fanout_accepted_expiry_replays_after_lost_checkpoint_and_regeneration() {
        let Some((state, directory, place)) = fanout_fixture(false).await else {
            return;
        };
        let id = fanout_access(&state, &place, CleanupCommand::KeyExpiry).await;
        *directory.fail_streaming_recipient.lock().unwrap() = Some(wallet(2));
        assert!(cleanup_access(&state, id).await.is_err());
        sqlx::query("UPDATE social.scene_stream_cleanup SET recipient_cursor=0 WHERE access_id=$1 AND notification_started").bind(id).execute(&state.pool).await.unwrap();
        sqlx::query("INSERT INTO social.scene_stream_access(place_id,streaming_url,streaming_key,generated_by) VALUES ($1,'rtmp://example.invalid/live','replacement','test')")
            .bind(&place).execute(&state.pool).await.unwrap();
        sqlx::query("UPDATE social.scene_stream_cleanup SET due_at=timezone('UTC',now()) WHERE access_id=$1").bind(id).execute(&state.pool).await.unwrap();
        cleanup_access(&state, id).await.unwrap();
        let notices = directory.streaming_notifications.lock().unwrap();
        assert_eq!(notices.len(), 6);
        assert!(
            notices
                .iter()
                .all(|notice| notice.kind == StreamingNotificationKind::KeyExpired)
        );
        assert_eq!(
            notices
                .iter()
                .map(|notice| notice.recipient.clone())
                .collect::<std::collections::BTreeSet<_>>(),
            (1..=6).map(wallet).collect()
        );
    }

    #[tokio::test]
    async fn streaming_fanout_resumes_after_the_production_sixty_second_budget() {
        use std::sync::atomic::Ordering;
        let Some((mut state, directory, place)) = fanout_fixture(false).await else {
            return;
        };
        state.policy = Arc::new(crate::interfaces::test_support::StubPolicy {
            land: Ok(Some(Arc::new(Some(crate::access::LandOperators {
                owner: wallet(1).to_string(),
                operator: None,
                update_operator: None,
                update_managers: (2..=14).map(|n| wallet(n).to_string()).collect(),
                approved_for_all: vec![],
            })))),
            ..Default::default()
        });
        let id = fanout_access(&state, &place, CleanupCommand::Revoke).await;
        directory.streaming_delay_ms.store(4_900, Ordering::SeqCst);
        let state = Arc::new(state);
        // Keep the clock from auto-advancing while real PostgreSQL I/O finishes.
        // Each explicit advance models one successful slow Social write.
        let awake = tokio::spawn(async {
            loop {
                tokio::task::yield_now().await;
            }
        });
        tokio::time::pause();
        let work_state = state.clone();
        let pass = tokio::spawn(async move {
            run_budgeted(
                &tokio_util::sync::CancellationToken::new(),
                cleanup_access(&work_state, id),
            )
            .await
        });
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        for index in 0..12 {
            while directory.streaming_attempts.lock().unwrap().len() <= index {
                assert!(
                    std::time::Instant::now() < deadline,
                    "timed out waiting for the next notification attempt"
                );
                tokio::task::yield_now().await;
            }
            tokio::time::advance(Duration::from_millis(4_901)).await;
            loop {
                let cursor: i32 = sqlx::query_scalar(
                    "SELECT recipient_cursor FROM social.scene_stream_cleanup WHERE access_id=$1",
                )
                .bind(id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
                if cursor == index as i32 + 1 {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "timed out waiting for durable delivery progress"
                );
                tokio::task::yield_now().await;
            }
        }
        while directory.streaming_attempts.lock().unwrap().len() < 13 {
            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for interrupted notification"
            );
            tokio::task::yield_now().await;
        }
        tokio::time::advance(Duration::from_millis(1_200)).await;
        assert!(
            pass.await.unwrap().is_err(),
            "the production 60-second budget must cancel the unfinished suffix"
        );
        tokio::time::resume();
        awake.abort();
        let cursor: i32 = sqlx::query_scalar(
            "SELECT recipient_cursor FROM social.scene_stream_cleanup WHERE access_id=$1",
        )
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert_eq!(cursor, 12);
        directory.streaming_delay_ms.store(0, Ordering::SeqCst);
        // A restarted worker resumes after the canceled worker's lease expires.
        sqlx::query("UPDATE social.scene_stream_cleanup SET claim_until=timezone('UTC',now())-interval '1 second' WHERE access_id=$1")
            .bind(id).execute(&state.pool).await.unwrap();
        cleanup_access(&state, id).await.unwrap();
        assert_eq!(directory.streaming_notifications.lock().unwrap().len(), 14);
        let attempts = directory.streaming_attempts.lock().unwrap();
        assert_eq!(
            attempts.iter().filter(|a| **a == wallet(1)).count(),
            1,
            "the accepted prefix must not consume the next pass budget"
        );
        assert_eq!(
            attempts.iter().filter(|a| **a == wallet(13)).count(),
            2,
            "only the interrupted recipient is replayed"
        );
    }

    #[tokio::test]
    async fn streaming_fanout_expands_scalar_era_intents_without_losing_the_saved_recipient() {
        let Some((state, directory, place)) = fanout_fixture(false).await else {
            return;
        };
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO social.scene_stream_access (id,place_id,streaming_url,streaming_key,generated_by) VALUES ($1,$2,'rtmp://example.invalid/live','scalar-test','test')")
            .bind(id).bind(&place).execute(&state.pool).await.unwrap();
        let context = RemovedPlaces::new(state.pool.clone())
            .place(&place)
            .await
            .unwrap()
            .unwrap();
        let context = serde_json::to_value(context).unwrap();
        let event = StreamingNotification {
            event_key: format!("streaming_key_revoke-{id}"),
            recipient: wallet(11),
            kind: StreamingNotificationKind::KeyRevoke,
            occurred_at: chrono::Utc::now().naive_utc(),
            metadata: serde_json::json!({"title":"saved template","address":wallet(11).as_str()}),
        };
        assert!(
            enqueue_prepared(
                &state.pool,
                id,
                CleanupCommand::Revoke,
                0,
                Some(&event),
                Some(&context),
                None
            )
            .await
            .unwrap()
        );
        cleanup_access(&state, id).await.unwrap();
        let notices = directory.streaming_notifications.lock().unwrap();
        let actual: std::collections::BTreeSet<_> =
            notices.iter().map(|n| n.recipient.clone()).collect();
        let mut expected: std::collections::BTreeSet<_> = (1..=6).map(wallet).collect();
        expected.insert(wallet(11));
        assert_eq!(actual, expected);
        assert!(
            notices
                .iter()
                .all(|n| n.metadata["title"] == "saved template"
                    && n.metadata["address"] == n.recipient.as_str())
        );
    }

    #[tokio::test]
    async fn streaming_fanout_reset_conversion_keeps_the_resolved_recipient_snapshot() {
        let Some((mut state, directory, place)) = fanout_fixture(false).await else {
            return;
        };
        let id = fanout_access(&state, &place, CleanupCommand::Reset).await;
        sqlx::query("DELETE FROM social.scene_stream_access WHERE place_id=$1 AND id<>$2")
            .bind(&place)
            .bind(id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE social.scene_stream_cleanup SET occurred_at=timezone('UTC',now())-interval '2 hours' WHERE access_id=$1")
            .bind(id).execute(&state.pool).await.unwrap();
        assert!(
            cleanup_access(&state, id).await.is_err(),
            "reset without replacement converts to revoke on retry"
        );
        state.policy = Arc::new(crate::interfaces::test_support::StubPolicy::default());
        sqlx::query("UPDATE social.scene_admins SET active=false,removed_at=timezone('UTC',now()) WHERE place_id='10,20'")
            .execute(&state.pool).await.unwrap();
        sqlx::query("UPDATE social.scene_stream_cleanup SET due_at=timezone('UTC',now()) WHERE access_id=$1")
            .bind(id).execute(&state.pool).await.unwrap();
        cleanup_access(&state, id).await.unwrap();
        let notices = directory.streaming_notifications.lock().unwrap();
        assert_eq!(notices.len(), 6);
        assert!(
            notices
                .iter()
                .all(|n| n.kind == StreamingNotificationKind::KeyRevoke
                    && n.metadata["title"] == "Stream Key Deactivated")
        );
    }

    #[test]
    fn cleanup_commands_keep_persisted_labels_and_reject_non_cleanup_kinds() {
        for (command, label) in [
            (CleanupCommand::Reset, "streaming_key_reset"),
            (CleanupCommand::Revoke, "streaming_key_revoke"),
            (CleanupCommand::KeyExpiry, "streaming_key_expired"),
            (CleanupCommand::DurationExpiry, "streaming_time_exceeded"),
        ] {
            assert_eq!(command.notification().unwrap().as_str(), label);
            assert_eq!(
                notification_kind(label).unwrap(),
                command.notification().unwrap()
            );
            assert!(command.permits_running());
        }
        assert!(CleanupCommand::IdleReplacement.notification().is_none());
        assert!(!CleanupCommand::IdleReplacement.permits_running());
        for invalid in [
            "streaming_place_updated",
            "",
            "STREAMING_KEY_RESET",
            "unknown",
        ] {
            assert_eq!(
                notification_kind(invalid).unwrap_err().message,
                "unknown streaming notification kind"
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn production_ttl_orchestration_reconciles_during_owner_backlog_and_failure() {
        for fail_immediately in [false, true] {
            let token = tokio_util::sync::CancellationToken::new();
            let reconciled = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let observed = reconciled.clone();
            let pass = tokio::spawn(async move {
                run_budgeted(
                    &token,
                    run_streaming_workflows(
                        async move {
                            if fail_immediately {
                                return Err(ApiError::internal("owner source failed"));
                            }
                            for _ in 0..20 {
                                tokio::time::sleep(Duration::from_secs(5)).await;
                            }
                            Ok(())
                        },
                        async move {
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            observed.store(true, std::sync::atomic::Ordering::SeqCst);
                            Ok(())
                        },
                    ),
                )
                .await
            });
            tokio::task::yield_now().await;
            tokio::time::advance(Duration::from_secs(2)).await;
            tokio::task::yield_now().await;
            assert!(
                reconciled.load(std::sync::atomic::Ordering::SeqCst),
                "production TTL orchestration must poll reconciliation before owner backlog completes"
            );
            if !fail_immediately {
                assert!(!pass.is_finished());
                tokio::time::advance(Duration::from_secs(59)).await;
            }
            assert!(pass.await.unwrap().is_err());
        }
    }

    #[tokio::test(start_paused = true)]
    async fn cleanup_budget_progresses_independently_of_repeated_owner_timeouts() {
        let cancel = tokio_util::sync::CancellationToken::new();
        let deletions = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        for _ in 0..2 {
            let owner_cancel = cancel.clone();
            let enforcement = tokio::spawn(async move {
                run_budgeted(&owner_cancel, async {
                    for _ in 0..20 {
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                    Ok(())
                })
                .await
            });
            let cleanup_cancel = cancel.clone();
            let attempted = deletions.clone();
            let cleanup = tokio::spawn(async move {
                run_budgeted(&cleanup_cancel, async {
                    // A failed due delivery does not consume a different workflow's budget.
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    attempted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                })
                .await
            });
            tokio::task::yield_now().await;
            tokio::time::advance(Duration::from_secs(6)).await;
            assert!(cleanup.await.unwrap().is_ok());
            assert!(
                !enforcement.is_finished(),
                "cleanup ran while owner work remained blocked"
            );
            tokio::time::advance(Duration::from_secs(55)).await;
            assert!(enforcement.await.unwrap().is_err());
        }
        assert_eq!(deletions.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn cancellation_drops_preparation_and_delivery_and_allows_next_pass() {
        for _stage in ["preparation", "delivery"] {
            let token = tokio_util::sync::CancellationToken::new();
            let child = token.clone();
            let stopped = Arc::new(std::sync::atomic::AtomicBool::new(false));
            struct OnDrop(Arc<std::sync::atomic::AtomicBool>);
            impl Drop for OnDrop {
                fn drop(&mut self) {
                    self.0.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            }
            let guard = OnDrop(stopped.clone());
            let pass = tokio::spawn(async move {
                run_budgeted(&child, async move {
                    let _guard = guard;
                    std::future::pending::<()>().await;
                    Ok(())
                })
                .await
            });
            tokio::task::yield_now().await;
            token.cancel();
            assert!(pass.await.unwrap().is_ok());
            assert!(stopped.load(std::sync::atomic::Ordering::SeqCst));
            let fresh = tokio_util::sync::CancellationToken::new();
            assert!(run_budgeted(&fresh, async { Ok(()) }).await.is_ok());
        }
    }

    #[test]
    fn settling_window_and_check_cadence_protect_new_streams_from_inactive_reads() {
        let now = chrono::DateTime::from_timestamp(1_700_000_000, 0)
            .unwrap()
            .naive_utc();
        let mut row = Snapshot {
            id: Uuid::nil(),
            ingress_id: "IN_test".into(),
            streaming: true,
            streaming_start_time: Some(now),
            streaming_state_version: 0,
            streaming_checked_at: None,
        };
        assert!(!row.eligible(now));
        assert!(!row.eligible(now + chrono::Duration::seconds(119)));
        assert!(row.eligible(now + chrono::Duration::seconds(120)));
        row.streaming_checked_at = Some(now + chrono::Duration::seconds(120));
        assert!(!row.eligible(now + chrono::Duration::seconds(179)));
        assert!(row.eligible(now + chrono::Duration::seconds(180)));
        row.streaming_start_time = None;
        assert!(!row.eligible(now + chrono::Duration::hours(1)));
        // A flag that is down has no settling window.
        row.streaming = false;
        assert!(row.eligible(now + chrono::Duration::hours(1)));
    }

    #[tokio::test]
    async fn cleanup_intent_and_deactivation_commit_together_and_stream_start_blocks_idle_renewal()
    {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            eprintln!(
                "SKIP: streaming lifecycle PostgreSQL test requires CATALYST_TEST_DATABASE_URL"
            );
            return;
        };
        let id = Uuid::new_v4();
        let place_id = format!("stream-cleanup-test-{id}");
        sqlx::query("INSERT INTO social.scene_stream_access(id,place_id,streaming_url,streaming_key,generated_by,streaming) VALUES($1,$2,'rtmp://example.org','test-key','test',true)")
            .bind(id).bind(&place_id).execute(&pool).await.unwrap();
        let event = StreamingNotification {
            event_key: format!("streaming_key_revoke-{id}"),
            recipient: "0x0000000000000000000000000000000000000001"
                .parse()
                .unwrap(),
            kind: StreamingNotificationKind::KeyRevoke,
            metadata: serde_json::json!({"title":"snapshot"}),
            occurred_at: chrono::Utc::now().naive_utc(),
        };
        assert!(
            !enqueue_prepared(
                &pool,
                id,
                CleanupCommand::IdleReplacement,
                0,
                None,
                None,
                None
            )
            .await
            .unwrap()
        );
        let unchanged: (bool, bool, bool) = sqlx::query_as(
            "SELECT active,streaming,EXISTS(SELECT 1 FROM social.scene_stream_cleanup WHERE access_id=$1) FROM social.scene_stream_access WHERE id=$1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(unchanged, (true, true, false));
        // Revoke intentionally permits a running stream; IdleReplacement is the
        // only non-forced command exposed by the lifecycle API.
        // A start/end webhook after metadata capture invalidates that snapshot.
        assert!(
            !enqueue_prepared(
                &pool,
                id,
                CleanupCommand::Revoke,
                1,
                Some(&event),
                None,
                None
            )
            .await
            .unwrap()
        );
        assert!(
            enqueue_prepared(
                &pool,
                id,
                CleanupCommand::Revoke,
                0,
                Some(&event),
                None,
                None
            )
            .await
            .unwrap()
        );
        assert!(
            !enqueue_prepared(
                &pool,
                id,
                CleanupCommand::Reset,
                0,
                Some(&event),
                None,
                None
            )
            .await
            .unwrap()
        );
        let facts: (bool,String,bool) = sqlx::query_as("SELECT a.active,c.notification_kind,c.ingress_deleted FROM social.scene_stream_access a JOIN social.scene_stream_cleanup c ON a.id=c.access_id WHERE a.id=$1")
            .bind(id).fetch_one(&pool).await.unwrap();
        assert_eq!(facts, (false, "streaming_key_revoke".into(), false));
        let stored: (Address, serde_json::Value) = sqlx::query_as(
            "SELECT recipient,metadata FROM social.scene_stream_cleanup WHERE access_id=$1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(stored, (event.recipient, event.metadata));
        sqlx::query("DELETE FROM social.scene_stream_cleanup WHERE access_id=$1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM social.scene_stream_access WHERE id=$1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        // The same idle command succeeds once the stream has stopped and commits
        // its cleanup intent together with deactivation, without a notification.
        let idle = Uuid::new_v4();
        sqlx::query("INSERT INTO social.scene_stream_access(id,place_id,streaming_url,streaming_key,generated_by,streaming) VALUES($1,$2,'rtmp://example.org','idle-key','test',false)")
            .bind(idle).bind(format!("idle-test-{idle}")).execute(&pool).await.unwrap();
        assert!(
            enqueue_prepared(
                &pool,
                idle,
                CleanupCommand::IdleReplacement,
                0,
                None,
                None,
                None
            )
            .await
            .unwrap()
        );
        let idle_facts: (bool, bool, Option<String>) = sqlx::query_as(
            "SELECT a.active,a.streaming,c.notification_kind FROM social.scene_stream_access a JOIN social.scene_stream_cleanup c ON a.id=c.access_id WHERE a.id=$1",
        ).bind(idle).fetch_one(&pool).await.unwrap();
        assert_eq!(idle_facts, (false, false, None));
        sqlx::query("DELETE FROM social.scene_stream_cleanup WHERE access_id=$1")
            .bind(idle)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM social.scene_stream_access WHERE id=$1")
            .bind(idle)
            .execute(&pool)
            .await
            .unwrap();

        // The four-day credential deadline waits for a stream running since before it.
        let running = Uuid::new_v4();
        sqlx::query("INSERT INTO social.scene_stream_access(id,place_id,streaming_url,streaming_key,generated_by,streaming,streaming_start_time,expiration_time) VALUES($1,$2,'rtmp://example.org','test-key','test',true,timezone('UTC',now())-interval '1 minute',timezone('UTC',now())+interval '4 days')")
            .bind(running).bind(format!("deadline-test-{running}")).execute(&pool).await.unwrap();
        assert!(
            !enqueue_prepared(
                &pool,
                running,
                CleanupCommand::KeyExpiry,
                0,
                None,
                None,
                None
            )
            .await
            .unwrap()
        );
        sqlx::query("UPDATE social.scene_stream_access SET expiration_time=timezone('UTC',now())-interval '1 second' WHERE id=$1")
            .bind(running).execute(&pool).await.unwrap();
        assert!(
            !enqueue_prepared(
                &pool,
                running,
                CleanupCommand::KeyExpiry,
                0,
                None,
                None,
                None
            )
            .await
            .unwrap(),
            "a live broadcast keeps its expired key"
        );
        sqlx::query("UPDATE social.scene_stream_access SET streaming=false WHERE id=$1")
            .bind(running)
            .execute(&pool)
            .await
            .unwrap();
        // Unresolved historic owner does not prevent durable revocation intent.
        assert!(
            enqueue_prepared(
                &pool,
                running,
                CleanupCommand::KeyExpiry,
                0,
                None,
                None,
                None
            )
            .await
            .unwrap()
        );
        let stopped: bool = sqlx::query_scalar(
            "SELECT NOT active AND NOT streaming FROM social.scene_stream_access WHERE id=$1",
        )
        .bind(running)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(stopped);
        sqlx::query("DELETE FROM social.scene_stream_cleanup WHERE access_id=$1")
            .bind(running)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM social.scene_stream_access WHERE id=$1")
            .bind(running)
            .execute(&pool)
            .await
            .unwrap();
        // A streaming flag with no recorded start has no grace.
        let unstarted = Uuid::new_v4();
        sqlx::query("INSERT INTO social.scene_stream_access(id,place_id,streaming_url,streaming_key,generated_by,streaming,expiration_time) VALUES($1,$2,'rtmp://example.org','test-key','test',true,timezone('UTC',now())-interval '1 second')")
            .bind(unstarted).bind(format!("unstarted-test-{unstarted}")).execute(&pool).await.unwrap();
        assert!(
            enqueue_prepared(
                &pool,
                unstarted,
                CleanupCommand::KeyExpiry,
                0,
                None,
                None,
                None
            )
            .await
            .unwrap()
        );
        sqlx::query("DELETE FROM social.scene_stream_cleanup WHERE access_id=$1")
            .bind(unstarted)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM social.scene_stream_access WHERE id=$1")
            .bind(unstarted)
            .execute(&pool)
            .await
            .unwrap();
        let settling = Uuid::new_v4();
        let now = chrono::Utc::now().naive_utc();
        sqlx::query("INSERT INTO social.scene_stream_access(id,place_id,ingress_id,streaming_url,streaming_key,generated_by,streaming,streaming_start_time) VALUES($1,$2,'IN_settling','rtmp://example.org','test','test',true,$3)")
            .bind(settling).bind(format!("settling-test-{settling}")).bind(now).execute(&pool).await.unwrap();
        let mut snapshot = Snapshot {
            id: settling,
            ingress_id: "IN_settling".into(),
            streaming: true,
            streaming_start_time: Some(now),
            streaming_state_version: 0,
            streaming_checked_at: None,
        };
        assert_eq!(
            clear_stale(&pool, &snapshot, now).await.unwrap(),
            0,
            "recent start cannot be cleared"
        );
        let started = now - chrono::Duration::minutes(3);
        snapshot.streaming_start_time = Some(started);
        sqlx::query("UPDATE social.scene_stream_access SET streaming_start_time=$2,streaming_checked_at=$3 WHERE id=$1")
            .bind(settling).bind(started).bind(now).execute(&pool).await.unwrap();
        assert_eq!(
            clear_stale(&pool, &snapshot, now).await.unwrap(),
            0,
            "recently checked cannot be cleared"
        );
        sqlx::query("UPDATE social.scene_stream_access SET streaming_checked_at=NULL,streaming_state_version=1 WHERE id=$1")
            .bind(settling).execute(&pool).await.unwrap();
        assert_eq!(
            clear_stale(&pool, &snapshot, now).await.unwrap(),
            0,
            "duplicate start after snapshot fences stale read"
        );
        snapshot.streaming_state_version = 1;
        assert_eq!(
            clear_stale(&pool, &snapshot, now).await.unwrap(),
            1,
            "unchanged settled snapshot clears"
        );
        sqlx::query("DELETE FROM social.scene_stream_access WHERE id=$1")
            .bind(settling)
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
    }
}
