//! Streaming notification recipients and messages shared by cleanup workflows.
use std::collections::BTreeSet;

use crate::gatekeeper::removed_places::PlaceContext;
use crate::gatekeeper::{CommsState, admission::scene, errors::ApiError};
use crate::interfaces::{StreamingNotification, StreamingNotificationKind};
use crate::{
    Address, PermissionDecision,
    world_identity::{Parcel, WorldName},
};
use chrono::NaiveDateTime;
use serde_json::{Value, json};

/// The catalogue place as the permission surfaces read it.
pub(in crate::gatekeeper) fn place_info(context: &PlaceContext) -> scene::PlaceInfo {
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

/// What a streaming notice is about, which decides the scene admins it
/// reaches besides the place's owner and operators.
pub(in crate::gatekeeper) enum Scope<'a> {
    /// A stream key; `room` is its room, which names the key's scene.
    Key { room: Option<&'a str> },
    /// The removal of the place at `revoked_at`: the grants that removal
    /// ended are still told, so a retry after its cleanup reaches them.
    Removal { revoked_at: NaiveDateTime },
}

/// Everyone a streaming notice for `context` goes to, `owner` included, as
/// upstream's `getAdminsAndExtraAddresses`: for a Genesis place its scene
/// admins (granted on its base parcel) and the LAND owner, operators and
/// managers; for a world its owner, the deployment/streaming grantees of its
/// parcels, and the scene admins of the key's scene (of every active scene
/// for a removal) whose granter still holds the world-wide authority
/// (`streaming::world_authority`). With per-scene world keys (#1948) the
/// admins of the key's scene are exactly those who hold it. A surface that
/// cannot be read is an error, never a smaller set.
pub(in crate::gatekeeper) async fn recipients(
    state: &CommsState,
    context: &PlaceContext,
    owner: Address,
    scope: Scope<'_>,
) -> Result<BTreeSet<Address>, ApiError> {
    let mut out = BTreeSet::from([owner]);
    let extra = scene::try_get_extra_addresses(state, &place_info(context)).await?;
    out.extend(extra.iter().filter_map(|a| Address::parse(a).ok()));
    let revoked_at = match scope {
        Scope::Removal { revoked_at } => Some(revoked_at),
        Scope::Key { .. } => None,
    };
    let world = context
        .world_name
        .as_deref()
        .filter(|_| context.world)
        .and_then(|name| WorldName::parse(name).ok());
    let Some(world) = world else {
        if !context.world
            && let Ok(base) = Parcel::parse(&context.base_position)
        {
            let admins = state
                .moderation
                .notice_admins(&base.canonical(), revoked_at)
                .await?;
            out.extend(admins.iter().filter_map(|(a, _)| Address::parse(a).ok()));
        }
        return Ok(out);
    };
    let admins: Vec<(String, String)> = match scope {
        Scope::Key { room } => match key_scene_place(state, &world, room).await? {
            Some(key) => state.moderation.notice_admins(&key, None).await?,
            None => Vec::new(),
        },
        // The grants of the active scenes, and those this removal ended on
        // any scene of the world: a scene that left since still has them.
        Scope::Removal { revoked_at } => {
            sqlx::query_as(
                "SELECT a.admin_address, a.added_by FROM social.scene_admins a \
             JOIN explore.world_scene_place p ON a.place_id = p.identity::text \
             WHERE p.world_name = $1 AND ((a.active AND p.active) OR a.removed_at = $2)",
            )
            .bind(world.key())
            .bind(revoked_at)
            .fetch_all(&state.pool)
            .await?
        }
    };
    let authority = super::world_authority(state, &context.place_id, &world);
    // Most grants share a granter (the owner): one decision for each.
    let mut decided: std::collections::HashMap<Address, bool> = Default::default();
    for (admin, granter) in admins {
        let (Ok(admin), Ok(granter)) = (Address::parse(&admin), Address::parse(&granter)) else {
            continue;
        };
        let holds = match decided.get(&granter) {
            Some(holds) => *holds,
            None => {
                let holds = match scene::scene_manager_decision(state, &authority, &granter).await {
                    PermissionDecision::Allowed => true,
                    decision if super::lacks_authority(&decision) => false,
                    PermissionDecision::Refused(why) => return Err(why.into()),
                };
                decided.insert(granter, holds);
                holds
            }
        };
        if holds {
            out.insert(admin);
        }
    }
    Ok(out)
}

/// The place (`ActiveDeployment::place_key`) of the world scene a key's room
/// names, while that scene is deployed in `world`.
async fn key_scene_place(
    state: &CommsState,
    world: &WorldName,
    room: Option<&str>,
) -> Result<Option<String>, ApiError> {
    let Some((room_world, scene)) = room.and_then(crate::rooms::decode_world_scene_room) else {
        return Ok(None);
    };
    if &room_world != world {
        return Ok(None);
    }
    let deployment = state
        .scenes
        .world_scene(world, &scene)
        .await
        .map_err(|_| ApiError::http(503, "Scene identity is unavailable"))?;
    Ok(deployment.map(|deployment| deployment.place_key()))
}

/// `event` for each of `recipients`, its metadata naming that recipient.
pub(in crate::gatekeeper) fn to_each(
    event: &StreamingNotification,
    recipients: &BTreeSet<Address>,
) -> Vec<StreamingNotification> {
    recipients
        .iter()
        .map(|recipient| {
            let mut notice = event.clone();
            notice.recipient = recipient.clone();
            if let Some(metadata) = notice.metadata.as_object_mut() {
                metadata.insert("address".into(), json!(recipient.as_str()));
            }
            notice
        })
        .collect()
}

pub(in crate::gatekeeper) async fn owner(
    state: &CommsState,
    context: &PlaceContext,
) -> Result<Address, ApiError> {
    if let Some(address) = context
        .owner_address
        .as_deref()
        .and_then(|a| Address::parse(a).ok())
        .filter(|_| context.world)
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

pub(in crate::gatekeeper) fn notification(
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

pub(in crate::gatekeeper) async fn prepare_streaming_for_context(
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

#[cfg(test)]
pub(in crate::gatekeeper) mod test_support {
    use super::*;
    use crate::access::{Access, AccessGrant, AccessType, LandOperators, SceneRef};
    use crate::shared::identity::AuthorityNotEstablished;
    use crate::world_identity::{ParcelSet, SceneId};
    use std::sync::Arc;

    /// A world whose `owner` administers it, whose `wide` grantees hold it
    /// with no parcel restriction, and whose `protected` set is the owner and
    /// every grantee of the place's parcels.
    pub(in crate::gatekeeper) struct WorldPolicy {
        pub owner: Address,
        pub wide: BTreeSet<Address>,
        pub protected: BTreeSet<Address>,
    }

    #[async_trait::async_trait]
    impl crate::interfaces::AccessPolicy for WorldPolicy {
        async fn access_type(
            &self,
            _: &WorldName,
        ) -> Result<Option<AccessType>, AuthorityNotEstablished> {
            Ok(Some(AccessType::Unrestricted))
        }
        async fn world_owner(
            &self,
            _: &WorldName,
        ) -> Result<Option<Address>, AuthorityNotEstablished> {
            Ok(Some(self.owner.clone()))
        }
        async fn land_document(
            &self,
            _: Parcel,
        ) -> Result<Option<Arc<Option<LandOperators>>>, AuthorityNotEstablished> {
            Ok(None)
        }
        async fn can_access(
            &self,
            _: &WorldName,
            _: &Address,
            _: Option<&str>,
            _: Option<&SceneId>,
            _: Option<&str>,
            _: NaiveDateTime,
        ) -> Result<Access, AuthorityNotEstablished> {
            Ok(Access::Allowed(AccessGrant::Unrestricted))
        }
        async fn is_blocked(&self, _: &Address) -> Result<bool, AuthorityNotEstablished> {
            Ok(false)
        }
        async fn is_admin(&self, _: &SceneRef<'_>, actor: &Address) -> PermissionDecision {
            if actor == &self.owner {
                PermissionDecision::Allowed
            } else {
                PermissionDecision::refused("not the owner")
            }
        }
        async fn protected_addresses(
            &self,
            _: &WorldName,
            _: &ParcelSet,
        ) -> Result<BTreeSet<Address>, AuthorityNotEstablished> {
            Ok(self.protected.clone())
        }
        async fn world_grantees(
            &self,
            _: &WorldName,
            parcels: &ParcelSet,
        ) -> Result<BTreeSet<Address>, AuthorityNotEstablished> {
            // The world-wide question carries no parcel.
            assert!(parcels.canonical().is_empty());
            Ok(self.wide.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::LandOperators;
    use crate::interfaces::test_support::{StubDirectory, StubPolicy, StubScenes};
    use crate::shared::identity::AuthorityNotEstablished;
    use std::sync::Arc;

    fn address(byte: char) -> Address {
        Address::parse(&format!("0x{}", byte.to_string().repeat(40))).unwrap()
    }

    fn state(pool: sqlx::PgPool, policy: StubPolicy) -> CommsState {
        CommsState::new(
            pool,
            crate::http::Outbound::for_tests(),
            crate::gatekeeper::Settings::for_tests(),
            None,
            None,
            Arc::new(policy),
            Arc::new(StubScenes::default()),
            Arc::new(crate::presence::Presence::new()),
            Arc::new(StubDirectory::default()),
        )
    }

    async fn grant(pool: &sqlx::PgPool, key: &str, admin: &Address, active: bool) {
        sqlx::query(
            "INSERT INTO social.scene_admins (place_id, admin_address, added_by, active, removed_at) \
             VALUES ($1, $2, $2, $3, CASE WHEN $3 THEN NULL ELSE timezone('UTC', now()) END)",
        )
        .bind(key)
        .bind(admin.as_str())
        .bind(active)
        .execute(pool)
        .await
        .unwrap();
    }

    fn context(world: bool, base: &str) -> PlaceContext {
        PlaceContext {
            place_id: format!("catalogue-{}", uuid::Uuid::new_v4()),
            owner_address: None,
            world,
            world_name: world.then(|| "scene.dcl.eth".into()),
            base_position: base.into(),
            positions: json!([base, "-120,7"]),
        }
    }

    #[tokio::test]
    async fn a_genesis_notice_reaches_the_admins_and_every_land_manager_once() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let (owner, operator, updater, manager, approved) = (
            address('a'),
            address('b'),
            address('c'),
            address('d'),
            address('e'),
        );
        let (admin, removed, catalogue_admin) = (address('f'), address('1'), address('2'));
        let policy = StubPolicy {
            land: Ok(Some(Arc::new(Some(LandOperators {
                owner: owner.as_str().to_uppercase().replace("0X", "0x"),
                operator: Some(operator.as_str().into()),
                update_operator: Some(updater.as_str().into()),
                update_managers: vec![manager.as_str().into(), owner.as_str().into()],
                approved_for_all: vec![approved.as_str().into()],
            })))),
            ..Default::default()
        };
        let state = state(pool.clone(), policy);
        let context = context(false, "-120,6");
        // Grants are on the base parcel; the catalogue id holds none.
        grant(&pool, "-120,6", &admin, true).await;
        grant(&pool, "-120,6", &owner, true).await;
        grant(&pool, "-120,6", &removed, false).await;
        grant(&pool, &context.place_id, &catalogue_admin, true).await;

        let recipients = recipients(&state, &context, owner.clone(), Scope::Key { room: None })
            .await
            .unwrap();
        assert_eq!(
            recipients,
            BTreeSet::from([owner, operator, updater, manager, approved, admin])
        );
        let at = chrono::DateTime::from_timestamp(123, 0)
            .unwrap()
            .naive_utc();
        let event = notification(
            &context,
            address('a'),
            "streaming_key_revoke-key",
            StreamingNotificationKind::KeyRevoke,
            at,
        );
        let notices = to_each(&event, &recipients);
        assert_eq!(notices.len(), recipients.len());
        for (notice, recipient) in notices.iter().zip(&recipients) {
            assert_eq!(&notice.recipient, recipient);
            assert_eq!(notice.metadata["address"], recipient.as_str());
            assert_eq!(notice.event_key, event.event_key);
            assert_eq!(notice.metadata["title"], event.metadata["title"]);
        }
    }

    /// A world with its owner `a`, a world-wide grantee `b` and a grantee of
    /// some parcels `c`; its scene `scene` is deployed on the place `place`.
    struct World {
        state: CommsState,
        pool: sqlx::PgPool,
        context: PlaceContext,
        room: String,
        place: String,
    }

    async fn world() -> Option<World> {
        let pool = crate::db::scratch::isolated_pool(2).await?;
        let name = WorldName::parse("scene.dcl.eth").unwrap();
        let scene = crate::world_identity::SceneId::parse("bafkreiworldscene").unwrap();
        let identity = uuid::Uuid::new_v4();
        let realm = crate::world_identity::RealmId::World(name.clone());
        let base = Parcel::parse("-120,6").unwrap();
        let deployment = crate::rooms::ActiveDeployment {
            title: None,
            scene_id: scene.clone(),
            base,
            parcels: [base].into_iter().collect(),
            namespace: crate::world_identity::SceneNamespace::new(&realm, base, Some(identity)),
            local: true,
        };
        let mut state = state(pool.clone(), StubPolicy::default());
        state.policy = Arc::new(test_support::WorldPolicy {
            owner: address('a'),
            wide: BTreeSet::from([address('b')]),
            protected: BTreeSet::from([address('a'), address('b'), address('c')]),
        });
        state.scenes = Arc::new(StubScenes {
            scene: Ok(Some(deployment)),
            ..Default::default()
        });
        Some(World {
            state,
            pool,
            context: context(true, "-120,6"),
            room: crate::rooms::worlds_service_scene_room(&name, &scene),
            place: identity.to_string(),
        })
    }

    async fn grant_by(pool: &sqlx::PgPool, key: &str, admin: &Address, granter: &Address) {
        sqlx::query(
            "INSERT INTO social.scene_admins (place_id, admin_address, added_by) VALUES ($1, $2, $3)",
        )
        .bind(key)
        .bind(admin.as_str())
        .bind(granter.as_str())
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn a_world_key_notice_reaches_the_scene_admins_who_hold_the_key() {
        let Some(w) = world().await else { return };
        let (by_owner, by_wide) = (address('d'), address('e'));
        let (by_parcel_grantee, by_admin, other_scene) = (address('f'), address('1'), address('2'));
        grant_by(&w.pool, &w.place, &by_owner, &address('a')).await;
        grant_by(&w.pool, &w.place, &by_wide, &address('b')).await;
        // A grantee of some parcels, or another scene admin, never widens
        // its own authority to the key.
        grant_by(&w.pool, &w.place, &by_parcel_grantee, &address('c')).await;
        grant_by(&w.pool, &w.place, &by_admin, &by_owner).await;
        // An admin of another scene of the world does not hold this key.
        let elsewhere = uuid::Uuid::new_v4().to_string();
        grant_by(&w.pool, &elsewhere, &other_scene, &address('a')).await;
        grant_by(&w.pool, &w.context.place_id, &other_scene, &address('a')).await;

        let scope = Scope::Key {
            room: Some(&w.room),
        };
        let told = recipients(&w.state, &w.context, address('a'), scope)
            .await
            .unwrap();
        assert_eq!(
            told,
            BTreeSet::from([address('a'), address('b'), address('c'), by_owner, by_wide])
        );
        // Without the key's room no scene admin is told.
        let told = recipients(
            &w.state,
            &w.context,
            address('a'),
            Scope::Key { room: None },
        )
        .await
        .unwrap();
        assert_eq!(
            told,
            BTreeSet::from([address('a'), address('b'), address('c')])
        );
    }

    #[tokio::test]
    async fn a_world_removal_notice_reaches_the_admins_its_cleanup_revoked() {
        let Some(w) = world().await else { return };
        let world_name = w.context.world_name.clone().unwrap();
        sqlx::query("INSERT INTO explore.world_scene_place (world_name, identity, entity_id, parcels) VALUES ($1, $2::uuid, 'bafkreiworldscene', '{\"-120,6\"}')")
            .bind(&world_name).bind(&w.place).execute(&w.pool).await.unwrap();
        let (revoked, earlier, by_parcel_grantee) = (address('d'), address('e'), address('f'));
        grant_by(&w.pool, &w.place, &revoked, &address('a')).await;
        grant_by(&w.pool, &w.place, &earlier, &address('a')).await;
        grant_by(&w.pool, &w.place, &by_parcel_grantee, &address('c')).await;
        let removed_at = chrono::Utc::now().naive_utc();
        sqlx::query(
            "UPDATE social.scene_admins SET active = false, removed_at = $2 WHERE place_id = $1",
        )
        .bind(&w.place)
        .bind(removed_at)
        .execute(&w.pool)
        .await
        .unwrap();
        // Removed before, by its granter: not this removal's grant.
        sqlx::query("UPDATE social.scene_admins SET removed_at = $3 WHERE place_id = $1 AND admin_address = $2")
            .bind(&w.place).bind(earlier.as_str()).bind(removed_at - chrono::Duration::seconds(5))
            .execute(&w.pool).await.unwrap();
        // The scene left before the retry: its revoked grants are still told,
        // and a grant still active on a scene that left is not.
        let (left, still_active) = (uuid::Uuid::new_v4(), address('2'));
        sqlx::query("INSERT INTO explore.world_scene_place (world_name, identity, parcels, active) VALUES ($1, $2, '{\"-121,6\"}', false)")
            .bind(&world_name).bind(left).execute(&w.pool).await.unwrap();
        grant_by(&w.pool, &left.to_string(), &still_active, &address('a')).await;
        sqlx::query("UPDATE explore.world_scene_place SET active = false, entity_id = NULL WHERE identity = $1::uuid")
            .bind(&w.place).execute(&w.pool).await.unwrap();

        let told = recipients(
            &w.state,
            &w.context,
            address('a'),
            Scope::Removal {
                revoked_at: removed_at,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            told,
            BTreeSet::from([address('a'), address('b'), address('c'), revoked])
        );
    }

    #[tokio::test]
    async fn an_unreadable_permission_surface_is_an_error_not_the_owner_alone() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let unavailable = || AuthorityNotEstablished::UndeterminedStoreUnavailable {
            store: "stub",
            reason_for_operators: "unavailable".into(),
        };
        for (world, policy) in [
            (
                false,
                StubPolicy {
                    land: Err(unavailable()),
                    ..Default::default()
                },
            ),
            (
                true,
                StubPolicy {
                    protected: Err(unavailable()),
                    ..Default::default()
                },
            ),
        ] {
            let state = state(pool.clone(), policy);
            let error = recipients(
                &state,
                &context(world, "-120,6"),
                address('a'),
                Scope::Key { room: None },
            )
            .await
            .unwrap_err();
            assert_eq!(error.code, 503);
        }
    }

    #[test]
    fn messages_preserve_location_recipient_and_event_identity() {
        let recipient = Address::parse("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let at = chrono::DateTime::from_timestamp(123, 0)
            .unwrap()
            .naive_utc();
        for world in [false, true] {
            let context = PlaceContext {
                place_id: "place".into(),
                owner_address: None,
                world,
                world_name: Some("scene.dcl.eth".into()),
                base_position: "5,6".into(),
                positions: json!(["5,6"]),
            };
            for (kind, title) in [
                (StreamingNotificationKind::KeyReset, "Stream Key Reset"),
                (
                    StreamingNotificationKind::KeyRevoke,
                    "Stream Key Deactivated",
                ),
                (StreamingNotificationKind::KeyExpired, "Stream Key Expired"),
                (StreamingNotificationKind::TimeExceeded, "Stream Timed Out"),
                (
                    StreamingNotificationKind::PlaceUpdated,
                    "Stream Location Altered",
                ),
            ] {
                let event = notification(&context, recipient.clone(), "event-key", kind, at);
                assert_eq!(event.event_key, "event-key");
                assert_eq!(event.recipient, recipient);
                assert_eq!(event.occurred_at, at);
                assert_eq!(event.metadata["title"], title);
                assert_eq!(event.metadata["address"], recipient.as_str());
                assert_eq!(event.metadata["position"], "5,6");
                assert_eq!(event.metadata["worldName"], "scene.dcl.eth");
                assert_eq!(event.metadata["isWorld"], world);
                assert_eq!(
                    event.metadata["url"],
                    if world {
                        "https://decentraland.org/jump/?realm=scene.dcl.eth"
                    } else {
                        "https://decentraland.org/jump/?position=5,6"
                    }
                );
                assert!(
                    event.metadata["description"]
                        .as_str()
                        .unwrap()
                        .contains(if world { "scene.dcl.eth" } else { "5,6" })
                );
            }
        }
    }
}
