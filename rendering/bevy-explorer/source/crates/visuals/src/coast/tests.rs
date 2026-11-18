use bevy::render::mesh::VertexAttributeValues;

use super::*;

fn bounds(min: IVec2, max: IVec2) -> Vec4 {
    let (min, max) = ((min * 16).as_vec2(), ((max + 1) * 16).as_vec2());
    Vec4::new(min.x, min.y, max.x, max.y)
}

fn outside(point: Vec3, bounds: Vec4) -> f32 {
    let unity = Vec2::new(point.x, -point.z);
    (bounds.xy() - unity)
        .max(unity - bounds.zw())
        .max(Vec2::ZERO)
        .length()
}

/// coast_profile.wgsl's `coast_distance`.
fn coast_distance(point: Vec3, bounds: Vec4) -> f32 {
    let unity = Vec2::new(point.x, -point.z);
    outside(point, bounds) - coast_profile(unity.clamp(bounds.xy(), bounds.zw())).y
}

fn positions(mesh: &Mesh) -> &[[f32; 3]] {
    match mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap() {
        VertexAttributeValues::Float32x3(points) => points,
        _ => panic!("cliff positions must be Float32x3"),
    }
}

fn chunks(bounds: Vec4, side: u32) -> i32 {
    (side_length(bounds, side) / CHUNK).ceil() as i32
}

#[test]
fn cliffs_face_outward_and_stay_outside_the_ground() {
    let bounds = bounds(IVec2::new(-2, -2), IVec2::new(2, 2));
    let near = near_chunks(bounds, Vec3::ZERO, CLIFF_RANGE);
    assert_eq!(near.len(), 8);
    for (side, chunk) in near {
        let mesh = cliff_mesh(bounds, side, chunk);
        let Some(VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("cliff normals missing")
        };
        let points = positions(&mesh);
        assert!(!points.is_empty());
        for (index, triangle) in points.as_chunks::<3>().0.iter().enumerate() {
            let p = [0, 1, 2].map(|i| Vec3::from_array(triangle[i]));
            let n = Vec3::from_array(normals[index * 3]);
            if p.iter().all(|point| point.y == UNDERGROUND) {
                // the hidden lid, from the bounds to the crest, facing up
                assert_eq!(n, Vec3::Y);
                assert!((p[1] - p[0]).cross(p[2] - p[0]).y > 0.0);
                continue;
            }
            assert!(n.is_finite() && (n.length() - 1.0).abs() < 0.001);
            // counter-clockwise seen from the side the normal faces
            assert!((p[1] - p[0]).cross(p[2] - p[0]).dot(n) > 0.0001);
            let centre = (p[0] + p[1] + p[2]) / 3.0;
            let unity = Vec2::new(centre.x, -centre.z);
            let outward = unity - unity.clamp(bounds.xy(), bounds.zw());
            assert!(Vec2::new(n.x, -n.z).dot(outward) > 0.0 || n.y > 0.0);
            // the crest is the nearest the cliff comes to the bounds
            for point in p {
                assert!(outside(point, bounds) > 1.9);
                assert!(point.y <= GROUND_LEVEL);
            }
        }
    }
    let huge = Vec4::new(-16000.0, -16000.0, 16000.0, 16000.0);
    assert!(near_chunks(huge, Vec3::ZERO, CLIFF_RANGE).is_empty());
    assert!(near_chunks(huge, Vec3::new(16000.0, 0.0, -16000.0), CLIFF_RANGE).len() <= 28);
}

