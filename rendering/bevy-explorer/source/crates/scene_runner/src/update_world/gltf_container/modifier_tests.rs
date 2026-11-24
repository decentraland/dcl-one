use dcl_component::proto_components::sdk::components::pb_gltf_node_modifiers::GltfNodeModifier;
use scene_material::SceneMaterialExt;

use super::*;

fn app() -> App {
    let mut app = App::new();
    app.init_resource::<Assets<SceneMaterial>>()
        .add_systems(Update, debug_modifiers);
    app
}

fn mesh(app: &mut App, unlit: bool) -> Entity {
    let material =
        app.world_mut()
            .resource_mut::<Assets<SceneMaterial>>()
            .add(SceneMaterial::new_unbounded(StandardMaterial {
                unlit,
                ..Default::default()
            }));
    app.world_mut()
        .spawn((Mesh3d::default(), MeshMaterial3d(material)))
        .id()
}

fn modifier(path: &str, cast_shadows: bool, metallic: f32) -> GltfNodeModifier {
    GltfNodeModifier {
        path: path.to_owned(),
        cast_shadows: Some(cast_shadows),
        material: Some(PbMaterial {
            material: Some(pb_material::Material::Pbr(pb_material::PbrMaterial {
                metallic: Some(metallic),
                ..Default::default()
            })),
            ..Default::default()
        }),
    }
}

fn assert_override(app: &App, entity: Entity, modifier: &GltfNodeModifier) {
    let world = app.world();
    assert_eq!(
        &world.get::<PbMaterialComponent>(entity).unwrap().0,
        modifier.material.as_ref().unwrap()
    );
    assert_eq!(
        world.get::<ShadowCasterOverride>(entity).unwrap().0,
        modifier.cast_shadows.unwrap()
    );
    assert_eq!(
        world.get::<NotShadowCaster>(entity).is_none(),
        modifier.cast_shadows.unwrap()
    );
    assert!(world.get::<HiddenMaterial>(entity).is_some());
}

fn assert_original(app: &App, entity: Entity, unlit: bool) {
    let world = app.world();
    assert!(world.get::<PbMaterialComponent>(entity).is_none());
    assert!(world.get::<ShadowCasterOverride>(entity).is_none());
    assert!(world.get::<HiddenMaterial>(entity).is_none());
    assert_eq!(world.get::<NotShadowCaster>(entity).is_some(), unlit);
    let material = world.get::<MeshMaterial3d<SceneMaterial>>(entity).unwrap();
    assert_eq!(
        world
            .resource::<Assets<SceneMaterial>>()
            .get(material)
            .unwrap()
            .base
            .unlit,
        unlit
    );
}

