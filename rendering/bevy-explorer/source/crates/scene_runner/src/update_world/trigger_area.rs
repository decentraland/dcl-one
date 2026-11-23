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
            MeshColliderShape, SceneColliderData,
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

/// The overlaps a trigger area reported last frame and the scene tick it reported them at. It is
/// updated in place each frame: a stay goes out once per scene tick, an enter or exit when the
/// overlaps change.
#[derive(Component, Default)]
pub struct ActiveTriggers {
    scene: Vec<ColliderId>,
    avatars: Vec<ColliderId>,
    pointer: Vec<ColliderId>,
    tick: u32,
}

/// What `update_triggers` reports one overlap kind with, for one trigger area this frame.
struct TriggerReport<'a, 'w, 's> {
    triggers: &'a Query<'w, 's, (&'static MeshCollider<CtTrigger>, &'static GlobalTransform)>,
    container: &'a ContainerEntity,
    translation: Vec3,
    rotation: Quat,
    timestamp: &'a MonotonicTimestamp<PbTriggerAreaResult>,
    tick: u32,
}

impl TriggerReport<'_, '_, '_> {
    fn trigger(&self, colliders: &SceneColliderData, collider_id: &ColliderId) -> Trigger {
        colliders
            .get_collider_entity(collider_id)
            .and_then(|e| self.triggers.get(e).ok())
            .map(|(collider, gt)| {
                let (s, r, t) = gt.to_scale_rotation_translation();
                Trigger {
                    entity: collider_id.entity.as_proto_u32().unwrap(),
                    layers: collider.collision_mask,
                    position: Some(Vector3::world_vec_from_vec3(&t)),
                    rotation: Some(r.into()),
                    scale: Some(Vector3::abs_vec_from_vec3(&s)),
                }
            })
            .unwrap_or_else(|| Trigger {
                entity: collider_id.entity.as_proto_u32().unwrap(),
                layers: 0,
                position: None,
                rotation: None,
                scale: None,
            })
    }

    fn send(
        &self,
        scene: &mut Mut<RendererSceneContext>,
        colliders: &SceneColliderData,
        collider_id: &ColliderId,
        ty: TriggerAreaEventType,
    ) {
        let result = PbTriggerAreaResult {
            triggered_entity: self.container.container_id.as_proto_u32().unwrap(),
            triggered_entity_position: Some(Vector3::world_vec_from_vec3(&self.translation)),
            triggered_entity_rotation: Some(self.rotation.into()),
            event_type: ty as i32,
            timestamp: self.timestamp.next_timestamp(),
            trigger: Some(self.trigger(colliders, collider_id)),
        };
        scene.update_crdt(
            SceneComponentId::TRIGGER_AREA_RESULT,
            CrdtType::GO_ENT,
            self.container.container_id,
            &result,
        );
    }

    /// Exits and stays for `previous` (reported at `previous_tick`), then enters, against `current`.
    fn events(
        &self,
        scene: &mut Mut<RendererSceneContext>,
        colliders: &SceneColliderData,
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
                        prev_collider,
                        TriggerAreaEventType::TaetStay,
                    );
                }
            } else {
                self.send(
                    scene,
                    colliders,
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
                    new_collider,
                    TriggerAreaEventType::TaetEnter,
                );
            }
        }
    }
}

fn update_triggers(
    mut commands: Commands,
    mut trigger_areas: Query<(
        Entity,
        &ContainerEntity,
        &MeshCollider<CtTrigger>,
        &HasCollider<CtTrigger>,
        &GlobalTransform,
        Option<&mut ActiveTriggers>,
    )>,
    mut scenes: Query<(&mut RendererSceneContext, &mut SceneColliderData)>,
    mut avatar_colliders: ResMut<AvatarColliders>,
    triggers: Query<(&'static MeshCollider<CtTrigger>, &'static GlobalTransform)>,
    pointer_ray: Res<PointerRay>,
    timestamp: Res<MonotonicTimestamp<PbTriggerAreaResult>>,
) {
    let no_triggers = ActiveTriggers::default();

    for (entity, container, trigger_def, collider, gt, mut maybe_active) in trigger_areas.iter_mut()
    {
        let Ok((mut scene, mut colliders)) = scenes.get_mut(container.root) else {
            continue;
        };

        let (_, rotation, translation) = gt.to_scale_rotation_translation();
        let report = TriggerReport {
            triggers: &triggers,
            container,
            translation,
            rotation,
            timestamp: &timestamp,
            tick: scene.last_update_frame,
        };
        let previous = maybe_active.as_deref().unwrap_or(&no_triggers);

        // get intersecting colliders
        let new_colliders = colliders.intersect_id(&collider.0, trigger_def.collision_mask);
        report.events(
            &mut scene,
            &colliders,
            &previous.scene,
            previous.tick,
            &new_colliders,
        );

        // get avatar colliders
        let mut new_avatars = Vec::default();
        if trigger_def.collision_mask
            & (ColliderLayer::ClPlayer as u32 | ColliderLayer::ClMainPlayer as u32)
            != 0
        {
            new_avatars = colliders
                .get_collider(&collider.0)
                .map(|c| {
                    avatar_colliders
                        .collider_data
                        .intersect_collider(c, trigger_def.collision_mask)
                })
                .unwrap_or_default();
            report.events(
                &mut scene,
                &avatar_colliders.collider_data,
                &previous.avatars,
                previous.tick,
                &new_avatars,
            );
        }

        // get pointer ray
        let mut new_pointers = Vec::default();
        if trigger_def.collision_mask & (ColliderLayer::ClPointer as u32) != 0 {
            if let Some(ray) = pointer_ray.0 {
                let pointer_hit = colliders.cast_ray_nearest(
                    ray.origin,
                    ray.direction.as_vec3(),
                    f32::MAX,
                    ColliderLayer::ClPointer as u32,
                    false,
                    true,
                    Some(&collider.0),
                );

                if pointer_hit.is_some() {
                    new_pointers.push(ColliderId::default());
                }

                report.events(
                    &mut scene,
                    &colliders,
                    &previous.pointer,
                    previous.tick,
                    &new_pointers,
                );
            }
        }

        let current = ActiveTriggers {
            scene: new_colliders,
            avatars: new_avatars,
            pointer: new_pointers,
            tick: scene.last_update_frame,
        };
        match maybe_active.as_deref_mut() {
            Some(active) => *active = current,
            None => {
                commands.entity(entity).try_insert(current);
            }
        }
    }
}