#[test]
fn profiles_descend_from_the_ground_to_below_the_sea_on_the_shared_waterline() {
    let bounds = bounds(IVec2::new(-30, -20), IVec2::new(40, 35));
    for side in 0..4 {
        for chunk in 0..chunks(bounds, side) {
            let columns = columns(bounds, side, chunk);
            for column in &columns {
                assert_eq!(column[0].y, GROUND_LEVEL);
                assert!(column.windows(2).all(|pair| pair[0].y > pair[1].y));
                let waterline = column[5];
                assert_eq!(waterline.y, SEA_LEVEL);
                assert!(coast_distance(waterline, bounds).abs() < 0.001);
                assert!(column.iter().all(|point| outside(*point, bounds) <= 41.001));
            }
            // 1 m chords and corner facets stay within 12 cm of the smooth waterline
            for pair in columns.windows(2) {
                let midpoint = (pair[0][5] + pair[1][5]) * 0.5;
                assert!(coast_distance(midpoint, bounds).abs() < 0.12);
            }
        }
    }
}

#[test]
fn neighbouring_chunks_and_corner_halves_share_their_end_columns() {
    let bounds = bounds(IVec2::new(-30, -20), IVec2::new(40, 35));
    for side in 0..4 {
        let count = chunks(bounds, side);
        for chunk in 0..count {
            let (next_side, next_chunk) = if chunk + 1 == count {
                ((side + 1) % 4, 0)
            } else {
                (side, chunk + 1)
            };
            let a = columns(bounds, side, chunk);
            let b = columns(bounds, next_side, next_chunk);
            assert_eq!(a.last(), b.first());
        }
    }
}

#[test]
fn a_single_parcel_world_gets_a_closed_coast() {
    let bounds = bounds(IVec2::new(-2, -2), IVec2::new(2, 2));
    for side in 0..4 {
        assert_eq!(chunks(bounds, side), 2);
        let last = columns(bounds, side, 1).last().copied();
        let first = columns(bounds, (side + 1) % 4, 0).first().copied();
        assert_eq!(last, first);
    }
}

#[test]
fn walls_stand_outside_the_crest_with_their_inner_face_on_it() {
    let bounds = bounds(IVec2::new(-3, -1), IVec2::new(4, 6));
    for side in 0..4 {
        for chunk in 0..chunks(bounds, side) {
            let crest: Vec<_> = wall_columns(bounds, side, chunk)
                .iter()
                .map(|column| column[0])
                .collect();
            let walls = crest_walls(bounds, side, chunk);
            assert_eq!(walls.len(), crest.len() - 1);
            for ((centre, half, rotation), pair) in walls.iter().zip(crest.windows(2)) {
                assert_eq!(half.y * 2.0, WALL_HEIGHT);
                assert_eq!(half.z * 2.0, WALL_THICKNESS);
                assert!(centre.y - half.y == 0.0);
                // both crest ends lie on the box's inner face, at its ends
                for (point, end) in pair.iter().zip([-1.0, 1.0]) {
                    let local = rotation.inverse() * (point.with_y(centre.y) - *centre);
                    assert!((local.x - end * half.x).abs() < 0.001, "{local}");
                    assert!((local.z.abs() - half.z).abs() < 0.001, "{local}");
                }
                // and the rest of the wall is further out than the crest
                assert!(
                    outside(*centre, bounds)
                        > outside(pair[0], bounds).min(outside(pair[1], bounds))
                );
            }
        }
    }
}

#[test]
fn shader_profile_matches_the_cpu_profile() {
    let shader = include_str!("../coast_profile.wgsl");
    for expression in [
        "sin(dot(edge, vec2(0.013, 0.019)) + 0.7)",
        "sin(dot(edge, vec2(0.041, -0.027)) + 2.2)",
        "sin(dot(edge, vec2(0.007, -0.011)) - 0.4)",
        "8.0 + 4.0 * a + 2.0 * b",
        "crest + 18.0 + 2.0 * c + b",
    ] {
        assert!(shader.contains(expression), "profile drift: {expression}");
    }
    assert_eq!(shader.matches("sin(").count(), 3);
}

