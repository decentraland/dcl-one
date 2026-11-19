//! Distant avatars cast no shadow.
//!
//! Every mesh of an avatar is drawn again into each shadow cascade it falls in, skinned again
//! each time, as far as the Shadow Distance setting reaches (up to 300 m). An avatar's shadow is
//! a few pixels at that range, so beyond [`AvatarShadowDistance`] from the local player an
//! avatar's meshes are marked `NotShadowCaster`, and unmarked when it comes back within it. The
//! local player's avatar always casts one. The distance is from the player, not the camera, so
//! with a camera away from the player the avatars near the camera may cast none.

use bevy::{pbr::NotShadowCaster, prelude::*, render::view::RenderLayers};
use bevy_console::ConsoleCommand;
use common::structs::PrimaryUser;
use console::DoAddConsoleCommand;

use crate::AvatarProcessed;

pub struct AvatarShadowDistancePlugin;

impl Plugin for AvatarShadowDistancePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AvatarShadowDistance>()
            .add_systems(Update, set_avatar_shadows)
            .add_console_command::<AvatarShadowDistanceCommand, _>(avatar_shadow_distance);
    }
}

/// How far from the local player (metres) another avatar still casts a shadow.
#[derive(Resource)]
pub struct AvatarShadowDistance(pub f32);

impl Default for AvatarShadowDistance {
    fn default() -> Self {
        Self(20.0)
    }
}

/// An avatar past the distance gets its shadow back only once it is this much nearer than it,
/// so one standing at the distance does not switch back and forth.
const RETURN_FACTOR: f32 = 0.9;

/// On a built avatar that casts no shadow for being far: the meshes marked `NotShadowCaster`
/// for it, which are the ones to unmark.
#[derive(Component)]
struct DistantShadowOff(Vec<Entity>);

/// Whether an avatar at `distance` from the local player casts a shadow, given whether it does
/// now.
fn casts_shadow(distance: f32, max_distance: f32, casting: bool) -> bool {
    distance
        <= if casting {
            max_distance
        } else {
            max_distance * RETURN_FACTOR
        }
}

fn set_avatar_shadows(
    mut commands: Commands,
    distance: Res<AvatarShadowDistance>,
    builds: Query<
        (
            Entity,
            &GlobalTransform,
            &ChildOf,
            Option<&RenderLayers>,
            Option<&DistantShadowOff>,
        ),
        With<AvatarProcessed>,
    >,
    player: Query<&GlobalTransform, With<PrimaryUser>>,
    children: Query<&Children>,
    meshes: Query<(), (With<Mesh3d>, Without<NotShadowCaster>)>,
    mut unmarked: Query<&mut Mesh3d>,
) {
    let Ok(player_pos) = player.single().map(GlobalTransform::translation) else {
        return;
    };
    let default_layer = RenderLayers::layer(0);
    for (build, transform, parent, layers, off) in &builds {
        // an avatar on a layer of its own (a preview) is lit by its own stage, wherever it stands
        if layers.is_some_and(|layers| !layers.intersects(&default_layer)) {
            continue;
        }
        let casts = player.contains(parent.parent())
            || casts_shadow(
                transform.translation().distance(player_pos),
                distance.0,
                off.is_none(),
            );
        match (casts, off) {
            (true, Some(DistantShadowOff(marked))) => {
                for mesh in marked {
                    commands.entity(*mesh).try_remove::<NotShadowCaster>();
                    // bevy's mesh extraction doesn't watch the mark's removal (0.16); Mesh3d is
                    // on its list
                    if let Ok(mut mesh) = unmarked.get_mut(*mesh) {
                        mesh.set_changed();
                    }
                }
                commands.entity(build).try_remove::<DistantShadowOff>();
            }
            (false, None) => {
                // only the meshes that cast one: a mesh that never did keeps its own mark
                let marked: Vec<_> = children
                    .iter_descendants(build)
                    .filter(|mesh| meshes.contains(*mesh))
                    .collect();
                for mesh in &marked {
                    commands.entity(*mesh).try_insert(NotShadowCaster);
                }
                commands.entity(build).try_insert(DistantShadowOff(marked));
            }
            _ => (),
        }
    }
}

/// Read or set how far from the player other avatars cast shadows, in metres
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/avatar_shadow_distance")]
struct AvatarShadowDistanceCommand {
    metres: Option<f32>,
}

fn avatar_shadow_distance(
    mut input: ConsoleCommand<AvatarShadowDistanceCommand>,
    mut distance: ResMut<AvatarShadowDistance>,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    if let Some(metres) = command.metres {
        if metres.is_nan() || metres < 0.0 {
            input.reply_failed("choose a distance of 0 or more metres");
            return;
        }
        distance.0 = metres;
    }
    input.reply_ok(serde_json::json!({ "metres": distance.0 }).to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_avatar_loses_its_shadow_past_the_distance_and_regains_it_nearer() {
        assert!(casts_shadow(19.0, 20.0, true));
        assert!(!casts_shadow(21.0, 20.0, true));
        // without one, it needs to come nearer than the distance to cast again
        assert!(!casts_shadow(19.0, 20.0, false));
        assert!(casts_shadow(17.0, 20.0, false));
    }

    #[test]
    fn a_far_avatars_meshes_stop_casting_shadows_until_it_is_near() {
        let mut app = App::new();
        app.init_resource::<AvatarShadowDistance>()
            .add_systems(Update, set_avatar_shadows);
        let world = app.world_mut();
        world.spawn((PrimaryUser::default(), GlobalTransform::default()));
        let root = world.spawn_empty().id();
        let build = world
            .spawn((
                AvatarProcessed,
                GlobalTransform::from_translation(Vec3::X * 50.0),
                ChildOf(root),
            ))
            .id();
        let mesh = world.spawn((Mesh3d::default(), ChildOf(build))).id();
        let never_cast = world
            .spawn((Mesh3d::default(), NotShadowCaster, ChildOf(build)))
            .id();

        app.update();
        assert!(app.world().get::<NotShadowCaster>(mesh).is_some());

        *app.world_mut().get_mut::<GlobalTransform>(build).unwrap() =
            GlobalTransform::from_translation(Vec3::X * 5.0);
        app.update();
        assert!(app.world().get::<NotShadowCaster>(mesh).is_none());
        assert!(
            app.world().get::<NotShadowCaster>(never_cast).is_some(),
            "a mesh that never cast a shadow keeps its own mark"
        );
    }

    #[test]
    fn an_avatar_on_a_layer_of_its_own_keeps_its_shadow() {
        let mut app = App::new();
        app.init_resource::<AvatarShadowDistance>()
            .add_systems(Update, set_avatar_shadows);
        let world = app.world_mut();
        world.spawn((PrimaryUser::default(), GlobalTransform::default()));
        let root = world.spawn_empty().id();
        let build = world
            .spawn((
                AvatarProcessed,
                GlobalTransform::from_translation(Vec3::X * 50.0),
                RenderLayers::layer(1),
                ChildOf(root),
            ))
            .id();
        let mesh = world.spawn((Mesh3d::default(), ChildOf(build))).id();

        app.update();
        assert!(app.world().get::<NotShadowCaster>(mesh).is_none());
    }
}
