//! What each instance in range draws: its model near the camera, its baked impostor further out,
//! nothing beyond, at the reference's screen heights.
//!
//! An instance that draws something is one entity carrying its place and its colliders, with its
//! model or its impostor as a child; the child changes only when the instance's class changes,
//! and a culled instance has no entity at all, so the far landscape costs nothing but its entry
//! in the list. The classes are worked out again only when the camera has moved a step, its
//! field of view has changed, the instances in range have changed, or impostors are switched.

use bevy::{
    pbr::{NotShadowCaster, NotShadowReceiver},
    platform::collections::HashMap,
    prelude::*,
};
use boimp::Imposter;
use common::structs::{PrimaryCamera, PLAYER_CAMERA_FOV};

use super::{prototypes::PROTOTYPES, LandscapeTreeAssets, Tree};
use crate::{
    solids::{prototype_solids, Solid},
    terrain::TerrainSurface,
};

/// What an instance draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lod {
    Mesh,
    Impostor,
    Culled,
}

/// The reference's quality bias at its middle preset (its three are 0.75, 1 and 1.25): the screen
/// heights apply as they are.
pub(crate) const LOD_BIAS: f32 = 1.0;

/// How far the camera moves, in metres, before the classes are worked out again. At the nearest
/// switch (a small bush, about 30 m out) a step moves the switch by about 3%.
pub(crate) const LOD_STEP: f32 = 1.0;

/// The impostor's scales per prototype, evenly over its scale range: each has its own material,
/// since an impostor's size lives in its material. The nearest is at most 1/30 of the range off.
pub(crate) const IMPOSTOR_SCALES: usize = 16;

/// The class of a prototype's instance that covers `screen_height` of the view.
pub(crate) fn lod_for(prototype: usize, screen_height: f32) -> Lod {
    let lod = &PROTOTYPES[prototype].lod;
    if screen_height >= lod.mesh {
        Lod::Mesh
    } else if lod
        .impostor
        .is_some_and(|impostor| screen_height >= impostor)
    {
        Lod::Impostor
    } else {
        Lod::Culled
    }
}

/// The share of the view's height that a prototype's group covers at `scale`, `distance` metres
/// from a camera with the vertical field of view `fov` (radians), as the reference measures it.
pub(crate) fn screen_height(prototype: usize, scale: f32, distance: f32, fov: f32) -> f32 {
    let size = PROTOTYPES[prototype].lod.size * scale;
    size * 0.5 / (distance.max(1e-3) * (fov * 0.5).tan()) * LOD_BIAS
}

/// The impostor scale nearest to `scale`.
pub(crate) fn impostor_scale_step(prototype: usize, scale: f32) -> usize {
    let (low, high) = PROTOTYPES[prototype].scale_y;
    let last = (IMPOSTOR_SCALES - 1) as f32;
    ((scale - low) / (high - low) * last)
        .round()
        .clamp(0.0, last) as usize
}

pub(crate) fn impostor_step_scale(prototype: usize, step: usize) -> f32 {
    let (low, high) = PROTOTYPES[prototype].scale_y;
    low + (high - low) * step as f32 / (IMPOSTOR_SCALES - 1) as f32
}

/// The impostor of an instance at `scale`: its scale step, and the transform under the instance
/// that turns the unit cube it draws on into the model's bounds at the step's scale.
pub(crate) fn impostor_transform(prototype: usize, scale: f32) -> (usize, Transform) {
    let step = impostor_scale_step(prototype, scale);
    let relative = impostor_step_scale(prototype, step) / scale;
    let prototype = &PROTOTYPES[prototype];
    (
        step,
        Transform {
            translation: prototype.centre() * relative,
            rotation: Quat::IDENTITY,
            scale: prototype.size() * relative,
        },
    )
}

/// Whether instances past their model's range draw their impostor; without, they are culled.
#[derive(Resource)]
pub(crate) struct ImpostorsEnabled(pub bool);

impl Default for ImpostorsEnabled {
    fn default() -> Self {
        Self(true)
    }
}

/// What impostors draw with: the unit cube, and per prototype one material per scale step (none
/// for a prototype without an impostor).
#[derive(Resource, Default)]
pub(crate) struct ImpostorDraw {
    pub cube: Handle<Mesh>,
    pub materials: [Vec<Handle<Imposter>>; 8],
}