#[test]
fn ripples_tile_and_are_deterministic() {
    for cells in [4, 5, 7, 13, 29, 61] {
        for i in 0..=128 {
            let t = i as f32 / 128.0;
            assert_eq!(
                ripples::noise(Vec2::new(0.0, t), cells, 37),
                ripples::noise(Vec2::new(1.0, t), cells, 37)
            );
            assert_eq!(
                ripples::noise(Vec2::new(t, 0.0), cells, 37),
                ripples::noise(Vec2::new(t, 1.0), cells, 37)
            );
        }
    }
    for i in 0..=128 {
        let p = Vec2::new(-0.25, i as f32 / 128.0);
        for offset in [Vec2::X, Vec2::Y] {
            assert!((ripples::height_field(p) - ripples::height_field(p + offset)).abs() < 0.00005);
        }
    }
    let image = ripples::image();
    let bytes = image.data.as_ref().unwrap();
    // every mip of a 256 square RGBA8 texture
    assert_eq!(bytes.len(), 349524);
    assert_eq!(image.texture_descriptor.mip_level_count, 9);
    assert_eq!(bytes, ripples::image().data.as_ref().unwrap());
    let top = &bytes[..ripples::SIZE * ripples::SIZE * 4];
    let channel = |c: usize| top.iter().skip(c).step_by(4).copied().collect::<Vec<u8>>();
    for slope in [channel(0), channel(1)] {
        assert!(slope.iter().any(|&v| v < 100) && slope.iter().any(|&v| v > 155));
    }
    // the foam noise spans the full range, so the foam threshold is reached
    let foam = channel(2);
    assert_eq!(foam.iter().min(), Some(&0));
    assert_eq!(foam.iter().max(), Some(&255));
}

#[test]
fn foam_noise_tiles() {
    let foam = ripples::foam_field();
    let size = ripples::SIZE;
    // the last texel and the first sit one texel apart across the seam, as any two neighbours
    let seam = (0..size)
        .map(|y| (foam[y * size] - foam[y * size + size - 1]).abs())
        .fold(0.0, f32::max);
    let inner = (0..size)
        .map(|y| (foam[y * size + 1] - foam[y * size]).abs())
        .fold(0.0, f32::max);
    assert!(seam < inner * 2.0 + 1e-3, "seam {seam}, inner {inner}");
}

/// The sea is drawn only within the Landscape Distance of it, and its wave grid only while the
/// grid reaches past the land.
#[test]
fn the_sea_is_gated_by_distance() {
    let bounds = bounds(IVec2::splat(-150), IVec2::splat(150));
    // in the middle of the land, 2.4 km from the nearest sides
    let centre = Vec3::ZERO;
    assert!((sea_distance(bounds, centre) - 2400.0).abs() < 1e-3);
    assert_eq!(sea_view(bounds, centre, 1000.0), SeaView::Hidden);
    assert_eq!(sea_view(bounds, centre, 7000.0), SeaView::Flat);
    // 600 m in from the west side
    let inland = Vec3::new(bounds.x + 600.0, 30.0, 0.0);
    assert!((sea_distance(bounds, inland) - 600.0).abs() < 1e-3);
    assert_eq!(sea_view(bounds, inland, 500.0), SeaView::Hidden);
    assert_eq!(sea_view(bounds, inland, 1000.0), SeaView::Flat);
    // near the shore, and out over the sea
    let shore = Vec3::new(bounds.x + 100.0, 0.0, 0.0);
    assert_eq!(sea_view(bounds, shore, 500.0), SeaView::Waves);
    let sea = Vec3::new(bounds.x - 100.0, SEA_LEVEL + 2.0, 0.0);
    assert_eq!(sea_distance(bounds, sea), 0.0);
    assert_eq!(sea_view(bounds, sea, 500.0), SeaView::Waves);
}

#[test]
fn the_flat_ocean_stays_below_the_deepest_trough() {
    assert!(OCEAN_DROP > waves::tests::wave_bound() + 0.2);
}

