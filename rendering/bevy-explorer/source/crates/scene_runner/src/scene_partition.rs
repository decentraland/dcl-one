use std::time::Duration;

use bevy::prelude::*;
use common::{
    partition::{scene_partition, ScenePartitionData},
    structs::{PrimaryCamera, PrimaryUser},
};

use crate::renderer_context::RendererSceneContext;

/// Where the scene is relative to the camera, refreshed every frame for live scenes by
/// `update_scene_partitions`, and written by the lifecycle for scenes still loading.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ScenePartition(pub ScenePartitionData);

/// A scene behind the camera is sent at most this often (unity-explorer's `behindFps = 1`).
pub const BEHIND_SCENE_TICK_INTERVAL: Duration = Duration::from_secs(1);

/// The camera's position and forward, or the player's when there is no camera (headless).
pub fn sampling_transform(
    camera: &Query<&GlobalTransform, With<PrimaryCamera>>,
    player: &Query<&GlobalTransform, With<PrimaryUser>>,
) -> Option<(Vec3, Vec3)> {
    camera
        .single()
        .ok()
        .or_else(|| player.single().ok())
        .map(|transform| (transform.translation(), transform.forward().as_vec3()))
}

pub fn update_scene_partitions(
    camera: Query<&GlobalTransform, With<PrimaryCamera>>,
    player: Query<&GlobalTransform, With<PrimaryUser>>,
    mut scenes: Query<(Entity, &RendererSceneContext, Option<&mut ScenePartition>)>,
    mut commands: Commands,
) {
    let Some((position, forward)) = sampling_transform(&camera, &player) else {
        return;
    };

    for (entity, context, maybe_partition) in scenes.iter_mut() {
        let partition = ScenePartition(scene_partition(
            position,
            forward,
            context.parcels.iter().copied(),
        ));
        match maybe_partition {
            Some(mut current) => {
                current.set_if_neq(partition);
            }
            None => {
                commands.entity(entity).try_insert(partition);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::{ecs::system::RunSystemOnce, prelude::*};
    use common::{
        partition::ScenePartitionData,
        structs::{PrimaryCamera, PrimaryUser},
    };
    use dcl_types::SceneId;
    use scene_material::SceneBounds;

    use super::*;
    use crate::renderer_context::RendererSceneContext;

    const PLAYER_POS: Vec3 = Vec3::new(8.0, 0.0, -8.0);

    fn world_with_player(camera_forward: Option<Vec3>) -> World {
        let mut world = World::new();
        world.spawn((
            PrimaryUser::default(),
            GlobalTransform::from_translation(PLAYER_POS),
        ));
        if let Some(forward) = camera_forward {
            world.spawn((
                PrimaryCamera::default(),
                GlobalTransform::from(
                    Transform::from_translation(PLAYER_POS).looking_to(forward, Vec3::Y),
                ),
            ));
        }
        world
    }

    fn scene_with_parcels(world: &mut World, parcels: &[IVec2]) -> Entity {
        let root = world.spawn_empty().id();
        let context = RendererSceneContext::new(
            SceneId(root),
            "hash".to_owned(),
            String::new(),
            false,
            0,
            String::new(),
            IVec2::ZERO,
            parcels.iter().copied().collect(),
            Vec::new(),
            SceneBounds::default(),
            Vec::new(),
            root,
            0.0,
            false,
            "",
            false,
            false,
        );
        world.entity_mut(root).insert(context);
        root
    }

    #[test]
    fn partitions_a_live_scene_to_its_nearest_parcel() {
        let mut world = world_with_player(Some(Vec3::X));
        let scene = scene_with_parcels(&mut world, &[IVec2::new(2, 0), IVec2::new(3, 0)]);

        world.run_system_once(update_scene_partitions).unwrap();

        let partition = world.get::<ScenePartition>(scene).unwrap().0;
        assert_eq!(partition.sqr_distance, 576.0);
        assert_eq!(partition.parcel_distance, 2);
        assert!(!partition.is_behind);
    }

    #[test]
    fn uses_the_player_when_there_is_no_camera() {
        let mut world = world_with_player(None);
        // the player's default rotation looks along -z, which is parcel +y
        let scene = scene_with_parcels(&mut world, &[IVec2::new(0, -3)]);

        world.run_system_once(update_scene_partitions).unwrap();

        let partition = world.get::<ScenePartition>(scene).unwrap().0;
        assert!(partition.is_behind);
        assert_eq!(partition.parcel_distance, 3);
    }

    #[test]
    fn a_scene_without_parcels_is_nearest() {
        let mut world = world_with_player(Some(Vec3::X));
        let scene = scene_with_parcels(&mut world, &[]);

        world.run_system_once(update_scene_partitions).unwrap();

        assert_eq!(
            world.get::<ScenePartition>(scene).unwrap().0,
            ScenePartitionData::NEAREST
        );
    }
}
