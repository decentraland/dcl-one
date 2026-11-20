use bevy::{
    math::{FloatOrd, Mat3A, Vec3A},
    prelude::*,
};
use common::{
    rpc::{NametagAnchorOffset, RpcCall, RpcCallEvent},
    sets::PostUpdateSets,
    structs::{AttachPoints, AvatarDynamicState, MoveKind},
};
use dcl_component::transform_and_parent::DclTranslation;
use scene_runner::{initialize_scene::SuperUserScene, renderer_context::RendererSceneContext};

use crate::ghost::AvatarGhost;
use bevy_console::ConsoleCommand;
use console::DoAddConsoleCommand;

/// where the nametag sits while a player's ghost stands in for their avatar
const GHOST_NAMETAG_HEIGHT: f32 = 2.0;
/// the gap between the top of what a player wears and their nametag
const HEAD_OFFSET_BUFFER: f32 = 0.025;
/// a measured offset above this is taken for a broken model, and replaced
const HEAD_OFFSET_MAX: f32 = 2.0;
const HEAD_OFFSET_BOUNDED: f32 = 1.0;
/// the least a head reaches above its bone
const HEAD_TOP_MIN: f32 = 0.1;
/// the offset of an avatar whose meshes gave no bounds
const HEAD_OFFSET_DEFAULT: f32 = 0.4;
/// an avatar scaled below this on an axis is scaled to nothing
const SCALE_MIN: f32 = 1e-6;
/// how much higher the nametag sits over a glider
const GLIDE_OFFSET: f32 = 0.8;

pub struct DynamicNametagPlugin;

impl Plugin for DynamicNametagPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            dynamic_nametag_position.in_set(PostUpdateSets::Nametag),
        );
        app.add_systems(Update, serve_avatar_nametag_anchor);

        app.insert_resource(NametagsVisible(true))
            .add_console_command::<Nametags, _>(nametags_command);
    }
}

/// Visibility of the nametags; photo mode flips it on its nametags key.
#[derive(Resource)]
pub struct NametagsVisible(pub bool);

/// Read or set visibility of the nametags.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/nametags")]
struct Nametags {
    visible: Option<bool>,
}

fn nametags_command(mut cmd: ConsoleCommand<Nametags>, mut visible: ResMut<NametagsVisible>) {
    let Some(Ok(request)) = cmd.take() else {
        return;
    };
    if let Some(value) = request.visible {
        visible.0 = value;
    }
    cmd.reply_ok(serde_json::json!({"visible": visible.0}).to_string());
}

/// Answers `avatarNametagAnchor`: for an avatar of the system scene (its `AvatarShape` entity),
/// where its nametag anchor stands from the avatar's origin, as `dynamic_nametag_position` last
/// placed it (over the head, clear of what the avatar wears, following the head across too), in
/// world metres along the scene's axes. None for another scene's entity, an entity that is no such
/// avatar, or an avatar whose body isn't measured yet (no `NametagHeadOffset`). Read in `Update`: the
/// transforms as the last frame's `PostUpdate` propagated them.
///
/// Only the system scene is answered, and this is where that holds: a call's origin is the scene
/// the engine stamped on it, so a scene that reaches the op directly (on web, any scene can, from
/// its own sandbox) gets None whatever it asks.
fn serve_avatar_nametag_anchor(
    mut requests: EventReader<RpcCallEvent>,
    scenes: Query<&RendererSceneContext, With<SuperUserScene>>,
    avatars: Query<&AttachPoints, With<NametagHeadOffset>>,
    global_transforms: Query<&GlobalTransform>,
) {
    for event in requests.read() {
        let RpcCall::AvatarNametagAnchor { entity, response } = &event.call else {
            continue;
        };
        let anchor = event
            .origin
            .scene()
            .and_then(|scene| scenes.get(scene).ok())
            .and_then(|context| context.bevy_entity(*entity))
            .and_then(|avatar| avatars.get(avatar).ok())
            .and_then(|points| {
                nametag_anchor_from(
                    global_transforms.get(points.position).ok()?,
                    global_transforms.get(points.nametag).ok()?,
                )
            });
        response.send(anchor);
    }
}

/// The nametag anchor's offset from the avatar's origin, in world metres along a scene's axes (the
/// engine's z, reversed); None for a transform that isn't finite.
fn nametag_anchor_from(
    origin: &GlobalTransform,
    nametag: &GlobalTransform,
) -> Option<NametagAnchorOffset> {
    let offset = nametag.translation() - origin.translation();
    if !offset.is_finite() {
        return None;
    }
    let [x, y, z] = DclTranslation::from_bevy_translation(offset).0;
    Some(NametagAnchorOffset { x, y, z })
}