/// The sea writes no depth, so the flat ocean discards itself under the wave grid: the hole stays
/// inside the grid wherever the snapped grid centre sits, and covers every moving wave.
#[test]
fn the_flat_ocean_gives_way_under_the_wave_grid() {
    const {
        assert!(
            OCEAN_PLANE_HOLE + waves::WAVE_GRID_SPACING_M * 0.5 <= waves::WAVE_GRID_SIZE_M * 0.5
        );
        assert!(OCEAN_PLANE_HOLE >= waves::WAVE_FADE.y);
    }
    // the shader tells the plane by height: it sits under `OCEAN_PLANE_TOP`, the deepest trough
    // above it
    assert!(SEA_LEVEL - waves::tests::wave_bound() > SEA_LEVEL - OCEAN_DROP + 0.1);
}

#[test]
fn shader_wave_constants_match_the_cpu_constants() {
    use std::collections::BTreeSet;

    use bevy::render::render_resource::ShaderDefVal;
    use waves::*;

    let bits = |value: f32| value.to_bits();
    let mut expected = vec![
        ("WAVE_COUNT".to_string(), WAVE_COUNT),
        ("WAVE_HEIGHT".to_string(), bits(WAVE_HEIGHT)),
        ("WAVE_DISTANCE".to_string(), bits(WAVE_DISTANCE)),
        ("WAVE_SPEED".to_string(), bits(WAVE_SPEED)),
        ("WAVE_STEEPNESS".to_string(), bits(WAVE_STEEPNESS)),
        (
            "WATER_DEPTH_HORIZONTAL".to_string(),
            bits(WATER_DEPTH_HORIZONTAL),
        ),
        (
            "WATER_DEPTH_VERTICAL".to_string(),
            bits(WATER_DEPTH_VERTICAL),
        ),
        ("WATER_DEPTH_EXP".to_string(), bits(WATER_DEPTH_EXP)),
        ("WAVE_FADE_X".to_string(), bits(WAVE_FADE.x)),
        ("WAVE_FADE_Y".to_string(), bits(WAVE_FADE.y)),
        ("WAVE_DIRECTION_X".to_string(), bits(WAVE_DIRECTION.x)),
        ("WAVE_DIRECTION_Y".to_string(), bits(WAVE_DIRECTION.y)),
    ];
    for (name, vector) in [
        ("WAVE_AMP", WAVE_AMP),
        ("WAVE_FREQ", WAVE_FREQ),
        ("WAVE_DIR1", WAVE_DIR1),
        ("WAVE_DIR2", WAVE_DIR2),
    ] {
        for (axis, value) in ["X", "Y", "Z", "W"].iter().zip(vector.to_array()) {
            expected.push((format!("{name}_{axis}"), bits(value)));
        }
    }
    let defs: Vec<_> = waves::shader_defs()
        .into_iter()
        .map(|def| match def {
            ShaderDefVal::UInt(name, value) => (name, value),
            other => panic!("unexpected def {other:?}"),
        })
        .collect();
    assert_eq!(defs, expected);

    // every `#{NAME}` the shaders read is one of the defs, and every def is read
    let mut used = BTreeSet::new();
    for shader in [
        include_str!("../coast_waves.wgsl"),
        include_str!("../coast_water.wgsl"),
    ] {
        for piece in shader.split("#{").skip(1) {
            used.insert(piece.split('}').next().unwrap().to_string());
        }
    }
    let names: BTreeSet<_> = expected
        .into_iter()
        .map(|(name, _)| name)
        .chain(["OCEAN_PLANE_TOP".to_string()])
        .collect();
    assert_eq!(used, names);
}

