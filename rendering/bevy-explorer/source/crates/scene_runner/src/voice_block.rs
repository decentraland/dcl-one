//! Nearby voice turned off by the scene the player stands in.
//!
//! A scene whose `scene.json` turns nearby voice off asks for it: the block applies only while
//! the player's `DisableVoice` permission for that scene allows it, and the permission system
//! records the use like any other. A ban from the scene's voice room is not a request and is
//! applied elsewhere (`MicState::set_scene_banned`).

use bevy::prelude::*;
use common::structs::{MicState, NearbyVoiceDisabled, PermissionType, PrimaryUser};

use crate::{permissions::Permission, ContainingScene};

/// Only the parcel scene under the player counts: a portable experience never blocks. The
/// permission is asked once per visit, so leaving and returning asks again. `request` is the scene
/// asked for and the answer, once there is one.
pub(crate) fn update_scene_voice_block(
    mic_state: Option<ResMut<MicState>>,
    containing_scene: ContainingScene,
    player: Query<Entity, With<PrimaryUser>>,
    disabling_scenes: Query<(), With<NearbyVoiceDisabled>>,
    mut perms: Permission<Entity>,
    mut request: Local<Option<(Entity, Option<bool>)>>,
) {
    let Some(mut mic_state) = mic_state else {
        return;
    };
    let asking = player
        .single()
        .ok()
        .and_then(|player| containing_scene.get_parcel(player))
        .filter(|scene| disabling_scenes.contains(*scene));

    if asking != request.map(|(scene, _)| scene) {
        // a question about a scene the player has left is withdrawn: dropping it closes the
        // prompt, and no use is recorded for a visit where nothing was turned off
        perms
            .pending
            .retain(|(_, ty, ..)| *ty != PermissionType::DisableVoice);
        *request = asking.map(|scene| (scene, None));
        if let Some(scene) = asking {
            perms.check_unique(PermissionType::DisableVoice, scene, scene, None, false);
        }
    }
    // asked before draining, so that an answer the stored rules give at once applies this frame:
    // nearby voice stays off across two adjacent scenes that both turn it off
    let allowed: Vec<_> = perms.drain_success(PermissionType::DisableVoice).collect();
    let refused: Vec<_> = perms.drain_fail(PermissionType::DisableVoice).collect();
    if let Some((scene, answer)) = request.as_mut() {
        if allowed.contains(scene) {
            *answer = Some(true);
        } else if refused.contains(scene) {
            *answer = Some(false);
        }
    }

    let answer = request.and_then(|(_, answer)| answer);
    let disabled = answer == Some(true);
    if mic_state.scene_disabled() != disabled {
        mic_state.set_scene_disabled(disabled);
    }
    let refused = answer == Some(false);
    if mic_state.scene_disable_refused() != refused {
        mic_state.set_scene_disable_refused(refused);
    }
}

#[cfg(test)]
mod tests {
    use bevy::platform::collections::{HashMap, HashSet};
    use common::structs::{
        AppConfig, CurrentRealm, PermissionUsed, PermissionValue, PrimaryPlayerRes,
    };
    use dcl_types::SceneId;

    use super::*;
    use crate::{
        initialize_scene::{LiveScenes, PointerResult, PortableScenes, ScenePointers},
        permissions::PermissionManager,
        renderer_context::RendererSceneContext,
    };

    const HASH: &str = "voice-off-scene";
    const NEXT_HASH: &str = "next-voice-off-scene";

    fn open_mic() -> MicState {
        let mut mic = MicState::default();
        mic.available = true;
        mic.enabled = true;
        mic
    }

    /// The player on parcel 0,0 with the mic open, in a scene that turns nearby voice off.
    fn app() -> (App, Entity) {
        let mut app = App::new();
        app.init_resource::<ScenePointers>()
            .init_resource::<LiveScenes>()
            .init_resource::<PortableScenes>()
            .init_resource::<AppConfig>()
            .init_resource::<CurrentRealm>()
            .init_resource::<PermissionManager>()
            .add_event::<PermissionUsed>()
            .insert_resource(open_mic())
            .add_systems(Update, update_scene_voice_block);
        let player = app
            .world_mut()
            .spawn((
                PrimaryUser::default(),
                GlobalTransform::from_xyz(8.0, 0.0, -8.0),
            ))
            .id();
        app.insert_resource(PrimaryPlayerRes(player));

        voice_off_scene(&mut app, HASH, IVec2::ZERO);
        (app, player)
    }

    /// A scene on `parcel` that turns nearby voice off.
    fn voice_off_scene(app: &mut App, hash: &str, parcel: IVec2) -> Entity {
        let scene = app.world_mut().spawn(NearbyVoiceDisabled).id();
        let context = RendererSceneContext::new(
            SceneId::DUMMY,
            hash.to_owned(),
            "storage_root".to_owned(),
            false,
            0,
            "title".to_owned(),
            parcel,
            HashSet::from_iter([parcel]),
            vec![],
            Default::default(),
            vec![],
            scene,
            0.0,
            false,
            "sdk_version",
            false,
            false,
        );
        app.world_mut().entity_mut(scene).insert(context);
        app.world_mut()
            .resource_mut::<LiveScenes>()
            .scenes
            .insert(hash.to_owned(), scene);
        app.world_mut().resource_mut::<ScenePointers>().insert(
            parcel,
            PointerResult::Exists {
                hash: hash.to_owned(),
                urn: None,
            },
        );
        scene
    }

