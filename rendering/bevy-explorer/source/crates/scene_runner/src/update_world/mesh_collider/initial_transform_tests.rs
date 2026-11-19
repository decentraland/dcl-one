use std::sync::Arc;

use bevy::{
    asset::io::memory::{Dir, MemoryAssetReader},
    gltf::{Gltf, GltfMesh},
    transform::TransformSystem,
};
use dcl_types::SceneId;
use ipfs::{IpfsIo, IpfsResource};
use scene_material::SceneBounds;

use super::*;
use crate::update_world::trigger_area::CtTrigger;

const CUSTOM1: u32 = 1 << 8;
const CUSTOM2: u32 = 1 << 9;

fn id(value: u16) -> ColliderId {
    ColliderId::new(
        SceneEntityId {
            id: value,
            generation: 0,
        },
        None,
        0,
    )
}

#[derive(Resource, Default)]
struct InputIntersections(Vec<(Vec<ColliderId>, Vec<ColliderId>)>);

fn inspect_input(
    mut scenes: Query<&mut SceneColliderData>,
    mut observations: ResMut<InputIntersections>,
) {
    let mut scene = scenes.single_mut().unwrap();
    // The fixture's negative trigger has a wrong-layer ball at its location and
    // a matching-layer ball in another column. Neither may cause an enter.
    observations.0.push((
        scene.intersect_id(&id(512), CUSTOM1),
        scene.intersect_id(&id(514), CUSTOM1),
    ));
}

fn spawn_in_scene_loop(
    mut commands: Commands,
    scenes: Query<Entity, With<SceneColliderData>>,
    mut spawned: Local<bool>,
) {
    if *spawned {
        return;
    }
    *spawned = true;
    let root = scenes.single().unwrap();
    for (value, x, mask) in [(512, 13.0, CUSTOM1), (514, 8.0, CUSTOM1)] {
        let entity = commands.spawn_empty().id();
        commands.entity(entity).insert((
            ContainerEntity {
                container: entity,
                root,
                container_id: id(value).entity,
            },
            Transform::from_xyz(x, 2.5, 11.0).with_scale(Vec3::splat(2.5)),
            MeshCollider::<CtTrigger> {
                collision_mask: mask,
                ..Default::default()
            },
        ));
    }
    for (value, x, mask) in [(513, 13.0, CUSTOM2), (515, 9.0, CUSTOM1)] {
        let entity = commands.spawn_empty().id();
        commands.entity(entity).insert((
            ContainerEntity {
                container: entity,
                root,
                container_id: id(value).entity,
            },
            Transform::from_xyz(x, 2.5, 11.0).with_scale(Vec3::splat(0.7)),
            MeshCollider::<CtCollider> {
                shape: MeshColliderShape::Sphere,
                collision_mask: mask,
                ..Default::default()
            },
        ));
    }
}

#[test]
fn first_input_uses_propagated_transforms_for_new_and_replaced_shapes() -> anyhow::Result<()> {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin))
        .init_asset::<Mesh>()
        .init_asset::<Gltf>()
        .init_asset::<GltfMesh>()
        .init_resource::<FrameCount>()
        .init_resource::<InputIntersections>()
        .insert_resource(IpfsResource {
            inner: Arc::new(IpfsIo::new(
                ipfs::http_client()?,
                false,
                Box::new(MemoryAssetReader {
                    root: Dir::default(),
                }),
                None,
                Default::default(),
                1,
                tokio::sync::mpsc::unbounded_channel().0,
            )),
        })
        .configure_sets(
            Update,
            (SceneSets::Init, SceneSets::Input, SceneSets::RunLoop).chain(),
        )
        .add_systems(
            Update,
            (
                update_colliders::<CtCollider>,
                update_colliders::<CtTrigger>,
            )
                .in_set(SceneSets::Init),
        )
        .add_systems(Update, inspect_input.in_set(SceneSets::Input))
        .add_systems(Update, spawn_in_scene_loop.in_set(SceneSets::RunLoop))
        .add_systems(
            PostUpdate,
            (
                update_collider_transforms::<CtCollider>,
                update_collider_transforms::<CtTrigger>,
            )
                .after(TransformSystem::TransformPropagate),
        );

    let root = app.world_mut().spawn_empty().id();
    app.world_mut().entity_mut(root).insert((
        SceneColliderData::default(),
        RendererSceneContext::new(
            SceneId(root),
            "hash".to_owned(),
            String::new(),
            false,
            0,
            String::new(),
            IVec2::ZERO,
            HashSet::default(),
            Vec::new(),
            SceneBounds::default(),
            Vec::new(),
            root,
            0.0,
            false,
            "",
            false,
            false,
        ),
    ));

    // Scene messages introduce Transform and collider components after Init/Input.
    // Let Bevy propagate GlobalTransform itself; supplying it by hand would hide
    // a registration-order bug. No HasCollider exists in this first PostUpdate.
    app.update();
    let mut transforms = app
        .world_mut()
        .query::<(&ContainerEntity, &GlobalTransform)>();
    let negative = transforms
        .iter(app.world())
        .find(|(container, _)| container.container_id == id(512).entity)
        .unwrap()
        .1;
    assert_eq!(negative.translation(), Vec3::new(13.0, 2.5, 11.0));
    app.update();

    // Replacing the shape with an unchanged Transform must also preserve its
    // position before Input, without relying on Changed<GlobalTransform>.
    let mut colliders = app.world_mut().query::<&mut MeshCollider<CtTrigger>>();
    for mut collider in colliders.iter_mut(app.world_mut()) {
        collider.shape = MeshColliderShape::Sphere;
    }
    app.update();

    let observations = &app.world().resource::<InputIntersections>().0;
    assert_eq!(observations.len(), 3);
    assert_eq!(observations[0], (Vec::new(), Vec::new()));
    for (negative, positive) in &observations[1..] {
        assert!(
            negative.is_empty(),
            "wrong-layer trigger received {negative:?}"
        );
        assert_eq!(positive, &[id(515)]);
    }
    Ok(())
}