/// Cliffs are built no farther than the Landscape Distance.
#[test]
fn cliffs_culled_beyond_distance() {
    let bounds = bounds(IVec2::splat(-150), IVec2::splat(150));
    // 600 m in from the west side, far from the others
    let parcel = Vec3::new(bounds.x + 600.0, 0.0, 0.0);
    let west = |range: f32| {
        near_chunks(bounds, parcel, range)
            .into_iter()
            .filter(|(side, _)| *side == 3)
            .count()
    };
    assert_eq!(west(cliff_range(500.0)), 0);
    assert!(west(cliff_range(1000.0)) > 0);
    // the longest distance builds no farther than CLIFF_RANGE
    assert_eq!(cliff_range(LandscapeDistance::MAX), CLIFF_RANGE);
}

/// The sea's vertex and fragment pipelines compile on this machine's adapter, in a renderer with
/// no window: `setup`'s wave grid and flat ocean, made visible, seen by a multisampled camera and
/// an HDR one without MSAA. Needs a GPU, so `#[ignore]`d and skipped by CI:
///
/// ```text
/// cargo test -p visuals water_pipelines_compile -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn water_pipelines_compile() {
    use std::path::Path;

    use bevy::{asset::io::embedded::EmbeddedAssetRegistry, render::camera::RenderTarget};
    use pipeline_test::{compile_pipelines, fragment_pipelines, headless_app, render_target};

    let mut app = headless_app();
    // the coast's shaders as `CoastPlugin` embeds them; the rest of the plugin needs the terrain
    let registry = app.world().resource::<EmbeddedAssetRegistry>();
    for (name, source) in [
        ("coast_profile.wgsl", include_str!("../coast_profile.wgsl")),
        ("coast_surf.wgsl", include_str!("../coast_surf.wgsl")),
        ("coast_cliff.wgsl", include_str!("../coast_cliff.wgsl")),
        ("coast_water.wgsl", include_str!("../coast_water.wgsl")),
        ("coast_waves.wgsl", include_str!("../coast_waves.wgsl")),
    ] {
        let path = Path::new("visuals").join(name);
        registry.insert_asset(path.clone(), &path, source.as_bytes());
    }
    app.add_plugins((
        MaterialPlugin::<CliffMaterial>::default(),
        MaterialPlugin::<WaterMaterial>::default(),
    ))
    .add_systems(Startup, setup);
    app.finish();
    app.cleanup();
    app.update();

    let targets = [render_target(&mut app), render_target(&mut app)];
    let world = app.world_mut();
    let mut oceans = world.query_filtered::<&mut Visibility, With<Ocean>>();
    for mut visibility in oceans.iter_mut(world) {
        *visibility = Visibility::Inherited;
    }
    world.spawn((
        DirectionalLight::default(),
        Transform::default().looking_to(Vec3::new(0.3, -1.0, -0.4), Vec3::Y),
    ));
    for (target, hdr, msaa) in [
        (&targets[0], false, Msaa::Sample4),
        (&targets[1], true, Msaa::Off),
    ] {
        world.spawn((
            Camera3d::default(),
            Camera {
                target: RenderTarget::Image(target.clone().into()),
                hdr,
                ..default()
            },
            msaa,
            // above the sea, looking down at it
            Transform::from_xyz(0.0, SEA_LEVEL + 10.0, 0.0)
                .looking_at(Vec3::new(0.0, SEA_LEVEL, -40.0), Vec3::Y),
        ));
    }

    compile_pipelines(&mut app);

    // the transparent main pass, for each camera's key and each of the two meshes' layouts
    let compiled = fragment_pipelines(&app, "embedded://visuals/coast_water.wgsl");
    assert!(compiled >= 4, "only {compiled} water pipelines");
}

