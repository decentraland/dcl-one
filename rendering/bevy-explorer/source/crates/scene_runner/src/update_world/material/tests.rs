use std::time::Duration;

use common::structs::{OutOfWorld, PrimaryUser};
use comms::profile::ProfileCache;
use dcl_component::proto_components::{
    common::Color4,
    sdk::components::pb_material::{self, PbrMaterial},
};

use super::*;
use crate::{
    scene_spawn_slice::{slice, tests::SIXTY_FPS, InstantiationCharges},
    update_world::mesh_renderer::{
        tests::{a_box, fit, record_charges, scene_entity, test_app, Charged},
        PendingPrimitive, PRIMITIVE_COST,
    },
};

/// `mesh_renderer`'s test app with the material systems as `MaterialDefinitionPlugin` adds
/// them, after `update_mesh`; the charges are recorded after both.
fn material_app() -> (App, Entity) {
    let (mut app, root) = test_app();
    app.init_asset::<Image>()
        .init_asset::<StandardMaterial>()
        .init_resource::<ProfileCache>()
        .init_resource::<CachedMaterialOwners>()
        .add_systems(
            Update,
            (
                init_cache,
                remove_freed_cached_materials,
                update_materials,
                update_bias,
                update_loading_materials,
            )
                .chain()
                .after(update_mesh)
                .before(record_charges),
        );
    (app, root)
}

/// A distinct untextured material for each `index`.
fn material(index: usize) -> PbMaterialComponent {
    PbMaterialComponent(PbMaterial {
        material: Some(pb_material::Material::Pbr(PbrMaterial {
            albedo_color: Some(Color4 {
                r: index as f32 / 10_000.0,
                g: 0.5,
                b: 0.5,
                a: 1.0,
            }),
            ..Default::default()
        })),
        gltf: None,
    })
}

fn container(root: Entity) -> ContainerEntity {
    ContainerEntity {
        container: root,
        root,
        container_id: dcl_component::SceneEntityId::ROOT,
    }
}

/// Entities with only a material, as glTF node modifiers make them.
fn spawn_materials(
    app: &mut App,
    root: Entity,
    indices: impl Iterator<Item = usize>,
) -> Vec<Entity> {
    indices
        .map(|index| {
            app.world_mut()
                .spawn((container(root), material(index)))
                .id()
        })
        .collect()
}

fn created(app: &App, entities: &[Entity]) -> usize {
    entities
        .iter()
        .filter(|e| {
            app.world()
                .get::<MeshMaterial3d<SceneMaterial>>(**e)
                .is_some()
        })
        .count()
}

fn waiting(app: &App, entities: &[Entity]) -> usize {
    entities
        .iter()
        .filter(|e| app.world().get::<PendingMaterial>(**e).is_some())
        .count()
}

fn preset_charges(app: &mut App, charges: Duration) {
    app.world_mut().resource_mut::<InstantiationCharges>().0 = charges;
}

fn charged(app: &App) -> Duration {
    app.world().resource::<Charged>().0
}

#[test]
fn new_materials_are_created_until_the_slice_is_charged() {
    let (mut app, root) = material_app();
    let fit = fit(NEW_MATERIAL_COST);
    let entities = spawn_materials(&mut app, root, 0..fit + 2);

    app.update();

    assert_eq!(created(&app, &entities), fit);
    assert_eq!(waiting(&app, &entities), 2);
    let refused: Vec<_> = entities
        .iter()
        .filter(|e| app.world().get::<PendingMaterial>(**e).is_some())
        .collect();
    for entity in refused {
        assert!(app.world().get::<MeshMaterial3dLoading>(*entity).is_none());
    }
}

#[test]
fn a_cached_material_costs_less_than_a_new_one() {
    let (mut app, root) = material_app();
    spawn_materials(&mut app, root, [7, 7].into_iter());

    app.update();

    assert_eq!(charged(&app), NEW_MATERIAL_COST + MATERIAL_COST);
}

#[test]
fn waiting_materials_are_created_first_next_frame() {
    let (mut app, root) = material_app();
    let fit = fit(NEW_MATERIAL_COST);
    let first = spawn_materials(&mut app, root, 0..fit + 3);
    app.update();
    let left: Vec<_> = first
        .iter()
        .copied()
        .filter(|e| app.world().get::<PendingMaterial>(*e).is_some())
        .collect();

    let second = spawn_materials(&mut app, root, fit + 3..2 * fit + 3);
    app.update();

    assert_eq!(created(&app, &left), 3);
    assert_eq!(waiting(&app, &left), 0);
    assert_eq!(created(&app, &second), fit - 3);
}

// guard: the consumer goes through the budget's bypass
#[test]
fn behind_the_loading_backdrop_every_material_is_created() {
    let (mut app, root) = material_app();
    let player = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryUser>>()
        .single(app.world())
        .unwrap();
    app.world_mut().entity_mut(player).insert(OutOfWorld);
    let entities = spawn_materials(&mut app, root, 0..fit(NEW_MATERIAL_COST) + 50);

    app.update();

    assert_eq!(created(&app, &entities), entities.len());
    assert_eq!(charged(&app), Duration::ZERO);
}

fn material_handle(app: &App, entity: Entity) -> Option<AssetId<SceneMaterial>> {
    app.world()
        .get::<MeshMaterial3d<SceneMaterial>>(entity)
        .map(|material| material.id())
}

