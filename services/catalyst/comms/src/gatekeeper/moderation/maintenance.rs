//! Periodic removal cleanup and room metadata reconciliation.
use crate::gatekeeper::removed_places::{PlaceContext, RemovedPlace, RemovedPlaces};
use crate::gatekeeper::streaming::notifications::{
    Scope, notification, owner, place_info as info, recipients,
};
use crate::gatekeeper::{CommsState, SharedState, admission::scene, errors::ApiError, streaming};
use crate::interfaces::StreamingNotificationKind;
use serde_json::{Value, json};

#[cfg(test)]
use crate::interfaces::StreamingNotification;
#[cfg(test)]
use crate::world_identity::WorldName;
#[cfg(test)]
use crate::{Address, world_identity::Parcel};
#[cfg(test)]
use chrono::NaiveDateTime;

async fn cleanup(
    state: &SharedState,
    feed: &RemovedPlaces,
    event: &RemovedPlace,
) -> Result<(), ApiError> {
    let mut tx = state.pool.begin().await?;
    // Serialize safety cleanup for this event.
    let Some(mut event): Option<RemovedPlace> =
        sqlx::query_as("SELECT * FROM explore.place_removal_pending WHERE event_id=$1 FOR UPDATE")
            .bind(event.event_id)
            .fetch_optional(&mut *tx)
            .await?
    else {
        return Ok(());
    };
    let context = PlaceContext {
        place_id: event.place_id.clone(),
        owner_address: event.owner_address.clone(),
        world: event.world,
        world_name: event.world_name.clone(),
        base_position: event.base_position.clone(),
        positions: event.positions.clone(),
    };
    let mut admin_keys = event.admin_keys.clone();
    if event.world {
        let current: Vec<String> = sqlx::query_scalar("SELECT identity::text FROM explore.world_scene_place WHERE world_name=lower($1) AND active")
            .bind(&event.world_name).fetch_all(&mut *tx).await?;
        admin_keys.extend(current);
    }
    let mut ended = Vec::new();
    if event.cleaned_at.is_none() {
        // A removed world ends the key of each of its scenes, by its key as
        // `WorldName` spells it. A name no world can have names no scene room:
        // the key under its catalogue place still ends, and the cleanup commits.
        let world = event.world.then(|| {
            let name = event.world_name.as_deref().unwrap_or_default();
            name.trim().to_lowercase()
        });
        let world = world.as_deref();
        let ingress =
            streaming::removed_place_has_ingress(&mut *tx, &event.place_id, world).await?;
        if ingress && !state.livekit.configured {
            return Err(ApiError::http(503, "LiveKit ingress unavailable"));
        }
        // Bans are on the `<world>:<base>` keys of a world's scenes, or on a
        // Genesis base parcel, where they end with the last enabled place
        // based there. A ban made after the removal stays.
        let (bans, key) = if event.world {
            (
                "left(place_id, length($2) + 1) = $2 || ':' \
                 AND strpos(substr(place_id, length($2) + 2), ':') = 0",
                event
                    .world_name
                    .as_deref()
                    .unwrap_or(&event.place_id)
                    .to_lowercase(),
            )
        } else {
            (
                "place_id = $2 AND NOT EXISTS (SELECT 1 FROM explore.place WHERE disabled IS FALSE AND world IS FALSE AND base_position = $2)",
                event.base_position.clone(),
            )
        };
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DELETE FROM social.scene_bans WHERE banned_at <= $3 AND (place_id = $1 OR ({bans}))"
        )))
        .bind(&event.place_id)
        .bind(key)
        .bind(event.removed_at)
        .execute(&mut *tx)
        .await?;
        // Freeze each World admin this cleanup ends with its granter: a later
        // re-grant overwrites the row's granter and lifts its `removed_at`.
        if event.world {
            sqlx::query("UPDATE explore.place_removal_pending SET admin_grants=(SELECT COALESCE(jsonb_agg(DISTINCT g),'[]'::jsonb) FROM (SELECT jsonb_array_elements(admin_grants) AS g UNION SELECT jsonb_build_array(lower(btrim(admin_address)),lower(btrim(added_by))) FROM social.scene_admins WHERE active AND added_at <= $2 AND place_id=ANY($3::text[])) grants) WHERE event_id=$1")
                .bind(event.event_id).bind(event.removed_at).bind(&admin_keys)
                .execute(&mut *tx).await?;
        }
        // Include exact retired namespaces saved before catalogue removal.
        // A shared Genesis base keeps its live grant, but its affected admin
        // remains eligible for notification regardless of whether revocation is appropriate.
        sqlx::query("UPDATE social.scene_admins SET active=false,removed_at=$2 WHERE active AND added_at <= $2 AND (($3 AND place_id=ANY($4::text[])) OR (NOT $3 AND place_id=$1 AND NOT EXISTS (SELECT 1 FROM explore.place WHERE disabled IS FALSE AND world IS FALSE AND base_position=$1)))")
            .bind(&event.base_position).bind(event.removed_at).bind(event.world).bind(&admin_keys)
            .execute(&mut *tx).await?;
        ended =
            streaming::end_removed_place(&mut tx, &event.place_id, world, event.removed_at).await?;
        RemovedPlaces::mark_cleaned(&mut tx, event.event_id).await?;
    }
    tx.commit().await?;
    for id in ended {
        streaming::settle_cleanup(state, id, &event.place_id);
    }
    if !event.notify {
        feed.acknowledge(event.event_id).await?;
        return Ok(());
    }
    if !event.recipients_resolved {
        // External calls occur only after the safety transaction has committed.
        // The grants this removal ended keep its `removed_at`, so the retained
        // rule (`Scope::Removal`) still finds them; the trigger freezes Genesis
        // admins (they need no granter check) and the exact world scene places.
        let grants = if event.world {
            let saved: Option<Value> = sqlx::query_scalar(
                "SELECT admin_grants FROM explore.place_removal_pending WHERE event_id=$1",
            )
            .bind(event.event_id)
            .fetch_optional(&state.pool)
            .await?;
            streaming::notifications::grant_pairs(saved.as_ref())
        } else {
            Vec::new()
        };
        let scope = Scope::Removal {
            revoked_at: event.removed_at,
            admin_keys: &admin_keys,
            grants: &grants,
        };
        let (resolved_owner, all) =
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                let owner = owner(state, &context).await?;
                let all = recipients(state, &context, owner.clone(), scope).await?;
                Ok::<_, ApiError>((owner, all))
            })
            .await
            .map_err(|_| ApiError::http(503, "place notification authority timed out"))??;
        feed.save_owner(event.event_id, resolved_owner.as_str())
            .await?;
        let mut resolved: Vec<crate::Address> = if event.world {
            Vec::new()
        } else {
            event.recipients.clone()
        };
        resolved.extend(all);
        resolved.sort();
        resolved.dedup();
        let saved:Option<(Vec<crate::Address>,i32)>=sqlx::query_as("UPDATE explore.place_removal_pending SET recipients=$2,recipients_resolved=true WHERE event_id=$1 AND NOT recipients_resolved RETURNING recipients,recipient_cursor")
            .bind(event.event_id).bind(&resolved).fetch_optional(&state.pool).await?;
        if let Some((recipients, cursor)) = saved {
            event.recipients = recipients;
            event.recipient_cursor = cursor;
        } else {
            let Some((recipients,cursor)):Option<(Vec<crate::Address>,i32)>=sqlx::query_as("SELECT recipients,recipient_cursor FROM explore.place_removal_pending WHERE event_id=$1 AND recipients_resolved")
                .bind(event.event_id).fetch_optional(&state.pool).await? else {return Ok(())};
            event.recipients = recipients;
            event.recipient_cursor = cursor;
        }
    }
    let cursor = usize::try_from(event.recipient_cursor)
        .map_err(|_| ApiError::internal("invalid place notification cursor"))?;
    if cursor > event.recipients.len() {
        return Err(ApiError::internal("invalid place notification cursor"));
    }
    let key = format!("place-removed-{}", event.event_id);
    for (index, recipient) in event.recipients.iter().enumerate().skip(cursor) {
        let notice = notification(
            &context,
            recipient.clone(),
            &key,
            StreamingNotificationKind::PlaceUpdated,
            event.removed_at,
        );
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            state.social.notify_streaming_notification(&notice),
        )
        .await
        .map_err(|_| ApiError::http(503, "place notification delivery timed out"))??;
        let changed=sqlx::query("UPDATE explore.place_removal_pending SET recipient_cursor=$3 WHERE event_id=$1 AND recipient_cursor=$2 AND recipients_resolved")
            .bind(event.event_id).bind(index as i32).bind(index as i32+1).execute(&state.pool).await?;
        if changed.rows_affected() == 0 {
            return Err(ApiError::internal("place notification checkpoint changed"));
        }
    }
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

