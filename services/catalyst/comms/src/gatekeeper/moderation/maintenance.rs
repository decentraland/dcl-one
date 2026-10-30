//! Periodic removal cleanup and room metadata reconciliation.
use crate::explore::places::removed::{PlaceContext, RemovedPlace, RemovedPlaces};
use crate::gatekeeper::{CommsState, admission::scene, errors::ApiError};
use crate::interfaces::{StreamingNotification, StreamingNotificationKind};
use crate::{
    Address,
    world_identity::{Parcel, WorldName},
};
use chrono::NaiveDateTime;
use serde_json::{Value, json};

fn info(context: &PlaceContext) -> scene::PlaceInfo {
    scene::PlaceInfo {
        world: context.world,
        world_name: context.world_name.clone(),
        base_position: Some(context.base_position.clone()),
        positions: context
            .positions
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_else(|| vec![context.base_position.clone()]),
    }
}
async fn owner(state: &CommsState, context: &PlaceContext) -> Result<Address, ApiError> {
    if let Some(address) = context
        .owner_address
        .as_deref()
        .and_then(|a| Address::parse(a).ok())
    {
        return Ok(address);
    }
    if context.world {
        let name = WorldName::parse(
            context
                .world_name
                .as_deref()
                .ok_or_else(|| ApiError::http(503, "place owner unavailable"))?,
        )
        .map_err(|_| ApiError::http(503, "place owner unavailable"))?;
        return state
            .policy
            .world_owner(&name)
            .await
            .map_err(|_| ApiError::http(503, "place owner unavailable"))?
            .ok_or_else(|| ApiError::http(503, "place owner unavailable"));
    }
    let parcel = Parcel::parse(&context.base_position)
        .map_err(|_| ApiError::http(503, "place owner unavailable"))?;
    let document = state
        .policy
        .land_document(parcel)
        .await
        .map_err(|_| ApiError::http(503, "place owner unavailable"))?;
    document
        .as_deref()
        .and_then(|d| d.as_ref())
        .and_then(|d| Address::parse(&d.owner).ok())
        .ok_or_else(|| ApiError::http(503, "place owner unavailable"))
}

fn notification(
    context: &PlaceContext,
    recipient: Address,
    key: &str,
    kind: StreamingNotificationKind,
    at: NaiveDateTime,
) -> StreamingNotification {
    let location = context
        .world_name
        .as_deref()
        .filter(|_| context.world)
        .unwrap_or(&context.base_position);
    let (title, description) = match kind {
        StreamingNotificationKind::KeyReset => (
            "Stream Key Reset",
            format!("Check the Admin Tools panel at {location} to get the new key."),
        ),
        StreamingNotificationKind::KeyRevoke => (
            "Stream Key Deactivated",
            format!("Check the Admin Tools panel at {location} if you need a new key."),
        ),
        StreamingNotificationKind::KeyExpired => (
            "Stream Key Expired",
            format!("Go to the Admin Tools panel at {location} if you need a new key."),
        ),
        StreamingNotificationKind::TimeExceeded => (
            "Stream Timed Out",
            format!(
                "Restart your stream in your broadcasting software (e.g. OBS) if you'd like to continue streaming to the scene at {location}"
            ),
        ),
        StreamingNotificationKind::PlaceUpdated => (
            "Stream Location Altered",
            format!(
                "If you'd like to stream to {location} please generate a new stream key from the Admin Tools panel in Decentraland."
            ),
        ),
    };
    let query = if context.world {
        format!("realm={location}")
    } else {
        format!("position={location}")
    };
    StreamingNotification {
        event_key: key.to_string(),
        recipient: recipient.clone(),
        kind,
        occurred_at: at,
        metadata: json!({"title": title, "description": description, "position": context.base_position, "worldName": context.world_name, "isWorld": context.world, "url": format!("https://decentraland.org/jump/?{query}"), "address": recipient.as_str(), "image": "https://assets-cdn.decentraland.org/streaming/streaming-notification.png"}),
    }
}

pub async fn prepare_streaming_for_context(
    state: &CommsState,
    context: &PlaceContext,
    event_key: &str,
    kind: StreamingNotificationKind,
    occurred_at: NaiveDateTime,
) -> Result<StreamingNotification, ApiError> {
    let recipient = owner(state, context).await?;
    Ok(notification(
        context,
        recipient,
        event_key,
        kind,
        occurred_at,
    ))
}

pub async fn prepare_streaming_for_place(
    state: &CommsState,
    place_id: &str,
    event_key: &str,
    kind: StreamingNotificationKind,
    occurred_at: NaiveDateTime,
) -> Result<StreamingNotification, ApiError> {
    let context = RemovedPlaces::new(state.pool.clone())
        .place(place_id)
        .await?
        .ok_or_else(|| ApiError::http(503, "place notification context unavailable"))?;
    let recipient = owner(state, &context).await?;
    Ok(notification(
        &context,
        recipient,
        event_key,
        kind,
        occurred_at,
    ))
}

pub async fn notify_streaming_for_place(
    state: &CommsState,
    place_id: &str,
    event_key: &str,
    kind: StreamingNotificationKind,
    occurred_at: NaiveDateTime,
) -> Result<(), ApiError> {
    let event = prepare_streaming_for_place(state, place_id, event_key, kind, occurred_at).await?;
    state.social.notify_streaming_notification(&event).await?;
    Ok(())
}

