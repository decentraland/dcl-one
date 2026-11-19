//! Avatars that are hidden, or outside every camera's view, stop animating.
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
//! An avatar no camera can see pauses the same way. Every active 3D camera that draws one of the
//! avatar's render layers counts: the player's, a scene's texture cameras, the profile booth's
//! and a scene test's snapshot cameras. It resumes before it comes into view: the test is a
//! sphere around the avatar wider than the avatar ([`RESUME_MARGIN`]), so the camera has to turn
//! through that margin first, and the avatar's shadow near the edge of the view stays live. It
//! pauses only once a wider sphere ([`PAUSE_MARGIN`]) is out of every view, so an avatar at the
//! edge does not switch back and forth. An entity a scene attached to a bone of an avatar out of
//! view stays where the bone was until the avatar resumes.
//!
//! The margin is sized for shadows under a high sun. Under a low sun (dawn, dusk, or a time a
//! scene sets) an avatar's shadow reaches further than the margin, so the shadow of an avatar
//! outside it can fall into view and stays still until the avatar resumes.
//!
//! The views are the ones the last frame was drawn with: the cameras move, and their frusta are
//! rebuilt, after the animation is evaluated. The margin covers a turning or moving camera. A
//! teleport moves the view further than that, so in the frame of a teleport every shown avatar
//! animates.
//!
//! The local player's avatar never pauses: first person hides it by render layer, not by
//! visibility, and its bones carry attach points and the camera's head tracking.

use bevy::{
    animation::AnimationTarget,
    prelude::*,
    render::{
        primitives::{Frustum, Sphere as ViewSphere},
        view::RenderLayers,
    },
};
use common::{
    sets::SceneSets,
    structs::{PlayerTeleported, PrimaryUser},
};

use crate::AvatarProcessed;

pub struct PauseHiddenAvatarsPlugin;

impl Plugin for PauseHiddenAvatarsPlugin {
    fn build(&self, app: &mut App) {
        // comms registers the event in the Explorer; an app without comms still runs the system
        app.add_event::<PlayerTeleported>().add_systems(
            Update,
            // after the systems that teleport the player, to see the event in its own frame
            pause_hidden_avatar_animation.after(SceneSets::RestrictedActions),
        );
    }
}

/// On an avatar whose build is hidden: the bones of that build taken out of the animation
/// evaluation, with the target each one had (none, for a build with no bones to pause). Systems
/// that adjust the animated pose skip an avatar that has it.
#[derive(Component)]
pub struct PausedAnimation {
    /// the build the bones belong to; a rebuild replaces it, leaving this record stale
    build: Entity,
    bones: Vec<(Entity, AnimationTarget)>,
}

/// A paused avatar resumes when a sphere of this radius (metres) around it enters the view.
const RESUME_MARGIN: f32 = 6.0;
/// An animating avatar pauses when a sphere of this radius (metres) around it leaves the view.
const PAUSE_MARGIN: f32 = 9.0;

/// Whether an avatar animates: shown, and near enough to the view. One already paused resumes
/// at the nearer margin and one animating pauses at the farther, so neither flips at the edge.
fn animates(shown: bool, paused: bool, within: impl Fn(f32) -> bool) -> bool {
    shown && within(if paused { RESUME_MARGIN } else { PAUSE_MARGIN })
}

