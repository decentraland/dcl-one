use std::{sync::Arc, time::Duration};

use bevy::{
    asset::io::memory::{Dir, MemoryAssetReader},
    ecs::component::Tick,
    gltf::{Gltf, GltfMesh},
    platform::collections::HashSet,
    scene::{scene_spawner_system, ScenePlugin},
};
use common::structs::{OutOfWorld, PrimaryUser};
use dcl_component::{proto_components::sdk::components::PbMaterial, SceneEntityId};
use dcl_types::SceneId;
use ipfs::{IpfsIo, IpfsResource};
use scene_material::SceneBounds;

use super::*;
use crate::{
    scene_spawn_slice::{
        slice, slice_scene_spawning,
        tests::{scene_updates, SIXTY_FPS},
        InstantiationCharges, ProcessingDeadline,
    },
    update_world::material::{MaterialPrepaid, PbMaterialComponent, NEW_MATERIAL_COST},
};

/// The charges as they stood right after the systems under test ran, before `SpawnScene`
/// reset them.
#[derive(Resource, Default)]
pub(crate) struct Charged(pub Duration);

pub(crate) fn record_charges(charges: Res<InstantiationCharges>, mut charged: ResMut<Charged>) {
    charged.0 = charges.this_frame;
}

/// An app that runs `update_mesh` as the explorer does: in `Update`, with the slice handed to
/// the spawner (and the charges reset) in `SpawnScene`. A player in world and one scene root with
/// a context.
pub(crate) fn test_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin))
        .init_asset::<Mesh>()
        .init_asset::<SceneMaterial>()
        .init_asset::<Gltf>()
        .init_asset::<GltfMesh>()
        .init_resource::<AppConfig>()
        .init_resource::<InstantiationCharges>()
        .init_resource::<ProcessingDeadline>()
        .init_resource::<Charged>()
        .insert_resource(scene_updates(Some(SIXTY_FPS)))
        .insert_resource(IpfsResource {
            inner: Arc::new(IpfsIo::new(
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
        .add_systems(Update, (update_mesh, record_charges).chain())
        .add_systems(
            SpawnScene,
            slice_scene_spawning.before(scene_spawner_system),
        );

    let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
    let defaults = MeshPrimitiveDefaults {
        boxx: meshes.add(Cuboid::default()),
        plane: meshes.add(Plane3d::default()),
        cylinder: meshes.add(Cylinder::default()),
        sphere: meshes.add(Sphere::default()),
    };
    app.insert_resource(defaults);

    app.world_mut().spawn(PrimaryUser::default());
    let root = app.world_mut().spawn_empty().id();
    give_context(&mut app, root);
    (app, root)
}

pub(crate) fn give_context(app: &mut App, root: Entity) {
    app.world_mut()
        .entity_mut(root)
        .insert(RendererSceneContext::new(
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
        ));
}

pub(crate) fn scene_entity(app: &mut App, root: Entity) -> SceneEntity {
    let count = app
        .world_mut()
        .query::<&SceneEntity>()
        .iter(app.world())
        .count();
    SceneEntity {
        root,
        scene_id: SceneId(root),
        id: SceneEntityId {
            id: 512 + count as u16,
            generation: 0,
        },
    }
}

pub(crate) fn a_box() -> MeshDefinition {
    MeshDefinition::Box { uvs: Vec::new() }
}

pub(crate) fn spawn_boxes(app: &mut App, root: Entity, count: usize) -> Vec<Entity> {
    (0..count)
        .map(|_| {
            let scene_entity = scene_entity(app, root);
            app.world_mut().spawn((scene_entity, a_box())).id()
        })
        .collect()
}

fn attached(app: &App, entities: &[Entity]) -> usize {
    entities
        .iter()
        .filter(|e| app.world().get::<Mesh3d>(**e).is_some())
        .count()
}

/// How many of `entities` carry the marker `M`.
pub(crate) fn marked<M: Component>(app: &App, entities: &[Entity]) -> usize {
    entities
        .iter()
        .filter(|e| app.world().get::<M>(**e).is_some())
        .count()
}

/// When `entity`'s marker `M` was last written: a waiting entity left alone keeps it.
pub(crate) fn marker_written<M: Component>(app: &App, entity: Entity) -> Option<Tick> {
    app.world()
        .entity(entity)
        .get_ref::<M>()
        .map(|marker| marker.last_changed())
}

/// How many primitives with a shared mesh fit in one slice.
pub(crate) fn fit(cost: Duration) -> usize {
    (slice(Some(SIXTY_FPS)).as_nanos() / cost.as_nanos()) as usize
}

pub(crate) fn preset_charges(app: &mut App, charges: Duration) {
    app.world_mut()
        .resource_mut::<InstantiationCharges>()
        .this_frame = charges;
}

pub(crate) fn charged(app: &App) -> Duration {
    app.world().resource::<Charged>().0
}

#[test]
fn primitives_are_attached_until_the_slice_is_charged() {
    let (mut app, root) = test_app();
    let fit = fit(PRIMITIVE_COST);
    let boxes = spawn_boxes(&mut app, root, fit + 3);

    app.update();

    assert_eq!(attached(&app, &boxes), fit);
    assert_eq!(marked::<PendingPrimitive>(&app, &boxes), 3);
}

#[test]
fn waiting_primitives_are_attached_first_next_frame() {
    let (mut app, root) = test_app();
    let fit = fit(PRIMITIVE_COST);
    let first = spawn_boxes(&mut app, root, fit + 3);
    app.update();
    let left: Vec<_> = first
        .iter()
        .copied()
        .filter(|e| app.world().get::<Mesh3d>(*e).is_none())
        .collect();

    let second = spawn_boxes(&mut app, root, fit);
    app.update();

    assert_eq!(attached(&app, &left), 3);
    assert_eq!(marked::<PendingPrimitive>(&app, &left), 0);
    assert_eq!(attached(&app, &second), fit - 3);
}

#[test]
fn the_first_primitive_of_a_frame_is_attached_however_much_was_spent() {
    let (mut app, root) = test_app();
    let boxes = spawn_boxes(&mut app, root, 2);
    preset_charges(&mut app, Duration::from_millis(10));

    app.update();

    assert_eq!(attached(&app, &boxes), 1);
}

#[test]
fn custom_texture_coordinates_cost_a_new_mesh() {
    let (mut app, root) = test_app();
    let scene_entity = scene_entity(&mut app, root);
    app.world_mut().spawn((
        scene_entity,
        MeshDefinition::Box {
            uvs: vec![[0.0, 0.0]; 24],
        },
    ));

    app.update();

    assert_eq!(charged(&app), PRIMITIVE_COST + NEW_MESH_COST);
}

#[test]
fn a_primitive_with_a_new_material_is_charged_for_both_and_marked_prepaid() {
    let (mut app, root) = test_app();
    let scene_entity = scene_entity(&mut app, root);
    let entity = app
        .world_mut()
        .spawn((
            scene_entity,
            a_box(),
            PbMaterialComponent(PbMaterial::default()),
        ))
        .id();

    app.update();

    assert_eq!(charged(&app), PRIMITIVE_COST + NEW_MATERIAL_COST);
    assert!(app.world().get::<MaterialPrepaid>(entity).is_some());
}

// guard: the consumer goes through the budget's bypass
#[test]
fn behind_the_loading_backdrop_every_primitive_is_attached() {
    let (mut app, root) = test_app();
    let player = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryUser>>()
        .single(app.world())
        .unwrap();
    app.world_mut().entity_mut(player).insert(OutOfWorld);
    let boxes = spawn_boxes(&mut app, root, fit(PRIMITIVE_COST) + 100);

    app.update();

    assert_eq!(attached(&app, &boxes), boxes.len());
    assert_eq!(charged(&app), Duration::ZERO);
}

#[test]
fn a_changed_primitive_is_neither_charged_nor_deferred() {
    let (mut app, root) = test_app();
    let boxes = spawn_boxes(&mut app, root, 2);
    app.update();

    preset_charges(&mut app, Duration::from_millis(10));
    for entity in &boxes {
        app.world_mut()
            .entity_mut(*entity)
            .insert(MeshDefinition::Sphere);
    }
    app.update();

    let sphere = app
        .world()
        .resource::<MeshPrimitiveDefaults>()
        .sphere
        .clone();
    for entity in &boxes {
        assert_eq!(app.world().get::<Mesh3d>(*entity).unwrap().0, sphere);
    }
    assert_eq!(charged(&app), Duration::from_millis(10));
}

#[test]
fn a_primitive_removed_while_waiting_is_never_attached() {
    let (mut app, root) = test_app();
    preset_charges(&mut app, Duration::from_millis(10));
    let boxes = spawn_boxes(&mut app, root, 2);
    app.update();
    let waiting_box = boxes[1];
    assert!(app.world().get::<PendingPrimitive>(waiting_box).is_some());

    app.world_mut()
        .entity_mut(waiting_box)
        .remove::<MeshDefinition>();
    app.update();
    app.update();

    assert!(app.world().get::<Mesh3d>(waiting_box).is_none());
    assert!(app.world().get::<PendingPrimitive>(waiting_box).is_none());
}

#[test]
fn a_primitive_removed_and_added_in_one_tick_is_attached() {
    let (mut app, root) = test_app();
    let boxes = spawn_boxes(&mut app, root, 1);
    app.update();

    let mut entity = app.world_mut().entity_mut(boxes[0]);
    entity.remove::<MeshDefinition>();
    entity.insert(a_box());
    app.update();

    assert_eq!(attached(&app, &boxes), 1);
}

#[test]
fn a_waiting_primitive_whose_scene_has_no_context_is_dropped() {
    let (mut app, _) = test_app();
    let root = app.world_mut().spawn_empty().id();
    let scene_entity = scene_entity(&mut app, root);
    let entity = app
        .world_mut()
        .spawn((
            scene_entity,
            MeshDefinition::Gltf {
                src: "model.glb".to_owned(),
                name: "mesh".to_owned(),
            },
            PendingPrimitive,
        ))
        .id();

    app.update();

    assert!(app.world().get::<PendingPrimitive>(entity).is_none());
}

#[test]
fn waiting_primitives_are_left_alone_once_the_slice_is_spent() {
    let (mut app, root) = test_app();
    preset_charges(&mut app, Duration::from_millis(10));
    let boxes = spawn_boxes(&mut app, root, 3);
    app.update();
    let waiting_boxes: Vec<_> = boxes
        .iter()
        .copied()
        .filter(|e| app.world().get::<PendingPrimitive>(*e).is_some())
        .collect();
    assert_eq!(waiting_boxes.len(), 2);
    let before: Vec<_> = waiting_boxes
        .iter()
        .map(|e| marker_written::<PendingPrimitive>(&app, *e))
        .collect();

    preset_charges(&mut app, Duration::from_millis(10));
    app.update();

    // one goes as the frame's first; the other is not touched again
    assert_eq!(attached(&app, &waiting_boxes), 1);
    let still_waiting = waiting_boxes
        .iter()
        .position(|e| app.world().get::<Mesh3d>(*e).is_none())
        .unwrap();
    assert_eq!(
        marker_written::<PendingPrimitive>(&app, waiting_boxes[still_waiting]),
        before[still_waiting]
    );
}

#[test]
fn a_waiting_primitive_removed_and_added_in_one_tick_keeps_waiting() {
    let (mut app, root) = test_app();
    preset_charges(&mut app, Duration::from_millis(10));
    let boxes = spawn_boxes(&mut app, root, 3);
    app.update();
    let waiting_boxes: Vec<_> = boxes
        .iter()
        .copied()
        .filter(|e| app.world().get::<PendingPrimitive>(*e).is_some())
        .collect();
    assert_eq!(waiting_boxes.len(), 2);

    // both renderers are deleted and put again before `update_mesh` runs, with the slice spent
    for entity in &waiting_boxes {
        let mut entity = app.world_mut().entity_mut(*entity);
        entity.remove::<MeshDefinition>();
        entity.insert(a_box());
    }
    preset_charges(&mut app, Duration::from_millis(10));
    app.update();
    app.update();

    assert_eq!(attached(&app, &waiting_boxes), 2);
}

// guard: what waits when the player goes behind the backdrop is attached at once
#[test]
fn waiting_primitives_are_attached_when_the_player_goes_behind_the_backdrop() {
    let (mut app, root) = test_app();
    preset_charges(&mut app, Duration::from_millis(10));
    let boxes = spawn_boxes(&mut app, root, 50);
    app.update();
    assert_eq!(marked::<PendingPrimitive>(&app, &boxes), 49);

    let player = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryUser>>()
        .single(app.world())
        .unwrap();
    app.world_mut().entity_mut(player).insert(OutOfWorld);
    preset_charges(&mut app, Duration::from_millis(10));
    app.update();

    assert_eq!(attached(&app, &boxes), 50);
    assert_eq!(marked::<PendingPrimitive>(&app, &boxes), 0);
}