/// An instance of the layout in range on confirmed empty land.
struct Instance {
    parcel: IVec2,
    index: u32,
    prototype: usize,
    /// At its height on the terrain once it has drawn, at height 0 before.
    transform: Transform,
    lod: Lod,
    entity: Option<Entity>,
}

/// Parcels along a side of the square of land whose instances are worked out together.
const CELL_PARCELS: i32 = 8;

/// The instances of a square of land, and how far the camera may move from where they were last
/// worked out before one of them could change class (none: at the next pass).
#[derive(Default)]
struct Cell {
    instances: Vec<Instance>,
    checked: Option<(Vec3, f32)>,
}

/// The instances in range, whatever they draw, by square of land.
#[derive(Resource, Default)]
pub(crate) struct TreeInstances {
    cells: HashMap<IVec2, Cell>,
    /// Something changed that the classes have not followed yet.
    changed: bool,
}

impl TreeInstances {
    fn instances(&self) -> impl Iterator<Item = &Instance> {
        self.cells.values().flat_map(|cell| &cell.instances)
    }

    /// Instances per class: mesh, impostor, culled.
    pub(crate) fn counts(&self) -> [usize; 3] {
        let mut counts = [0; 3];
        for instance in self.instances() {
            counts[instance.lod as usize] += 1;
        }
        counts
    }

    /// Drop every instance and its entity.
    pub(crate) fn clear(&mut self, commands: &mut Commands) {
        for instance in self.cells.drain().flat_map(|(_, cell)| cell.instances) {
            if let Some(entity) = instance.entity {
                commands.entity(entity).despawn();
            }
        }
        self.changed = true;
    }

    /// Keep the instances `keep` takes, which it removes from its set, drop the others with their
    /// entities, and add what is left in `keep` as culled instances.
    pub(crate) fn replace(
        &mut self,
        commands: &mut Commands,
        mut keep: HashMap<(IVec2, u32), (usize, Transform)>,
    ) {
        self.cells.retain(|_, cell| {
            cell.instances.retain(|instance| {
                let kept = keep.remove(&(instance.parcel, instance.index)).is_some();
                if !kept {
                    if let Some(entity) = instance.entity {
                        commands.entity(entity).despawn();
                    }
                }
                kept
            });
            !cell.instances.is_empty()
        });
        for ((parcel, index), (prototype, transform)) in keep {
            let cell = self
                .cells
                .entry(parcel.div_euclid(IVec2::splat(CELL_PARCELS)))
                .or_default();
            cell.instances.push(Instance {
                parcel,
                index,
                prototype,
                transform,
                lod: Lod::Culled,
                entity: None,
            });
            cell.checked = None;
        }
        self.changed = true;
    }

    /// Set the height of the drawn instances `resnap` picks by their parcel.
    pub(crate) fn resnap(
        &mut self,
        surface: &TerrainSurface,
        resnap: impl Fn(IVec2) -> bool,
        mut place: impl FnMut(Entity, f32),
    ) {
        let instances = self.cells.values_mut().flat_map(|cell| &mut cell.instances);
        for instance in instances {
            let Some(entity) = instance.entity else {
                continue;
            };
            if !resnap(instance.parcel) {
                continue;
            }
            let translation = &mut instance.transform.translation;
            let y = surface.height(translation.x, -translation.z);
            if translation.y != y {
                translation.y = y;
                place(entity, y);
            }
        }
    }
}

/// How far past a switch, as a share of its distance, an instance may be before it switches:
/// a far instance is worked out again only once the camera has moved this share of its
/// distance, or as far as its nearest switch.
pub(crate) const LOD_TOLERANCE: f32 = 0.01;

#[derive(Default)]
pub(crate) struct LodState {
    camera: Option<Vec3>,
    fov: f32,
    enabled: bool,
}

