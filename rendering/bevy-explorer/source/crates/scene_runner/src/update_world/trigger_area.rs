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
    translation: Vec3,
    rotation: Quat,
    timestamp: &'a MonotonicTimestamp<PbTriggerAreaResult>,
    tick: u32,
}

impl TriggerReport<'_, '_, '_> {
    /// The entity that entered, stayed or left: its collider's layers, and the world transform of
    /// the entity that holds the collider. For a scene collider that is the scene entity, which
    /// for a glTF collider is the container rather than the mesh node; for an avatar collider it
    /// is the avatar. The pointer reports neither.
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
            colliders
                .get_collider(collider_id)
                .map_or(0, |collider| collider.collision_groups().memberships.bits())
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
            position: transform.map(|(_, _, t)| Vector3::world_vec_from_vec3(&t)),
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
fn update_triggers(
    mut commands: Commands,
    mut trigger_areas: Query<(
        Entity,
        &ContainerEntity,
        Ref<MeshCollider<CtTrigger>>,
        Ref<HasCollider<CtTrigger>>,
        &GlobalTransform,
        Option<&mut ActiveTriggers>,
    )>,
    mut scenes: Query<(&mut RendererSceneContext, &mut SceneColliderData)>,
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
        let Ok((mut scene, mut colliders)) = scenes.get_mut(container.root) else {
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
        let report = TriggerReport {
            transforms: &transforms,
            container,
            translation,
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
                    // not entered and does not hide other areas.
                    let direction = ray.direction.as_vec3();
                    let pointed = colliders
                        .ray_entry_distance(&collider.0, ray.origin, direction, f32::MAX)
                        .is_some_and(|area_toi| {
                            colliders
                                .cast_ray_nearest(
                                    ray.origin,
                                    direction,
                                    area_toi,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update_world::mesh_collider::regions_of_parcels;
    use bevy::ecs::system::RunSystemOnce;
    use dcl_component::{DclReader, FromDclReader, SceneEntityId};
    use dcl_types::SceneId;
    use rapier3d_f64::prelude::{ColliderBuilder, Group, InteractionGroups, InteractionTestMode};
    use scene_material::SceneBounds;

    const TRIGGER_ID: SceneEntityId = SceneEntityId {
        id: 512,
        generation: 0,
    };
    const AREA_CENTRE: Vec3 = Vec3::new(8.0, 1.0, -8.0);

    fn groups(layers: u32) -> InteractionGroups {
        let layers = Group::from_bits_truncate(layers);
        InteractionGroups::new(layers, layers, InteractionTestMode::And)
    }

    /// A world with one scene in parcel (0, 0) holding a 4 m trigger area at `AREA_CENTRE` that
    /// reacts to `mask`.
    fn world_with_trigger_area(mask: u32) -> (World, Entity) {
        let regions = regions_of_parcels(&[(0, 0)]);
        let context = RendererSceneContext::new(
            SceneId::DUMMY,
            "scene".to_owned(),
            "storage_root".to_owned(),
            false,
            0,
            "title".to_owned(),
            IVec2::ZERO,
            [IVec2::ZERO].into_iter().collect(),
            regions.clone(),
            SceneBounds::default(),
            vec![],
            Entity::PLACEHOLDER,
            0.,
            false,
            "sdk7",
            false,
            false,
        );
        let mut world = World::new();
        let scene = world
            .spawn((context, SceneColliderData::for_regions(&regions, 1)))
            .id();
        world.init_resource::<AvatarColliders>();
        world.init_resource::<PointerRay>();
        world.init_resource::<MonotonicTimestamp<PbTriggerAreaResult>>();

        let area = world.spawn_empty().id();
        let id = ColliderId::new(TRIGGER_ID, None, 0);
        let collider = ColliderBuilder::cuboid(2.0, 2.0, 2.0)
            .sensor(true)
            .collision_groups(groups(mask))
            .build();
        let transform = GlobalTransform::from_translation(AREA_CENTRE);
        let mut colliders = world.get_mut::<SceneColliderData>(scene).unwrap();
        colliders.set_collider(&id, collider, Some(area));
        colliders.update_collider_transform(&id, &transform);
        world.entity_mut(area).insert((
            ContainerEntity {
                container: area,
                root: scene,
                container_id: TRIGGER_ID,
            },
            MeshCollider::<CtTrigger> {
                collision_mask: mask,
                ..Default::default()
            },
            HasCollider::<CtTrigger>::new(id),
            transform,
        ));
        (world, scene)
    }

    /// Adds a solid scene collider (a 1 m cube) on `layers` as scene entity `id` and returns that
    /// entity. The trigger area is entity 512.
    fn add_scene_collider(
        world: &mut World,
        scene: Entity,
        id: u16,
        layers: u32,
        position: Vec3,
    ) -> SceneEntityId {
        let scene_entity = SceneEntityId { id, generation: 0 };
        let entity = world
            .spawn(GlobalTransform::from_translation(position))
            .id();
        world
            .get_mut::<RendererSceneContext>(scene)
            .unwrap()
            .associate_bevy_entity(scene_entity, entity);
        let id = ColliderId::new(scene_entity, None, 0);
        let collider = ColliderBuilder::cuboid(0.5, 0.5, 0.5)
            .collision_groups(groups(layers))
            .build();
        let mut colliders = world.get_mut::<SceneColliderData>(scene).unwrap();
        colliders.set_collider(&id, collider, Some(entity));
        colliders.update_collider_transform(&id, &GlobalTransform::from_translation(position));
        scene_entity
    }

    /// A ray along -z from `(from_x, 1, from_z)`; the area spans z -10 to -6.
    fn point_pointer_ray_from(world: &mut World, from_x: f32, from_z: f32) {
        world.insert_resource(PointerRay(Some(Ray3d::new(
            Vec3::new(from_x, 1.0, from_z),
            Dir3::NEG_Z,
        ))));
    }

    fn point_pointer_ray_at_area(world: &mut World, from_x: f32) {
        point_pointer_ray_from(world, from_x, -1.0);
    }

    fn results(world: &mut World, scene: Entity) -> Vec<PbTriggerAreaResult> {
        world.run_system_once(update_triggers).unwrap();
        let context = world.get::<RendererSceneContext>(scene).unwrap();
        context
            .crdt_store
            .go
            .get(&SceneComponentId::TRIGGER_AREA_RESULT)
            .and_then(|state| state.0.get(&TRIGGER_ID))
            .into_iter()
            .flatten()
            .map(|entry| {
                PbTriggerAreaResult::from_reader(&mut DclReader::new(&entry.data)).unwrap()
            })
            .collect()
    }

    fn event_types(results: &[PbTriggerAreaResult]) -> Vec<i32> {
        results.iter().map(|result| result.event_type).collect()
    }

    #[test]
    fn a_pointer_trigger_area_is_entered_when_the_pointer_ray_hits_it_first() {
        let pointer = ColliderLayer::ClPointer as u32;
        let (mut world, scene) = world_with_trigger_area(pointer);
        point_pointer_ray_at_area(&mut world, 8.0);

        let results = results(&mut world, scene);

        assert_eq!(
            event_types(&results),
            [TriggerAreaEventType::TaetEnter as i32]
        );
    }

    #[test]
    fn a_pointer_trigger_area_is_not_entered_when_another_pointer_collider_is_in_front() {
        let pointer = ColliderLayer::ClPointer as u32;
        let (mut world, scene) = world_with_trigger_area(pointer);
        add_scene_collider(&mut world, scene, 513, pointer, Vec3::new(8.0, 1.0, -3.0));
        point_pointer_ray_at_area(&mut world, 8.0);

        assert!(results(&mut world, scene).is_empty());
    }

    #[test]
    fn a_pointer_trigger_area_is_not_entered_when_the_pointer_ray_only_hits_another_collider() {
        let pointer = ColliderLayer::ClPointer as u32;
        let (mut world, scene) = world_with_trigger_area(pointer);
        add_scene_collider(&mut world, scene, 513, pointer, Vec3::new(13.0, 1.0, -8.0));
        point_pointer_ray_at_area(&mut world, 13.0);

        assert!(results(&mut world, scene).is_empty());
    }

    #[test]
    fn a_pointer_trigger_area_is_entered_when_the_pointer_ray_starts_inside_another_collider() {
        let pointer = ColliderLayer::ClPointer as u32;
        let (mut world, scene) = world_with_trigger_area(pointer);
        add_scene_collider(&mut world, scene, 513, pointer, Vec3::new(8.0, 1.0, -1.0));
        point_pointer_ray_at_area(&mut world, 8.0);

        let results = results(&mut world, scene);

        assert_eq!(
            event_types(&results),
            [TriggerAreaEventType::TaetEnter as i32]
        );
    }

    #[test]
    fn a_pointer_trigger_area_is_not_entered_when_the_pointer_ray_starts_inside_it() {
        let pointer = ColliderLayer::ClPointer as u32;
        let (mut world, scene) = world_with_trigger_area(pointer);
        point_pointer_ray_from(&mut world, 8.0, AREA_CENTRE.z);

        assert!(results(&mut world, scene).is_empty());
    }

    #[test]
    fn a_pointer_trigger_area_is_entered_when_it_ties_with_a_coincident_pointer_collider() {
        let pointer = ColliderLayer::ClPointer as u32;
        let (mut world, scene) = world_with_trigger_area(pointer);
        // a lower entity id than the area, whose front face (z = -6) it shares; inside the area on
        // its mask's layer, it also enters as a scene collider
        let coincident =
            add_scene_collider(&mut world, scene, 511, pointer, Vec3::new(8.0, 1.0, -6.5));
        point_pointer_ray_at_area(&mut world, 8.0);

        let results = results(&mut world, scene);
        let pointer_results = results
            .into_iter()
            .filter(|result| {
                result.trigger.as_ref().unwrap().entity != coincident.as_proto_u32().unwrap()
            })
            .collect::<Vec<_>>();

        assert_eq!(
            event_types(&pointer_results),
            [TriggerAreaEventType::TaetEnter as i32]
        );
    }

    #[test]
    fn an_avatar_entering_a_trigger_area_reports_its_layers_and_position() {
        let (mut world, scene) = world_with_trigger_area(ColliderLayer::ClPlayer as u32);
        let avatar = world
            .spawn(GlobalTransform::from_translation(AREA_CENTRE))
            .id();
        let layers = ColliderLayer::ClPlayer as u32 | ColliderLayer::ClMainPlayer as u32;
        let id = ColliderId::new(SceneEntityId::PLAYER, None, 0);
        let mut avatar_colliders = world.resource_mut::<AvatarColliders>();
        avatar_colliders.collider_data.set_collider(
            &id,
            ColliderBuilder::capsule_y(0.5, 0.25)
                .collision_groups(groups(layers))
                .build(),
            Some(avatar),
        );
        avatar_colliders
            .collider_data
            .update_collider_transform(&id, &GlobalTransform::from_translation(AREA_CENTRE));

        let results = results(&mut world, scene);

        assert_eq!(
            event_types(&results),
            [TriggerAreaEventType::TaetEnter as i32]
        );
        let trigger = results[0].trigger.as_ref().unwrap();
        assert_eq!(trigger.layers, layers);
        assert_eq!(
            trigger.position,
            Some(Vector3::world_vec_from_vec3(&AREA_CENTRE))
        );
    }

    #[test]
    fn a_scene_collider_entering_a_trigger_area_reports_its_layers_and_its_entitys_position() {
        let physics = ColliderLayer::ClPhysics as u32;
        let layers = physics | ColliderLayer::ClPointer as u32;
        let (mut world, scene) = world_with_trigger_area(physics);
        let position = AREA_CENTRE + Vec3::X;
        let entered = add_scene_collider(&mut world, scene, 513, layers, position);

        let results = results(&mut world, scene);

        assert_eq!(
            event_types(&results),
            [TriggerAreaEventType::TaetEnter as i32]
        );
        let trigger = results[0].trigger.as_ref().unwrap();
        assert_eq!(trigger.entity, entered.as_proto_u32().unwrap());
        assert_eq!(trigger.layers, layers);
        assert_eq!(
            trigger.position,
            Some(Vector3::world_vec_from_vec3(&position))
        );
    }
}
