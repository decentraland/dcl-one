use bevy::{asset::LoadState, render::mesh::VertexAttributeValues};

use super::*;

fn layers() -> LandscapeLayers {
    LandscapeLayers {
        ground: true,
        trees: true,
        grass: true,
        satellite: true,
    }
}

fn app(setup: impl FnOnce(&mut App)) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ImagePlugin::default(),
        assets::EmbedAssetsPlugin,
    ))
    .init_asset::<Mesh>()
    .init_asset::<StandardMaterial>();
    setup(&mut app);
    app.finish();
    app.cleanup();
    app
}

fn floor_app(world: bool) -> App {
    let mut pointers = ScenePointers::default();
    pointers.reset_terrain(world);
    let mut app = app(|app| {
        app.insert_resource(pointers)
            .insert_resource(CurrentRealm {
                about_url: "https://realm.example/about".to_owned(),
                ..default()
            })
            .insert_resource(layers())
            .add_plugins(SatelliteFloorPlugin);
    });
    app.update();
    app
}

/// The floor's visibility, `None` while it has never been spawned.
fn floor_visibility(app: &mut App) -> Option<Visibility> {
    let mut floor = app
        .world_mut()
        .query_filtered::<&Visibility, With<SatelliteFloor>>();
    floor.iter(app.world()).next().copied()
}

fn tiles(app: &mut App) -> usize {
    let mut tiles = app.world_mut().query_filtered::<(), With<SatelliteTile>>();
    tiles.iter(app.world()).count()
}

fn set_world(app: &mut App, world: bool) {
    app.world_mut()
        .resource_mut::<ScenePointers>()
        .reset_terrain(world);
    app.update();
}

fn set_toggle(app: &mut App, enabled: bool) {
    app.world_mut().resource_mut::<LandscapeLayers>().satellite = enabled;
    app.update();
}

#[test]
fn satellite_assets_load_embedded() {
    let mut app = app(|_| {});
    let server = app.world().resource::<AssetServer>().clone();
    let handles: Vec<Handle<Image>> = ["0,0", "7,7"]
        .map(|tile| server.load(format!("embedded://landscape/satellite/{tile}.jpg")))
        .into();
    for _ in 0..200 {
        app.update();
        if handles
            .iter()
            .all(|handle| matches!(server.load_state(handle), LoadState::Loaded))
        {
            break;
        }
    }
    for handle in &handles {
        assert!(
            matches!(server.load_state(handle), LoadState::Loaded),
            "{:?}: {:?}",
            handle.path(),
            server.load_state(handle)
        );
        let image = app.world().resource::<Assets<Image>>().get(handle).unwrap();
        assert_eq!(image.size(), UVec2::splat(512));
    }
}

/// Unity places tile (x, y) at `(x, y) * 640 - 2400 + 320 + margins` in its x/z, with the margins
/// (-32, -271.7) and y = 0 - 0.2 - 0.005. Bevy's z is Unity's -z.
#[test]
fn chunk_transform_matches_unity() {
    for (x, y, expected) in [
        (0, 0, Vec3::new(-2112.0, -0.205, 2351.7)),
        (7, 7, Vec3::new(2368.0, -0.205, -2128.3)),
    ] {
        let transform = chunk_transform(x, y);
        assert!(
            transform.translation.abs_diff_eq(expected, 1e-3),
            "chunk ({x}, {y}) at {}, expected {expected}",
            transform.translation
        );
        assert_eq!(transform.rotation, Quat::IDENTITY);
        assert_eq!(transform.scale, Vec3::new(640.0, 1.0, 640.0));
    }
}

/// Unity's chunk (x, y) shows the minimap's image (x, 7 - y): image rows run north to south.
#[test]
fn chunk_texture_flips_rows() {
    assert_eq!(
        chunk_texture(0, 0),
        "embedded://landscape/satellite/0,7.jpg"
    );
    assert_eq!(
        chunk_texture(3, 5),
        "embedded://landscape/satellite/3,2.jpg"
    );
    assert_eq!(
        chunk_texture(7, 7),
        "embedded://landscape/satellite/7,0.jpg"
    );
}

/// The floor stops at 2400 m from the origin on each axis, where the images' black margins
/// begin; the uv keeps each texel where the uncut quad would have put it.
#[test]
fn chunks_cut_at_the_city_square() {
    for (x, y) in [(0, 0), (7, 7), (3, 4)] {
        let transform = chunk_transform(x, y);
        let mesh = chunk_mesh(x, y);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions")
        };
        let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0)
        else {
            panic!("uvs")
        };
        for (position, uv) in positions.iter().zip(uvs) {
            let local = Vec3::from(*position);
            let world = transform.transform_point(local);
            assert!(world.x.abs() <= 2400.0 + 1e-2 && world.z.abs() <= 2400.0 + 1e-2);
            // the full quad maps its west edge to u 0 and its north (-z) edge to v 0
            assert!((uv[0] - (local.x + 0.5)).abs() < 1e-6);
            assert!((uv[1] - (local.z + 0.5)).abs() < 1e-6);
        }
    }
    // the west column loses the strip past -2400 m, the middle keeps its whole width
    let width = |x| {
        let Some(VertexAttributeValues::Float32x3(positions)) = chunk_mesh(x, 3)
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .cloned()
        else {
            panic!("positions")
        };
        let xs = positions.iter().map(|p| p[0]);
        (xs.clone().fold(f32::MIN, f32::max) - xs.fold(f32::MAX, f32::min)) * 640.0
    };
    assert!((width(0) - 608.0).abs() < 1e-2, "{}", width(0));
    assert!((width(3) - 640.0).abs() < 1e-2, "{}", width(3));
}

#[test]
fn waits_for_a_realm() {
    let mut app = app(|app| {
        app.insert_resource(ScenePointers::default())
            .init_resource::<CurrentRealm>()
            .insert_resource(layers())
            .add_plugins(SatelliteFloorPlugin);
    });
    app.update();
    assert_eq!(
        floor_visibility(&mut app),
        None,
        "nothing loads before a realm"
    );

    app.world_mut().resource_mut::<CurrentRealm>().about_url = "https://realm.example/about".into();
    app.update();
    assert_eq!(floor_visibility(&mut app), Some(Visibility::Inherited));
}

#[test]
fn hidden_in_worlds() {
    let mut app = floor_app(true);
    assert_eq!(floor_visibility(&mut app), None, "nothing loads in a World");

    set_world(&mut app, false);
    assert_eq!(floor_visibility(&mut app), Some(Visibility::Inherited));
    assert_eq!(tiles(&mut app), 64);

    set_world(&mut app, true);
    assert_eq!(floor_visibility(&mut app), Some(Visibility::Hidden));
    assert_eq!(tiles(&mut app), 64, "kept for the way back");

    set_world(&mut app, false);
    assert_eq!(floor_visibility(&mut app), Some(Visibility::Inherited));
}

#[test]
fn toggle_hides() {
    let mut app = floor_app(false);
    assert_eq!(floor_visibility(&mut app), Some(Visibility::Inherited));

    set_toggle(&mut app, false);
    assert_eq!(floor_visibility(&mut app), Some(Visibility::Hidden));

    set_toggle(&mut app, true);
    assert_eq!(floor_visibility(&mut app), Some(Visibility::Inherited));

    // off before the floor ever showed: nothing is loaded until it is wanted
    let mut app = floor_app(true);
    set_toggle(&mut app, false);
    set_world(&mut app, false);
    assert_eq!(floor_visibility(&mut app), None);
    set_toggle(&mut app, true);
    assert_eq!(tiles(&mut app), 64);
}