/// Work out the classes of the instances whose class may have changed, when something they
/// depend on has changed, and change what those whose class changed draw.
pub(crate) fn update_lods(
    mut commands: Commands,
    mut instances: ResMut<TreeInstances>,
    camera: Query<(&GlobalTransform, Option<&Projection>), With<PrimaryCamera>>,
    (enabled, draw, assets, surface): (
        Res<ImpostorsEnabled>,
        Res<ImpostorDraw>,
        Res<LandscapeTreeAssets>,
        Res<TerrainSurface>,
    ),
    mut state: Local<LodState>,
) {
    let Ok((camera, projection)) = camera.single() else {
        return;
    };
    let position = camera.translation();
    if !position.is_finite() {
        return;
    }
    let fov = match projection {
        Some(Projection::Perspective(perspective)) => perspective.fov,
        _ => PLAYER_CAMERA_FOV,
    };
    let moved = state
        .camera
        .is_none_or(|last| last.distance_squared(position) >= LOD_STEP * LOD_STEP);
    let switches_moved = fov != state.fov || enabled.0 != state.enabled;
    if !(moved || instances.changed || switches_moved) {
        return;
    }
    *state = LodState {
        camera: Some(position),
        fov,
        enabled: enabled.0,
    };
    instances.changed = false;

    // the distances at which each prototype at scale 1 leaves its model, and draws nothing
    let switch = |prototype: usize, height: f32| {
        PROTOTYPES[prototype].lod.size * 0.5 * LOD_BIAS / (height * (fov * 0.5).tan())
    };
    let switches: [(f32, f32); 8] = std::array::from_fn(|prototype| {
        let lod = &PROTOTYPES[prototype].lod;
        let mesh = switch(prototype, lod.mesh);
        let far = match lod.impostor {
            Some(impostor) if enabled.0 => switch(prototype, impostor),
            _ => mesh,
        };
        (mesh, far)
    });

    for cell in instances.cells.values_mut() {
        if let Some((at, slack)) = cell.checked {
            if !switches_moved && at.distance_squared(position) < slack * slack {
                continue;
            }
        }
        let mut slack = f32::INFINITY;
        for instance in &mut cell.instances {
            let prototype = &PROTOTYPES[instance.prototype];
            let scale = instance.transform.scale.y;
            let reference = instance.transform.transform_point(prototype.lod.reference);
            let distance = reference.distance(position);
            let (mesh, far) = switches[instance.prototype];
            let nearest = (distance - mesh * scale)
                .abs()
                .min((distance - far * scale).abs());
            slack = slack.min(nearest.max(distance * LOD_TOLERANCE));
            let height = screen_height(instance.prototype, scale, distance, fov);
            let mut lod = lod_for(instance.prototype, height);
            if lod == Lod::Impostor && !enabled.0 {
                lod = Lod::Culled;
            }
            if lod != instance.lod {
                instance.lod = lod;
                switch_lod(&mut commands, instance, &draw, &assets, &surface);
            }
        }
        // whatever the slack, the camera moves a step before anything is worked out again
        cell.checked = Some((position, slack.max(LOD_STEP)));
    }
}

/// Change what an instance draws to its class.
fn switch_lod(
    commands: &mut Commands,
    instance: &mut Instance,
    draw: &ImpostorDraw,
    assets: &LandscapeTreeAssets,
    surface: &TerrainSurface,
) {
    let prototype = &PROTOTYPES[instance.prototype];
    let entity = match (instance.entity, instance.lod) {
        (Some(entity), Lod::Culled) => {
            commands.entity(entity).despawn();
            instance.entity = None;
            return;
        }
        (None, Lod::Culled) => return,
        (Some(entity), _) => {
            commands.entity(entity).despawn_related::<Children>();
            entity
        }
        (None, _) => {
            let translation = &mut instance.transform.translation;
            translation.y = surface.height(translation.x, -translation.z);
            let mut root = commands.spawn((
                Name::new(prototype.name),
                Tree {
                    parcel: instance.parcel,
                    index: instance.index,
                },
                instance.transform,
                Visibility::default(),
            ));
            let solids = prototype_solids(instance.prototype);
            if !solids.is_empty() {
                root.insert(Solid(solids));
            }
            let entity = root.id();
            instance.entity = Some(entity);
            entity
        }
    };
    if instance.lod == Lod::Mesh {
        commands
            .entity(entity)
            .with_child(SceneRoot(assets.scenes[instance.prototype].clone()));
    } else {
        let scale = instance.transform.scale.y;
        let (step, transform) = impostor_transform(instance.prototype, scale);
        let material = draw.materials[instance.prototype]
            .get(step)
            .cloned()
            .unwrap_or_default();
        // boimp's impostor shader has no shadow pass, and the scene impostors receive none either
        commands.entity(entity).with_child((
            Mesh3d(draw.cube.clone()),
            MeshMaterial3d(material),
            transform,
            NotShadowCaster,
            NotShadowReceiver,
        ));
    }
}
