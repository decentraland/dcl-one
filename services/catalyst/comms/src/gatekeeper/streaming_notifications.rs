//! Streaming notification recipients and messages shared by cleanup workflows.
use std::collections::BTreeSet;

use crate::gatekeeper::removed_places::PlaceContext;
use crate::gatekeeper::{CommsState, admission::scene, errors::ApiError};
use crate::interfaces::{StreamingNotification, StreamingNotificationKind};
use crate::{
    Address,
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

/// Everyone a streaming notice for `context` goes to, `owner` included, as
/// upstream's `getAdminsAndExtraAddresses`: for a Genesis place its active
/// scene admins (granted on its base parcel) and the LAND owner, operators
/// and managers; for a world its owner and the deployment/streaming grantees
/// of its parcels. The admins of a world's scenes are not told. A place
/// removal passes its `removed_at`, so a retry after its cleanup still tells
/// the admins that cleanup revoked. A surface that cannot be read is an
/// error, never a smaller set.
pub(in crate::gatekeeper) async fn recipients(
    state: &CommsState,
    context: &PlaceContext,
    owner: Address,
    revoked_at: Option<NaiveDateTime>,
) -> Result<BTreeSet<Address>, ApiError> {
    let mut out = BTreeSet::from([owner]);
    let extra = scene::try_get_extra_addresses(state, &place_info(context)).await?;
    out.extend(extra.iter().filter_map(|a| Address::parse(a).ok()));
    if !context.world
        && let Ok(base) = Parcel::parse(&context.base_position)
    {
        let admins = state
            .moderation
            .notice_admins(&base.canonical(), revoked_at)
            .await?;
        out.extend(admins.iter().filter_map(|a| Address::parse(a).ok()));
    }
    Ok(out)
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

        let recipients = recipients(&state, &context, owner.clone(), None)
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

    #[tokio::test]
    async fn a_world_notice_reaches_its_owner_and_grantees_and_no_scene_admin() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let (owner, deployer, streamer, scene_admin) =
            (address('a'), address('b'), address('c'), address('d'));
        let policy = StubPolicy {
            protected: Ok(BTreeSet::from([
                owner.clone(),
                deployer.clone(),
                streamer.clone(),
            ])),
            ..Default::default()
        };
        let state = state(pool.clone(), policy);
        let context = context(true, "-120,6");
        // A grant on a world scene's place or on the catalogue id names no
        // holder of the world's one credential.
        grant(&pool, "-120,6", &scene_admin, true).await;
        grant(&pool, &context.place_id, &scene_admin, true).await;

        let recipients = recipients(&state, &context, owner.clone(), None)
            .await
            .unwrap();
        assert_eq!(recipients, BTreeSet::from([owner, deployer, streamer]));
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
            let error = recipients(&state, &context(world, "-120,6"), address('a'), None)
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
