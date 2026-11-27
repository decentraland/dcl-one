use bevy::prelude::*;
use common::{sets::SceneSets, structs::MonotonicTimestamp};
use dcl_component::{
    proto_components::{
        common::Vector3,
        sdk::components::{
            pb_trigger_area_result::Trigger, ColliderLayer, PbTriggerArea, PbTriggerAreaResult,
            TriggerAreaEventType, TriggerAreaMeshType,
        },
    },
    SceneComponentId,
};
use dcl_types::interface::{ComponentPosition, CrdtType};

use crate::{
    renderer_context::RendererSceneContext,
    update_scene::pointer_results::{AvatarColliders, PointerRay},
    update_world::{
        mesh_collider::{
            add_collider_systems, ColliderId, ColliderType, HasCollider, MeshCollider,
            MeshColliderShape, SceneColliderData, RAYCAST_EPSILON,
        },
        AddCrdtInterfaceExt,
    },
    ContainerEntity,
};

#[derive(Clone)]
pub struct CtTrigger;
impl ColliderType for CtTrigger {
    fn is_trigger() -> bool {
        true
    }

    fn primitive_debug_color() -> Color {
        Color::srgba(1.0, 0.0, 1.0, 0.2)
    }

    fn gltf_debug_color() -> Color {
        Color::srgba(0.0, 1.0, 1.0, 0.2)
    }
}

impl Default for MeshCollider<CtTrigger> {
    fn default() -> Self {
        Self {
            shape: MeshColliderShape::Box,
            collision_mask: ColliderLayer::ClPlayer as u32,
            mesh_name: Default::default(),
            index: Default::default(),
            _p: Default::default(),
        }
    }
}

impl From<PbTriggerArea> for MeshCollider<CtTrigger> {
    fn from(value: PbTriggerArea) -> Self {
        let shape = match value.mesh() {
            TriggerAreaMeshType::TamtBox => MeshColliderShape::Box,
            TriggerAreaMeshType::TamtSphere => MeshColliderShape::Sphere,
        };

        Self {
            shape,
            collision_mask: value
                .collision_mask
                .unwrap_or(ColliderLayer::ClPlayer as u32),
            ..Default::default()
        }
    }
}

pub struct TriggerAreaPlugin;

impl Plugin for TriggerAreaPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MonotonicTimestamp<PbTriggerAreaResult>>();

        app.add_crdt_lww_component::<PbTriggerArea, MeshCollider<CtTrigger>>(
            SceneComponentId::TRIGGER_AREA,
            ComponentPosition::EntityOnly,
        );

        add_collider_systems::<CtTrigger>(app);

        app.add_systems(Update, (update_triggers).chain().in_set(SceneSets::Input));
    }
}

/// The overlaps a trigger area reported last and the scene tick it reported them at: a stay goes
/// out once per scene tick, an enter or exit when the overlaps change. `inputs` is what the
/// overlaps were computed from.
#[derive(Component, Default)]
pub struct ActiveTriggers {
    scene: Vec<ColliderId>,
    avatars: Vec<ColliderId>,
    pointer: Vec<ColliderId>,
    tick: u32,
    inputs: Option<OverlapInputs>,
}

/// Everything a trigger area's overlap queries read besides its own definition.
#[derive(Clone, Copy, PartialEq)]
struct OverlapInputs {
    scene: Entity,
    collision_mask: u32,
    /// [`SceneColliderData::layers_version`] of the scene's colliders on the trigger's layers,
    /// which include the trigger's own collider
    scene_layers: u64,
    /// the same for the avatar colliders, 0 when the trigger ignores players
    avatar_layers: u64,
    /// the pointer ray, `None` when the trigger ignores the pointer
    pointer_ray: Option<Ray3d>,
}

/// Where the overlaps `update_triggers` reports came from.
#[derive(Clone, Copy)]
enum OverlapSource {
    /// the scene's own colliders
    Scene,
    /// the avatar colliders
    Avatar,
    /// the pointer ray, reported with a placeholder id
    Pointer,
}

/// What `update_triggers` reports one overlap kind with, for one trigger area this frame.
struct TriggerReport<'a, 'w, 's> {
    transforms: &'a Query<'w, 's, &'static GlobalTransform>,
    container: &'a ContainerEntity,
    /// the area's collision mask, which the reported layers are limited to
    collision_mask: u32,
    /// the world position of the scene's origin, which reported positions are relative to
    scene_origin: Vec3,
    /// the area's position, relative to the scene
    translation: Vec3,
    rotation: Quat,
    timestamp: &'a MonotonicTimestamp<PbTriggerAreaResult>,
    tick: u32,
}