#[test]
fn numbered_nodes_keep_independent_materials_and_shadows_on_update_and_removal() {
    let mut app = app();
    let first = mesh(&mut app, false);
    let second = mesh(&mut app, true);
    let drawer = mesh(&mut app, false);
    let similarly_named = mesh(&mut app, false);
    let primitive_alias = mesh(&mut app, false);
    let first_original = app
        .world()
        .get::<MeshMaterial3d<SceneMaterial>>(first)
        .unwrap()
        .clone();
    let second_original = app
        .world()
        .get::<MeshMaterial3d<SceneMaterial>>(second)
        .unwrap()
        .clone();
    let first_modifier = modifier("Droid_01", false, 0.2);
    let second_modifier = modifier("Droid_02", true, 0.8);
    let container = app
        .world_mut()
        .spawn((
            GltfProcessed {
                // Actual node/mesh names in the swap_material Robots.glb fixture.
                named_nodes: HashMap::from_iter([
                    ("Droid_01/BaseMesh".into(), first),
                    ("Droid_02/BaseMesh.002".into(), second),
                    ("Drawer_03/BaseMesh.003".into(), drawer),
                    ("Droid_1/BaseMesh".into(), similarly_named),
                    ("Droid/Primitive1".into(), primitive_alias),
                ]),
                ..Default::default()
            },
            GltfNodeModifiers(PbGltfNodeModifiers {
                modifiers: vec![first_modifier.clone(), second_modifier.clone()],
            }),
        ))
        .id();
    app.update();
    assert_override(&app, first, &first_modifier);
    assert_override(&app, second, &second_modifier);
    for sibling in [drawer, similarly_named, primitive_alias] {
        assert_original(&app, sibling, false);
    }

    // Model the material consumer replacing the mesh handle after applying the override.
    let replacement = app
        .world_mut()
        .resource_mut::<Assets<SceneMaterial>>()
        .add(SceneMaterial::new_unbounded(StandardMaterial::default()));
    app.world_mut()
        .entity_mut(first)
        .insert(MeshMaterial3d(replacement.clone()));
    app.world_mut()
        .entity_mut(second)
        .insert(MeshMaterial3d(replacement));

    // Replacing the list restores nodes no longer targeted, including unlit shadow defaults.
    let updated = modifier("Droid_01", true, 0.5);
    app.world_mut()
        .entity_mut(container)
        .insert(GltfNodeModifiers(PbGltfNodeModifiers {
            modifiers: vec![updated.clone()],
        }));
    app.update();
    assert_override(&app, first, &updated);
    assert_original(&app, second, true);
    assert_eq!(
        app.world()
            .get::<MeshMaterial3d<SceneMaterial>>(second)
            .unwrap()
            .0,
        second_original.0
    );
    for sibling in [drawer, similarly_named, primitive_alias] {
        assert_original(&app, sibling, false);
    }

    app.world_mut()
        .entity_mut(container)
        .remove::<GltfNodeModifiers>();
    app.update();
    assert_original(&app, first, false);
    assert_original(&app, second, true);
    assert_eq!(
        app.world()
            .get::<MeshMaterial3d<SceneMaterial>>(first)
            .unwrap()
            .0,
        first_original.0
    );
    assert_eq!(
        app.world()
            .get::<MeshMaterial3d<SceneMaterial>>(second)
            .unwrap()
            .0,
        second_original.0
    );
}

#[test]
fn primitive_alias_is_only_used_without_a_literal_hierarchy_match() {
    let mut app = app();
    let primitive = mesh(&mut app, false);
    let modification = modifier("Cube1_4", false, 0.3);
    let container = app
        .world_mut()
        .spawn((
            GltfProcessed {
                named_nodes: HashMap::from_iter([("Cube1/Primitive4".into(), primitive)]),
                ..Default::default()
            },
            GltfNodeModifiers(PbGltfNodeModifiers {
                modifiers: vec![modification.clone()],
            }),
        ))
        .id();
    app.update();
    assert_override(&app, primitive, &modification);

    // An exact empty group must not redirect the modifier to an unrelated primitive.
    let group = app.world_mut().spawn_empty().id();
    app.world_mut()
        .get_mut::<GltfProcessed>(container)
        .unwrap()
        .named_nodes
        .insert("Cube1_4".into(), group);
    app.update();
    assert_original(&app, primitive, false);

    let literal = mesh(&mut app, false);
    app.world_mut()
        .get_mut::<GltfProcessed>(container)
        .unwrap()
        .named_nodes
        .insert("Cube1_4/BaseMesh".into(), literal);
    app.update();
    assert_override(&app, literal, &modification);
    assert_original(&app, primitive, false);
}

#[test]
fn separator_only_path_does_not_apply_a_global_override() {
    let mut app = app();
    let child = mesh(&mut app, false);
    app.world_mut().spawn((
        GltfProcessed {
            named_nodes: HashMap::from_iter([("Droid_01/BaseMesh".into(), child)]),
            ..Default::default()
        },
        GltfNodeModifiers(PbGltfNodeModifiers {
            modifiers: vec![modifier("///", false, 0.3)],
        }),
    ));
    app.update();
    assert_original(&app, child, false);
}
