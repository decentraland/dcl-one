use std::time::Duration;

use bevy::ecs::system::RunSystemOnce;

use common::structs::{OutOfWorld, PrimaryUser};
use comms::profile::ProfileCache;
use dcl_component::proto_components::{
    common::{Color4, VideoTexture},
    sdk::components::pb_material::{self, PbrMaterial},
};

use super::*;
use crate::{
    scene_spawn_slice::{slice, tests::SIXTY_FPS},
    update_world::mesh_renderer::{
        tests::{
            a_box, charged, fit, marked, marker_written, preset_charges, record_charges,
            scene_entity, test_app,
        },
        PendingPrimitive, PRIMITIVE_COST,
    },
};
use scene_material::SceneMaterialExt;

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

#[test]
fn new_materials_are_created_until_the_slice_is_charged() {
    let (mut app, root) = material_app();
    let fit = fit(NEW_MATERIAL_COST);
    let entities = spawn_materials(&mut app, root, 0..fit + 2);

    app.update();

    assert_eq!(created(&app, &entities), fit);
    assert_eq!(marked::<PendingMaterial>(&app, &entities), 2);
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
    assert_eq!(marked::<PendingMaterial>(&app, &left), 0);
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
    let before: Vec<_> = waiting_entities
        .iter()
        .map(|e| marker_written::<PendingMaterial>(&app, *e))
        .collect();

    preset_charges(&mut app, Duration::from_millis(10));
    app.update();

    // one goes as the frame's first; the other is not touched again
    assert_eq!(created(&app, &waiting_entities), 1);
    let still_waiting = waiting_entities
        .iter()
        .position(|e| material_handle(&app, *e).is_none())
        .unwrap();
    assert_eq!(
        marker_written::<PendingMaterial>(&app, waiting_entities[still_waiting]),
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
    let before = marker_written::<PendingMaterial>(&app, entity);
    assert!(before.is_some());

    app.update();

    assert_eq!(marker_written::<PendingMaterial>(&app, entity), before);
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

#[test]
fn a_prepaid_material_whose_source_is_not_ready_is_gated_again() {
    let (mut app, root) = material_app();
    let video_id = SceneEntityId {
        id: 600,
        generation: 0,
    };
    let video = app.world_mut().spawn_empty().id();
    app.world_mut()
        .get_mut::<RendererSceneContext>(root)
        .unwrap()
        .associate_bevy_entity(video_id, video);
    let video_material = PbMaterialComponent(PbMaterial {
        material: Some(pb_material::Material::Pbr(PbrMaterial {
            texture: Some(TextureUnion {
                tex: Some(texture_union::Tex::VideoTexture(VideoTexture {
                    video_player_entity: video_id.as_proto_u32().unwrap(),
                    ..Default::default()
                })),
            }),
            ..Default::default()
        })),
        gltf: None,
    });
    let entity = app
        .world_mut()
        .spawn((container(root), video_material, MaterialPrepaid))
        .id();

    app.update();

    // it waits for the video, so the slice decides it again once the video is there
    assert!(material_handle(&app, entity).is_none());
    assert!(app.world().get::<PendingMaterial>(entity).is_some());
    assert!(app.world().get::<MaterialPrepaid>(entity).is_none());
}

// the material cache
fn material_id(materials: &mut Assets<SceneMaterial>) -> AssetId<SceneMaterial> {
    materials
        .add(SceneMaterial::new_unbounded(StandardMaterial::default()))
        .id()
}

#[test]
fn removed_materials_leave_the_cache() {
    let mut app = App::new();
    app.add_event::<AssetEvent<SceneMaterial>>()
        .init_resource::<CachedMaterialOwners>();
    let mut materials = Assets::<SceneMaterial>::default();
    let (freed, live, stale, rebuilt) = (
        material_id(&mut materials),
        material_id(&mut materials),
        material_id(&mut materials),
        material_id(&mut materials),
    );

    // hash 3 was rebuilt with a new material before the stale one's removal was seen
    let scene = app
        .world_mut()
        .spawn(CachedMaterials(HashMap::from_iter([
            (1, (freed, true)),
            (2, (live, false)),
            (3, (rebuilt, true)),
        ])))
        .id();
    app.world_mut()
        .resource_mut::<CachedMaterialOwners>()
        .0
        .extend([
            (freed, (scene, 1)),
            (live, (scene, 2)),
            (stale, (scene, 3)),
            (rebuilt, (scene, 3)),
        ]);

    app.world_mut()
        .send_event_batch([freed, stale].map(|id| AssetEvent::Removed { id }));
    app.world_mut()
        .run_system_once(remove_freed_cached_materials)
        .unwrap();

    let cache = app.world().get::<CachedMaterials>(scene).unwrap();
    assert_eq!(cache.0.len(), 2);
    assert_eq!(cache.0.get(&2), Some(&(live, false)));
    assert_eq!(cache.0.get(&3), Some(&(rebuilt, true)));
    let owners = app.world().resource::<CachedMaterialOwners>();
    assert_eq!(owners.0.len(), 2);
    assert!(owners.0.contains_key(&live) && owners.0.contains_key(&rebuilt));
}

#[test]
fn removed_materials_of_despawned_scenes_leave_the_owners() {
    let mut app = App::new();
    app.add_event::<AssetEvent<SceneMaterial>>()
        .init_resource::<CachedMaterialOwners>();
    let id = material_id(&mut Assets::<SceneMaterial>::default());
    let scene = app.world_mut().spawn(CachedMaterials::default()).id();
    app.world_mut()
        .resource_mut::<CachedMaterialOwners>()
        .0
        .insert(id, (scene, 1));
    app.world_mut().despawn(scene);

    app.world_mut().send_event(AssetEvent::Removed { id });
    app.world_mut()
        .run_system_once(remove_freed_cached_materials)
        .unwrap();

    assert!(app.world().resource::<CachedMaterialOwners>().0.is_empty());
}

// guard: what waits when the player goes behind the backdrop is created at once
#[test]
fn waiting_materials_are_created_when_the_player_goes_behind_the_backdrop() {
    let (mut app, root) = material_app();
    preset_charges(&mut app, Duration::from_millis(10));
    let entities = spawn_materials(&mut app, root, 0..50);
    app.update();
    assert_eq!(marked::<PendingMaterial>(&app, &entities), 49);

    let player = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryUser>>()
        .single(app.world())
        .unwrap();
    app.world_mut().entity_mut(player).insert(OutOfWorld);
    preset_charges(&mut app, Duration::from_millis(10));
    app.update();

    assert_eq!(created(&app, &entities), 50);
    assert_eq!(marked::<PendingMaterial>(&app, &entities), 0);
}
