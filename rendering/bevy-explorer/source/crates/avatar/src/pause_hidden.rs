//! Hidden avatars stop animating.
//!
//! An avatar hidden by the Max Avatars cap, an avatar modifier area, a block or ban, or its
//! scene keeps its animation player running, but its bones leave the animation evaluation: each
//! bone's `AnimationTarget` is taken off and kept on the avatar in [`PausedAnimation`] until it
//! shows again. Sampling every clip for every bone is the per-avatar animation cost, so a crowd
//! beyond the cap no longer pays it.
//!
//! The foot and point-at IK passes skip a paused avatar: they adjust the pose the animation
//! wrote that frame, so without it they would compound on their own output (feet turning on a
//! slope, fingers curling further) and the bones, and anything attached to them, would drift.
//! Skipping them also saves a hidden avatar's foot raycasts, and the foot IK resets its state so
//! it ramps back in from the live pose on reveal. Head IK rests its chain every frame and keeps
//! running.
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

/// On an avatar whose build is hidden: the bones of that build taken out of the animation
/// evaluation, with the target each one had. Systems that adjust the animated pose skip an avatar
/// that has it.
#[derive(Component)]
pub struct PausedAnimation {
    /// the build the bones belong to; a rebuild replaces it, leaving this record stale
    build: Entity,
    bones: Vec<(Entity, AnimationTarget)>,
}

#[allow(clippy::type_complexity)]
fn pause_hidden_avatar_animation(
    mut commands: Commands,
    builds: Query<
        (
            Entity,
            &InheritedVisibility,
            Option<&Visibility>,
            Ref<AvatarProcessed>,
            &ChildOf,
        ),
        (
            With<AvatarProcessed>,
            Or<(Changed<InheritedVisibility>, Added<AvatarProcessed>)>,
        ),
    >,
    roots: Query<&InheritedVisibility>,
    primary: Query<(), With<PrimaryUser>>,
    paused: Query<&PausedAnimation>,
    children: Query<&Children>,
    targets: Query<&AnimationTarget>,
) {
    for (build, inherited, visibility, processed, parent) in &builds {
        let avatar = parent.parent();
        // A build loads hidden and is shown by the same commands that mark it processed, so in
        // the frame it is processed its `InheritedVisibility` may not have been propagated yet.
        // Work it out from its own `Visibility` (Max Avatars hides the build) and the root's,
        // which are both current, so a visible avatar's new build is never paused.
        let visible = match (processed.is_added(), visibility) {
            (true, Some(Visibility::Hidden)) => false,
            (true, Some(Visibility::Visible)) => true,
            (true, Some(Visibility::Inherited)) => roots
                .get(avatar)
                .map_or(inherited.get(), |root| root.get()),
            _ => inherited.get(),
        };
        let paused = paused.get(avatar).ok();
        match (visible, paused) {
            // shown again, or rebuilt shown: the bones rejoin the evaluation and take the current
            // pose (those of a replaced build are gone, and skipped)
            (true, Some(PausedAnimation { bones, .. })) => {
                for (bone, target) in bones {
                    commands.entity(*bone).try_insert(*target);
                }
                commands.entity(avatar).try_remove::<PausedAnimation>();
            }
            // already paused on this build
            (false, Some(paused)) if paused.build == build => (),
            // hidden, or rebuilt hidden while the earlier build was paused
            (false, _) if !primary.contains(avatar) => {
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
                    .entity(avatar)
                    .try_insert(PausedAnimation { build, bones });
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
        assert!(app.world().get::<PausedAnimation>(avatar.root).is_some());

        set_visible(&mut app, avatar.build, true);
        let target = app
            .world()
            .get::<AnimationTarget>(avatar.bone)
            .expect("the bone rejoins the evaluation");
        assert_eq!(target.player, avatar.root);
        assert!(app.world().get::<PausedAnimation>(avatar.root).is_none());
    }

    /// A build under `root` with one bone `root` animates.
    fn spawn_build(
        app: &mut App,
        root: Entity,
        visibility: InheritedVisibility,
    ) -> (Entity, Entity) {
        let world = app.world_mut();
        let build = world
            .spawn((AvatarProcessed, visibility, ChildOf(root)))
            .id();
        let bone = world
            .spawn((
                ChildOf(build),
                AnimationTarget {
                    id: AnimationTargetId::from_name(&Name::new("Avatar_Hips")),
                    player: root,
                },
            ))
            .id();
        (build, bone)
    }

    #[test]
    fn a_rebuild_replaces_the_paused_record() {
        let (mut app, avatar) = app_with_avatar(false);
        set_visible(&mut app, avatar.build, false);

        // rebuilt while hidden: the new build's bones pause in place of the old ones
        app.world_mut().entity_mut(avatar.build).despawn();
        let (hidden_build, hidden_bone) =
            spawn_build(&mut app, avatar.root, InheritedVisibility::HIDDEN);
        app.update();
        assert!(app.world().get::<AnimationTarget>(hidden_bone).is_none());
        let paused = app.world().get::<PausedAnimation>(avatar.root).unwrap();
        assert_eq!(paused.build, hidden_build);

        // rebuilt shown: nothing is left paused
        app.world_mut().entity_mut(hidden_build).despawn();
        let (_, shown_bone) = spawn_build(&mut app, avatar.root, InheritedVisibility::VISIBLE);
        app.update();
        assert!(app.world().get::<AnimationTarget>(shown_bone).is_some());
        assert!(app.world().get::<PausedAnimation>(avatar.root).is_none());
    }

    #[test]
    fn a_new_build_of_a_visible_avatar_does_not_wait_for_propagation() {
        let (mut app, avatar) = app_with_avatar(false);
        app.world_mut()
            .entity_mut(avatar.root)
            .insert(InheritedVisibility::VISIBLE);
        app.world_mut().entity_mut(avatar.build).despawn();

        // processed this frame: shown, but `InheritedVisibility` still holds the load-time hidden
        let (build, bone) = spawn_build(&mut app, avatar.root, InheritedVisibility::HIDDEN);
        app.world_mut()
            .entity_mut(build)
            .insert(Visibility::Inherited);
        app.update();
        assert!(app.world().get::<AnimationTarget>(bone).is_some());
        assert!(app.world().get::<PausedAnimation>(avatar.root).is_none());

        // a new build Max Avatars hides pauses at once
        app.world_mut().entity_mut(build).despawn();
        let (build, bone) = spawn_build(&mut app, avatar.root, InheritedVisibility::HIDDEN);
        app.world_mut().entity_mut(build).insert(Visibility::Hidden);
        app.update();
        assert!(app.world().get::<AnimationTarget>(bone).is_none());
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
