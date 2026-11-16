//! Hidden avatars stop animating.
//!
//! An avatar hidden by the Max Avatars cap, an avatar modifier area or its scene keeps its
//! animation player running, but its bones leave the animation evaluation: each bone's
//! `AnimationTarget` is taken off and kept on the built avatar until it shows again. Sampling
//! every clip for every bone is the per-avatar animation cost, so a crowd beyond the cap no
//! longer pays it, and unchanged bones skip transform propagation too.
//!
//! The player, its transitions and the emote logic keep running, so the clock stays in step
//! with other clients: when the avatar shows again its bones take the current pose.
//!
//! The local player's avatar never pauses: first person hides it by render layer, not by
//! visibility, and its bones carry attach points and the camera's head tracking. Avatars that
//! are only off camera keep animating: their bones still place scene-attached entities and
//! cast shadows into view.

use bevy::{animation::AnimationTarget, prelude::*};
use common::structs::PrimaryUser;

use crate::AvatarProcessed;

pub struct PauseHiddenAvatarsPlugin;

impl Plugin for PauseHiddenAvatarsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, pause_hidden_avatar_animation);
    }
}

/// The bones of a built avatar taken out of the animation evaluation while it is hidden, with
/// the target each one had.
#[derive(Component)]
pub struct PausedAnimationTargets(Vec<(Entity, AnimationTarget)>);

#[allow(clippy::type_complexity)]
fn pause_hidden_avatar_animation(
    mut commands: Commands,
    builds: Query<
        (
            Entity,
            &InheritedVisibility,
            &ChildOf,
            Option<&PausedAnimationTargets>,
        ),
        (
            With<AvatarProcessed>,
            Or<(Changed<InheritedVisibility>, Added<AvatarProcessed>)>,
        ),
    >,
    primary: Query<(), With<PrimaryUser>>,
    children: Query<&Children>,
    targets: Query<&AnimationTarget>,
) {
    for (build, visibility, parent, paused) in &builds {
        let avatar = parent.parent();
        match (visibility.get(), paused) {
            // shown again: the bones rejoin the evaluation and take the current pose
            (true, Some(PausedAnimationTargets(bones))) => {
                for (bone, target) in bones {
                    commands.entity(*bone).try_insert(*target);
                }
                commands.entity(build).remove::<PausedAnimationTargets>();
            }
            (false, None) if !primary.contains(avatar) => {
                let bones: Vec<_> = children
                    .iter_descendants(build)
                    .filter_map(|bone| {
                        targets
                            .get(bone)
                            .ok()
                            .filter(|target| target.player == avatar)
                            .map(|target| (bone, *target))
                    })
                    .collect();
                if bones.is_empty() {
                    continue;
                }
                for (bone, _) in &bones {
                    commands.entity(*bone).remove::<AnimationTarget>();
                }
                commands
                    .entity(build)
                    .try_insert(PausedAnimationTargets(bones));
            }
            _ => (),
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::animation::AnimationTargetId;

    use super::*;

    struct Avatar {
        root: Entity,
        build: Entity,
        bone: Entity,
    }

    fn app_with_avatar(primary: bool) -> (App, Avatar) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(PauseHiddenAvatarsPlugin);
        let world = app.world_mut();
        let root = world.spawn_empty().id();
        if primary {
            world.entity_mut(root).insert(PrimaryUser::default());
        }
        let build = world
            .spawn((AvatarProcessed, InheritedVisibility::VISIBLE, ChildOf(root)))
            .id();
        let armature = world.spawn(ChildOf(build)).id();
        let bone = world
            .spawn((
                ChildOf(armature),
                AnimationTarget {
                    id: AnimationTargetId::from_name(&Name::new("Avatar_Hips")),
                    player: root,
                },
            ))
            .id();
        app.update();
        (app, Avatar { root, build, bone })
    }

    fn set_visible(app: &mut App, build: Entity, visible: bool) {
        app.world_mut().entity_mut(build).insert(if visible {
            InheritedVisibility::VISIBLE
        } else {
            InheritedVisibility::HIDDEN
        });
        app.update();
    }

    #[test]
    fn a_hidden_avatar_stops_animating_and_resumes_when_shown() {
        let (mut app, avatar) = app_with_avatar(false);
        assert!(app.world().get::<AnimationTarget>(avatar.bone).is_some());

        set_visible(&mut app, avatar.build, false);
        assert!(app.world().get::<AnimationTarget>(avatar.bone).is_none());
        assert!(app
            .world()
            .get::<PausedAnimationTargets>(avatar.build)
            .is_some());

        set_visible(&mut app, avatar.build, true);
        let target = app
            .world()
            .get::<AnimationTarget>(avatar.bone)
            .expect("the bone rejoins the evaluation");
        assert_eq!(target.player, avatar.root);
        assert!(app
            .world()
            .get::<PausedAnimationTargets>(avatar.build)
            .is_none());
    }

    #[test]
    fn the_local_players_avatar_keeps_animating_when_hidden() {
        let (mut app, avatar) = app_with_avatar(true);
        set_visible(&mut app, avatar.build, false);
        assert!(app.world().get::<AnimationTarget>(avatar.bone).is_some());
    }

    #[test]
    fn bones_another_player_animates_are_left_alone() {
        let (mut app, avatar) = app_with_avatar(false);
        let other = app.world_mut().spawn_empty().id();
        app.world_mut()
            .entity_mut(avatar.bone)
            .get_mut::<AnimationTarget>()
            .unwrap()
            .player = other;
        set_visible(&mut app, avatar.build, false);
        assert!(app.world().get::<AnimationTarget>(avatar.bone).is_some());
    }
}
