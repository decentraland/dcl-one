//! Colliders for solid landscape props (trees, bush trunks, stones) over the ground collider's
//! patch, rebuilt with it.

use std::sync::LazyLock;

use bevy::{platform::collections::HashMap, prelude::*};
use common::sets::PostUpdateSets;
use dcl_component::SceneEntityId;
use scene_runner::update_world::mesh_collider::{ColliderId, SceneColliderData};

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
#[derive(Component, Clone, Copy)]
pub(crate) struct Solid(pub &'static [Capsule]);

/// A collider of a prototype's collider prefab as the reference stores it, in its frame (x east,
/// z north): the collider's object's local position, rotation (x, y, z, w) and scale, then the
/// collider in that object's space.
struct PrefabCollider {
    position: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
    shape: PrefabShape,
}

enum PrefabShape {
    /// `height` runs end to end, caps included, along local axis `direction` (0 x, 1 y, 2 z).
    Capsule {
        center: [f32; 3],
        radius: f32,
        height: f32,
        direction: usize,
    },
    Sphere {
        radius: f32,
    },
}

/// A capsule along its object's x axis, unscaled.
const fn capsule(
    position: [f32; 3],
    rotation: [f32; 4],
    radius: f32,
    height: f32,
) -> PrefabCollider {
    PrefabCollider {
        position,
        rotation,
        scale: [1.0; 3],
        shape: PrefabShape::Capsule {
            center: [0.0; 3],
            radius,
            height,
            direction: 0,
        },
    }
}

const fn sphere(position: [f32; 3], rotation: [f32; 4], radius: f32) -> PrefabCollider {
    PrefabCollider {
        position,
        rotation,
        scale: [1.0; 3],
        shape: PrefabShape::Sphere { radius },
    }
}

/// The stones' collider: an upright capsule of radius 0.5 and height 2, stretched by `scale`.
const fn stone(position: [f32; 3], scale: [f32; 3]) -> PrefabCollider {
    PrefabCollider {
        position,
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale,
        shape: PrefabShape::Capsule {
            center: [0.000000059604645, 0.0, -0.00000008940697],
            radius: 0.5000001,
            height: 2.0,
            direction: 1,
        },
    }
}

/// By prototype, in the layout's order. Bush01 has no collider.
// the numbers as the prefabs store them, quarter turns included
#[allow(clippy::approx_constant)]
const PREFABS: [&[PrefabCollider]; 8] = [
    &[],
    &[capsule(
        [0.006637088, 0.53919345, -0.0029117058],
        [0.000000026692538, 0.0000000041955577, 0.7071068, 0.7071068],
        0.054864246,
        1.1490948,
    )],
    &[stone([0.0, 0.0, 0.0], [1.71718, 1.01659, 1.7924199])],
    &[stone([0.0, 0.0, 0.011], [1.2314928, 0.96884936, 1.2854519])],
    &[stone([0.0, 0.0, 0.011], [1.5168672, 0.94172555, 1.0701506])],
    &[
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
    ],
    &[
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
    ],
    &[
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
    ],
];

impl PrefabCollider {
    /// The collider as a capsule in the prefab root's space, with z negated into bevy's axes.
    /// A scaled capsule takes its axis's scale on its height and the larger other scale on its
    /// radius; a scaled sphere takes the largest scale. A capsule shorter than its diameter is a
    /// sphere.
    fn capsule(&self) -> Capsule {
        let rotation = Quat::from_array(self.rotation);
        let position = Vec3::from_array(self.position);
        let scale = Vec3::from_array(self.scale).abs();
        let (center, radius, half) = match self.shape {
            PrefabShape::Capsule {
                center,
                radius,
                height,
                direction,
            } => {
                let across = (0..3)
                    .filter(|axis| *axis != direction)
                    .map(|axis| scale[axis])
                    .fold(0.0, f32::max);
                let radius = radius * across;
                let mut half = Vec3::ZERO;
                half[direction] = (height * scale[direction] * 0.5 - radius).max(0.0);
                (Vec3::from_array(center) * scale, radius, half)
            }
            PrefabShape::Sphere { radius } => {
                (Vec3::ZERO, radius * scale.max_element(), Vec3::ZERO)
            }
        };
        let to_bevy = |point: Vec3| {
            let point = position + rotation * point;
            Vec3::new(point.x, point.y, -point.z)
        };
        Capsule {
            base: to_bevy(center - half),
            top: to_bevy(center + half),
            radius,
        }
    }
}

static PROTOTYPE_SOLIDS: LazyLock<[Vec<Capsule>; 8]> = LazyLock::new(|| {
    PREFABS.map(|colliders| colliders.iter().map(PrefabCollider::capsule).collect())
});

/// The colliders of a layout prototype, in its local space.
pub(crate) fn prototype_solids(prototype: usize) -> &'static [Capsule] {
    &PROTOTYPE_SOLIDS[prototype]
}

fn collider_id(entity: Entity, index: usize) -> ColliderId {
    ColliderId::new(
        SceneEntityId::ROOT,
        Some(format!("landscape {entity}")),
        index as u32,
    )
}

/// Colliders for the solids over the ground collider's patch, rebuilt when the patch changes or
/// solids move.
fn update_colliders(
    collider: Single<(Ref<TerrainCollider>, &mut SceneColliderData)>,
    solids: Query<(Entity, &Solid, &Transform)>,
    moved: Query<(), (With<Solid>, Changed<Transform>)>,
    mut removed: RemovedComponents<Solid>,
    mut built: Local<HashMap<Entity, usize>>,
) {
    let (patch, mut data) = collider.into_inner();
    let remove = |data: &mut SceneColliderData, entity: Entity, count: usize| {
        for index in 0..count {
            data.remove_collider(&collider_id(entity, index));
        }
    };
    for entity in removed.read() {
        if let Some(count) = built.remove(&entity) {
            remove(&mut data, entity, count);
        }
    }
    if !patch.is_changed() && moved.is_empty() {
        return;
    }
    let Some((min, max)) = patch.patch() else {
        return;
    };
    let mut current = HashMap::new();
    for (entity, Solid(capsules), transform) in &solids {
        let position = transform.translation.xz();
        if position.cmplt(min).any() || position.cmpgt(max).any() {
            continue;
        }
        for (index, capsule) in capsules.iter().enumerate() {
            data.set_physics_capsule(
                &collider_id(entity, index),
                transform.transform_point(capsule.base),
                transform.transform_point(capsule.top),
                capsule.radius * transform.scale.x,
            );
        }
        current.insert(entity, capsules.len());
    }
    for (entity, count) in built.iter() {
        if !current.contains_key(entity) {
            remove(&mut data, *entity, *count);
        }
    }
    *built = current;
}
