//! Colliders for solid landscape props (trees, bush trunks, stones) over the ground collider's
//! patch, rebuilt with it.

use std::sync::LazyLock;

use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::*,
};
use common::sets::PostUpdateSets;
use dcl_component::SceneEntityId;
use scene_runner::update_world::{
    gltf_container::mesh_to_parry_shape,
    mesh_collider::{ColliderId, SceneColliderData, SharedShape},
};

use crate::terrain::{update_collider, TerrainCollider};

pub(crate) struct SolidsPlugin;

impl Plugin for SolidsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            update_colliders
                .in_set(PostUpdateSets::ColliderUpdate)
                .after(update_collider),
        );
    }
}

/// A capsule in an entity's local space; a sphere where its ends meet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Capsule {
    pub base: Vec3,
    pub top: Vec3,
    pub radius: f32,
}

/// The entity's collision shapes, in its local space.
#[derive(Component, Clone)]
pub(crate) enum Solid {
    /// Capsules and spheres (the trees and bushes).
    Capsules(&'static [Capsule]),
    /// The mesh's own triangles at the entity's scale (the stones), so the collider follows the
    /// rock's lumps rather than a capsule around them.
    Mesh(Handle<Mesh>),
}

/// A collider of a prototype's collider prefab as stored in its frame (x east, z north): the
/// collider's object's local position and rotation (x, y, z, w), then the collider in that
/// object's space.
struct PrefabCollider {
    position: [f32; 3],
    rotation: [f32; 4],
    shape: PrefabShape,
}

enum PrefabShape {
    /// Centred on its object, `height` running end to end, caps included, along local x.
    Capsule {
        radius: f32,
        height: f32,
    },
    Sphere {
        radius: f32,
    },
}

const fn capsule(
    position: [f32; 3],
    rotation: [f32; 4],
    radius: f32,
    height: f32,
) -> PrefabCollider {
    PrefabCollider {
        position,
        rotation,
        shape: PrefabShape::Capsule { radius, height },
    }
}

const fn sphere(position: [f32; 3], rotation: [f32; 4], radius: f32) -> PrefabCollider {
    PrefabCollider {
        position,
        rotation,
        shape: PrefabShape::Sphere { radius },
    }
}

/// How a prototype collides.
enum PrefabSolid {
    /// Its prefab's colliders; none for a prototype the player walks through.
    Colliders(&'static [PrefabCollider]),
    /// Its model's mesh.
    ModelMesh,
}

use PrefabSolid::{Colliders, ModelMesh};

/// By prototype, in the layout's order. Bush01 has no collider; the stones collide with their mesh.
// the numbers as the prefabs store them, quarter turns included
#[allow(clippy::approx_constant)]
const PREFABS: [PrefabSolid; 8] = [
    Colliders(&[]),
    Colliders(&[capsule(
        [0.006637088, 0.53919345, -0.0029117058],
        [0.000000026692538, 0.0000000041955577, 0.7071068, 0.7071068],
        0.054864246,
        1.1490948,
    )]),
    ModelMesh,
    ModelMesh,
    ModelMesh,
    Colliders(&[
        capsule(
            [0.0, 0.7581423, -0.035903018],
            [0.000000026692538, 0.000000004652657, 0.7071068, 0.7071068],
            0.4907274,
            2.7462466,
        ),
        capsule(
            [0.3414444, 2.7732556, 0.46374393],
            [0.000000030977358, -0.28700593, 0.49418452, 0.8206152],
            0.4175765,
            2.804162,
        ),
        capsule(
            [0.88153124, 4.960762, 0.96845853],
            [0.000000027804244, 0.033656098, 0.6755378, 0.7365568],
            0.3718561,
            2.7371037,
        ),
        sphere(
            [1.5728973, 9.667379, 1.2116734],
            [0.000000037748947, 0.0, 0.0, 1.0],
            5.3309493,
        ),
    ]),
    Colliders(&[
        capsule(
            [-0.040914174, 0.71437424, 0.017361246],
            [0.000000026692538, 0.000000004419805, 0.7071068, 0.7071068],
            0.5059674,
            2.535935,
        ),
        capsule(
            [0.5023511, 2.990413, -0.39822882],
            [0.00000003222979, 0.21263117, 0.47521088, 0.8537931],
            0.45719942,
            3.2217376,
        ),
        capsule(
            [1.0252383, 5.3001704, -1.2380418],
            [0.00000002488314, 0.16919187, 0.73270947, 0.65917444],
            0.3657592,
            2.7736797,
        ),
        capsule(
            [1.2450789, 7.248599, -2.2465386],
            [0.000000030054675, 0.3733784, 0.47612807, 0.79617256],
            0.37795207,
            2.4719284,
        ),
        sphere(
            [-0.4718325, 10.716929, -2.571674],
            [0.000000037748947, 0.0, 0.0, 1.0],
            5.3004665,
        ),
    ]),
    Colliders(&[
        capsule(
            [0.10291743, 2.5611644, 0.012515802],
            [0.00000002562954, -0.040001597, 0.7330966, 0.67894715],
            0.9814551,
            5.733285,
        ),
        capsule(
            [-0.8125392, 7.073642, -0.06717157],
            [0.0000000224114, 0.07989236, 0.8007136, 0.59369606],
            0.7772397,
            5.452872,
        ),
        capsule(
            [-0.9649907, 11.068095, -0.47577733],
            [0.000000030103948, 0.014946828, 0.60316306, 0.79747784],
            0.704088,
            4.392168,
        ),
        sphere(
            [-0.9900413, 16.53677, -0.47134134],
            [0.000000037748947, 0.0, 0.0, 1.0],
            7.2481403,
        ),
        sphere(
            [-0.98756313, 26.99674, -0.5728386],
            [0.000000037748947, 0.0, 0.0, 1.0],
            6.138674,
        ),
        capsule(
            [-2.0075727, 0.3675718, 2.3884017],
            [0.000000034558358, 0.39080316, 0.095768966, 0.9154787],
            0.35966468,
            4.1970997,
        ),
        capsule(
            [-0.9149767, 0.2277594, -2.155398],
            [0.00000003221855, -0.49287516, 0.16917358, 0.85349536],
            0.37185642,
            3.7520905,
        ),
        capsule(
            [2.8385353, -0.34983635, 0.13175736],
            [0.000000008166382, -0.024431711, 0.97601366, 0.21633404],
            0.36880827,
            4.86461,
        ),
    ]),
];

impl PrefabCollider {
    /// The collider as a capsule in the prefab root's space, with z negated into bevy's axes. A
    /// capsule shorter than its diameter is a sphere.
    fn capsule(&self) -> Capsule {
        let rotation = Quat::from_array(self.rotation);
        let position = Vec3::from_array(self.position);
        let (radius, half) = match self.shape {
            PrefabShape::Capsule { radius, height } => {
                (radius, Vec3::X * (height * 0.5 - radius).max(0.0))
            }
            PrefabShape::Sphere { radius } => (radius, Vec3::ZERO),
        };
        let to_bevy = |point: Vec3| {
            let point = position + rotation * point;
            Vec3::new(point.x, point.y, -point.z)
        };
        Capsule {
            base: to_bevy(-half),
            top: to_bevy(half),
            radius,
        }
    }
}

static PROTOTYPE_CAPSULES: LazyLock<[Vec<Capsule>; 8]> = LazyLock::new(|| {
    PREFABS.map(|solid| match solid {
        Colliders(colliders) => colliders.iter().map(PrefabCollider::capsule).collect(),
        ModelMesh => Vec::new(),
    })
});

/// The capsules of a layout prototype, in its local space; none for a stone.
pub(crate) fn prototype_capsules(prototype: usize) -> &'static [Capsule] {
    &PROTOTYPE_CAPSULES[prototype]
}

/// A layout prototype's solid, `mesh` being its model's mesh; None when it has no collider.
pub(crate) fn prototype_solid(prototype: usize, mesh: &Handle<Mesh>) -> Option<Solid> {
    match &PREFABS[prototype] {
        Colliders([]) => None,
        Colliders(_) => Some(Solid::Capsules(prototype_capsules(prototype))),
        ModelMesh => Some(Solid::Mesh(mesh.clone())),
    }
}

/// The most colliders an entity holds, so its collider ids run from 0 to below this.
const MAX_SOLIDS: usize = {
    let mut max = 1;
    let mut index = 0;
    while index < PREFABS.len() {
        if let Colliders(colliders) = &PREFABS[index] {
            if colliders.len() > max {
                max = colliders.len();
            }
        }
        index += 1;
    }
    max
};

/// The step a stone's scale is rounded to for its collider, so instances of one stone at close
/// scales share one shape: within 2.5% of the drawn scale.
const SHAPE_SCALE_STEP: f32 = 0.05;

fn collider_id(entity: Entity, index: usize) -> ColliderId {
    ColliderId::new(
        SceneEntityId::ROOT,
        Some(format!("landscape {entity}")),
        index as u32,
    )
}

fn remove_colliders(data: &mut SceneColliderData, entity: Entity) {
    for index in 0..MAX_SOLIDS {
        data.remove_collider(&collider_id(entity, index));
    }
}

/// The stones' collider shapes by mesh and rounded scale.
type MeshShapes = HashMap<(AssetId<Mesh>, IVec3), SharedShape>;

/// Sets an entity's colliders; false when its mesh has not loaded yet.
fn build_colliders(
    data: &mut SceneColliderData,
    entity: Entity,
    solid: &Solid,
    transform: &Transform,
    meshes: &Assets<Mesh>,
    shapes: &mut MeshShapes,
) -> bool {
    match solid {
        Solid::Capsules(capsules) => {
            for (index, capsule) in capsules.iter().enumerate() {
                data.set_physics_capsule(
                    &collider_id(entity, index),
                    transform.transform_point(capsule.base),
                    transform.transform_point(capsule.top),
                    capsule.radius * transform.scale.x,
                );
            }
            true
        }
        Solid::Mesh(mesh) => {
            let steps = (transform.scale / SHAPE_SCALE_STEP).round().as_ivec3();
            let key = (mesh.id(), steps);
            let shape = match shapes.get(&key) {
                Some(shape) => shape.clone(),
                None => {
                    let Some(model) = meshes.get(mesh) else {
                        return false;
                    };
                    let scaled = model.clone().scaled_by(steps.as_vec3() * SHAPE_SCALE_STEP);
                    let Some(shape) = mesh_to_parry_shape(&scaled) else {
                        warn!("landscape stone mesh has no triangles; no collider");
                        return true;
                    };
                    shapes.insert(key, shape.clone());
                    shape
                }
            };
            data.set_physics_shape(
                &collider_id(entity, 0),
                shape,
                transform.translation,
                transform.rotation,
            );
            true
        }
    }
}

/// Colliders for the solids over the ground collider's patch: all of them rebuilt when the
/// patch moves, else only the solids that spawned or moved, and those whose mesh was still
/// loading.
#[allow(clippy::too_many_arguments)]
fn update_colliders(
    collider: Single<(Ref<TerrainCollider>, &mut SceneColliderData)>,
    solids: Query<(Entity, &Solid, &Transform)>,
    moved: Query<Entity, (With<Solid>, Changed<Transform>)>,
    mut removed: RemovedComponents<Solid>,
    meshes: Res<Assets<Mesh>>,
    mut built: Local<HashSet<Entity>>,
    mut waiting: Local<HashSet<Entity>>,
    mut shapes: Local<MeshShapes>,
) {
    let (patch, mut data) = collider.into_inner();
    for entity in removed.read() {
        waiting.remove(&entity);
        if built.remove(&entity) {
            remove_colliders(&mut data, entity);
        }
    }
    let Some((min, max)) = patch.patch() else {
        return;
    };
    let rebuild_all = patch.is_changed();
    if !rebuild_all && moved.is_empty() && waiting.is_empty() {
        return;
    }
    // the solids that spawned or moved, and those still waiting for their mesh
    let changed: Vec<Entity> = if rebuild_all {
        Vec::new()
    } else {
        moved
            .iter()
            .chain(waiting.iter().copied())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect()
    };
    let mut update = |entity: Entity, solid: &Solid, transform: &Transform| {
        let position = transform.translation.xz();
        if position.cmplt(min).any() || position.cmpgt(max).any() {
            waiting.remove(&entity);
            if built.remove(&entity) {
                remove_colliders(&mut data, entity);
            }
            return;
        }
        if build_colliders(&mut data, entity, solid, transform, &meshes, &mut shapes) {
            waiting.remove(&entity);
            built.insert(entity);
        } else {
            waiting.insert(entity);
        }
    };
    if rebuild_all {
        for (entity, solid, transform) in &solids {
            update(entity, solid, transform);
        }
    } else {
        for entity in changed {
            if let Ok((entity, solid, transform)) = solids.get(entity) {
                update(entity, solid, transform);
            }
        }
    }
}
