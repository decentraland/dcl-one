use std::time::Duration;

use bevy::{ecs::system::RunSystemOnce, prelude::*};
use collectibles::base_wearables;
use dcl_component::proto_components::sdk::components::PbAvatarShape;

use super::AvatarProcessingRun;
use crate::{
    process_avatar, spawn_scenes,
    tests::{app_with_avatar_scenes, avatar_build, register_test_body, usable_body_gltf},
    update_render_avatar, AvatarLoaded, AvatarProcessed, AvatarSelection, AvatarShape,
};

/// An app with `n` avatars whose parts are written, so each is ready to be processed, and
/// their owners in the order they were spawned.
fn loaded_avatars(n: usize) -> (App, Vec<Entity>) {
    let mut app = app_with_avatar_scenes();
    let body = base_wearables::default_bodyshape_urn();
    let gltf = usable_body_gltf(app.world_mut());
    register_test_body(app.world_mut(), body.clone(), gltf);
    let owners: Vec<Entity> = (0..n)
        .map(|_| {
            app.world_mut()
                .spawn((
                    Transform::default(),
                    AvatarSelection {
                        scene: None,
                        shape: AvatarShape(PbAvatarShape {
                            body_shape: Some(body.as_str().to_owned()),
                            ..default()
                        }),
                        automatic_delete: true,
                        disable_dither: false,
                    },
                ))
                .id()
        })
        .collect();
    app.world_mut()
        .run_system_once(update_render_avatar)
        .unwrap();
    app.world_mut().run_system_once(spawn_scenes).unwrap();
    for owner in &owners {
        let build = avatar_build(app.world_mut(), *owner);
        assert!(app.world().get::<AvatarLoaded>(build).is_some());
    }
    // the scene spawner writes the parts
    app.update();
    (app, owners)
}

/// Whether each owner's avatar is processed.
fn processed(app: &mut App, owners: &[Entity]) -> Vec<bool> {
    owners
        .iter()
        .map(|owner| {
            let build = avatar_build(app.world_mut(), *owner);
            app.world().get::<AvatarProcessed>(build).is_some()
        })
        .collect()
}

#[test]
fn a_run_reports_what_it_processed() {
    let (mut app, owners) = loaded_avatars(3);

    app.world_mut().run_system_once(process_avatar).unwrap();

    let run = app.world().resource::<AvatarProcessingRun>();
    assert_eq!(run.processed, 3);
    assert!(run.spent > Duration::ZERO);
    assert_eq!(processed(&mut app, &owners), [true, true, true]);
}

#[test]
fn a_run_with_nothing_ready_reports_no_avatars() {
    let (mut app, _) = loaded_avatars(0);
    app.world_mut().insert_resource(AvatarProcessingRun {
        spent: Duration::from_millis(2),
        processed: 3,
    });

    app.world_mut().run_system_once(process_avatar).unwrap();

    assert_eq!(app.world().resource::<AvatarProcessingRun>().processed, 0);
}

#[cfg(feature = "explorer")]
#[test]
fn the_explorer_measures_the_run_into_the_diagnostic() {
    use bevy::diagnostic::{Diagnostic, DiagnosticsStore};
    use scene_runner::scene_spawn_slice::spawn_timing::AVATAR_PROCESSING;

    let mut world = World::new();
    let mut store = DiagnosticsStore::default();
    store.add(Diagnostic::new(AVATAR_PROCESSING));
    world.insert_resource(store);
    world.insert_resource(AvatarProcessingRun {
        spent: Duration::from_millis(2),
        processed: 3,
    });

    world
        .run_system_once(super::explorer::measure_avatar_processing)
        .unwrap();

    let store = world.resource::<DiagnosticsStore>();
    assert_eq!(store.get(&AVATAR_PROCESSING).unwrap().value(), Some(2.0));
}