impl TriggerReport<'_, '_, '_> {
    /// The entity that entered, stayed or left: its collider's layers that the area's mask holds,
    /// and the transform of the entity that holds the collider, its position relative to the
    /// scene. For a scene collider that entity is the scene entity, which for a glTF collider is
    /// the container rather than the mesh node; for an avatar collider it is the avatar. The
    /// pointer reports neither.
    ///
    /// By an exit the collider can be gone: its layers are then 0, and an avatar's transform is
    /// empty, while a scene entity that outlives its collider still reports its transform.
    fn trigger(
        &self,
        scene: &RendererSceneContext,
        colliders: &SceneColliderData,
        source: OverlapSource,
        collider_id: &ColliderId,
    ) -> Trigger {
        let layers = || {
            colliders.get_collider(collider_id).map_or(0, |collider| {
                collider.collision_groups().memberships.bits() & self.collision_mask
            })
        };
        let (layers, holder) = match source {
            OverlapSource::Scene => (layers(), scene.bevy_entity(collider_id.entity)),
            OverlapSource::Avatar => (layers(), colliders.get_collider_entity(collider_id)),
            OverlapSource::Pointer => (0, None),
        };
        let transform = holder
            .and_then(|entity| self.transforms.get(entity).ok())
            .map(GlobalTransform::to_scale_rotation_translation);
        Trigger {
            entity: collider_id.entity.as_proto_u32().unwrap(),
            layers,
            position: transform
                .map(|(_, _, t)| Vector3::world_vec_from_vec3(&(t - self.scene_origin))),
            rotation: transform.map(|(_, r, _)| r.into()),
            scale: transform.map(|(s, _, _)| Vector3::abs_vec_from_vec3(&s)),
        }
    }

    fn send(
        &self,
        scene: &mut Mut<RendererSceneContext>,
        colliders: &SceneColliderData,
        source: OverlapSource,
        collider_id: &ColliderId,
        ty: TriggerAreaEventType,
    ) {
        let result = PbTriggerAreaResult {
            triggered_entity: self.container.container_id.as_proto_u32().unwrap(),
            triggered_entity_position: Some(Vector3::world_vec_from_vec3(&self.translation)),
            triggered_entity_rotation: Some(self.rotation.into()),
            event_type: ty as i32,
            timestamp: self.timestamp.next_timestamp(),
            trigger: Some(self.trigger(scene, colliders, source, collider_id)),
        };
        scene.update_crdt(
            SceneComponentId::TRIGGER_AREA_RESULT,
            CrdtType::GO_ENT,
            self.container.container_id,
            &result,
        );
    }

    /// Exits and stays for `previous` (reported at `previous_tick`), then enters, against `current`.
    /// `colliders` hold the ids `source` reports, see [`Self::trigger`].
    fn events(
        &self,
        scene: &mut Mut<RendererSceneContext>,
        colliders: &SceneColliderData,
        source: OverlapSource,
        previous: &[ColliderId],
        previous_tick: u32,
        current: &[ColliderId],
    ) {
        for prev_collider in previous {
            if current.contains(prev_collider) {
                if previous_tick != self.tick {
                    // send only 1 stay per scene tick
                    self.send(
                        scene,
                        colliders,
                        source,
                        prev_collider,
                        TriggerAreaEventType::TaetStay,
                    );
                }
            } else {
                self.send(
                    scene,
                    colliders,
                    source,
                    prev_collider,
                    TriggerAreaEventType::TaetExit,
                );
            }
        }

        for new_collider in current {
            if !previous.contains(new_collider) {
                self.send(
                    scene,
                    colliders,
                    source,
                    new_collider,
                    TriggerAreaEventType::TaetEnter,
                );
            }
        }
    }
}