/// The scene behind `room`, with the key its explicit admins are granted on
/// and the key its bans live under.
async fn metadata_scene_context(
    state: &CommsState,
    room: &str,
) -> Result<Option<(PlaceContext, String, crate::access::SceneBanKey)>, ApiError> {
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
        Some(ParsedRoom::Local(crate::rooms::ResolvedRoom {
            realm,
            scene_id: Some(scene),
            ..
        })) => {
            // Adapter lookup also serves historical deployments. Metadata needs
            // the scene currently standing at its base, including after local
            // unpublish tombstones/repoints without setting a deleter.
            let resolved = state
                .scenes
                .resolve(crate::rooms::RoomRequest::Adapter {
                    realm: &realm,
                    scene: &scene,
                })
                .await
                .map_err(|_| ApiError::http(503, "Room identity is temporarily unavailable"))?;
            if resolved.room != room
                || resolved.realm != realm
                || resolved.scene_id.as_ref() != Some(&scene)
            {
                return Ok(None);
            }
            let Some(deployment) = resolved.deployment.as_ref() else {
                return Ok(None);
            };
            let current = match state
                .scenes
                .resolve(crate::rooms::RoomRequest::Parcel {
                    realm: &realm,
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
                && current.realm == realm
                && current.scene_id.as_ref() == Some(&scene)
                && current.deployment.as_ref().is_some_and(|current| {
                    current.scene_id == scene && current.parcels.contains(&deployment.base)
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
    let context = PlaceContext {
        place_id: scene.as_str().to_owned(),
        // Current policy is authoritative for the owner. A Places snapshot
        // cannot replace scene-keyed rows, owner or active parcel authority.
        owner_address: None,
        world: resolved.realm.is_world(),
        world_name: resolved.realm.world().map(|world| world.key().to_owned()),
        base_position: deployment.base.canonical(),
        positions: json!(deployment.parcels.canonical()),
    };
    let Some(ban_key) = resolved.ban_key() else {
        return Ok(None);
    };
    Ok(Some((context, deployment.place_key(), ban_key)))
}

async fn room_metadata_fields(
    state: &CommsState,
    room: &str,
) -> Result<Option<serde_json::Map<String, Value>>, ApiError> {
    let Some((context, admin_key, ban_key)) = metadata_scene_context(state, room).await? else {
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
        .list_active_admins(&admin_key, None)
        .await?
    {
        admins.insert(admin.admin);
    }
    let bans: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT banned_address FROM social.scene_bans WHERE place_id = ANY($1::text[]) ORDER BY banned_address",
    )
    .bind([ban_key.as_str().to_owned(), context.place_id.clone()].as_slice())
    .fetch_all(&state.pool)
    .await?;
    Ok(Some(serde_json::Map::from_iter([
        ("sceneAdmins".to_owned(), json!(admins)),
        ("bannedAddresses".to_owned(), json!(bans)),
    ])))
}

// Retired world rooms have no current role authority. Only reconcile their
// ban projection, using the resolver's exact-world retained mapping. Re-read
// durable bans each pass: an earlier unban must not erase a later reban.
async fn retired_room_ban_fields(
    state: &CommsState,
    room: &str,
) -> Result<Option<serde_json::Map<String, Value>>, ApiError> {
    let Some((world, scene)) = crate::rooms::decode_world_scene_room(room) else {
        return Ok(None);
    };
    let Some(replaced) = state
        .scenes
        .replaced_world_scene(&world, &scene)
        .await
        .map_err(|_| ApiError::http(503, "Retired room identity is temporarily unavailable"))?
    else {
        return Ok(None);
    };
    let mut keys: Vec<String> = replaced
        .ban_keys(&world)
        .iter()
        .map(|key| key.as_str().to_owned())
        .collect();
    keys.push(scene.as_str().to_owned());
    let bans: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT banned_address FROM social.scene_bans WHERE place_id = ANY($1::text[]) ORDER BY banned_address",
    )
    .bind(&keys)
    .fetch_all(&state.pool)
    .await?;
    Ok(Some(serde_json::Map::from_iter([(
        "bannedAddresses".to_owned(),
        json!(bans),
    )])))
}

async fn refresh_room(state: &CommsState, room: &str) -> Result<(), ApiError> {
    let fields = match room_metadata_fields(state, room).await? {
        Some(fields) => fields,
        None => match retired_room_ban_fields(state, room).await? {
            Some(fields) => fields,
            None => return Ok(()),
        },
    };
    state
        .room_service()
        .merge_room_metadata(room, fields)
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
    let after = state.metadata_cursor.lock().clone();
    let (rooms, next) = metadata_batch(rooms, after.as_deref());
    // Advance even across an unavailable room, so one room cannot starve later
    // rooms. A new process starts a fresh bounded pass over this derived state.
    *state.metadata_cursor.lock() = next;
    for room in rooms {
        let result = refresh_room(state, &room).await;
        if let Err(error) = result {
            tracing::warn!(room, error = %error.message, "room metadata refresh retained for the next pass");
        }
    }
    Ok(())
}

pub async fn run_once(state: &SharedState) -> anyhow::Result<()> {
    if let Err(error) = crate::gatekeeper::revocation::sweep(state).await {
        tracing::warn!(%error, "world access revocation sweep retained for retry");
    }
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
mod retired_scene_tests;

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
        async fn world_scene_count(
            &self,
            world: &crate::world_identity::WorldName,
        ) -> Result<usize, crate::rooms::ResolveError> {
            Ok(usize::from(self.world_active_scene(world).await?.is_some()))
        }
    }

    #[tokio::test]
    async fn genesis_owner_ignores_captured_scene_metadata() {
        let actual = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let state = owner_state(StubPolicy {
            land: Ok(Some(Arc::new(Some(crate::access::LandOperators {
                owner: actual.into(),
                operator: None,
                update_operator: None,
                update_managers: vec![],
                approved_for_all: vec![],
            })))),
            ..Default::default()
        });
        let mut context = world_context(Some("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        context.world = false;
        context.world_name = None;
        assert_eq!(owner(&state, &context).await.unwrap().as_str(), actual);
        assert_eq!(
            owner(&owner_state(StubPolicy::default()), &context)
                .await
                .unwrap_err()
                .code,
            503
        );
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
        async fn record_community_voice_ended(
            &self,
            c: CommunityId,
            k: &str,
            e: &Value,
            at: NaiveDateTime,
        ) -> Result<(), sqlx::Error> {
            self.stub.record_community_voice_ended(c, k, e, at).await
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
        async fn notify_user_notifications(
            &self,
            transaction: &mut sqlx::PgConnection,
            notifications: &[crate::interfaces::UserNotification],
        ) -> Result<(), sqlx::Error> {
            self.stub
                .notify_user_notifications(transaction, notifications)
                .await
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
        let event = sqlx::query_as("SELECT event_id, place_id, owner_address, world, world_name, base_position, positions, removed_at, cleaned_at, admin_keys, recipients, recipients_resolved, recipient_cursor, notify FROM explore.place_removal_pending WHERE place_id = $1").bind(&id).fetch_one(&pool).await.unwrap();
        (state, directory, event)
    }
    #[tokio::test]
    async fn failed_revocation_sweep_does_not_block_cleanup_or_metadata_inventory() {
        use axum::{Json, Router, extract::State, response::IntoResponse, routing::post};
        use std::sync::atomic::AtomicUsize;
        // A database of its own: `run_once` takes every pending removal, and
        // would clean those of the tests that share the scratch database.
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            return;
        };
        let (mut state, _, event) = setup(pool.clone()).await;
        let calls = Arc::new(AtomicUsize::new(0));
        async fn list_rooms(State(calls): State<Arc<AtomicUsize>>) -> axum::response::Response {
            if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();
            }
            Json(json!({"rooms":[]})).into_response()
        }
        let app = Router::new()
            .route("/twirp/livekit.RoomService/ListRooms", post(list_rooms))
            .with_state(calls.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        state.livekit.api_url = format!("http://{}", listener.local_addr().unwrap());
        state.livekit.configured = true;
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        run_once(&Arc::new(state)).await.unwrap();
        server.abort();
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "metadata inventory ran after the failed access sweep"
        );
        let cleaned: bool = sqlx::query_scalar(
            "SELECT cleaned_at IS NOT NULL FROM explore.place_removal_pending WHERE event_id=$1",
        )
        .bind(event.event_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            cleaned,
            "removed-place cleanup committed despite the access-sweep outage"
        );
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
            let (context, _, _) = metadata_scene_context(&state, &room)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                context.place_id,
                scene.as_str(),
                "Genesis bans use original scene ID, never a Places UUID/world key"
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
            async fn world_grantees(
                &self,
                world: &WorldName,
                parcels: &ParcelSet,
            ) -> Result<std::collections::BTreeSet<Address>, crate::AuthorityNotEstablished>
            {
                self.protected_addresses(world, parcels).await
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
        sqlx::query("UPDATE social.scene_bans SET place_id=$1 WHERE place_id=$2")
            .bind(format!("{}:5,6", first.key()))
            .bind(scene.as_str())
            .execute(&pool)
            .await
            .unwrap();
        state.policy = Arc::new(FootprintPolicy(parcels));
        state.scenes = Arc::new(StubScenes {
            room: Ok(resolved),
            ..Default::default()
        });
        let writes = Arc::new(parking_lot::Mutex::new(Vec::<Value>::new()));
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
                        captured.lock().push(body);
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
            writes.lock().len(),
            1,
            "missing competitor Place cannot affect exact scene identity"
        );
        let metadata: Value =
            serde_json::from_str(writes.lock()[0]["metadata"].as_str().unwrap()).unwrap();
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
            writes.lock().len(),
            2,
            "malformed competitor footprint cannot change the exact-world enrichment"
        );
        assert!(writes.lock().iter().all(|body| body["room"] == room));
        state.scenes = Arc::new(StubScenes {
            room: Err(crate::rooms::ResolveError::UnresolvedRoom(room.clone())),
            ..Default::default()
        });
        refresh_room(&state, &room).await.unwrap();
        assert_eq!(
            writes.lock().len(),
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
            updates: Arc<parking_lot::Mutex<Vec<Value>>>,
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
            mock.updates.lock().push(body);
            Json(json!({}))
        }
        let updates = Arc::new(parking_lot::Mutex::new(vec![]));
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
        let updates = updates.lock();
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
    async fn streaming_fanout_removal_enrichment_failure_follows_safety_commit() {
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            return;
        };
        let (state, _, event) = setup(pool.clone()).await;
        sqlx::query("INSERT INTO social.scene_admins(place_id,admin_address,added_by,added_at) VALUES ('0,0','0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb','owner',$1)")
            .bind(event.removed_at-chrono::Duration::seconds(1)).execute(&pool).await.unwrap();
        sqlx::raw_sql("ALTER TABLE social.scene_admins RENAME TO test_scene_admins; \
            CREATE FUNCTION social.fail_admin_read(text) RETURNS text LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'notification read unavailable'; END $$; \
            CREATE VIEW social.scene_admins AS SELECT place_id,social.fail_admin_read(admin_address) AS admin_address,added_by,added_at,active,removed_at FROM social.test_scene_admins")
            .execute(&pool).await.unwrap();
        let state = Arc::new(state);
        let feed = RemovedPlaces::new(pool.clone());
        assert!(cleanup(&state, &feed, &event).await.is_err());
        let cleaned: bool = sqlx::query_scalar(
            "SELECT cleaned_at IS NOT NULL FROM explore.place_removal_pending WHERE event_id=$1",
        )
        .bind(event.event_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            cleaned,
            "a notification-only SQL error must follow the safety commit"
        );
        let active: bool =
            sqlx::query_scalar("SELECT active FROM social.test_scene_admins WHERE place_id='0,0'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(!active);
    }

    #[tokio::test]
    async fn streaming_fanout_removed_place_preserves_admins_during_authority_outage() {
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            return;
        };
        let (state, directory, event) = setup(pool.clone()).await;
        let admin = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        sqlx::query("INSERT INTO social.scene_admins (place_id,admin_address,added_by,added_at) VALUES ('0,0',$1,$1,$2)")
            .bind(admin).bind(event.removed_at-chrono::Duration::seconds(1)).execute(&pool).await.unwrap();
        let feed = RemovedPlaces::new(pool.clone());
        let mut state = Arc::new(state);
        assert_eq!(cleanup(&state, &feed, &event).await.unwrap_err().code, 503);
        let active: bool = sqlx::query_scalar(
            "SELECT active FROM social.scene_admins WHERE place_id='0,0' AND admin_address=$1",
        )
        .bind(admin)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            !active,
            "unknown external authority must not prevent revocation"
        );
        // The grant row can change before notification authority recovers.
        sqlx::query("UPDATE social.scene_admins SET active=true,removed_at=NULL,added_at=$2 WHERE place_id='0,0' AND admin_address=$1")
            .bind(admin).bind(event.removed_at+chrono::Duration::seconds(1)).execute(&pool).await.unwrap();
        Arc::get_mut(&mut state).unwrap().policy = Arc::new(StubPolicy {
            land: Ok(Some(Arc::new(Some(crate::access::LandOperators {
                owner: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
                operator: Some("0xcccccccccccccccccccccccccccccccccccccccc".into()),
                update_operator: None,
                update_managers: vec![],
                approved_for_all: vec![],
            })))),
            ..Default::default()
        });
        let saved:RemovedPlace=sqlx::query_as("SELECT event_id, place_id, owner_address, world, world_name, base_position, positions, removed_at, cleaned_at, admin_keys, recipients, recipients_resolved, recipient_cursor, notify FROM explore.place_removal_pending WHERE event_id=$1")
            .bind(event.event_id).fetch_one(&pool).await.unwrap();
        assert!(saved.cleaned_at.is_some());
        cleanup(&state, &feed, &saved).await.unwrap();
        let notices = directory.stub.streaming_notifications.lock();
        let actual: std::collections::BTreeSet<_> =
            notices.iter().map(|n| n.recipient.as_str()).collect();
        assert_eq!(
            actual,
            [
                "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                admin,
                "0xcccccccccccccccccccccccccccccccccccccccc"
            ]
            .into_iter()
            .collect(),
            "the pre-revocation admin must survive an authority outage and a grant rewrite"
        );
        assert_eq!(notices.len(), 3);
        assert!(
            notices
                .iter()
                .all(|n| n.kind == StreamingNotificationKind::PlaceUpdated)
        );
    }

    #[tokio::test]
    async fn streaming_fanout_removal_keeps_admin_regranted_before_first_maintenance() {
        for world in [false, true] {
            let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
                return;
            };
            let (mut state, directory, old_event) = setup(pool.clone()).await;
            sqlx::query("DELETE FROM explore.place_removal_pending WHERE event_id=$1")
                .bind(old_event.event_id)
                .execute(&pool)
                .await
                .unwrap();
            let place = &old_event.place_id;
            let admin = "0XBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";
            let owner = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
            let admin_key = if world {
                let identity = uuid::Uuid::new_v4();
                sqlx::query("INSERT INTO explore.world_scene_place(world_name,identity,entity_id,parcels) VALUES ('readd.eth',$1,'readd',ARRAY['0,0'])")
                    .bind(identity).execute(&pool).await.unwrap();
                identity.to_string()
            } else {
                "0,0".to_owned()
            };
            let raw = json!({"world":world,"world_name":if world {Some("readd.eth")} else {None},"owner":owner,"positions":["0,0"]});
            sqlx::query("INSERT INTO explore.place(id,base_position,raw) VALUES ($1,'0,0',$2)")
                .bind(place)
                .bind(&raw)
                .execute(&pool)
                .await
                .unwrap();
            for value in [admin, "invalid-historical-admin"] {
                sqlx::query("INSERT INTO social.scene_admins(place_id,admin_address,added_by,added_at) VALUES ($1,$2,$3,timezone('UTC',clock_timestamp())-interval '1 second')")
                    .bind(&admin_key).bind(value).bind(owner).execute(&pool).await.unwrap();
            }
            sqlx::query("DELETE FROM explore.place WHERE id=$1")
                .bind(place)
                .execute(&pool)
                .await
                .unwrap();
            let event: RemovedPlace =
                sqlx::query_as("SELECT * FROM explore.place_removal_pending WHERE place_id=$1")
                    .bind(place)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            sqlx::query("INSERT INTO explore.place(id,base_position,raw) VALUES ($1,'0,0',$2)")
                .bind(place)
                .bind(raw)
                .execute(&pool)
                .await
                .unwrap();
            // A grant upsert replaces its prior added_at before maintenance has
            // captured any local recipients. The queued event must own them.
            sqlx::query("UPDATE social.scene_admins SET active=true,removed_at=NULL,added_at=$3 WHERE place_id=$1 AND admin_address=$2")
                .bind(&admin_key).bind(admin).bind(event.removed_at+chrono::Duration::seconds(1)).execute(&pool).await.unwrap();
            state.policy = if world {
                let owner = Address::parse(owner).unwrap();
                Arc::new(
                    crate::gatekeeper::streaming::notifications::test_support::WorldPolicy {
                        owner: owner.clone(),
                        wide: Default::default(),
                        protected: [owner].into_iter().collect(),
                    },
                )
            } else {
                Arc::new(StubPolicy {
                    land: Ok(Some(Arc::new(Some(crate::access::LandOperators {
                        owner: owner.into(),
                        operator: None,
                        update_operator: None,
                        update_managers: vec![],
                        approved_for_all: vec![],
                    })))),
                    ..Default::default()
                })
            };
            cleanup(&Arc::new(state), &RemovedPlaces::new(pool.clone()), &event)
                .await
                .unwrap();
            {
                let notices = directory.stub.streaming_notifications.lock();
                assert_eq!(
                    notices.len(),
                    2,
                    "world={world}: queued recipients survive pre-maintenance regrant"
                );
                assert!(
                    notices
                        .iter()
                        .any(|notice| notice.recipient.as_str() == admin.to_lowercase())
                );
            }
            let active: bool = sqlx::query_scalar(
                "SELECT active FROM social.scene_admins WHERE place_id=$1 AND admin_address=$2",
            )
            .bind(&admin_key)
            .bind(admin)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert!(active, "a grant added after removal remains active");
        }
    }

    #[tokio::test]
    async fn streaming_fanout_world_removal_keeps_original_granter_after_regrant_and_retirement() {
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            return;
        };
        let (mut state, directory, old_event) = setup(pool.clone()).await;
        sqlx::query("DELETE FROM explore.place_removal_pending WHERE event_id=$1")
            .bind(old_event.event_id)
            .execute(&pool)
            .await
            .unwrap();
        let place = &old_event.place_id;
        let owner = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let admin = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let unauthorized = "0xcccccccccccccccccccccccccccccccccccccccc";
        let stranger = "0xdddddddddddddddddddddddddddddddddddddddd";
        let identity = uuid::Uuid::new_v4();
        sqlx::query("INSERT INTO explore.world_scene_place(world_name,identity,entity_id,parcels) VALUES ('regrant.eth',$1,'regrant',ARRAY['0,0'])")
            .bind(identity).execute(&pool).await.unwrap();
        let key = identity.to_string();
        let raw =
            json!({"world":true,"world_name":"regrant.eth","owner":owner,"positions":["0,0"]});
        sqlx::query("INSERT INTO explore.place(id,base_position,raw) VALUES ($1,'0,0',$2)")
            .bind(place)
            .bind(&raw)
            .execute(&pool)
            .await
            .unwrap();
        // The owner granted `admin`; `unauthorized` was granted by a wallet
        // without the world-wide authority, so it is never told.
        for (value, granter) in [(admin, owner), (unauthorized, stranger)] {
            sqlx::query("INSERT INTO social.scene_admins(place_id,admin_address,added_by,added_at) VALUES ($1,$2,$3,timezone('UTC',clock_timestamp())-interval '1 second')")
                .bind(&key).bind(value).bind(granter).execute(&pool).await.unwrap();
        }
        sqlx::query("DELETE FROM explore.place WHERE id=$1")
            .bind(place)
            .execute(&pool)
            .await
            .unwrap();
        let event: RemovedPlace =
            sqlx::query_as("SELECT * FROM explore.place_removal_pending WHERE place_id=$1")
                .bind(place)
                .fetch_one(&pool)
                .await
                .unwrap();
        let frozen: Value = sqlx::query_scalar(
            "SELECT admin_grants FROM explore.place_removal_pending WHERE event_id=$1",
        )
        .bind(event.event_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            frozen.as_array().unwrap().contains(&json!([admin, owner])),
            "the event froze the original pair: {frozen}"
        );
        // Before maintenance: the scene retires and a wallet without authority
        // re-grants `admin`, overwriting the row's granter and time.
        sqlx::query(
            "UPDATE explore.world_scene_place SET active=false, entity_id=NULL WHERE identity=$1",
        )
        .bind(identity)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("UPDATE social.scene_admins SET added_by=$3,added_at=$4,active=true,removed_at=NULL WHERE place_id=$1 AND admin_address=$2")
            .bind(&key).bind(admin).bind(stranger).bind(event.removed_at+chrono::Duration::seconds(1)).execute(&pool).await.unwrap();
        let owner_address = Address::parse(owner).unwrap();
        state.policy = Arc::new(
            crate::gatekeeper::streaming::notifications::test_support::WorldPolicy {
                owner: owner_address.clone(),
                wide: Default::default(),
                protected: [owner_address].into_iter().collect(),
            },
        );
        cleanup(&Arc::new(state), &RemovedPlaces::new(pool.clone()), &event)
            .await
            .unwrap();
        let told: Vec<String> = directory
            .stub
            .streaming_notifications
            .lock()
            .iter()
            .map(|notice| notice.recipient.as_str().to_owned())
            .collect();
        assert!(
            told.iter().any(|r| r == admin),
            "the frozen original granter still qualifies the re-granted admin: {told:?}"
        );
        assert!(
            !told.iter().any(|r| r == unauthorized),
            "an admin whose granter lacks authority stays excluded: {told:?}"
        );
    }

    #[tokio::test]
    async fn streaming_fanout_place_removal_retries_all_genesis_manager_roles() {
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            return;
        };
        let (mut state, directory, event) = setup(pool.clone()).await;
        let wallet = |n: u8| Address::parse(&format!("0x{n:040x}")).unwrap();
        state.policy = Arc::new(StubPolicy {
            land: Ok(Some(Arc::new(Some(crate::access::LandOperators {
                owner: wallet(1).to_string(),
                operator: Some(wallet(3).to_string()),
                update_operator: Some(wallet(4).to_string()),
                update_managers: vec![wallet(5).to_string()],
                approved_for_all: vec![wallet(6).to_string()],
            })))),
            ..Default::default()
        });
        for (admin, active) in [(wallet(2), true), (wallet(3), true), (wallet(7), false)] {
            sqlx::query("INSERT INTO social.scene_admins (place_id,admin_address,added_by,added_at,active,removed_at) VALUES ('0,0',$1,$1,$2,$3,CASE WHEN $3 THEN NULL ELSE $2 END)")
                .bind(admin).bind(event.removed_at-chrono::Duration::seconds(1)).bind(active).execute(&pool).await.unwrap();
        }
        *directory.stub.fail_streaming_recipient.lock() = Some(wallet(2));
        let feed = RemovedPlaces::new(pool.clone());
        let mut state = Arc::new(state);
        assert!(
            cleanup(&state, &feed, &event).await.is_err(),
            "second-recipient failure must keep the remaining fanout pending"
        );
        assert_eq!(directory.stub.streaming_notifications.lock().len(), 1);
        Arc::get_mut(&mut state).unwrap().policy = Arc::new(StubPolicy::default());
        cleanup(&state, &feed, &event).await.unwrap();
        let notices = directory.stub.streaming_notifications.lock();
        assert_eq!(notices.len(), 6);
        assert_eq!(
            notices
                .iter()
                .map(|n| n.recipient.clone())
                .collect::<std::collections::BTreeSet<_>>(),
            (1..=6).map(wallet).collect()
        );
        assert!(
            notices
                .iter()
                .all(|n| n.kind == StreamingNotificationKind::PlaceUpdated
                    && n.metadata["address"] == n.recipient.as_str())
        );
        assert_eq!(
            *directory.stub.streaming_attempts.lock(),
            [1, 2, 2, 3, 4, 5, 6].map(wallet)
        );
    }

    #[tokio::test]
    async fn streaming_fanout_live_disable_freezes_keys_before_later_retirement() {
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            return;
        };
        let identity = uuid::Uuid::new_v4();
        sqlx::query("INSERT INTO worlds.worlds(name) VALUES ('disabled-first.eth')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO worlds.world_scenes(world_name,entity_id,deployment_auth_chain,entity,deployer,parcels,size) VALUES ('disabled-first.eth','live','[]','{}','owner',ARRAY['1,2'],0)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO explore.world_scene_place(world_name,identity,entity_id,parcels) VALUES ('disabled-first.eth',$1,'live',ARRAY['1,2'])")
            .bind(identity).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO explore.place(id,base_position,raw) VALUES ('disabled-first','1,2',$1)",
        )
        .bind(json!({"world":true,"world_name":"disabled-first.eth"}))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("UPDATE explore.place SET disabled=true WHERE id='disabled-first'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM worlds.world_scenes WHERE world_name='disabled-first.eth'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE explore.world_scene_place SET active=false,entity_id=NULL WHERE identity=$1",
        )
        .bind(identity)
        .execute(&pool)
        .await
        .unwrap();
        let keys: Vec<String> = sqlx::query_scalar(
            "SELECT admin_keys FROM explore.place_removal_pending WHERE place_id='disabled-first'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(keys, vec![identity.to_string()]);
    }

    #[tokio::test]
    async fn streaming_fanout_scene_readd_locks_identity_before_catalogue() {
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            return;
        };
        let identity = uuid::Uuid::new_v4();
        sqlx::query("INSERT INTO worlds.worlds(name) VALUES ('lock-order.eth')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO explore.world_scene_place(world_name,identity,entity_id,parcels) VALUES ('lock-order.eth',$1,'bound',ARRAY['1,2'])").bind(identity).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO explore.place(id,base_position,raw,removal_admin_keys) VALUES ('lock-order','1,2',$1,ARRAY['saved'])")
            .bind(json!({"world":true,"world_name":"lock-order.eth"})).execute(&pool).await.unwrap();
        let mut ranking = pool.begin().await.unwrap();
        sqlx::query("SELECT identity FROM explore.world_scene_place WHERE identity=$1 FOR UPDATE")
            .bind(identity)
            .execute(&mut *ranking)
            .await
            .unwrap();
        let mut publication = pool.begin().await.unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *publication)
            .await
            .unwrap();
        let publish = tokio::spawn(async move {
            sqlx::query("INSERT INTO worlds.world_scenes(world_name,entity_id,deployment_auth_chain,entity,deployer,parcels,size) VALUES ('lock-order.eth','bound','[]','{}','owner',ARRAY['1,2'],0)").execute(&mut *publication).await.unwrap();
            publication.commit().await.unwrap();
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1 AND wait_event_type='Lock')").bind(pid).fetch_one(&pool).await.unwrap();
            if waiting {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "publication did not reach identity lock"
            );
            tokio::task::yield_now().await;
        }
        // Publication must still be waiting on the identity, so ranking can
        // acquire the catalogue lock without forming the opposite lock cycle.
        sqlx::query("SELECT id FROM explore.place WHERE id='lock-order' FOR UPDATE NOWAIT")
            .execute(&mut *ranking)
            .await
            .unwrap();
        ranking.rollback().await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), publish)
            .await
            .unwrap()
            .unwrap();
        let empty: bool = sqlx::query_scalar(
            "SELECT cardinality(removal_admin_keys)=0 FROM explore.place WHERE id='lock-order'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(empty);
    }

    #[tokio::test]
    async fn streaming_fanout_retired_world_admins_survive_catalogue_delay_and_readd() {
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            return;
        };
        let (mut state, directory, _) = setup(pool.clone()).await;
        let wallet = |n: u8| Address::parse(&format!("0x{n:040x}")).unwrap();
        state.policy = Arc::new(
            crate::gatekeeper::streaming::notifications::test_support::WorldPolicy {
                owner: wallet(1),
                wide: Default::default(),
                protected: [wallet(1), wallet(4)].into_iter().collect(),
            },
        );
        let name = "retired-admins.dcl.eth";
        let ids = [
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
        ];
        sqlx::query("INSERT INTO worlds.worlds(name) VALUES ($1)")
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
        for (index, entity) in ["first", "second"].iter().enumerate() {
            sqlx::query("INSERT INTO worlds.world_scenes(world_name,entity_id,deployment_auth_chain,entity,deployer,parcels,size) VALUES ($1,$2,'[]','{}','owner',ARRAY[$3],0)")
                .bind(name).bind(entity).bind(if index==0 {"10,20"} else {"30,40"}).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO explore.world_scene_place(world_name,identity,entity_id,parcels) VALUES ($1,$2,$3,ARRAY[$4])")
                .bind(name).bind(ids[index]).bind(entity).bind(if index==0 {"10,20"} else {"30,40"}).execute(&pool).await.unwrap();
        }
        // Ancient inactive identities must never be mistaken for this removal.
        sqlx::query("INSERT INTO explore.world_scene_place(world_name,identity,entity_id,parcels,active) VALUES ($1,$2,NULL,ARRAY['10,20'],false)")
            .bind(name).bind(ids[2]).execute(&pool).await.unwrap();
        for (index, id) in ids.iter().enumerate() {
            sqlx::query("INSERT INTO social.scene_admins(place_id,admin_address,added_by) VALUES ($1,$2,$3)")
                .bind(id.to_string()).bind(wallet(if index==2 {9} else {index as u8+2})).bind(wallet(1)).execute(&pool).await.unwrap();
        }
        for local in [false, true] {
            let id = "retired-place";
            if local {
                sqlx::query("INSERT INTO explore.place_world_local(id,base_position,world,world_name,raw) VALUES ($1,'10,20',true,$2,$3)")
                    .bind(id).bind(name).bind(json!({"owner":wallet(1).as_str(),"positions":["10,20","30,40"]})).execute(&pool).await.unwrap();
            } else {
                sqlx::query("INSERT INTO explore.place(id,base_position,raw) VALUES ($1,'10,20',$2)")
                    .bind(id).bind(json!({"world":true,"world_name":name,"owner":wallet(1).as_str(),"positions":["10,20","30,40"]})).execute(&pool).await.unwrap();
            }
        }
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("DELETE FROM worlds.world_scenes WHERE world_name=$1")
            .bind(name)
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("UPDATE explore.world_scene_place SET active=false,entity_id=NULL WHERE world_name=$1 AND active").bind(name).execute(&mut *tx).await.unwrap();
        tx.commit().await.unwrap();
        for table in ["explore.place", "explore.place_world_local"] {
            let keys: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT removal_admin_keys FROM {table} WHERE world_name=$1"
            )))
            .bind(name)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(
                keys.into_iter().collect::<std::collections::BTreeSet<_>>(),
                ids[..2].iter().map(ToString::to_string).collect()
            );
        }
        // Identity-first import must retain the snapshot until a real scene returns.
        sqlx::query("INSERT INTO explore.world_scene_place(world_name,identity,entity_id,parcels) VALUES ($1,$2,'replacement',ARRAY['50,60'])")
            .bind(name).bind(uuid::Uuid::new_v4()).execute(&pool).await.unwrap();
        sqlx::query("UPDATE explore.place SET disabled=true WHERE id='retired-place'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE explore.place_world_local SET disabled=true WHERE id='retired-place'")
            .execute(&pool)
            .await
            .unwrap();
        let events: Vec<RemovedPlace> = sqlx::query_as(
            "SELECT * FROM explore.place_removal_pending WHERE world_name=$1 ORDER BY place_id",
        )
        .bind(name)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(events.len(), 1);
        assert!(events.iter().all(|event| {
            ids[..2]
                .iter()
                .all(|id| event.admin_keys.contains(&id.to_string()))
                && !event.admin_keys.contains(&ids[2].to_string())
        }));
        sqlx::query("INSERT INTO worlds.world_scenes(world_name,entity_id,deployment_auth_chain,entity,deployer,parcels,size) VALUES ($1,'replacement','[]','{}','owner',ARRAY['50,60'],0)")
            .bind(name).execute(&pool).await.unwrap();
        let remaining: i64 = sqlx::query_scalar("SELECT cardinality(removal_admin_keys)::bigint FROM explore.place WHERE id='retired-place'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(
            remaining, 0,
            "real scene re-add clears only catalogue evidence"
        );
        let feed = RemovedPlaces::new(pool.clone());
        let state = Arc::new(state);
        for event in events {
            cleanup(&state, &feed, &event).await.unwrap();
        }
        {
            let notices = directory.stub.streaming_notifications.lock();
            assert_eq!(notices.len(), 4);
            for key in notices
                .iter()
                .map(|notice| &notice.event_key)
                .collect::<std::collections::BTreeSet<_>>()
            {
                assert_eq!(
                    notices
                        .iter()
                        .filter(|notice| &notice.event_key == key)
                        .map(|notice| notice.recipient.clone())
                        .collect::<std::collections::BTreeSet<_>>(),
                    (1..=4).map(wallet).collect()
                );
            }
        }
        // The unrelated ancient grant remains active and receives no notice.
        let ancient_active: bool =
            sqlx::query_scalar("SELECT active FROM social.scene_admins WHERE place_id=$1")
                .bind(ids[2].to_string())
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(ancient_active);
    }

    #[tokio::test]
    async fn streaming_fanout_shared_genesis_base_notifies_without_revoking_live_grant() {
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            return;
        };
        let (mut state, directory, event) = setup(pool.clone()).await;
        let admin = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        state.policy = Arc::new(StubPolicy {
            land: Ok(Some(Arc::new(Some(crate::access::LandOperators {
                owner: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
                operator: None,
                update_operator: None,
                update_managers: vec![],
                approved_for_all: vec![],
            })))),
            ..Default::default()
        });
        sqlx::query(
            "INSERT INTO explore.place(id,base_position,raw) VALUES ('still-live','0,0','{}')",
        )
        .execute(&pool)
        .await
        .unwrap();
        for address in [admin, "invalid-historical-admin"] {
            sqlx::query("INSERT INTO social.scene_admins(place_id,admin_address,added_by,added_at) VALUES ('0,0',$1,$1,$2)")
                .bind(address).bind(event.removed_at-chrono::Duration::seconds(1)).execute(&pool).await.unwrap();
        }
        cleanup(&Arc::new(state), &RemovedPlaces::new(pool.clone()), &event)
            .await
            .unwrap();
        let active: bool =
            sqlx::query_scalar("SELECT active FROM social.scene_admins WHERE admin_address=$1")
                .bind(admin)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(active);
        let notices = directory.stub.streaming_notifications.lock();
        assert_eq!(notices.len(), 2);
        assert!(
            notices
                .iter()
                .any(|notice| notice.recipient.as_str() == admin)
        );
    }

    #[tokio::test]
    async fn streaming_fanout_imported_disabled_place_is_cleaned_without_notices() {
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            return;
        };
        let (state, directory, unused) = setup(pool.clone()).await;
        sqlx::query("DELETE FROM explore.place_removal_pending WHERE event_id=$1")
            .bind(unused.event_id)
            .execute(&pool)
            .await
            .unwrap();
        let tag = uuid::Uuid::new_v4();
        let raw = json!({"owner":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","positions":["0,0"]});
        let notify = |place: String| {
            let pool = pool.clone();
            async move {
                sqlx::query_scalar::<_, bool>(
                    "SELECT notify FROM explore.place_removal_pending WHERE place_id=$1",
                )
                .bind(place)
                .fetch_one(&pool)
                .await
                .unwrap()
            }
        };
        // An import (places' place.load.sql) inserting an already-disabled place.
        let imported = format!("imported-{tag}");
        sqlx::query("INSERT INTO social.scene_admins (place_id,admin_address,added_by,added_at) VALUES ('0,0','0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb','0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',timezone('UTC',now())-interval '1 hour')")
            .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO explore.place (id,base_position,disabled,raw) VALUES ($1,'0,0',true,$2)",
        )
        .bind(&imported)
        .bind(&raw)
        .execute(&pool)
        .await
        .unwrap();
        assert!(
            !notify(imported.clone()).await,
            "an imported historical removal is silent"
        );
        // An upgraded catalogue: an enabled place with no state row, disabled later.
        let upgraded = format!("upgraded-{tag}");
        sqlx::query("INSERT INTO explore.place (id,base_position,raw) VALUES ($1,'5,5',$2)")
            .bind(&upgraded)
            .bind(&raw)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM explore.place_removal_state WHERE place_id=$1")
            .bind(&upgraded)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE explore.place SET disabled=true WHERE id=$1")
            .bind(&upgraded)
            .execute(&pool)
            .await
            .unwrap();
        assert!(notify(upgraded).await, "a live disable notifies");
        // A disabled local row newly overriding an enabled catalogue row.
        let overridden = format!("overridden-{tag}");
        sqlx::query("INSERT INTO explore.place (id,base_position,raw) VALUES ($1,'6,6',$2)")
            .bind(&overridden)
            .bind(&raw)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO explore.place_world_local (id,base_position,disabled,raw) VALUES ($1,'6,6',true,$2)")
            .bind(&overridden).bind(&raw).execute(&pool).await.unwrap();
        assert!(
            notify(overridden).await,
            "a local override of a live place notifies"
        );

        let event: RemovedPlace =
            sqlx::query_as("SELECT * FROM explore.place_removal_pending WHERE place_id=$1")
                .bind(&imported)
                .fetch_one(&pool)
                .await
                .unwrap();
        cleanup(&Arc::new(state), &RemovedPlaces::new(pool.clone()), &event)
            .await
            .unwrap();
        assert!(directory.stub.streaming_notifications.lock().is_empty());
        let active: bool =
            sqlx::query_scalar("SELECT active FROM social.scene_admins WHERE place_id='0,0'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(!active, "historical cleanup still revokes the grant");
    }

    #[tokio::test]
    async fn streaming_fanout_historical_cleanup_suppresses_notices() {
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            return;
        };
        let (state, directory, unused) = setup(pool.clone()).await;
        sqlx::query("DELETE FROM explore.place_removal_pending WHERE event_id=$1")
            .bind(unused.event_id)
            .execute(&pool)
            .await
            .unwrap();
        let place = format!("historical-removal-{}", uuid::Uuid::new_v4());
        sqlx::query(
            "INSERT INTO explore.place (id,base_position,disabled,raw) VALUES ($1,'0,0',true,$2)",
        )
        .bind(&place)
        .bind(json!({"owner":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","positions":["0,0"]}))
        .execute(&pool)
        .await
        .unwrap();
        // Reproduce first feed activation against an already populated catalogue.
        sqlx::query("DELETE FROM explore.place_removal_pending WHERE place_id=$1")
            .bind(&place)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM explore.place_removal_state WHERE place_id=$1")
            .bind(&place)
            .execute(&pool)
            .await
            .unwrap();
        for statement in crate::db::schema_migrate::statements(crate::db::SCHEMA_SQL) {
            if statement.starts_with("INSERT INTO explore.place_removal_pending")
                || statement.starts_with("INSERT INTO explore.place_removal_state")
            {
                // Statements come only from this binary's embedded schema.
                sqlx::raw_sql(sqlx::AssertSqlSafe(statement))
                    .execute(&pool)
                    .await
                    .unwrap();
            }
        }
        let event:RemovedPlace=sqlx::query_as("SELECT event_id, place_id, owner_address, world, world_name, base_position, positions, removed_at, cleaned_at, admin_keys, recipients, recipients_resolved, recipient_cursor, notify FROM explore.place_removal_pending WHERE place_id=$1")
            .bind(&place).fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO social.scene_admins (place_id,admin_address,added_by,added_at) VALUES ('0,0','0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb','0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',$1)")
            .bind(event.removed_at-chrono::Duration::seconds(1)).execute(&pool).await.unwrap();
        let feed = RemovedPlaces::new(pool.clone());
        let mut state = Arc::new(state);
        cleanup(&state, &feed, &event)
            .await
            .expect("historical cleanup must not need notification authority");
        assert!(directory.stub.streaming_notifications.lock().is_empty());
        let active: bool =
            sqlx::query_scalar("SELECT active FROM social.scene_admins WHERE place_id='0,0'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(
            !active,
            "suppressing historical notifications must preserve admin cleanup"
        );
        let pending: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM explore.place_removal_pending WHERE place_id=$1)",
        )
        .bind(&place)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!pending);
        sqlx::query("UPDATE explore.place SET disabled=false WHERE id=$1")
            .bind(&place)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE explore.place SET disabled=true WHERE id=$1")
            .bind(&place)
            .execute(&pool)
            .await
            .unwrap();
        let later:RemovedPlace=sqlx::query_as("SELECT event_id, place_id, owner_address, world, world_name, base_position, positions, removed_at, cleaned_at, admin_keys, recipients, recipients_resolved, recipient_cursor, notify FROM explore.place_removal_pending WHERE place_id=$1")
            .bind(&place).fetch_one(&pool).await.unwrap();
        assert_ne!(later.event_id, event.event_id);
        Arc::get_mut(&mut state).unwrap().policy = Arc::new(StubPolicy {
            land: Ok(Some(Arc::new(Some(crate::access::LandOperators {
                owner: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
                operator: None,
                update_operator: None,
                update_managers: vec![],
                approved_for_all: vec![],
            })))),
            ..Default::default()
        });
        cleanup(&state, &feed, &later).await.unwrap();
        let notices = directory.stub.streaming_notifications.lock();
        assert_eq!(
            notices.len(),
            1,
            "a later genuine removal must still notify"
        );
        assert_eq!(notices[0].kind, StreamingNotificationKind::PlaceUpdated);
    }

    #[tokio::test]
    async fn notification_failure_keeps_checkpoint_and_readd_writes_survive_retry() {
        // A database of its own: the place's base parcel holds only this
        // test's grants.
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            return;
        };
        let (mut state, directory, event) = setup(pool.clone()).await;
        // Notification authority comes from LAND, independently of captured catalogue metadata.
        let land_owner = "0xcccccccccccccccccccccccccccccccccccccccc";
        state.policy = Arc::new(StubPolicy {
            land: Ok(Some(Arc::new(Some(crate::access::LandOperators {
                owner: land_owner.into(),
                operator: None,
                update_operator: None,
                update_managers: vec![],
                approved_for_all: vec![],
            })))),
            ..Default::default()
        });
        let wallet = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        // A scene admin on the base parcel, whose grant this cleanup revokes.
        let admin = "0xdddddddddddddddddddddddddddddddddddddddd";
        grant_before(&pool, &event, admin).await;
        sqlx::query("INSERT INTO social.scene_bans (place_id, banned_address, banned_by, banned_at) VALUES ($1, $2, $2, $3)").bind(&event.place_id).bind(wallet).bind(event.removed_at - chrono::Duration::seconds(1)).execute(&pool).await.unwrap();
        directory.fail.store(true, Ordering::SeqCst);
        let (feed, state) = (RemovedPlaces::new(pool.clone()), Arc::new(state));
        assert_eq!(
            cleanup(&state, &feed, &event).await.unwrap_err().code,
            500,
            "the injected notification failure must follow successful LAND resolution"
        );
        let saved: RemovedPlace = sqlx::query_as("SELECT event_id, place_id, owner_address, world, world_name, base_position, positions, removed_at, cleaned_at, admin_keys, recipients, recipients_resolved, recipient_cursor, notify FROM explore.place_removal_pending WHERE event_id = $1").bind(event.event_id).fetch_one(&pool).await.unwrap();
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
        let active: bool = sqlx::query_scalar(
            "SELECT active FROM social.scene_admins WHERE place_id = $1 AND admin_address = $2",
        )
        .bind(&event.base_position)
        .bind(admin)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!active, "the first pass revoked the grant");
        let notifications = directory.stub.streaming_notifications.lock();
        let mut told: Vec<&str> = notifications.iter().map(|n| n.recipient.as_str()).collect();
        told.sort();
        assert_eq!(
            told,
            [land_owner, admin],
            "the retry still tells the revoked admin"
        );
    }

    async fn grant_before(pool: &sqlx::PgPool, event: &RemovedPlace, admin: &str) {
        sqlx::query("INSERT INTO social.scene_admins (place_id, admin_address, added_by, added_at) VALUES ($1, $2, $2, $3)")
            .bind(&event.base_position)
            .bind(admin)
            .bind(event.removed_at - chrono::Duration::seconds(1))
            .execute(pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn a_removal_notice_reaches_the_owner_operators_and_admins_of_the_place() {
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            return;
        };
        let (mut state, directory, event) = setup(pool.clone()).await;
        let (owner, operator, admin) = (
            "0xcccccccccccccccccccccccccccccccccccccccc",
            "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            "0xdddddddddddddddddddddddddddddddddddddddd",
        );
        state.policy = Arc::new(StubPolicy {
            land: Ok(Some(Arc::new(Some(crate::access::LandOperators {
                owner: owner.into(),
                operator: Some(operator.into()),
                update_operator: None,
                update_managers: vec![admin.into()],
                approved_for_all: vec![],
            })))),
            ..Default::default()
        });
        grant_before(&pool, &event, admin).await;
        let state = Arc::new(state);
        cleanup(&state, &RemovedPlaces::new(pool.clone()), &event)
            .await
            .unwrap();
        let notifications = directory.stub.streaming_notifications.lock();
        let mut told: Vec<&str> = notifications.iter().map(|n| n.recipient.as_str()).collect();
        told.sort();
        assert_eq!(told, [owner, admin, operator], "once each");
        assert!(notifications.iter().all(|n| {
            n.kind == StreamingNotificationKind::PlaceUpdated
                && n.metadata["address"] == n.recipient.as_str()
        }));
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
        sqlx::query("INSERT INTO social.scene_bans (place_id, banned_address, banned_by, banned_at) VALUES ($1, '0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', '0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', $2)").bind(&event.place_id).bind(event.removed_at - chrono::Duration::seconds(1)).execute(&pool).await.unwrap();
        assert_eq!(
            cleanup(&Arc::new(state), &RemovedPlaces::new(pool.clone()), &event)
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
        assert!(directory.stub.streaming_notifications.lock().is_empty());
    }

    /// The place of a world published here has the world name as its id; the
    /// admins are granted on the places of the world's scenes.
    #[tokio::test]
    async fn a_removed_local_world_place_revokes_the_admins_of_its_scenes() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let state = Arc::new(setup(pool.clone()).await.0);
        let world = format!("removed-{}.dcl.eth", uuid::Uuid::new_v4().simple());
        let identity = uuid::Uuid::new_v4();
        sqlx::query("INSERT INTO explore.world_scene_place (world_name, identity, entity_id, parcels) VALUES ($1, $2, 'bafy', '{\"0,0\"}')")
            .bind(&world).bind(identity).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO explore.place_world_local (id, base_position, raw, world_name, world) VALUES ($1, '0,0', '{}', $1, true)")
            .bind(&world).execute(&pool).await.unwrap();
        let admin = Address::parse("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap();
        let grant = identity.to_string();
        state
            .moderation
            .add_admin(&grant, &admin, &admin)
            .await
            .unwrap();
        sqlx::query("DELETE FROM explore.place_world_local WHERE id = $1")
            .bind(&world)
            .execute(&pool)
            .await
            .unwrap();
        let event: RemovedPlace = sqlx::query_as("SELECT event_id, place_id, owner_address, world, world_name, base_position, positions, removed_at, cleaned_at, admin_keys, recipients, recipients_resolved, recipient_cursor, notify FROM explore.place_removal_pending WHERE place_id = $1").bind(&world).fetch_one(&pool).await.unwrap();
        assert!(event.world);
        // Bans of the world's scenes end; a later ban and another world's stay.
        for (key, at) in [
            (
                format!("{world}:0,0"),
                event.removed_at - chrono::Duration::seconds(1),
            ),
            (
                format!("{world}:1,1"),
                event.removed_at + chrono::Duration::seconds(1),
            ),
            (
                format!("{world}x:0,0"),
                event.removed_at - chrono::Duration::seconds(1),
            ),
        ] {
            sqlx::query("INSERT INTO social.scene_bans (place_id, banned_address, banned_by, banned_at) VALUES ($1, $2, $2, $3)")
                .bind(key).bind(admin.as_str()).bind(at).execute(&pool).await.unwrap();
        }
        // Each scene's key ends; another world's key stays.
        let parsed = WorldName::parse(&world).unwrap();
        let other = WorldName::parse(&format!("x{world}")).unwrap();
        let scene_room = |world: &WorldName, scene: &str| {
            crate::rooms::worlds_service_scene_room(
                world,
                &crate::world_identity::SceneId::parse(scene).unwrap(),
            )
        };
        let mut keys = Vec::new();
        for (place, room) in [
            (grant.clone(), scene_room(&parsed, "bafy")),
            (format!("{world}-b"), scene_room(&parsed, "bafyb")),
            (format!("{world}-c"), scene_room(&other, "bafy")),
        ] {
            let id: uuid::Uuid = sqlx::query_scalar("INSERT INTO social.scene_stream_access (place_id, streaming_url, streaming_key, ingress_id, room_id, generated_by, created_at) VALUES ($1, 'rtmp://example.invalid/live', $2, '', $3, 'test', $4) RETURNING id")
                .bind(place).bind(uuid::Uuid::new_v4().to_string()).bind(room)
                .bind(event.removed_at - chrono::Duration::seconds(1))
                .fetch_one(&pool).await.unwrap();
            keys.push(id);
        }
        // A grant made after the removal, before a late cleanup: it stays.
        let later = Address::parse("0xcccccccccccccccccccccccccccccccccccccccc").unwrap();
        state
            .moderation
            .add_admin(&grant, &later, &admin)
            .await
            .unwrap();
        // The owner is unknown to the stub: the cleanup commits, the notice waits.
        let _ = cleanup(&state, &RemovedPlaces::new(pool.clone()), &event).await;
        let active =
            "SELECT active FROM social.scene_admins WHERE place_id = $1 ORDER BY admin_address";
        let active: Vec<bool> = sqlx::query_scalar(active)
            .bind(&grant)
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(active, [false, true]);
        let bans: Vec<String> =
            sqlx::query_scalar("SELECT place_id FROM social.scene_bans WHERE place_id LIKE $1 || '%' ORDER BY place_id")
                .bind(&world)
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(bans, [format!("{world}:1,1"), format!("{world}x:0,0")]);
        let mut still = Vec::new();
        for id in &keys {
            let active: bool =
                sqlx::query_scalar("SELECT active FROM social.scene_stream_access WHERE id = $1")
                    .bind(id)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            still.push(active);
        }
        assert_eq!(still, [false, false, true], "the world's scene keys end");
    }

    /// A world event whose name no world can have is still cleaned: the key
    /// under its catalogue place ends, whatever room it streams into.
    #[tokio::test]
    async fn a_world_removal_without_a_valid_name_is_cleaned() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let (state, _, event) = setup(pool.clone()).await;
        sqlx::query("UPDATE explore.place_removal_pending SET world = true, world_name = 'not a world' WHERE event_id = $1")
            .bind(event.event_id)
            .execute(&pool)
            .await
            .unwrap();
        let room = crate::rooms::worlds_service_scene_room(
            &WorldName::parse("unnamed.dcl.eth").unwrap(),
            &crate::world_identity::SceneId::parse("bafy").unwrap(),
        );
        let key: uuid::Uuid = sqlx::query_scalar("INSERT INTO social.scene_stream_access (place_id, streaming_url, streaming_key, ingress_id, room_id, generated_by, created_at) VALUES ($1, 'rtmp://example.invalid/live', $2, '', $3, 'test', $4) RETURNING id")
            .bind(&event.place_id).bind(uuid::Uuid::new_v4().to_string()).bind(room)
            .bind(event.removed_at - chrono::Duration::seconds(1))
            .fetch_one(&pool).await.unwrap();
        // Only the notice may fail; the cleanup commits first.
        let _ = cleanup(&Arc::new(state), &RemovedPlaces::new(pool.clone()), &event).await;
        let cleaned: bool = sqlx::query_scalar(
            "SELECT cleaned_at IS NOT NULL FROM explore.place_removal_pending WHERE event_id = $1",
        )
        .bind(event.event_id)
        .fetch_optional(&pool)
        .await
        .unwrap()
        .unwrap_or(true);
        assert!(cleaned);
        let active: bool =
            sqlx::query_scalar("SELECT active FROM social.scene_stream_access WHERE id = $1")
                .bind(key)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(!active);
    }

    #[tokio::test]
    async fn a_retried_world_removal_notice_reaches_the_owner_granted_scene_admin() {
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            return;
        };
        let (mut state, directory, _) = setup(pool.clone()).await;
        let owner = Address::parse("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let parcel_grantee = Address::parse("0xcccccccccccccccccccccccccccccccccccccccc").unwrap();
        state.policy = Arc::new(
            crate::gatekeeper::streaming::notifications::test_support::WorldPolicy {
                owner: owner.clone(),
                wide: Default::default(),
                protected: [owner.clone(), parcel_grantee.clone()].into(),
            },
        );
        let world = format!("removed-{}.dcl.eth", uuid::Uuid::new_v4().simple());
        let identity = uuid::Uuid::new_v4();
        sqlx::query("INSERT INTO explore.world_scene_place (world_name, identity, entity_id, parcels) VALUES ($1, $2, 'bafy', '{\"0,0\"}')")
            .bind(&world).bind(identity).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO explore.place_world_local (id, base_position, raw, world_name, world) VALUES ($1, '0,0', '{}', $1, true)")
            .bind(&world).execute(&pool).await.unwrap();
        let (admin, skipped) = (
            Address::parse("0xdddddddddddddddddddddddddddddddddddddddd").unwrap(),
            Address::parse("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee").unwrap(),
        );
        let grant = identity.to_string();
        state
            .moderation
            .add_admin(&grant, &admin, &owner)
            .await
            .unwrap();
        state
            .moderation
            .add_admin(&grant, &skipped, &parcel_grantee)
            .await
            .unwrap();
        sqlx::query("DELETE FROM explore.place_world_local WHERE id = $1")
            .bind(&world)
            .execute(&pool)
            .await
            .unwrap();
        let event: RemovedPlace =
            sqlx::query_as("SELECT * FROM explore.place_removal_pending WHERE place_id = $1")
                .bind(&world)
                .fetch_one(&pool)
                .await
                .unwrap();
        let (feed, state) = (RemovedPlaces::new(pool.clone()), Arc::new(state));
        directory.fail.store(true, Ordering::SeqCst);
        assert!(cleanup(&state, &feed, &event).await.is_err());
        let active: bool = sqlx::query_scalar(
            "SELECT active FROM social.scene_admins WHERE place_id = $1 AND admin_address = $2",
        )
        .bind(&grant)
        .bind(admin.as_str())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!active, "the first pass revoked the grant");
        let saved: RemovedPlace =
            sqlx::query_as("SELECT * FROM explore.place_removal_pending WHERE event_id = $1")
                .bind(event.event_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        // The scene leaves before the retry.
        sqlx::query("UPDATE explore.world_scene_place SET active = false, entity_id = NULL WHERE identity = $1")
            .bind(identity).execute(&pool).await.unwrap();
        directory.fail.store(false, Ordering::SeqCst);
        cleanup(&state, &feed, &saved).await.unwrap();
        let notifications = directory.stub.streaming_notifications.lock();
        let told: std::collections::BTreeSet<&str> =
            notifications.iter().map(|n| n.recipient.as_str()).collect();
        assert_eq!(
            told,
            [owner.as_str(), parcel_grantee.as_str(), admin.as_str()].into()
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
            cleanup(&Arc::new(state), &RemovedPlaces::new(pool.clone()), &event)
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
        assert!(directory.stub.streaming_notifications.lock().is_empty());
    }
}