#[test]
fn a_changed_material_is_neither_charged_nor_deferred() {
    let (mut app, root) = material_app();
    let entities = spawn_materials(&mut app, root, 0..2);
    app.update();
    let before: Vec<_> = entities.iter().map(|e| material_handle(&app, *e)).collect();

    preset_charges(&mut app, Duration::from_millis(10));
    for (index, entity) in entities.iter().enumerate() {
        app.world_mut()
            .entity_mut(*entity)
            .insert(material(100 + index));
    }
    app.update();

    for (entity, before) in entities.iter().zip(before) {
        let after = material_handle(&app, *entity);
        assert!(after.is_some());
        assert_ne!(after, before);
    }
    assert_eq!(charged(&app), Duration::from_millis(10));
}

#[test]
fn a_prepaid_material_is_created_whatever_was_spent() {
    let (mut app, root) = material_app();
    let entity = app
        .world_mut()
        .spawn((container(root), material(1), MaterialPrepaid))
        .id();
    preset_charges(&mut app, Duration::from_millis(10));

    app.update();

    assert!(material_handle(&app, entity).is_some());
    assert!(app.world().get::<MaterialPrepaid>(entity).is_none());
    assert_eq!(charged(&app), Duration::from_millis(10));
}

#[test]
fn a_material_waits_for_its_waiting_primitive() {
    let (mut app, root) = material_app();
    let entity = app
        .world_mut()
        .spawn((container(root), material(1), PendingPrimitive))
        .id();

    app.update();

    assert!(material_handle(&app, entity).is_none());
    assert!(app.world().get::<PendingMaterial>(entity).is_some());
    assert_eq!(charged(&app), Duration::ZERO);
}

#[test]
fn a_material_removed_while_waiting_is_never_created() {
    let (mut app, root) = material_app();
    preset_charges(&mut app, Duration::from_millis(10));
    let entities = spawn_materials(&mut app, root, 0..2);
    app.update();
    let waiting_entity = entities[1];
    assert!(app.world().get::<PendingMaterial>(waiting_entity).is_some());

    app.world_mut()
        .entity_mut(waiting_entity)
        .remove::<PbMaterialComponent>();
    app.update();
    app.update();

    assert!(material_handle(&app, waiting_entity).is_none());
    assert!(app.world().get::<PendingMaterial>(waiting_entity).is_none());
}

#[test]
fn a_primitive_and_its_new_material_appear_in_the_same_frame() {
    let (mut app, root) = material_app();
    let primitives: Vec<_> = (0..3)
        .map(|index| {
            let scene_entity = scene_entity(&mut app, root);
            app.world_mut()
                .spawn((scene_entity, container(root), a_box(), material(index)))
                .id()
        })
        .collect();
    // room for exactly two primitives with their new materials
    preset_charges(
        &mut app,
        slice(Some(SIXTY_FPS)) - 2 * (PRIMITIVE_COST + NEW_MATERIAL_COST),
    );

    app.update();

    let materials = app.world().resource::<Assets<SceneMaterial>>();
    for entity in &primitives[..2] {
        assert!(app.world().get::<Mesh3d>(*entity).is_some());
        let own = materials
            .get(material_handle(&app, *entity).unwrap())
            .unwrap();
        // its own colour, not the scene's white default
        assert_ne!(own.base.base_color, StandardMaterial::default().base_color);
    }
    assert!(app.world().get::<Mesh3d>(primitives[2]).is_none());
    assert!(material_handle(&app, primitives[2]).is_none());
}

fn marked(app: &App, entity: Entity) -> Option<bevy::ecs::component::Tick> {
    app.world()
        .entity(entity)
        .get_ref::<PendingMaterial>()
        .map(|marker| marker.last_changed())
}

#[test]
fn waiting_materials_are_left_alone_once_the_slice_is_spent() {
    let (mut app, root) = material_app();
    preset_charges(&mut app, Duration::from_millis(10));
    let entities = spawn_materials(&mut app, root, 0..3);
    app.update();
    let waiting_entities: Vec<_> = entities
        .iter()
        .copied()
        .filter(|e| app.world().get::<PendingMaterial>(*e).is_some())
        .collect();
    assert_eq!(waiting_entities.len(), 2);
    let before: Vec<_> = waiting_entities.iter().map(|e| marked(&app, *e)).collect();

    preset_charges(&mut app, Duration::from_millis(10));
    app.update();

    // one goes as the frame's first; the other is not touched again
    assert_eq!(created(&app, &waiting_entities), 1);
    let still_waiting = waiting_entities
        .iter()
        .position(|e| material_handle(&app, *e).is_none())
        .unwrap();
    assert_eq!(
        marked(&app, waiting_entities[still_waiting]),
        before[still_waiting]
    );
}

#[test]
fn a_material_waiting_for_its_primitive_is_left_alone() {
    let (mut app, root) = material_app();
    let entity = app
        .world_mut()
        .spawn((container(root), material(1), PendingPrimitive))
        .id();
    app.update();
    let before = marked(&app, entity);
    assert!(before.is_some());

    app.update();

    assert_eq!(marked(&app, entity), before);
}

#[test]
fn a_waiting_material_removed_and_added_in_one_tick_keeps_waiting() {
    let (mut app, root) = material_app();
    preset_charges(&mut app, Duration::from_millis(10));
    let entities = spawn_materials(&mut app, root, 0..3);
    app.update();
    let waiting_entities: Vec<_> = entities
        .iter()
        .copied()
        .filter(|e| app.world().get::<PendingMaterial>(*e).is_some())
        .collect();
    assert_eq!(waiting_entities.len(), 2);

    // both materials are deleted and put again before `update_materials` runs, with the slice
    // spent
    for (index, entity) in waiting_entities.iter().enumerate() {
        let mut entity = app.world_mut().entity_mut(*entity);
        entity.remove::<PbMaterialComponent>();
        entity.insert(material(10 + index));
    }
    preset_charges(&mut app, Duration::from_millis(10));
    app.update();
    app.update();

    assert_eq!(created(&app, &waiting_entities), 2);
}
