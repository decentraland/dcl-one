//! `update_triggers` run on a small world, its `TRIGGER_AREA_RESULT` output decoded.

use bevy::ecs::system::RunSystemOnce;
use common::structs::MonotonicTimestamp;
use dcl_component::{
    proto_components::{
        common::Vector3,
        sdk::components::{ColliderLayer, PbTriggerAreaResult, TriggerAreaEventType},
    },
    DclReader, FromDclReader, SceneComponentId, SceneEntityId,
};
use dcl_types::SceneId;
use scene_material::SceneBounds;

use super::{bounds_tests::regions, *};
use crate::{
    update_scene::pointer_results::{AvatarColliders, PointerRay},
    update_world::trigger_area::{update_triggers, CtTrigger},
};

const POINTER: u32 = ColliderLayer::ClPointer as u32;
const PLAYER: u32 = ColliderLayer::ClPlayer as u32;
const MAIN_PLAYER: u32 = ColliderLayer::ClMainPlayer as u32;
const TRIGGER_ID: SceneEntityId = SceneEntityId {
    id: 512,
    generation: 0,
};
/// The area's centre, relative to its scene.
const AREA_CENTRE: Vec3 = Vec3::new(8.0, 1.0, -8.0);

fn groups(layers: u32) -> InteractionGroups {
    let layers = Group::from_bits_truncate(layers);
    InteractionGroups::new(layers, layers, InteractionTestMode::And)
}

/// The world position of parcel `(x, y)`'s origin.
fn parcel_origin((x, y): (i32, i32)) -> Vec3 {
    Vec3::new(16.0 * x as f32, 0.0, -16.0 * y as f32)
}

/// A world with one scene on `parcel` holding a 4 m trigger area at `AREA_CENTRE` that reacts to
/// `mask`.
fn world_with_trigger_area(mask: u32, parcel: (i32, i32)) -> (World, Entity) {
    let regions = regions(&[parcel]);
    let base = IVec2::new(parcel.0, parcel.1);
    let context = RendererSceneContext::new(
        SceneId::DUMMY,
        "scene".to_owned(),
        "storage_root".to_owned(),
        false,
        0,
        "title".to_owned(),
        base,
        [base].into_iter().collect(),
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
        .spawn((
            context,
            SceneColliderData::for_regions(&regions, 1),
            GlobalTransform::from_translation(parcel_origin(parcel)),
        ))
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
    let transform = GlobalTransform::from_translation(parcel_origin(parcel) + AREA_CENTRE);
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
        HasCollider::<CtTrigger>(id, PhantomData),
        transform,
    ));
    (world, scene)
}

/// Adds a solid scene collider (a 1 m cube) on `layers` as scene entity `id` at world `position`
/// and returns that entity. The trigger area is entity 512.
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

/// Adds the local player's avatar collider at world `position` and returns its id.
fn add_local_avatar(world: &mut World, position: Vec3) -> ColliderId {
    let avatar = world
        .spawn(GlobalTransform::from_translation(position))
        .id();
    let id = ColliderId::new(SceneEntityId::PLAYER, None, 0);
    let mut avatar_colliders = world.resource_mut::<AvatarColliders>();
    avatar_colliders.collider_data.set_collider(
        &id,
        ColliderBuilder::capsule_y(0.5, 0.25)
            .collision_groups(groups(PLAYER | MAIN_PLAYER))
            .build(),
        Some(avatar),
    );
    avatar_colliders
        .collider_data
        .update_collider_transform(&id, &GlobalTransform::from_translation(position));
    id
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

/// Runs `update_triggers` and returns every result the area has written so far.
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
        .map(|entry| PbTriggerAreaResult::from_reader(&mut DclReader::new(&entry.data)).unwrap())
        .collect()
}

fn event_types(results: &[PbTriggerAreaResult]) -> Vec<i32> {
    results.iter().map(|result| result.event_type).collect()
}

#[test]
fn a_pointer_trigger_area_is_entered_when_the_pointer_ray_hits_it_first() {
    let (mut world, scene) = world_with_trigger_area(POINTER, (0, 0));
    point_pointer_ray_at_area(&mut world, 8.0);

    let results = results(&mut world, scene);

    assert_eq!(
        event_types(&results),
        [TriggerAreaEventType::TaetEnter as i32]
    );
}

#[test]
fn a_pointer_trigger_area_is_not_entered_when_another_pointer_collider_is_in_front() {
    let (mut world, scene) = world_with_trigger_area(POINTER, (0, 0));
    add_scene_collider(&mut world, scene, 513, POINTER, Vec3::new(8.0, 1.0, -3.0));
    point_pointer_ray_at_area(&mut world, 8.0);

    assert!(results(&mut world, scene).is_empty());
}