/// The open sea, now alpha blended, still hides what lies below it: a red seabed under deep water
/// does not show through, and the water draws. Needs a GPU, so `#[ignore]`d and skipped by CI:
///
/// ```text
/// cargo test -p visuals deep_sea_hides_the_seabed -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn deep_sea_hides_the_seabed() {
    use std::path::Path;

    use bevy::{
        asset::io::embedded::EmbeddedAssetRegistry,
        core_pipeline::tonemapping::Tonemapping,
        render::{
            camera::RenderTarget,
            view::screenshot::{Screenshot, ScreenshotCaptured},
        },
    };
    use pipeline_test::{compile_pipelines, headless_app, render_target};

    #[derive(Resource, Default)]
    struct Captured(Option<Image>);

    let mut app = headless_app();
    let registry = app.world().resource::<EmbeddedAssetRegistry>();
    for (name, source) in [
        ("coast_profile.wgsl", include_str!("../coast_profile.wgsl")),
        ("coast_surf.wgsl", include_str!("../coast_surf.wgsl")),
        ("coast_cliff.wgsl", include_str!("../coast_cliff.wgsl")),
        ("coast_water.wgsl", include_str!("../coast_water.wgsl")),
        ("coast_waves.wgsl", include_str!("../coast_waves.wgsl")),
    ] {
        let path = Path::new("visuals").join(name);
        registry.insert_asset(path.clone(), &path, source.as_bytes());
    }
    app.add_plugins((
        MaterialPlugin::<CliffMaterial>::default(),
        MaterialPlugin::<WaterMaterial>::default(),
    ))
    .init_resource::<Captured>()
    .add_systems(Startup, setup);
    app.finish();
    app.cleanup();
    app.update();

    let target = render_target(&mut app);
    let world = app.world_mut();
    // land 1 km away, so the sea under the camera is deep
    let water = world.resource::<CoastAssets>().water.clone();
    let mut materials = world.resource_mut::<Assets<WaterMaterial>>();
    materials.get_mut(&water).unwrap().extension.bounds = Vec4::new(1000.0, 1000.0, 2000.0, 2000.0);
    // the wave grid only
    let mut oceans = world.query::<(&Ocean, &mut Visibility)>();
    for (ocean, mut visibility) in oceans.iter_mut(world) {
        if ocean.waves {
            *visibility = Visibility::Inherited;
        }
    }
    let red = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.0, 0.0),
            unlit: true,
            ..default()
        });
    let seabed = world
        .resource_mut::<Assets<Mesh>>()
        .add(Plane3d::default().mesh().size(200.0, 200.0));
    world.spawn((
        Mesh3d(seabed),
        MeshMaterial3d(red),
        Transform::from_xyz(0.0, SEA_LEVEL - 3.0, 0.0),
    ));
    world.spawn((
        DirectionalLight::default(),
        Transform::default().looking_to(Vec3::new(0.3, -1.0, -0.4), Vec3::Y),
    ));
    world.spawn((
        Camera3d::default(),
        Camera {
            target: RenderTarget::Image(target.clone().into()),
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        Msaa::Off,
        Tonemapping::None,
        // above the sea, looking straight down at it
        Transform::from_xyz(0.0, SEA_LEVEL + 10.0, 0.0).looking_to(Vec3::NEG_Y, Vec3::NEG_Z),
    ));

    compile_pipelines(&mut app);
    for _ in 0..5 {
        app.update();
    }
    app.world_mut().spawn(Screenshot::image(target)).observe(
        |event: Trigger<ScreenshotCaptured>, mut captured: ResMut<Captured>| {
            captured.0 = Some(event.event().0.clone());
        },
    );
    let start = std::time::Instant::now();
    let image = loop {
        app.update();
        if let Some(image) = app.world_mut().resource_mut::<Captured>().0.take() {
            break image;
        }
        assert!(start.elapsed().as_secs() < 30, "GPU capture timed out");
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let rgba = image.try_into_dynamic().unwrap().to_rgba8();
    let centre = rgba.get_pixel(32, 32).0;
    println!("sea seen from above: {centre:?}");
    // the water draws, and it is not the red seabed
    assert!(centre[0] < 100, "the seabed shows through: {centre:?}");
    assert!(
        centre[0] as u32 + centre[1] as u32 + centre[2] as u32 > 0,
        "the sea did not draw"
    );
}