async fn cleanup(
    state: &CommsState,
    feed: &RemovedPlaces,
    event: &RemovedPlace,
) -> Result<(), ApiError> {
    let context = PlaceContext {
        place_id: event.place_id.clone(),
        owner_address: event.owner_address.clone(),
        world: event.world,
        world_name: event.world_name.clone(),
        base_position: event.base_position.clone(),
        positions: event.positions.clone(),
    };
    let recipient = owner(state, &context).await;
    if let Ok(recipient) = &recipient {
        feed.save_owner(event.event_id, recipient.as_str()).await?;
    }
    if event.cleaned_at.is_none() {
        let accesses: Vec<(uuid::Uuid, Option<String>)> = sqlx::query_as(
            "SELECT id, ingress_id FROM social.scene_stream_access WHERE place_id = $1 AND active",
        )
        .bind(&event.place_id)
        .fetch_all(&state.pool)
        .await?;
        let ingresses: Vec<&str> = accesses
            .iter()
            .filter_map(|(_, ingress)| ingress.as_deref().filter(|id| !id.is_empty()))
            .collect();
        if !ingresses.is_empty() && !state.livekit.configured {
            return Err(ApiError::http(503, "LiveKit ingress unavailable"));
        }
        for ingress in ingresses {
            state
                .ingress()
                .delete_ingress(ingress)
                .await
                .map_err(|_| ApiError::http(503, "LiveKit ingress revocation failed"))?;
        }
        let access_ids: Vec<uuid::Uuid> = accesses.iter().map(|(id, _)| *id).collect();
        let mut tx = state.pool.begin().await?;
        sqlx::query("DELETE FROM social.scene_bans WHERE place_id = $1")
            .bind(&event.place_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE social.scene_admins SET active = false, removed_at = $2 WHERE place_id = $1 AND active").bind(&event.place_id).bind(event.removed_at).execute(&mut *tx).await?;
        sqlx::query("UPDATE social.scene_stream_access SET active = false, streaming = false WHERE id = ANY($1) AND active").bind(&access_ids).execute(&mut *tx).await?;
        RemovedPlaces::mark_cleaned(&mut tx, event.event_id).await?;
        tx.commit().await?;
    }
    let recipient = recipient?;
    let key = format!("place-removed-{}", event.event_id);
    state
        .social
        .notify_streaming_notification(&notification(
            &context,
            recipient,
            &key,
            StreamingNotificationKind::PlaceUpdated,
            event.removed_at,
        ))
        .await?;
    feed.acknowledge(event.event_id).await?;
    Ok(())
}

const METADATA_BATCH: usize = 100;
fn metadata_batch(mut rooms: Vec<String>, after: Option<&str>) -> (Vec<String>, Option<String>) {
    rooms.sort();
    rooms.dedup();
    let start = after
        .map(|key| rooms.partition_point(|room| room.as_str() <= key))
        .unwrap_or(0);
    let start = if start == rooms.len() { 0 } else { start };
    let batch: Vec<String> = rooms.into_iter().skip(start).take(METADATA_BATCH).collect();
    let next = if batch.len() == METADATA_BATCH {
        batch.last().cloned()
    } else {
        None
    };
    (batch, next)
}

async fn metadata_scene_context(
    state: &CommsState,
    room: &str,
) -> Result<Option<PlaceContext>, ApiError> {
    use crate::rooms::{ParsedRoom, parse_room, resolve_room_name};
    let resolved = match parse_room(room) {
        Some(ParsedRoom::WorldScene(_)) => resolve_room_name(state.scenes.as_ref(), room)
            .await
            .map_err(|_| ApiError::http(503, "Room identity is temporarily unavailable"))?,
        Some(ParsedRoom::Local(identity))
            if matches!(&identity.realm, crate::world_identity::RealmId::Shared(name)
                if name.eq_ignore_ascii_case("preview") || name.eq_ignore_ascii_case("localpreview")) =>
        {
            // Preview has no authoritative LAND footprint. Preserve the old
            // background exclusion even if its CID also exists in Genesis.
            None
        }
        Some(ParsedRoom::Local(identity)) if identity.scene_id.is_some() => {
            // Adapter lookup also serves historical deployments. Metadata needs
            // the scene currently standing at its base, including after local
            // unpublish tombstones/repoints without setting a deleter.
            let resolved = state
                .scenes
                .resolve(crate::rooms::RoomRequest::Adapter {
                    realm: &identity.realm,
                    scene: identity.scene_id.as_ref().unwrap(),
                })
                .await
                .map_err(|_| ApiError::http(503, "Room identity is temporarily unavailable"))?;
            if resolved.room != room
                || resolved.realm != identity.realm
                || resolved.scene_id != identity.scene_id
            {
                return Ok(None);
            }
            let Some(deployment) = resolved.deployment.as_ref() else {
                return Ok(None);
            };
            let current = match state
                .scenes
                .resolve(crate::rooms::RoomRequest::Parcel {
                    realm: &identity.realm,
                    parcel: deployment.base,
                })
                .await
            {
                Ok(current) => current,
                Err(crate::rooms::ResolveError::NoSceneAtParcel { .. }) => return Ok(None),
                Err(_) => {
                    return Err(ApiError::http(
                        503,
                        "Room identity is temporarily unavailable",
                    ));
                }
            };
            (current.room == room
                && current.realm == identity.realm
                && current.scene_id == identity.scene_id
                && current.deployment.as_ref().is_some_and(|current| {
                    Some(&current.scene_id) == identity.scene_id.as_ref()
                        && current.parcels.contains(&deployment.base)
                }))
            .then_some(current)
        }
        // A lobby has no scene moderation namespace. Keep its existing
        // metadata; do not synthesize a sibling-scene or world-Place key.
        Some(ParsedRoom::Local(_) | ParsedRoom::WorldLobby(_)) => None,
        None => None,
    };
    let Some(resolved) = resolved else {
        return Ok(None);
    };
    let (Some(scene), Some(deployment)) =
        (resolved.scene_id.as_ref(), resolved.deployment.as_ref())
    else {
        return Ok(None);
    };
    if &deployment.scene_id != scene {
        return Ok(None);
    }
    Ok(Some(PlaceContext {
        place_id: scene.as_str().to_owned(),
        // Current policy is authoritative for the owner. A Places snapshot
        // cannot replace scene-keyed rows, owner or active parcel authority.
        owner_address: None,
        world: resolved.realm.is_world(),
        world_name: resolved.realm.world().map(|world| world.key().to_owned()),
        base_position: deployment.base.canonical(),
        positions: json!(deployment.parcels.canonical()),
    }))
}

async fn room_metadata_fields(
    state: &CommsState,
    room: &str,
) -> Result<Option<serde_json::Map<String, Value>>, ApiError> {
    let Some(context) = metadata_scene_context(state, room).await? else {
        return Ok(None);
    };
    let mut admins = scene::try_get_extra_addresses(state, &info(&context)).await?;
    admins.insert(owner(state, &context).await?.as_str().to_string());
    if !context.world {
        admins.extend(
            scene::try_get_lease_holders_for_parcels(state, &info(&context).positions).await?,
        );
    }
    for admin in state
        .moderation
        .list_active_admins(&context.place_id, None)
        .await?
    {
        admins.insert(admin.admin);
    }
    let bans: Vec<String> = sqlx::query_scalar(
        "SELECT banned_address FROM social.scene_bans WHERE place_id = $1 ORDER BY banned_address",
    )
    .bind(&context.place_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Some(
        json!({"sceneAdmins": admins, "bannedAddresses": bans})
            .as_object()
            .unwrap()
            .clone(),
    ))
}

async fn refresh_room(state: &CommsState, room: &str) -> Result<(), ApiError> {
    let Some(fields) = room_metadata_fields(state, room).await? else {
        return Ok(());
    };
    state
        .room_service()
        .merge_room_metadata(&room, fields)
        .await
        .map_err(|_| ApiError::http(503, "LiveKit room metadata refresh failed"))?;
    Ok(())
}

pub(crate) async fn initialize_room(
    state: &CommsState,
    room: &str,
    expected_sid: Option<&str>,
) -> Result<Option<String>, ApiError> {
    let Some(fields) = room_metadata_fields(state, room).await? else {
        return Ok(None);
    };
    state
        .room_service()
        .merge_room_metadata_for_incarnation(room, expected_sid, fields)
        .await
        .map_err(|_| ApiError::http(503, "LiveKit room metadata initialization failed"))
}

async fn refresh(state: &CommsState) -> Result<(), ApiError> {
    if !state.livekit.configured {
        return Ok(());
    }
    let rooms = state
        .room_service()
        .list_rooms()
        .await
        .map_err(|_| ApiError::http(503, "LiveKit room listing failed"))?;
    let after = state.metadata_cursor.lock().unwrap().clone();
    let (rooms, next) = metadata_batch(rooms, after.as_deref());
    // Advance even across an unavailable room, so one room cannot starve later
    // rooms. A new process starts a fresh bounded pass over this derived state.
    *state.metadata_cursor.lock().unwrap() = next;
    for room in rooms {
        let result = refresh_room(state, &room).await;
        if let Err(error) = result {
            tracing::warn!(room, error = %error.message, "room metadata refresh retained for the next pass");
        }
    }
    Ok(())
}

pub async fn run_once(state: &CommsState) -> anyhow::Result<()> {
    let feed = RemovedPlaces::new(state.pool.clone());
    for event in feed.pending().await? {
        if let Err(error) = cleanup(state, &feed, &event).await {
            tracing::warn!(event_id = %event.event_id, place_id = %event.place_id, error = %error.message, "place cleanup retained for retry");
            feed.retry(event.event_id).await?;
        }
    }
    refresh(state)
        .await
        .map_err(|e| anyhow::anyhow!("room metadata refresh: {}", e.message))
}

#[cfg(test)]
mod current_scene_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interfaces::{
        SocialDirectory,
        test_support::{StubDirectory, StubPolicy, StubScenes},
    };
    use crate::{CommunityId, gatekeeper::Settings};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    fn owner_state(policy: StubPolicy) -> CommsState {
        CommsState::new(
            sqlx::PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap(),
            crate::http::Outbound::for_tests(),
            Settings::for_tests(),
            None,
            None,
            Arc::new(policy),
            Arc::new(StubScenes::default()),
            Arc::new(crate::presence::Presence::new()),
            Arc::new(StubDirectory::default()),
        )
    }

    fn world_context(captured: Option<&str>) -> PlaceContext {
        PlaceContext {
            place_id: "test-world".into(),
            owner_address: captured.map(str::to_owned),
            world: true,
            world_name: Some("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.dcl.eth".into()),
            base_position: "0,0".into(),
            positions: json!(["0,0"]),
        }
    }

    struct SweepScenes(crate::rooms::ResolvedRoom);
    #[async_trait::async_trait]
    impl crate::interfaces::SceneIdentity for SweepScenes {
        async fn classify_room(
            &self,
            room: &str,
        ) -> Result<crate::rooms::RoomClassification, crate::rooms::ResolveError> {
            crate::interfaces::test_support::classify_fixture(self, room).await
        }
        async fn resolve(
            &self,
            request: crate::rooms::RoomRequest<'_>,
        ) -> Result<crate::rooms::ResolvedRoom, crate::rooms::ResolveError> {
            if matches!(request,crate::rooms::RoomRequest::NamedWorldScene { room } if room == self.0.room)
            {
                Ok(self.0.clone())
            } else {
                Err(crate::rooms::ResolveError::Db(sqlx::Error::PoolClosed))
            }
        }
        async fn world_scene(
            &self,
            _: &WorldName,
            _: &crate::world_identity::SceneId,
        ) -> Result<Option<crate::rooms::ActiveDeployment>, crate::rooms::ResolveError> {
            Ok(None)
        }
        async fn world_active_scene(
            &self,
            _: &WorldName,
        ) -> Result<Option<crate::rooms::ActiveDeployment>, crate::rooms::ResolveError> {
            Ok(None)
        }
    }

    #[tokio::test]
    async fn captured_owner_precedes_unavailable_world_fallback() {
        let captured = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let state = owner_state(StubPolicy {
            world_owner: Err(
                crate::AuthorityNotEstablished::UndeterminedStoreUnavailable {
                    store: "worlds.worlds",
                    reason_for_operators: "unavailable".into(),
                },
            ),
            ..Default::default()
        });
        assert_eq!(
            owner(&state, &world_context(Some(captured)))
                .await
                .unwrap()
                .as_str(),
            captured
        );
    }

    #[tokio::test]
    async fn world_owner_fallback_uses_stored_answer_and_unavailable_answers_retry() {
        let stored = Address::parse("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap();
        let state = owner_state(StubPolicy {
            world_owner: Ok(Some(stored.clone())),
            ..Default::default()
        });
        assert_eq!(owner(&state, &world_context(None)).await.unwrap(), stored);
        assert_eq!(
            owner(&state, &world_context(Some("invalid")))
                .await
                .unwrap(),
            stored
        );
        let unknown = owner_state(StubPolicy::default());
        assert_eq!(
            owner(&unknown, &world_context(None))
                .await
                .unwrap_err()
                .code,
            503
        );
        let unavailable = owner_state(StubPolicy {
            world_owner: Err(
                crate::AuthorityNotEstablished::UndeterminedStoreUnavailable {
                    store: "worlds.worlds",
                    reason_for_operators: "unavailable".into(),
                },
            ),
            ..Default::default()
        });
        assert_eq!(
            owner(&unavailable, &world_context(None))
                .await
                .unwrap_err()
                .code,
            503
        );
    }

    struct Directory {
        fail: AtomicBool,
        stub: StubDirectory,
    }
    #[async_trait::async_trait]
    impl SocialDirectory for Directory {
        async fn notify_moderation_notification(
            &self,
            transaction: &mut sqlx::PgConnection,
            notice: &crate::interfaces::ModerationNotification,
        ) -> Result<(), sqlx::Error> {
            self.stub
                .notify_moderation_notification(transaction, notice)
                .await
        }
        async fn community_members(&self, _: CommunityId) -> Result<Vec<Address>, sqlx::Error> {
            Ok(vec![])
        }
        async fn private_messages_privacy(&self, a: &Address) -> Result<String, sqlx::Error> {
            self.stub.private_messages_privacy(a).await
        }
        async fn set_private_messages_privacy(
            &self,
            a: &Address,
            p: &str,
        ) -> Result<(), sqlx::Error> {
            self.stub.set_private_messages_privacy(a, p).await
        }
        async fn notify_community_voice_ended(
            &self,
            c: CommunityId,
            k: &str,
            e: &Value,
            at: NaiveDateTime,
        ) -> Result<(), sqlx::Error> {
            self.stub.notify_community_voice_ended(c, k, e, at).await
        }
        async fn notify_streaming_notification(
            &self,
            n: &StreamingNotification,
        ) -> Result<(), sqlx::Error> {
            if self.fail.load(Ordering::SeqCst) {
                Err(sqlx::Error::PoolClosed)
            } else {
                self.stub.notify_streaming_notification(n).await
            }
        }
    }
    async fn setup(pool: sqlx::PgPool) -> (CommsState, Arc<Directory>, RemovedPlace) {
        let directory = Arc::new(Directory {
            fail: AtomicBool::new(false),
            stub: StubDirectory::default(),
        });
        let state = CommsState::new(
            pool.clone(),
            crate::http::Outbound::for_tests(),
            Settings::for_tests(),
            None,
            None,
            Arc::new(StubPolicy::default()),
            Arc::new(StubScenes::default()),
            Arc::new(crate::presence::Presence::new()),
            directory.clone(),
        );
        let id = format!("maintenance-{}", uuid::Uuid::new_v4());
        sqlx::query("INSERT INTO explore.place (id, base_position, raw) VALUES ($1, '0,0', $2)")
            .bind(&id)
            .bind(json!({"owner":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","positions":["0,0"]}))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM explore.place WHERE id = $1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        let event = sqlx::query_as("SELECT event_id, place_id, owner_address, world, world_name, base_position, positions, removed_at, cleaned_at FROM explore.place_removal_pending WHERE place_id = $1").bind(&id).fetch_one(&pool).await.unwrap();
        (state, directory, event)
    }
    #[test]
    fn metadata_sweep_is_bounded_and_resumes_past_failed_rooms() {
        let rooms: Vec<String> = (0..205).map(|n| format!("scene:main:{n:03}")).collect();
        let (a, cursor) = metadata_batch(rooms.clone(), None);
        let (b, cursor) = metadata_batch(rooms.clone(), cursor.as_deref());
        let (c, cursor) = metadata_batch(rooms.clone(), cursor.as_deref());
        assert_eq!((a.len(), b.len(), c.len()), (100, 100, 5));
        assert_eq!(a.last().unwrap(), "scene:main:099");
        assert_eq!(b.first().unwrap(), "scene:main:100");
        assert_eq!(c.last().unwrap(), "scene:main:204");
        assert!(cursor.is_none());
        assert_eq!(metadata_batch(rooms, cursor.as_deref()).0, a);
    }

    #[tokio::test]
    async fn canonical_metadata_identity_is_checked_before_places_enrichment() {
        let mut state = owner_state(StubPolicy::default());
        state.scenes = Arc::new(StubScenes {
            room: Err(crate::rooms::ResolveError::UnresolvedRoom(
                "w1:g:metadata.dcl.ethscene-a".into(),
            )),
            ..Default::default()
        });
        // A missing/ambiguous deployment cannot become a metadata identity
        // merely because only one competing tuple happens to have a Place.
        // Closing the pool makes the required identity-before-enrichment
        // boundary observable without a PostgreSQL test guard.
        state.pool.close().await;
        refresh_room(&state, "w1:g:metadata.dcl.ethscene-a")
            .await
            .expect("unresolved identity must be skipped before any Places lookup");
    }

    #[tokio::test]
    async fn canonical_background_context_uses_original_scene_key_and_current_authority() {
        use crate::rooms::{ActiveDeployment, ResolvedRoom};
        use crate::world_identity::{RealmId, SceneId, SceneNamespace};
        let current_owner = Address::parse("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let mut state = owner_state(StubPolicy {
            world_owner: Ok(Some(current_owner.clone())),
            ..Default::default()
        });
        state.pool.close().await;
        for realm in [
            RealmId::World(WorldName::parse("mixed.dcl.eth").unwrap()),
            RealmId::Shared("main".into()),
        ] {
            let scene = SceneId::parse("OriginalCaseHash").unwrap();
            let base = Parcel::parse("5,6").unwrap();
            let room = match &realm {
                RealmId::World(world) => crate::rooms::worlds_service_scene_room(world, &scene),
                RealmId::Shared(shared) => {
                    crate::rooms::gatekeeper_scene_room(shared, scene.as_str())
                }
            };
            state.scenes = Arc::new(StubScenes {
                room: Ok(ResolvedRoom {
                    realm: realm.clone(),
                    scene_id: Some(scene.clone()),
                    room: room.clone(),
                    deployment: Some(ActiveDeployment {
                        title: None,
                        scene_id: scene.clone(),
                        parcels: [base].into_iter().collect(),
                        base,
                        namespace: SceneNamespace::new(&realm, base, None),
                        local: true,
                    }),
                }),
                ..Default::default()
            });
            let context = metadata_scene_context(&state, &room)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                context.place_id,
                scene.as_str(),
                "explicit admins/bans use original scene ID, never a Places UUID/world key"
            );
            assert_eq!(
                context.owner_address, None,
                "owner must come from current policy"
            );
            assert_eq!(context.base_position, "5,6");
            assert_eq!(context.positions, json!(["5,6"]));
            assert_eq!(context.world, realm.is_world());
            if realm.is_world() {
                assert_eq!(owner(&state, &context).await.unwrap(), current_owner);
            }
        }
        assert!(
            metadata_scene_context(&state, "world-mixed.dcl.eth")
                .await
                .unwrap()
                .is_none(),
            "a lobby cannot acquire a guessed scene moderation key"
        );
        refresh_room(&state, "world-mixed.dcl.eth").await.unwrap();
        state.scenes = Arc::new(StubScenes {
            room: Err(crate::rooms::ResolveError::Db(sqlx::Error::PoolClosed)),
            ..Default::default()
        });
        state.livekit.configured = true;
        state.livekit.api_url = "http://127.0.0.1:1".into();
        for room in [
            "scene:pReViEw:OriginalCaseHash",
            "scene:LoCaLpReViEw:OriginalCaseHash",
        ] {
            assert!(
                metadata_scene_context(&state, room)
                    .await
                    .unwrap()
                    .is_none(),
                "preview background refresh cannot consult Genesis scene authority"
            );
            refresh_room(&state, room).await.unwrap();
        }
    }

    #[tokio::test]
    async fn canonical_metadata_refresh_uses_scene_namespace_and_footprint_without_places() {
        use crate::interfaces::AccessPolicy;
        use crate::world_identity::{ParcelSet, RealmId, SceneId, SceneNamespace};
        struct FootprintPolicy(ParcelSet);
        #[async_trait::async_trait]
        impl AccessPolicy for FootprintPolicy {
            async fn access_type(
                &self,
                _: &WorldName,
            ) -> Result<Option<crate::access::AccessType>, crate::AuthorityNotEstablished>
            {
                Ok(None)
            }
            async fn world_owner(
                &self,
                _: &WorldName,
            ) -> Result<Option<Address>, crate::AuthorityNotEstablished> {
                Ok(Some(
                    Address::parse("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap(),
                ))
            }
            async fn land_document(
                &self,
                _: Parcel,
            ) -> Result<
                Option<Arc<Option<crate::access::LandOperators>>>,
                crate::AuthorityNotEstablished,
            > {
                Ok(None)
            }
            async fn can_access(
                &self,
                world: &WorldName,
                actor: &Address,
                secret: Option<&str>,
                scene: Option<&SceneId>,
                device: Option<&str>,
                now: NaiveDateTime,
            ) -> Result<crate::access::Access, crate::AuthorityNotEstablished> {
                StubPolicy::default()
                    .can_access(world, actor, secret, scene, device, now)
                    .await
            }
            async fn is_blocked(
                &self,
                _: &Address,
            ) -> Result<bool, crate::AuthorityNotEstablished> {
                Ok(false)
            }
            async fn is_admin(
                &self,
                scene: &crate::access::SceneRef<'_>,
                actor: &Address,
            ) -> crate::PermissionDecision {
                StubPolicy::default().is_admin(scene, actor).await
            }
            async fn protected_addresses(
                &self,
                _: &WorldName,
                parcels: &ParcelSet,
            ) -> Result<std::collections::BTreeSet<Address>, crate::AuthorityNotEstablished>
            {
                assert_eq!(
                    parcels, &self.0,
                    "world Place base/positions cannot replace this scene's footprint"
                );
                Ok(Default::default())
            }
        }
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let first =
            WorldName::parse(&format!("metadata-{}", uuid::Uuid::new_v4().simple())).unwrap();
        let second = WorldName::parse(&format!("{}-cid", first.key())).unwrap();
        let scene = SceneId::parse("Cid-with-hyphen").unwrap();
        let room = crate::rooms::worlds_service_scene_room(&first, &scene);
        let competitor = crate::rooms::worlds_service_scene_room(
            &second,
            &SceneId::parse("with-hyphen").unwrap(),
        );
        assert_ne!(room, competitor);
        for (world, entity, parcel) in [
            (&first, scene.as_str(), "5,6"),
            (&second, "with-hyphen", "invalid"),
        ] {
            sqlx::query("INSERT INTO worlds.worlds (name) VALUES ($1)")
                .bind(world.key())
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size) VALUES ($1, $2, '[]'::json, '{}'::jsonb, '0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', $3, 1)").bind(world.key()).bind(entity).bind(vec![parcel]).execute(&pool).await.unwrap();
        }
        // One world Place can have its base at a different sibling scene.
        sqlx::query("INSERT INTO explore.place (id, base_position, raw) VALUES ($1, '0,0', $2)").bind(first.key()).bind(json!({"world":true,"world_name":first.key(),"owner":"0xcccccccccccccccccccccccccccccccccccccccc","positions":["0,0"]})).execute(&pool).await.unwrap();
        let base = Parcel::parse("5,6").unwrap();
        let parcels: ParcelSet = [base].into_iter().collect();
        let resolved = crate::rooms::ResolvedRoom {
            realm: RealmId::World(first.clone()),
            scene_id: Some(scene.clone()),
            room: room.clone(),
            deployment: Some(crate::rooms::ActiveDeployment {
                title: None,
                scene_id: scene.clone(),
                parcels: parcels.clone(),
                base,
                namespace: SceneNamespace::new(&RealmId::World(first.clone()), base, None),
                local: true,
            }),
        };
        let (mut state, _, _) = setup(pool.clone()).await;
        let by = Address::parse("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        for (key, admin, banned) in [
            (
                scene.as_str(),
                "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            ),
            (
                first.key(),
                "0xdddddddddddddddddddddddddddddddddddddddd",
                "0xffffffffffffffffffffffffffffffffffffffff",
            ),
        ] {
            state
                .moderation
                .add_admin(key, &Address::parse(admin).unwrap(), &by)
                .await
                .unwrap();
            sqlx::query("INSERT INTO social.scene_bans (place_id,banned_address,banned_by) VALUES ($1,$2,$3)")
                .bind(key).bind(banned).bind(by.as_str()).execute(&pool).await.unwrap();
        }
        state.policy = Arc::new(FootprintPolicy(parcels));
        state.scenes = Arc::new(StubScenes {
            room: Ok(resolved),
            ..Default::default()
        });
        let writes = Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
        let captured = writes.clone();
        let api = axum::Router::new().route(
            "/twirp/livekit.RoomService/{method}",
            axum::routing::post(
                move |axum::extract::Path(method): axum::extract::Path<String>,
                      axum::Json(body): axum::Json<Value>| {
                    let captured = captured.clone();
                    async move {
                        if method == "ListRooms" {
                            return axum::Json(json!({"rooms":[{"name":body["names"][0],"sid":"RM_fixture","metadata":"{}"}]}));
                        }
                        captured.lock().unwrap().push(body);
                        axum::Json(json!({}))
                    }
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        state.livekit.configured = true;
        state.livekit.api_url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, api).await.unwrap() });
        refresh_room(&state, &room).await.unwrap();
        assert_eq!(
            writes.lock().unwrap().len(),
            1,
            "missing competitor Place cannot affect exact scene identity"
        );
        let metadata: Value =
            serde_json::from_str(writes.lock().unwrap()[0]["metadata"].as_str().unwrap()).unwrap();
        assert_eq!(
            metadata["sceneAdmins"],
            json!([by.as_str(), "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"])
        );
        assert_eq!(
            metadata["bannedAddresses"],
            json!(["0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"])
        );
        sqlx::query("DELETE FROM explore.place WHERE id = $1")
            .bind(first.key())
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO explore.place (id, base_position, raw) VALUES ($1, '7,8', $2)").bind(second.key()).bind(json!({"world":true,"world_name":second.key(),"owner":"0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","positions":["7,8"]})).execute(&pool).await.unwrap();
        refresh_room(&state, &room).await.unwrap();
        assert_eq!(
            writes.lock().unwrap().len(),
            2,
            "malformed competitor footprint cannot change the exact-world enrichment"
        );
        assert!(
            writes
                .lock()
                .unwrap()
                .iter()
                .all(|body| body["room"] == room)
        );
        state.scenes = Arc::new(StubScenes {
            room: Err(crate::rooms::ResolveError::UnresolvedRoom(room.clone())),
            ..Default::default()
        });
        refresh_room(&state, &room).await.unwrap();
        assert_eq!(
            writes.lock().unwrap().len(),
            2,
            "deleted scene receives no guessed historical parcel roles"
        );
        server.abort();
    }

    #[tokio::test]
    async fn a_failed_room_does_not_stop_later_room_metadata_refresh() {
        use axum::{
            Json, Router,
            extract::{Path, State},
            routing::post,
        };
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let (mut state, _, _) = setup(pool.clone()).await;
        let world = format!("metadata-{}.dcl.eth", uuid::Uuid::new_v4().simple());
        let bad = format!("bad-{world}");
        let good = format!("good-{world}");
        let good_world = WorldName::parse(&good).unwrap();
        let good_scene = crate::world_identity::SceneId::parse(&format!("entity-{good}")).unwrap();
        let good_room = crate::rooms::worlds_service_scene_room(&good_world, &good_scene);
        let bad_room = crate::rooms::worlds_service_scene_room(
            &WorldName::parse(&bad).unwrap(),
            &crate::world_identity::SceneId::parse(&format!("entity-{bad}")).unwrap(),
        );
        let base = Parcel::parse("0,0").unwrap();
        let realm = crate::world_identity::RealmId::World(good_world);
        state.scenes = Arc::new(SweepScenes(crate::rooms::ResolvedRoom {
            realm: realm.clone(),
            scene_id: Some(good_scene.clone()),
            room: good_room.clone(),
            deployment: Some(crate::rooms::ActiveDeployment {
                title: None,
                scene_id: good_scene,
                parcels: [base].into_iter().collect(),
                base,
                namespace: crate::world_identity::SceneNamespace::new(&realm, base, None),
                local: true,
            }),
        }));
        state.policy = Arc::new(StubPolicy {
            world_owner: Ok(Some(
                Address::parse("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap(),
            )),
            ..Default::default()
        });
        for (name, owner) in [
            (&bad, None),
            (&good, Some("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")),
        ] {
            sqlx::query("INSERT INTO worlds.worlds (name) VALUES ($1)")
                .bind(name)
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size) VALUES ($1, $2, '[]'::json, '{}'::jsonb, '0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', ARRAY['0,0'], 1)").bind(name).bind(format!("entity-{name}")).execute(&pool).await.unwrap();

            sqlx::query(
                "INSERT INTO explore.place (id, base_position, raw) VALUES ($1, '0,0', $2)",
            )
            .bind(name)
            .bind(json!({"world":true,"world_name":name,"owner":owner,"positions":["0,0"]}))
            .execute(&pool)
            .await
            .unwrap();
        }
        #[derive(Clone)]
        struct Mock {
            names: Vec<String>,
            updates: Arc<std::sync::Mutex<Vec<Value>>>,
        }
        async fn api(
            State(mock): State<Mock>,
            Path(method): Path<String>,
            Json(body): Json<Value>,
        ) -> Json<Value> {
            if method == "ListRooms" && body.get("names").is_none() {
                return Json(
                    json!({"rooms":mock.names.iter().map(|name| json!({"name":name})).collect::<Vec<_>>()}),
                );
            }
            if method == "ListRooms" {
                // A named listing answers with the room's name, as LiveKit does; the
                // client refuses a listed room whose name is not the one it asked for.
                return Json(json!({"rooms":[{
                    "name": body["names"][0].clone(),
                    "sid": "RM_fixture",
                    "metadata": json!({"presenters":["retained"]}).to_string()
                }]}));
            }
            mock.updates.lock().unwrap().push(body);
            Json(json!({}))
        }
        let updates = Arc::new(std::sync::Mutex::new(vec![]));
        let app = Router::new()
            .route("/twirp/livekit.RoomService/{method}", post(api))
            .with_state(Mock {
                names: vec![bad_room, good_room.clone()],
                updates: updates.clone(),
            });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        state.livekit.api_url = format!("http://{}", listener.local_addr().unwrap());
        state.livekit.configured = true;
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        refresh(&state).await.unwrap();
        server.abort();
        let updates = updates.lock().unwrap();
        assert_eq!(
            updates.len(),
            1,
            "failed owner lookup did not stop the next room"
        );
        assert_eq!(updates[0]["room"], good_room);
        let metadata: Value =
            serde_json::from_str(updates[0]["metadata"].as_str().unwrap()).unwrap();
        assert_eq!(
            metadata["sceneAdmins"],
            json!(["0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"])
        );
        assert_eq!(metadata["presenters"], json!(["retained"]));
    }

    #[tokio::test]
    async fn notification_failure_keeps_checkpoint_and_readd_writes_survive_retry() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let (state, directory, event) = setup(pool.clone()).await;
        let wallet = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        sqlx::query("INSERT INTO social.scene_bans (place_id, banned_address, banned_by, banned_at) VALUES ($1, $2, $2, $3)").bind(&event.place_id).bind(wallet).bind(event.removed_at - chrono::Duration::seconds(1)).execute(&pool).await.unwrap();
        directory.fail.store(true, Ordering::SeqCst);
        let feed = RemovedPlaces::new(pool.clone());
        assert!(cleanup(&state, &feed, &event).await.is_err());
        let saved: RemovedPlace = sqlx::query_as("SELECT event_id, place_id, owner_address, world, world_name, base_position, positions, removed_at, cleaned_at FROM explore.place_removal_pending WHERE event_id = $1").bind(event.event_id).fetch_one(&pool).await.unwrap();
        assert!(
            saved.cleaned_at.is_some(),
            "cleanup commit survived notification failure"
        );
        sqlx::query("INSERT INTO social.scene_bans (place_id, banned_address, banned_by, banned_at) VALUES ($1, $2, $2, $3)").bind(&event.place_id).bind(wallet).bind(event.removed_at + chrono::Duration::seconds(1)).execute(&pool).await.unwrap();
        directory.fail.store(false, Ordering::SeqCst);
        cleanup(&state, &feed, &saved).await.unwrap();
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM social.scene_bans WHERE place_id = $1")
                .bind(&event.place_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 1, "later writes were not replay-deleted");
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM explore.place_removal_pending WHERE event_id = $1",
        )
        .bind(event.event_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0, "ack follows successful notification");
        assert_eq!(
            directory.stub.streaming_notifications.lock().unwrap().len(),
            1
        );
    }
    #[tokio::test]
    async fn unknown_owner_does_not_block_cleanup_and_notification_remains_pending() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let (state, directory, mut event) = setup(pool.clone()).await;
        event.owner_address = None;
        sqlx::query(
            "UPDATE explore.place_removal_pending SET owner_address = NULL WHERE event_id = $1",
        )
        .bind(event.event_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO social.scene_bans (place_id, banned_address, banned_by) VALUES ($1, '0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', '0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa')").bind(&event.place_id).execute(&pool).await.unwrap();
        assert_eq!(
            cleanup(&state, &RemovedPlaces::new(pool.clone()), &event)
                .await
                .unwrap_err()
                .code,
            503
        );
        let cleaned: bool = sqlx::query_scalar(
            "SELECT cleaned_at IS NOT NULL FROM explore.place_removal_pending WHERE event_id = $1",
        )
        .bind(event.event_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(cleaned);
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM social.scene_bans WHERE place_id = $1")
                .bind(&event.place_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 0);
        assert!(
            directory
                .stub
                .streaming_notifications
                .lock()
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn ingress_unavailability_retains_uncleaned_removal_and_sends_no_notification() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let (mut state, directory, event) = setup(pool.clone()).await;
        state.livekit.configured = false;
        sqlx::query("INSERT INTO social.scene_stream_access (place_id, streaming_url, streaming_key, ingress_id, generated_by, created_at) VALUES ($1, 'rtmp://example.invalid/live', 'test-key', 'test-ingress', '0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', $2)").bind(&event.place_id).bind(event.removed_at - chrono::Duration::seconds(1)).execute(&pool).await.unwrap();
        assert_eq!(
            cleanup(&state, &RemovedPlaces::new(pool.clone()), &event)
                .await
                .unwrap_err()
                .code,
            503
        );
        let pending: bool = sqlx::query_scalar(
            "SELECT cleaned_at IS NULL FROM explore.place_removal_pending WHERE event_id = $1",
        )
        .bind(event.event_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(pending);
        assert!(
            directory
                .stub
                .streaming_notifications
                .lock()
                .unwrap()
                .is_empty()
        );
    }
}
