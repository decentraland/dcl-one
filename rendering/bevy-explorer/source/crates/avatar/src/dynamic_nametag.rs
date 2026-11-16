use bevy::{math::FloatOrd, prelude::*};
use common::{
    sets::PostUpdateSets,
    structs::{AttachPoints, AvatarDynamicState, MoveKind},
};

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
/// how much higher the nametag sits over a glider
const GLIDE_OFFSET: f32 = 0.8;

pub struct DynamicNametagPlugin;

impl Plugin for DynamicNametagPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            dynamic_nametag_position.in_set(PostUpdateSets::Nametag),
        );

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

/// On a player: how far above the head bone their nametag sits, clear of what they wear.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct NametagHeadOffset(pub f32);

impl NametagHeadOffset {
    /// From how far the avatar's meshes reach above the head bone, in metres. A reach that
    /// doesn't clear a bare head is not a measurement of one: the default stands in.
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

/// How high above the avatar's root its nametag goes: over the head, or over whichever of the
/// toes and hips an animation has lifted above it. `head_scale` is the head bone's own scale,
/// which enlarges what the head wears with it.
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

#[allow(clippy::type_complexity)]
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
        let position = position_gt.compute_transform();
        let head_position = global_transforms
            .get(attach_points.head)
            .unwrap_or(position_gt)
            .translation();
        let height_of = |point: Entity| {
            global_transforms
                .get(point)
                .map_or(0.0, |gt| gt.translation().y - position.translation.y)
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
        let position_rotation = {
            let (axis, angle) = position.rotation.to_axis_angle();
            Quat::from_axis_angle(axis, -angle)
        };
        nametag_transform.translation = position_rotation
            * Vec3::new(
                head_position.x - position.translation.x,
                height,
                head_position.z - position.translation.z,
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