    /// The questions the player is being asked.
    fn prompts(app: &mut App) -> usize {
        let mut manager = app.world_mut().resource_mut::<PermissionManager>();
        manager
            .pending
            .retain(|request| !request.sender.is_cancelled());
        manager.pending.len()
    }

    fn rule(app: &mut App, value: PermissionValue) {
        app.world_mut()
            .resource_mut::<AppConfig>()
            .scene_permissions
            .insert(
                HASH.to_owned(),
                HashMap::from_iter([(PermissionType::DisableVoice, value)]),
            );
    }

    fn stand(app: &mut App, player: Entity, in_scene: bool) {
        let x = if in_scene { 8.0 } else { 8.0 + 16.0 * 5.0 };
        app.world_mut()
            .entity_mut(player)
            .insert(GlobalTransform::from_xyz(x, 0.0, -8.0));
    }

    fn mic(app: &App) -> &MicState {
        app.world().resource::<MicState>()
    }

    fn uses(app: &mut App) -> Vec<bool> {
        app.world_mut()
            .resource_mut::<Events<PermissionUsed>>()
            .drain()
            .map(|used| {
                assert_eq!(used.ty, PermissionType::DisableVoice);
                used.was_allowed
            })
            .collect()
    }

    #[test]
    fn by_default_the_scene_turns_nearby_voice_off_and_the_use_is_recorded() {
        let (mut app, player) = app();
        app.update();
        assert!(mic(&app).scene_blocked(), "on the frame the player enters");
        assert!(!mic(&app).enabled);
        assert!(!mic(&app).scene_disable_refused());
        assert_eq!(uses(&mut app), [true]);

        stand(&mut app, player, false);
        app.update();
        assert!(!mic(&app).scene_blocked());

        // asked again on return
        stand(&mut app, player, true);
        app.update();
        app.update();
        assert!(mic(&app).scene_blocked());
        assert_eq!(uses(&mut app), [true]);
    }

    #[test]
    fn a_refused_permission_leaves_nearby_voice_on() {
        let (mut app, player) = app();
        rule(&mut app, PermissionValue::Deny);
        app.update();
        app.update();
        assert!(!mic(&app).scene_blocked());
        assert!(mic(&app).enabled);
        assert!(mic(&app).scene_disable_refused());
        assert_eq!(uses(&mut app), [false]);

        stand(&mut app, player, false);
        app.update();
        assert!(!mic(&app).scene_disable_refused());
    }

    #[test]
    fn nothing_is_blocked_until_the_player_answers() {
        let (mut app, _) = app();
        rule(&mut app, PermissionValue::Ask);
        app.update();
        app.update();
        assert!(!mic(&app).scene_blocked());
        assert!(!mic(&app).scene_disable_refused());

        let request = app
            .world_mut()
            .resource_mut::<PermissionManager>()
            .pending
            .pop_front()
            .expect("the player is asked");
        assert_eq!(request.ty, PermissionType::DisableVoice);
        request.sender.send(true);
        app.update();
        assert!(mic(&app).scene_blocked());
        assert_eq!(uses(&mut app), [true]);
    }

    #[test]
    fn a_ban_from_the_voice_room_blocks_whatever_the_permission_says() {
        let (mut app, _) = app();
        rule(&mut app, PermissionValue::Deny);
        app.world_mut()
            .resource_mut::<MicState>()
            .set_scene_banned(true);
        app.update();
        app.update();
        assert!(mic(&app).scene_blocked());
    }

    #[test]
    fn leaving_before_answering_withdraws_the_question_and_blocks_nothing() {
        let (mut app, player) = app();
        rule(&mut app, PermissionValue::Ask);
        app.update();
        assert_eq!(prompts(&mut app), 1);

        stand(&mut app, player, false);
        app.update();
        assert_eq!(prompts(&mut app), 0, "the prompt is closed");
        app.update();
        assert!(!mic(&app).scene_blocked());
        assert!(!mic(&app).scene_disable_refused());
        assert!(uses(&mut app).is_empty(), "nothing was used on that visit");
    }

    #[test]
    fn returning_before_answering_asks_once_and_the_answer_applies() {
        let (mut app, player) = app();
        rule(&mut app, PermissionValue::Ask);
        app.update();
        stand(&mut app, player, false);
        app.update();
        stand(&mut app, player, true);
        app.update();
        app.update();
        assert_eq!(prompts(&mut app), 1);
        assert!(!mic(&app).scene_blocked());

        let request = app
            .world_mut()
            .resource_mut::<PermissionManager>()
            .pending
            .pop_front()
            .unwrap();
        request.sender.send(true);
        app.update();
        assert!(mic(&app).scene_blocked());
        assert_eq!(uses(&mut app), [true]);
    }

    #[test]
    fn nearby_voice_stays_off_across_two_adjacent_scenes_that_turn_it_off() {
        let (mut app, player) = app();
        voice_off_scene(&mut app, NEXT_HASH, IVec2::new(1, 0));
        app.update();
        assert!(mic(&app).scene_blocked());
        assert_eq!(uses(&mut app), [true]);

        app.world_mut()
            .entity_mut(player)
            .insert(GlobalTransform::from_xyz(8.0 + 16.0, 0.0, -8.0));
        for frame in 0..3 {
            app.update();
            assert!(mic(&app).scene_blocked(), "frame {frame} in the next scene");
            // the next scene's use is recorded once, as the player enters
            assert_eq!(uses(&mut app), if frame == 0 { vec![true] } else { vec![] });
        }
    }
}
