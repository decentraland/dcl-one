use std::time::Duration;

use bevy::prelude::*;
use common::{
    partition::{scene_partition, LoadPriority, ScenePartitionData},
    structs::{PrimaryCamera, PrimaryUser},
};
use ipfs::IpfsResource;

use crate::{initialize_scene::SceneHash, renderer_context::RendererSceneContext, ContainingScene};

/// Where the scene is relative to the camera, refreshed every frame for live scenes by
/// `update_scene_partitions`, and written by the lifecycle for scenes still loading.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ScenePartition(pub ScenePartitionData);

/// A scene behind the camera is sent at most this often (unity-explorer's `behindFps = 1`).
pub const BEHIND_SCENE_TICK_INTERVAL: Duration = Duration::from_secs(1);

/// Where partitions are measured from: the rendering camera's position and forward, or
/// the player's position and no forward (zero: nothing is behind) when nothing renders. The
/// headless server has a stand-in `PrimaryCamera` without a `Camera` that never turns.
pub fn sampling_transform(
    camera: &Query<&GlobalTransform, (With<PrimaryCamera>, With<Camera>)>,
    player: &Query<&GlobalTransform, With<PrimaryUser>>,
) -> Option<(Vec3, Vec3)> {
    if let Ok(camera) = camera.single() {
        return Some((camera.translation(), camera.forward().as_vec3()));
    }
    player
        .single()
        .ok()
        .map(|player| (player.translation(), Vec3::ZERO))
}

pub fn update_scene_partitions(
    camera: Query<&GlobalTransform, (With<PrimaryCamera>, With<Camera>)>,
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

/// Every scene's download priority, by hash, for the ipfs request slots: the player's scenes
/// first, then by partition.
pub fn publish_download_priorities(
    scenes: Query<(Entity, &SceneHash, &ScenePartition)>,
    player: Query<Entity, With<PrimaryUser>>,
    containing_scene: ContainingScene,
    ipfs: Res<IpfsResource>,
) {
    let active = player
        .single()
        .map(|player| containing_scene.get(player))
        .unwrap_or_default();
    let priorities = scenes
        .iter()
        .map(|(entity, hash, partition)| {
            let priority = if active.contains(&entity) {
                LoadPriority::TOP
            } else {
                partition.0.load_priority()
            };
            (hash.0.clone(), priority)
        })
        .collect();
    ipfs.set_scene_priorities(priorities);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bevy::{
        asset::io::memory::{Dir, MemoryAssetReader},
        ecs::system::RunSystemOnce,
        math::FloatOrd,
        prelude::*,
    };
    use common::{
        partition::{LoadPriority, ScenePartitionData},
        structs::{PrimaryCamera, PrimaryUser},
    };
    use dcl_types::SceneId;
    use ipfs::{IpfsIo, IpfsResource};
    use scene_material::SceneBounds;

    use super::*;
    use crate::{
        initialize_scene::{LiveScenes, PointerResult, PortableScenes, SceneHash, ScenePointers},
        renderer_context::RendererSceneContext,
    };

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
                Camera::default(),
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
    fn uses_the_players_position_when_there_is_no_camera() {
        let mut world = world_with_player(None);
        // the player looks along -z (parcel +y), so this scene is behind them; with no
        // rendering camera no facing is known and nothing is behind
        let scene = scene_with_parcels(&mut world, &[IVec2::new(0, -3)]);

        world.run_system_once(update_scene_partitions).unwrap();

        let partition = world.get::<ScenePartition>(scene).unwrap().0;
        assert!(!partition.is_behind);
        assert_eq!(partition.parcel_distance, 3);
    }

    #[test]
    fn a_camera_that_does_not_render_gives_no_facing() {
        // the headless server's stand-in camera: a PrimaryCamera with no Camera, never turned
        let mut world = world_with_player(None);
        world.spawn((
            PrimaryCamera::default(),
            GlobalTransform::from(
                Transform::from_translation(PLAYER_POS).looking_to(Vec3::X, Vec3::Y),
            ),
        ));
        let scene = scene_with_parcels(&mut world, &[IVec2::new(-3, 0)]);

        world.run_system_once(update_scene_partitions).unwrap();

        let partition = world.get::<ScenePartition>(scene).unwrap().0;
        assert!(!partition.is_behind);
        assert_eq!(partition.parcel_distance, 3);
        assert_eq!(partition.sqr_distance, 1600.0);
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

    #[test]
    fn publishes_each_scenes_key_with_the_players_scene_on_top() {
        let mut world = world_with_player(Some(Vec3::X));
        world.insert_resource(IpfsResource {
            inner: Arc::new(IpfsIo::new(
                false,
                Box::new(MemoryAssetReader {
                    root: Dir::default(),
                }),
                None,
                Default::default(),
                1,
                tokio::sync::mpsc::unbounded_channel().0,
            )),
        });
        // the player stands on "home"; "far" is behind the camera, five parcels away
        let mut pointers = ScenePointers::default();
        pointers.insert(
            IVec2::ZERO,
            PointerResult::Exists {
                hash: "home".to_owned(),
                urn: None,
            },
        );
        world.insert_resource(pointers);
        world.init_resource::<PortableScenes>();
        world.init_resource::<LiveScenes>();
        let home = world
            .spawn((
                SceneHash("home".to_owned()),
                ScenePartition(ScenePartitionData::NEAREST),
            ))
            .id();
        world.spawn((
            SceneHash("far".to_owned()),
            ScenePartition(ScenePartitionData {
                sqr_distance: 6400.0,
                parcel_distance: 5,
                is_behind: true,
            }),
        ));
        world
            .resource_mut::<LiveScenes>()
            .scenes
            .insert("home".to_owned(), home);

        world.run_system_once(publish_download_priorities).unwrap();

        let ipfs = world.resource::<IpfsResource>();
        assert_eq!(ipfs.scene_priority("home"), Some(LoadPriority::TOP));
        assert_eq!(
            ipfs.scene_priority("far"),
            Some(LoadPriority {
                parcel_distance: 5,
                is_behind: true,
                sqr_distance: FloatOrd(6400.0),
            })
        );
        assert_eq!(ipfs.scene_priority("other"), None);
    }
}