/// Reports each trigger area's enters, exits and stays to its scene.
///
/// A trigger area's overlaps are a function of its collider and collision mask, the scene's
/// colliders on its mask's layers, the avatar colliders on those layers and the pointer ray. Every
/// change to a collider (added, removed, moved, reshaped, switched on or off) goes through
/// `SceneColliderData`, which stamps the layers of the collider it changed, and a trigger's
/// collider is on its own mask's layers. So while the stamps for the mask, the mask, the collider
/// and the ray hold, the queries would return what they returned last time: the overlaps are
/// reused, and an area whose scene has not ticked since has nothing to report.
pub(super) fn update_triggers(
    mut commands: Commands,
    mut trigger_areas: Query<(
        Entity,
        &ContainerEntity,
        Ref<MeshCollider<CtTrigger>>,
        Ref<HasCollider<CtTrigger>>,
        &GlobalTransform,
        Option<&mut ActiveTriggers>,
    )>,
    mut scenes: Query<(
        &mut RendererSceneContext,
        &mut SceneColliderData,
        &GlobalTransform,
    )>,
    mut avatar_colliders: ResMut<AvatarColliders>,
    transforms: Query<&'static GlobalTransform>,
    pointer_ray: Res<PointerRay>,
    timestamp: Res<MonotonicTimestamp<PbTriggerAreaResult>>,
) {
    const PLAYER_LAYERS: u32 = ColliderLayer::ClPlayer as u32 | ColliderLayer::ClMainPlayer as u32;
    const POINTER_LAYER: u32 = ColliderLayer::ClPointer as u32;
    let no_triggers = ActiveTriggers::default();

    for (entity, container, trigger_def, collider, gt, mut maybe_active) in trigger_areas.iter_mut()
    {
        let Ok((mut scene, mut colliders, scene_transform)) = scenes.get_mut(container.root) else {
            continue;
        };

        let mask = trigger_def.collision_mask;
        let inputs = OverlapInputs {
            scene: container.root,
            collision_mask: mask,
            scene_layers: colliders.layers_version(mask),
            avatar_layers: if mask & PLAYER_LAYERS != 0 {
                avatar_colliders.collider_data.layers_version(mask)
            } else {
                0
            },
            pointer_ray: pointer_ray.0.filter(|_| mask & POINTER_LAYER != 0),
        };
        let previous = maybe_active.as_deref().unwrap_or(&no_triggers);
        let known = previous
            .inputs
            .filter(|_| !trigger_def.is_changed() && !collider.is_changed());
        let scene_known = known.is_some_and(|known| {
            (known.scene, known.collision_mask, known.scene_layers)
                == (inputs.scene, inputs.collision_mask, inputs.scene_layers)
        });
        let avatars_known =
            scene_known && known.is_some_and(|known| known.avatar_layers == inputs.avatar_layers);
        let pointer_known =
            scene_known && known.is_some_and(|known| known.pointer_ray == inputs.pointer_ray);
        if avatars_known && pointer_known && previous.tick == scene.last_update_frame {
            continue;
        }

        let (_, rotation, translation) = gt.to_scale_rotation_translation();
        let scene_origin = scene_transform.translation();
        let report = TriggerReport {
            transforms: &transforms,
            container,
            collision_mask: mask,
            scene_origin,
            translation: translation - scene_origin,
            rotation,
            timestamp: &timestamp,
            tick: scene.last_update_frame,
        };

        // get intersecting colliders
        let new_colliders = (!scene_known).then(|| colliders.intersect_id(&collider.0, mask));
        report.events(
            &mut scene,
            &colliders,
            OverlapSource::Scene,
            &previous.scene,
            previous.tick,
            new_colliders.as_deref().unwrap_or(&previous.scene),
        );

        // get avatar colliders
        let new_avatars = if mask & PLAYER_LAYERS != 0 {
            let new_avatars = (!avatars_known).then(|| {
                colliders
                    .get_collider(&collider.0)
                    .map(|c| avatar_colliders.collider_data.intersect_collider(c, mask))
                    .unwrap_or_default()
            });
            report.events(
                &mut scene,
                &avatar_colliders.collider_data,
                OverlapSource::Avatar,
                &previous.avatars,
                previous.tick,
                new_avatars.as_deref().unwrap_or(&previous.avatars),
            );
            new_avatars
        } else {
            Some(Vec::new())
        };

        // get pointer ray
        let new_pointers = match inputs.pointer_ray {
            Some(ray) => {
                let new_pointers = (!pointer_known).then(|| {
                    // the pointer is in the area when the ray hits the area no farther than anything
                    // else on the pointer layer, trigger areas included. As for hover, a collider
                    // the ray starts inside is skipped, so an area containing the ray's origin is
                    // not entered and does not hide other areas. An area switched off (too small)
                    // is not entered.
                    let direction = ray.direction.as_vec3();
                    let pointed = colliders
                        .get_collider(&collider.0)
                        .is_some_and(|area| area.is_enabled())
                        .then(|| {
                            colliders.cast_ray_at(&collider.0, ray.origin, direction, f32::MAX)
                        })
                        .flatten()
                        .is_some_and(|area_toi| {
                            colliders
                                .cast_ray_nearest(
                                    ray.origin,
                                    direction,
                                    area_toi + RAYCAST_EPSILON as f32,
                                    POINTER_LAYER,
                                    true,
                                    true,
                                    Some(&collider.0),
                                )
                                .is_none_or(|hit| area_toi <= hit.toi + RAYCAST_EPSILON as f32)
                        });
                    if pointed {
                        vec![ColliderId::default()]
                    } else {
                        Vec::new()
                    }
                });
                report.events(
                    &mut scene,
                    &colliders,
                    OverlapSource::Pointer,
                    &previous.pointer,
                    previous.tick,
                    new_pointers.as_deref().unwrap_or(&previous.pointer),
                );
                new_pointers
            }
            None => Some(Vec::new()),
        };

        let tick = scene.last_update_frame;
        match maybe_active.as_deref_mut() {
            Some(active) => {
                if let Some(new_colliders) = new_colliders {
                    active.scene = new_colliders;
                }
                if let Some(new_avatars) = new_avatars {
                    active.avatars = new_avatars;
                }
                if let Some(new_pointers) = new_pointers {
                    active.pointer = new_pointers;
                }
                active.tick = tick;
                active.inputs = Some(inputs);
            }
            None => {
                commands.entity(entity).try_insert(ActiveTriggers {
                    scene: new_colliders.unwrap_or_default(),
                    avatars: new_avatars.unwrap_or_default(),
                    pointer: new_pointers.unwrap_or_default(),
                    tick,
                    inputs: Some(inputs),
                });
            }
        }
    }
}
