use bevy::{diagnostic::DiagnosticsStore, scene::ScenePlugin};

use super::*;

/// An app with bevy's scene spawner and the timing, without glTF processing.
fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ScenePlugin,
        SpawnTimingPlugin,
    ));
    app
}

fn processed(milliseconds: u64) -> ProcessedModel {
    ProcessedModel {
        container: Entity::PLACEHOLDER,
        nodes: 1,
        time: Duration::from_millis(milliseconds),
    }
}

#[test]
fn every_frame_measures_the_spawner_and_processing() {
    let mut app = app();

    app.update();
    app.update();

    let store = app.world().resource::<DiagnosticsStore>();
    for path in [&SCENE_SPAWNER, &GLTF_PROCESSING] {
        let diagnostic = store.get(path).expect("a registered diagnostic");
        assert_eq!(diagnostic.history_len(), 2, "{path}");
        assert!(diagnostic.value().is_some_and(|ms| ms >= 0.0), "{path}");
    }
}

#[test]
fn the_slowest_model_is_the_one_that_took_longest() {
    let mut slowest = SlowestProcessing::default();

    for milliseconds in [2, 5, 3] {
        slowest.record(processed(milliseconds));
    }

    assert_eq!(slowest.0, Some(processed(5)));
}

#[test]
fn the_slowest_model_is_forgotten_once_the_frame_is_measured() {
    let mut app = app();
    app.world_mut()
        .resource_mut::<SlowestProcessing>()
        .record(processed(5));

    app.update();

    assert_eq!(app.world().resource::<SlowestProcessing>().0, None);
}