/// On an avatar: how far above the head bone their nametag sits, clear of what they wear, in
/// the avatar's own units (metres at a scale of one). It is there while the avatar has a body
/// it was measured on.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct NametagHeadOffset(pub f32);

impl NametagHeadOffset {
    /// From how far the avatar's meshes reach above the head bone, in the avatar's units. A
    /// reach that doesn't clear a bare head is not a measurement of one: the default stands in.
    pub fn above_head(top: f32) -> Self {
        let offset = top + HEAD_OFFSET_BUFFER;
        Self(if !offset.is_finite() || top < HEAD_TOP_MIN {
            HEAD_OFFSET_DEFAULT
        } else if offset <= HEAD_OFFSET_MAX {
            offset
        } else {
            HEAD_OFFSET_BOUNDED
        })
    }
}

/// How high above the avatar's root its nametag goes, in the avatar's units: over the head, or
/// over whichever of the toes and hips an animation has lifted above it. `head_scale` is the
/// head bone's own scale, which enlarges what the head wears with it.
fn nametag_height(head: f32, head_scale: f32, others: [f32; 3], offset: f32, gliding: bool) -> f32 {
    let other = others.into_iter().map(FloatOrd).max().map_or(head, |y| y.0);
    let height = if head >= other {
        head + offset * head_scale
    } else {
        other + offset
    };
    if gliding {
        height + GLIDE_OFFSET
    } else {
        height
    }
}

/// An avatar's height scale, and the way from the world's axes back to the avatar's own. A
/// scene can scale, turn and tilt an avatar: heights are the world's, straight up, counted in
/// the avatar's units, so the offsets grow and shrink with it, and the anchor, the avatar's
/// child, takes what is placed in the world back through the avatar's whole transform. An
/// avatar scaled to nothing has no such way back, and no place for a nametag.
pub(crate) fn avatar_units(avatar: &GlobalTransform) -> Option<(f32, Mat3A)> {
    let to_world = avatar.affine().matrix3;
    // a scale of nothing reaches here as a sliver, on any of the axes
    let scales = axis_scales(avatar);
    let to_avatar = to_world.inverse();
    (scales.into_iter().all(|scale| scale > SCALE_MIN) && to_avatar.is_finite())
        .then_some((scales[1], to_avatar))
}

fn axis_scales(avatar: &GlobalTransform) -> [f32; 3] {
    let to_world = avatar.affine().matrix3;
    [to_world.x_axis, to_world.y_axis, to_world.z_axis].map(Vec3A::length)
}

/// The least an avatar's transform scales any of its axes by; `None` for an avatar scaled to
/// nothing.
pub(crate) fn smallest_scale(avatar: &GlobalTransform) -> Option<f32> {
    let smallest = axis_scales(avatar).into_iter().map(FloatOrd).min()?.0;
    (smallest > SCALE_MIN).then_some(smallest)
}

