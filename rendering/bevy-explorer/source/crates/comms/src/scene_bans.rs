use alloy_core::primitives::Address;
use bevy::prelude::*;
use common::structs::{server_mode, CurrentRealm, PreviewMode};
use wallet::Wallet;

use crate::{
    room_connection::{
        AdmissionError, BanRecheck, CredentialRequest, CredentialTask, FailureKind, NeedsAdapter,
        RoomRecovery, RoomSuspended, BAN_RECHECK_SECS,
    },
    SceneAdmission, SceneRoomModeration, SceneRoomTarget,
};

/// A retry may predate the latest metadata ban for this wallet.
#[derive(Component)]
pub(crate) struct MetadataSceneBan(pub(crate) Address);

pub(crate) fn apply_scene_bans(
    mut commands: Commands,
    rooms: Query<
        (
            Entity,
            &SceneRoomTarget,
            &SceneRoomModeration,
            Option<&MetadataSceneBan>,
        ),
        (Changed<SceneRoomModeration>, Without<RoomSuspended>),
    >,
    mut recovery: Query<&mut RoomRecovery, With<CredentialRequest>>,
    mut admission: ResMut<SceneAdmission>,
    (realm, wallet, preview, time): (Res<CurrentRealm>, Res<Wallet>, Res<PreviewMode>, Res<Time>),
) {
    if server_mode() || preview.is_preview || realm.is_local() {
        return;
    }
    let Some(address) = wallet.address() else {
        return;
    };
    for (entity, target, metadata, observed_ban) in &rooms {
        if target.scene.realm_about_url != realm.about_url {
            continue;
        }
        if metadata.banned_addresses.contains(&address) {
            admission.set_if_neq(
                SceneAdmission::from_room_ban(&target.scene.scene_id)
                    .for_session(&realm.about_url, address),
            );
            if observed_ban.map(|ban| ban.0) != Some(address) {
                commands
                    .entity(entity)
                    .try_insert(MetadataSceneBan(address));
                if let Ok(mut recovery) = recovery.get_mut(entity) {
                    let mut error = AdmissionError::new(FailureKind::Denied);
                    error.retry_after = Some(BAN_RECHECK_SECS);
                    let due = recovery.retry.due;
                    recovery.fail(time.elapsed_secs_f64(), error);
                    recovery.retry.due = recovery.retry.due.max(due);
                    commands
                        .entity(entity)
                        .try_remove::<CredentialTask>()
                        .try_insert((NeedsAdapter, BanRecheck));
                }
            }
        } else if observed_ban.is_some() {
            commands.entity(entity).try_remove::<MetadataSceneBan>();
        }
        // Only a fresh successful admission clears a ban. Missing metadata is not an unban.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_metadata_ban_is_session_bound_and_needs_fresh_admission_to_clear() {
        for mode in ["current", "stale_realm", "preview"] {
            let mut app = App::new();
            let mut wallet = Wallet::default();
            wallet.finalize_as_guest_with_seed([7; 32]);
            let address = wallet.address().unwrap();
            app.insert_resource(wallet)
                .init_resource::<SceneAdmission>()
                .init_resource::<Time>()
                .init_resource::<CurrentRealm>()
                .init_resource::<PreviewMode>()
                .add_systems(Update, apply_scene_bans);
            app.world_mut().resource_mut::<PreviewMode>().is_preview = mode == "preview";
            let mut metadata = SceneRoomModeration::default();
            metadata.banned_addresses.insert(address);
            let room = app
                .world_mut()
                .spawn((
                    SceneRoomTarget {
                        scene: crate::SetCurrentScene {
                            realm_name: "realm".into(),
                            realm_about_url: if mode == "stale_realm" { "old" } else { "" }.into(),
                            scene_id: "scene".into(),
                        },
                        is_world: false,
                    },
                    metadata,
                    CredentialRequest {
                        method: "POST",
                        url: "https://rooms.example/adapter".into(),
                        metadata: "{}".into(),
                    },
                ))
                .id();
            app.update();
            let expected = (mode == "current").then_some("scene");
            assert_eq!(
                app.world()
                    .resource::<SceneAdmission>()
                    .banned_scene("", address),
                expected
            );
            assert_eq!(
                app.world().get::<BanRecheck>(room).is_some(),
                mode == "current"
            );
            app.world_mut()
                .get_mut::<SceneRoomModeration>(room)
                .unwrap()
                .banned_addresses
                .clear();
            app.update();
            assert_eq!(
                app.world()
                    .resource::<SceneAdmission>()
                    .banned_scene("", address),
                expected
            );
        }
    }
}