#[test]
fn a_pointer_trigger_area_is_not_entered_when_the_pointer_ray_only_hits_another_collider() {
    let (mut world, scene) = world_with_trigger_area(POINTER, (0, 0));
    add_scene_collider(&mut world, scene, 513, POINTER, Vec3::new(13.0, 1.0, -8.0));
    point_pointer_ray_at_area(&mut world, 13.0);

    assert!(results(&mut world, scene).is_empty());
}

#[test]
fn a_pointer_trigger_area_is_entered_when_the_pointer_ray_starts_inside_another_collider() {
    let (mut world, scene) = world_with_trigger_area(POINTER, (0, 0));
    add_scene_collider(&mut world, scene, 513, POINTER, Vec3::new(8.0, 1.0, -1.0));
    point_pointer_ray_at_area(&mut world, 8.0);

    let results = results(&mut world, scene);

    assert_eq!(
        event_types(&results),
        [TriggerAreaEventType::TaetEnter as i32]
    );
}

#[test]
fn a_pointer_trigger_area_is_not_entered_when_the_pointer_ray_starts_inside_it() {
    let (mut world, scene) = world_with_trigger_area(POINTER, (0, 0));
    point_pointer_ray_from(&mut world, 8.0, AREA_CENTRE.z);

    assert!(results(&mut world, scene).is_empty());
}

#[test]
fn a_pointer_trigger_area_is_entered_when_it_ties_with_a_coincident_pointer_collider() {
    let (mut world, scene) = world_with_trigger_area(POINTER, (0, 0));
    // a lower entity id than the area, whose front face (z = -6) it shares; inside the area on
    // its mask's layer, it also enters as a scene collider
    let coincident = add_scene_collider(&mut world, scene, 511, POINTER, Vec3::new(8.0, 1.0, -6.5));
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
fn the_local_avatar_entering_a_player_trigger_area_reports_the_layers_the_area_reacts_to() {
    let (mut world, scene) = world_with_trigger_area(PLAYER, (0, 0));
    add_local_avatar(&mut world, AREA_CENTRE);

    let results = results(&mut world, scene);

    assert_eq!(
        event_types(&results),
        [TriggerAreaEventType::TaetEnter as i32]
    );
    let trigger = results[0].trigger.as_ref().unwrap();
    assert_eq!(trigger.layers, PLAYER);
    assert_eq!(
        trigger.position,
        Some(Vector3::world_vec_from_vec3(&AREA_CENTRE))
    );
}

#[test]
fn an_avatar_whose_collider_is_gone_exits_with_no_layers_and_no_transform() {
    let (mut world, scene) = world_with_trigger_area(PLAYER, (0, 0));
    let avatar = add_local_avatar(&mut world, AREA_CENTRE);
    results(&mut world, scene);
    world
        .resource_mut::<AvatarColliders>()
        .collider_data
        .remove_collider(&avatar);

    let results = results(&mut world, scene);

    assert_eq!(
        event_types(&results),
        [
            TriggerAreaEventType::TaetEnter as i32,
            TriggerAreaEventType::TaetExit as i32
        ]
    );
    let trigger = results[1].trigger.as_ref().unwrap();
    assert_eq!(trigger.layers, 0);
    assert_eq!(trigger.position, None);
}

#[test]
fn a_scene_collider_entering_a_trigger_area_reports_its_layers_and_its_entitys_position() {
    let physics = ColliderLayer::ClPhysics as u32;
    let layers = physics | POINTER;
    let (mut world, scene) = world_with_trigger_area(physics, (0, 0));
    let position = AREA_CENTRE + Vec3::X;
    let entered = add_scene_collider(&mut world, scene, 513, layers, position);

    let results = results(&mut world, scene);

    assert_eq!(
        event_types(&results),
        [TriggerAreaEventType::TaetEnter as i32]
    );
    let trigger = results[0].trigger.as_ref().unwrap();
    assert_eq!(trigger.entity, entered.as_proto_u32().unwrap());
    assert_eq!(trigger.layers, physics);
    assert_eq!(
        trigger.position,
        Some(Vector3::world_vec_from_vec3(&position))
    );
}

#[test]
fn a_trigger_area_in_a_scene_off_the_origin_reports_positions_relative_to_the_scene() {
    let parcel = (2, 1);
    let (mut world, scene) = world_with_trigger_area(PLAYER, parcel);
    add_local_avatar(&mut world, parcel_origin(parcel) + AREA_CENTRE + Vec3::X);

    let results = results(&mut world, scene);

    assert_eq!(
        event_types(&results),
        [TriggerAreaEventType::TaetEnter as i32]
    );
    assert_eq!(
        results[0].triggered_entity_position,
        Some(Vector3::world_vec_from_vec3(&AREA_CENTRE))
    );
    assert_eq!(
        results[0].trigger.as_ref().unwrap().position,
        Some(Vector3::world_vec_from_vec3(&(AREA_CENTRE + Vec3::X)))
    );
}