pub(crate) fn dynamic_nametag_position(
    players: Query<(
        &AttachPoints,
        Option<&AvatarGhost>,
        Option<&NametagHeadOffset>,
        Option<&AvatarDynamicState>,
    )>,
    mut transforms: Query<&mut Transform>,
    parents: Query<&ChildOf>,
    global_transforms: Query<&GlobalTransform>,
) {
    for (attach_points, ghost, head_offset, dynamic_state) in &players {
        let Ok(position_gt) = global_transforms.get(attach_points.position) else {
            continue;
        };
        let origin = position_gt.translation();
        let Some((scale, to_avatar)) = avatar_units(position_gt) else {
            continue;
        };
        let head_position = global_transforms
            .get(attach_points.head)
            .unwrap_or(position_gt)
            .translation();
        let height_of = |point: Entity| {
            global_transforms
                .get(point)
                .map_or(0.0, |gt| (gt.translation().y - origin.y) / scale)
        };

        // the head attach point only reaches the head once the avatar is built; the ghost stands
        // in for it at a fixed height until then
        let height = if ghost.is_some_and(AvatarGhost::showing) {
            GHOST_NAMETAG_HEIGHT
        } else {
            // the attach point hangs off the head bone once the avatar is built
            let head_scale = parents
                .get(attach_points.head)
                .ok()
                .and_then(|parent| transforms.get(parent.parent()).ok())
                .map_or(1.0, |bone| bone.scale.y);
            nametag_height(
                height_of(attach_points.head),
                head_scale,
                [
                    attach_points.left_toe_base,
                    attach_points.right_toe_base,
                    attach_points.hip,
                ]
                .map(height_of),
                head_offset.map_or(HEAD_OFFSET_DEFAULT, |offset| offset.0),
                dynamic_state.is_some_and(|state| state.move_kind == MoveKind::Glide),
            )
        };

        let Ok(mut nametag_transform) = transforms.get_mut(attach_points.nametag) else {
            continue;
        };
        nametag_transform.translation = to_avatar
            * Vec3::new(
                head_position.x - origin.x,
                height * scale,
                head_position.z - origin.z,
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::rpc::{RpcOrigin, RpcResultSender};
    use dcl_component::SceneEntityId;

    /// A scene (the system scene if `system`), and one of its avatars (an `AvatarShape`'s attach
    /// points) whose origin and nametag anchor stand here in the world (the engine's axes);
    /// `measured`: its body has given its offset.
    fn scene_with_avatar(
        world: &mut World,
        system: bool,
        origin: Vec3,
        nametag: Vec3,
        measured: bool,
    ) -> (Entity, SceneEntityId) {
        let root = world.spawn_empty().id();
        let mut context = RendererSceneContext::new(
            dcl_types::SceneId(root),
            "scene".to_owned(),
            String::new(),
            false,
            0,
            "scene".to_owned(),
            IVec2::ZERO,
            [IVec2::ZERO].into_iter().collect(),
            Vec::new(),
            scene_material::SceneBounds::default(),
            Vec::new(),
            root,
            0.0,
            false,
            "",
            false,
            false,
        );
        let attach_points = AttachPoints::new(&mut world.commands());
        world.flush();
        world
            .entity_mut(attach_points.position)
            .insert(GlobalTransform::from_translation(origin));
        world
            .entity_mut(attach_points.nametag)
            .insert(GlobalTransform::from_translation(nametag));
        let mut avatar = world.spawn(attach_points);
        if measured {
            avatar.insert(NametagHeadOffset(0.3));
        }
        let avatar = avatar.id();
        let id = SceneEntityId::new(600, 0);
        context.associate_bevy_entity(id, avatar);
        world.entity_mut(root).insert(context);
        if system {
            world.entity_mut(root).insert(SuperUserScene);
        }
        (root, id)
    }

    fn nametag_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_event::<RpcCallEvent>()
            .add_systems(Update, serve_avatar_nametag_anchor);
        app
    }

    fn ask(app: &mut App, origin: RpcOrigin, entity: SceneEntityId) -> Option<NametagAnchorOffset> {
        let (response, mut answer) = RpcResultSender::channel();
        app.world_mut().send_event(RpcCallEvent {
            origin,
            call: RpcCall::AvatarNametagAnchor { entity, response },
        });
        app.update();
        answer
            .try_recv()
            .expect("answered in the frame it was asked")
    }

    #[test]
    fn the_system_scene_learns_where_its_avatar_has_its_nametag_along_its_own_axes() {
        let mut app = nametag_app();
        // the head leans east and toward the engine's -z, which is the scene's north
        let (scene, avatar) = scene_with_avatar(
            app.world_mut(),
            true,
            Vec3::new(8.0, 0.5, -8.0),
            Vec3::new(8.2, 4.1, -8.3),
            true,
        );
        let from_scene = RpcOrigin::Scene {
            entity: scene,
            hash: "scene".into(),
        };
        let anchor = ask(&mut app, from_scene.clone(), avatar).expect("a measured avatar");
        let close = |a: f32, b: f32| (a - b).abs() < 1e-5;
        assert!(
            close(anchor.x, 0.2) && close(anchor.y, 3.6) && close(anchor.z, 0.3),
            "{anchor:?}"
        );
        // the engine itself is no scene, and an entity the scene doesn't hold is no avatar
        assert_eq!(ask(&mut app, RpcOrigin::Engine, avatar), None);
        assert_eq!(ask(&mut app, from_scene, SceneEntityId::new(601, 0)), None);
    }

    #[test]
    fn an_ordinary_scene_learns_no_nametag_anchor() {
        let mut app = nametag_app();
        // the same measured avatar as the system scene's above, held by a scene that isn't it
        let (scene, avatar) = scene_with_avatar(
            app.world_mut(),
            false,
            Vec3::new(8.0, 0.5, -8.0),
            Vec3::new(8.2, 4.1, -8.3),
            true,
        );
        let from_scene = RpcOrigin::Scene {
            entity: scene,
            hash: "scene".into(),
        };
        assert_eq!(ask(&mut app, from_scene, avatar), None);
    }

    #[test]
    fn an_avatar_whose_body_isnt_measured_has_no_nametag_anchor_yet() {
        let mut app = nametag_app();
        let (scene, avatar) = scene_with_avatar(
            app.world_mut(),
            true,
            Vec3::new(8.0, 0.5, -8.0),
            Vec3::new(8.0, 4.1, -8.0),
            false,
        );
        let from_scene = RpcOrigin::Scene {
            entity: scene,
            hash: "scene".into(),
        };
        assert_eq!(ask(&mut app, from_scene, avatar), None);
    }

    #[test]
    fn the_offset_clears_what_the_avatar_wears() {
        assert_eq!(
            NametagHeadOffset::above_head(0.3).0,
            0.3 + HEAD_OFFSET_BUFFER
        );
        // a model reaching absurdly high is not followed
        assert_eq!(NametagHeadOffset::above_head(5.0).0, HEAD_OFFSET_BOUNDED);
        // no meshes measured, or nothing that reaches over the head
        for unmeasured in [f32::NEG_INFINITY, f32::NAN, -1.4, 0.02] {
            assert_eq!(
                NametagHeadOffset::above_head(unmeasured).0,
                HEAD_OFFSET_DEFAULT
            );
        }
    }

    #[test]
    fn a_scaled_avatar_keeps_its_nametag_over_its_head() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin))
            .add_systems(PostUpdate, dynamic_nametag_position);
        let world = app.world_mut();
        let attach_points = AttachPoints::new(&mut world.commands());
        world.flush();
        let (head, nametag) = (attach_points.head, attach_points.nametag);
        let avatar = world
            .spawn((
                Transform::from_xyz(3.0, 1.0, -2.0).with_scale(Vec3::splat(1.5)),
                NametagHeadOffset(0.3),
            ))
            .add_children(&attach_points.entities())
            .insert(attach_points)
            .id();
        // a built avatar: the head attach point hangs off the head bone
        let bone = world
            .spawn((Transform::from_xyz(0.0, 1.6, 0.0), ChildOf(avatar)))
            .id();
        world.entity_mut(head).insert(ChildOf(bone));
        // once to propagate the transforms, once to read them
        app.update();
        app.update();

        // the anchor is the avatar's child: it stays in the avatar's units, scaled with it
        let anchor = app.world().get::<Transform>(nametag).unwrap().translation;
        assert!(
            (anchor - Vec3::new(0.0, 1.9, 0.0)).length() < 1e-4,
            "{anchor}"
        );

        // turned, tilted and stretched: the tag stays straight over the head in the world, by
        // the offset at the avatar's height scale
        let stretched = Transform::from_xyz(3.0, 1.0, -2.0)
            .with_rotation(Quat::from_euler(EulerRot::YXZ, 1.0, 0.4, 0.0))
            .with_scale(Vec3::new(0.5, 2.0, 3.0));
        app.world_mut().entity_mut(avatar).insert(stretched);
        app.update();
        app.update();
        let world = |entity| {
            app.world()
                .get::<GlobalTransform>(entity)
                .unwrap()
                .translation()
        };
        let over_head = world(nametag) - world(head);
        assert!(
            (over_head - Vec3::new(0.0, 0.3 * 2.0, 0.0)).length() < 1e-3,
            "{over_head}"
        );
    }

    #[test]
    fn the_nametag_follows_the_highest_of_head_toes_and_hips() {
        let close = |a: f32, b: f32| (a - b).abs() < 1e-5;
        // standing: over the head, by the scaled offset
        assert!(close(
            nametag_height(1.6, 1.0, [0.1, 0.1, 0.9], 0.3, false),
            1.9
        ));
        assert!(close(
            nametag_height(1.6, 2.0, [0.1, 0.1, 0.9], 0.3, false),
            2.2
        ));
        // a handstand: over the toes, by the unscaled offset
        assert!(close(
            nametag_height(0.2, 2.0, [1.7, 1.5, 0.9], 0.3, false),
            2.0
        ));
        // gliding lifts it over the glider
        assert!(close(
            nametag_height(1.6, 1.0, [0.1, 0.1, 0.9], 0.3, true),
            1.9 + GLIDE_OFFSET
        ));
    }
}