fn pause_hidden_avatar_animation(
    mut commands: Commands,
    builds: Query<
        (
            Entity,
            &InheritedVisibility,
            Option<&Visibility>,
            Ref<AvatarProcessed>,
            &ChildOf,
            &GlobalTransform,
            Option<&RenderLayers>,
        ),
        With<AvatarProcessed>,
    >,
    roots: Query<&InheritedVisibility>,
    cameras: Query<(&Camera, &Frustum, Option<&RenderLayers>), With<Camera3d>>,
    primary: Query<(), With<PrimaryUser>>,
    paused: Query<&PausedAnimation>,
    children: Query<&Children>,
    targets: Query<&AnimationTarget>,
    mut teleports: EventReader<PlayerTeleported>,
) {
    // an entity without `RenderLayers` is on the default layer
    let default_layers = RenderLayers::default();
    // the views that draw this frame, each with the layers it draws
    let views: Vec<_> = cameras
        .iter()
        .filter(|(camera, ..)| camera.is_active)
        .map(|(_, frustum, layers)| (frustum, layers.unwrap_or(&default_layers)))
        .collect();
    // without a view, or in the frame the player teleports, every shown avatar animates
    let all_in_view = teleports.read().count() > 0 || views.is_empty();
    for (build, inherited, visibility, processed, parent, transform, layers) in &builds {
        let layers = layers.unwrap_or(&default_layers);
        let avatar = parent.parent();
        // A build loads hidden and is shown by the same commands that mark it processed, so in
        // the frame it is processed its `InheritedVisibility` may not have been propagated yet.
        // Work it out from its own `Visibility` (Max Avatars hides the build) and the root's,
        // which are both current, so a visible avatar's new build is never paused.
        let visible = match (processed.is_added(), visibility) {
            (true, Some(Visibility::Hidden)) => false,
            (true, Some(Visibility::Visible)) => true,
            (true, Some(Visibility::Inherited)) => {
                roots.get(avatar).map_or(inherited.get(), |root| root.get())
            }
            _ => inherited.get(),
        };
        let paused = paused.get(avatar).ok();
        let should_animate = animates(
            visible,
            paused.is_some_and(|paused| paused.build == build),
            |margin| {
                let sphere = ViewSphere {
                    center: (transform.translation() + Vec3::Y).into(),
                    radius: margin,
                };
                all_in_view
                    || views.iter().any(|(frustum, drawn)| {
                        drawn.intersects(layers) && frustum.intersects_sphere(&sphere, true)
                    })
            },
        );
        match (should_animate, paused) {
            // shown and in view again, or rebuilt so: the bones rejoin the evaluation and take
            // the current pose (those of a replaced build are gone, and skipped)
            (true, Some(PausedAnimation { bones, .. })) => {
                for (bone, target) in bones {
                    commands.entity(*bone).try_insert(*target);
                }
                commands.entity(avatar).try_remove::<PausedAnimation>();
            }
            // already paused on this build
            (false, Some(paused)) if paused.build == build => (),
            // hidden or out of view, or rebuilt so while the earlier build was paused; a build
            // with no bones to pause is recorded too, so it is walked once and not every frame
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
            .spawn((
                AvatarProcessed,
                InheritedVisibility::VISIBLE,
                GlobalTransform::default(),
                ChildOf(root),
            ))
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
            .spawn((
                AvatarProcessed,
                visibility,
                GlobalTransform::default(),
                ChildOf(root),
            ))
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
    fn an_avatar_pauses_and_resumes_at_different_margins() {
        // in view up to 7 m of margin: inside the pause margin, outside the resume margin
        let within = |margin: f32| margin >= 7.0;
        assert!(animates(true, false, within), "animating, it carries on");
        assert!(!animates(true, true, within), "paused, it stays paused");
        // well inside the view either resumes
        assert!(animates(true, true, |_| true));
        // far outside either pauses, and a hidden avatar never animates
        assert!(!animates(true, false, |_| false));
        assert!(!animates(false, false, |_| true));
    }

    #[test]
    fn an_avatar_behind_the_camera_stops_animating_until_the_camera_turns() {
        use bevy::render::camera::CameraProjection;

        let (mut app, avatar) = app_with_avatar(false);
        // the avatar stands at the origin; the camera is 30 m away, looking away from it
        let projection = PerspectiveProjection::default();
        let away = GlobalTransform::from(
            Transform::from_xyz(0.0, 0.0, -30.0).looking_to(Vec3::NEG_Z, Vec3::Y),
        );
        let camera = app
            .world_mut()
            .spawn((Camera3d::default(), projection.compute_frustum(&away)))
            .id();
        app.update();
        assert!(app.world().get::<AnimationTarget>(avatar.bone).is_none());
        assert!(app.world().get::<PausedAnimation>(avatar.root).is_some());

        let towards = GlobalTransform::from(
            Transform::from_xyz(0.0, 0.0, -30.0).looking_to(Vec3::Z, Vec3::Y),
        );
        app.world_mut()
            .entity_mut(camera)
            .insert(projection.compute_frustum(&towards));
        app.update();
        assert!(app.world().get::<AnimationTarget>(avatar.bone).is_some());
        assert!(app.world().get::<PausedAnimation>(avatar.root).is_none());
    }

    /// A view whose camera stands `distance` metres in front of the avatar's centre, at the
    /// origin, facing away from it.
    fn behind(distance: f32) -> Frustum {
        use bevy::render::camera::CameraProjection;

        PerspectiveProjection::default().compute_frustum(&GlobalTransform::from(
            Transform::from_xyz(0.0, 1.0, -distance).looking_to(Vec3::NEG_Z, Vec3::Y),
        ))
    }

    #[test]
    fn an_avatar_between_the_margins_keeps_its_state() {
        let (mut app, avatar) = app_with_avatar(false);
        let animating = |app: &App| app.world().get::<AnimationTarget>(avatar.bone).is_some();
        // 7.5 m outside the view (with the near plane): between the two margins
        let camera = app
            .world_mut()
            .spawn((Camera3d::default(), behind(7.4)))
            .id();
        app.update();
        assert!(animating(&app), "animating, it carries on");

        app.world_mut().entity_mut(camera).insert(behind(30.0));
        app.update();
        assert!(!animating(&app));
        app.world_mut().entity_mut(camera).insert(behind(7.4));
        app.update();
        assert!(!animating(&app), "paused, it stays paused");

        app.world_mut().entity_mut(camera).insert(behind(4.0));
        app.update();
        assert!(animating(&app), "inside the resume margin it resumes");
    }

    /// A frustum 30 m from the origin, where the avatars stand, looking at it or away from it.
    fn view(towards: bool) -> Frustum {
        use bevy::render::camera::CameraProjection;

        let facing = if towards { Vec3::Z } else { Vec3::NEG_Z };
        PerspectiveProjection::default().compute_frustum(&GlobalTransform::from(
            Transform::from_xyz(0.0, 0.0, -30.0).looking_to(facing, Vec3::Y),
        ))
    }

    #[test]
    fn an_avatar_another_camera_draws_keeps_animating() {
        let (mut app, avatar) = app_with_avatar(false);
        app.world_mut().spawn((Camera3d::default(), view(false)));
        // a second camera faces the avatar, but draws a layer the avatar is not on
        let other = app
            .world_mut()
            .spawn((Camera3d::default(), view(true), RenderLayers::layer(3)))
            .id();
        app.update();
        assert!(app.world().get::<AnimationTarget>(avatar.bone).is_none());

        // on that layer too, the avatar is in the second camera's view
        app.world_mut()
            .entity_mut(avatar.build)
            .insert(RenderLayers::from_layers(&[0, 3]));
        app.update();
        assert!(app.world().get::<AnimationTarget>(avatar.bone).is_some());

        // an inactive camera draws nothing
        app.world_mut()
            .entity_mut(other)
            .get_mut::<Camera>()
            .unwrap()
            .is_active = false;
        app.update();
        assert!(app.world().get::<AnimationTarget>(avatar.bone).is_none());
    }

    #[test]
    fn every_shown_avatar_animates_in_the_frame_of_a_teleport() {
        let (mut app, avatar) = app_with_avatar(false);
        app.world_mut().spawn((Camera3d::default(), view(false)));
        app.update();
        assert!(app.world().get::<AnimationTarget>(avatar.bone).is_none());

        app.world_mut().send_event(PlayerTeleported {
            position: Vec3::ZERO,
        });
        app.update();
        assert!(app.world().get::<AnimationTarget>(avatar.bone).is_some());

        app.update();
        assert!(app.world().get::<AnimationTarget>(avatar.bone).is_none());
    }

    #[test]
    fn a_build_with_no_bones_to_pause_is_recorded_as_paused() {
        let (mut app, avatar) = app_with_avatar(false);
        app.world_mut()
            .entity_mut(avatar.bone)
            .remove::<AnimationTarget>();
        set_visible(&mut app, avatar.build, false);
        let paused = app.world().get::<PausedAnimation>(avatar.root).unwrap();
        assert!(paused.bones.is_empty());

        set_visible(&mut app, avatar.build, true);
        assert!(app.world().get::<PausedAnimation>(avatar.root).is_none());
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
