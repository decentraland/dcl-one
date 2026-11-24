//! What the scene the player stands in restricts, gathered from the systems that own each
//! restriction: the HUD's minimap lists it (`SystemApi::GetSceneRestrictionsStream`) and
//! `/scene_restrictions_state` reports it.

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::{
    rpc::RpcStreamSender,
    structs::{
        MicState, PlayerModifiers, PortableExperiencesToggle, PrimaryCamera, PrimaryUser,
        SceneTimeLock,
    },
};
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use scene_runner::{
    portables::current_toggle, renderer_context::RendererSceneContext, ContainingScene,
};
use system_bridge::{SceneRestrictions, SystemApi};

pub struct SceneRestrictionsPlugin;

impl Plugin for SceneRestrictionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CurrentSceneRestrictions>()
            .add_systems(
                Update,
                (update_scene_restrictions, pipe_scene_restrictions).chain(),
            )
            .add_console_command::<SceneRestrictionsStateCommand, _>(scene_restrictions_state)
            .add_debug_section(DebugSection {
                title: "Scene restrictions",
                query: "/scene_restrictions_state",
                fields: &[
                    ("Camera locked", "cameraLocked"),
                    ("Avatar hidden", "avatarHidden"),
                    ("Avatar movement blocked", "avatarMovementsBlocked"),
                    ("Passport disabled", "passportCannotBeOpened"),
                    ("Experiences blocked", "experiencesBlocked"),
                    ("Scene sets the time of day", "skyboxTimeUiBlocked"),
                    ("Nearby voice blocked", "nearbyVoiceChatBlocked"),
                ],
                inputs: &[],
                actions: &[],
            });
    }
}

/// The restrictions in force this frame.
#[derive(Resource, Default)]
pub struct CurrentSceneRestrictions(pub SceneRestrictions);

/// Any one blocked input counts: a scene that only stops jumping still restricts movement.
fn movements_blocked(modifiers: &PlayerModifiers) -> bool {
    modifiers.block_all
        || modifiers.block_walk
        || modifiers.block_run
        || modifiers.block_jump
        || modifiers.block_emote
}

fn update_scene_restrictions(
    mut current: ResMut<CurrentSceneRestrictions>,
    camera: Query<&PrimaryCamera>,
    player: Query<(Entity, Option<&PlayerModifiers>), With<PrimaryUser>>,
    containing_scene: ContainingScene,
    contexts: Query<&RendererSceneContext>,
    time_lock: Option<Res<SceneTimeLock>>,
    mic_state: Option<Res<MicState>>,
) {
    let player = player.single().ok();
    let modifiers = player.and_then(|(_, modifiers)| modifiers);
    let toggle = current_toggle(
        player.map(|(entity, _)| entity),
        &containing_scene,
        |scene| contexts.get(scene).ok(),
    );
    let restrictions = SceneRestrictions {
        camera_locked: camera
            .single()
            .is_ok_and(|camera| camera.scene_override.is_some()),
        avatar_hidden: modifiers.is_some_and(|modifiers| modifiers.hide),
        avatar_movements_blocked: modifiers.is_some_and(movements_blocked),
        passport_cannot_be_opened: modifiers.is_some_and(|modifiers| modifiers.hide_profile),
        experiences_blocked: toggle != PortableExperiencesToggle::Enabled,
        skybox_time_ui_blocked: time_lock.is_some_and(|lock| lock.0),
        nearby_voice_chat_blocked: mic_state.is_some_and(|mic| mic.scene_blocked()),
    };
    // compare first so the resource is marked changed only on a real change
    if current.0 != restrictions {
        current.0 = restrictions;
    }
}

fn pipe_scene_restrictions(
    mut requests: EventReader<SystemApi>,
    mut senders: Local<Vec<RpcStreamSender<SceneRestrictions>>>,
    current: Res<CurrentSceneRestrictions>,
) {
    let new_senders = requests
        .read()
        .filter_map(|ev| {
            if let SystemApi::GetSceneRestrictionsStream(sender) = ev {
                Some(sender.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    // a new subscriber gets the state as it is
    for sender in &new_senders {
        let _ = sender.send(current.0);
    }
    senders.retain(|sender| !sender.is_closed());
    if current.is_changed() {
        for sender in senders.iter() {
            let _ = sender.send(current.0);
        }
    }
    senders.extend(new_senders);
}

/// Report, as JSON, what the scene the player stands in restricts: the fields of
/// [`SceneRestrictions`], as the minimap lists them.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/scene_restrictions_state")]
struct SceneRestrictionsStateCommand;

fn scene_restrictions_state(
    mut input: ConsoleCommand<SceneRestrictionsStateCommand>,
    current: Res<CurrentSceneRestrictions>,
) {
    if let Some(Ok(_)) = input.take() {
        input.reply_ok(serde_json::to_string(&current.0).unwrap_or_default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_blocked_input_counts_as_blocked_movement() {
        assert!(!movements_blocked(&PlayerModifiers::default()));
        for modifiers in [
            PlayerModifiers {
                block_all: true,
                ..Default::default()
            },
            PlayerModifiers {
                block_walk: true,
                ..Default::default()
            },
            PlayerModifiers {
                block_run: true,
                ..Default::default()
            },
            PlayerModifiers {
                block_jump: true,
                ..Default::default()
            },
            PlayerModifiers {
                block_emote: true,
                ..Default::default()
            },
        ] {
            assert!(movements_blocked(&modifiers));
        }
    }

    #[test]
    fn the_reply_names_the_fields_the_debug_panel_reads() {
        let reply = serde_json::to_value(SceneRestrictions {
            camera_locked: true,
            ..Default::default()
        })
        .unwrap();
        for key in [
            "cameraLocked",
            "avatarHidden",
            "avatarMovementsBlocked",
            "passportCannotBeOpened",
            "experiencesBlocked",
            "skyboxTimeUiBlocked",
            "nearbyVoiceChatBlocked",
        ] {
            assert!(reply.get(key).is_some_and(|v| v.is_boolean()), "{key}");
        }
        assert_eq!(reply["cameraLocked"], true);
    }
}
