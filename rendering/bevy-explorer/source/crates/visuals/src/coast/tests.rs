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
        for (index, triangle) in points.chunks_exact(3).enumerate() {
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
    // every mip of a 256 square RG8 texture
    assert_eq!(bytes.len(), 174762);
    assert_eq!(image.texture_descriptor.mip_level_count, 9);
    assert_eq!(bytes, ripples::image().data.as_ref().unwrap());
    let top = &bytes[..ripples::SIZE * ripples::SIZE * 2];
    assert!(top.iter().any(|&v| v < 100) && top.iter().any(|&v| v > 155));
}

#[test]
fn the_flat_ocean_stays_below_the_deepest_trough() {
    assert!(OCEAN_DROP > waves::tests::wave_bound() + 0.2);
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
        ("REFRACTION_STRENGTH".to_string(), bits(REFRACTION_STRENGTH)),
        (
            "REFRACTION_CHROMATIC".to_string(),
            bits(REFRACTION_CHROMATIC),
        ),
        (
            "WATER_DEPTH_HORIZONTAL".to_string(),
            bits(WATER_DEPTH_HORIZONTAL),
        ),
        (
            "WATER_DEPTH_VERTICAL".to_string(),
            bits(WATER_DEPTH_VERTICAL),
        ),
        ("WATER_DEPTH_EXP".to_string(), bits(WATER_DEPTH_EXP)),
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
    let names: BTreeSet<_> = expected.into_iter().map(|(name, _)| name).collect();
    assert_eq!(used, names);
}

/// The sea's vertex and fragment pipelines compile on this machine's adapter, in a renderer with
/// no window: `setup`'s wave grid and flat ocean, made visible, seen by a multisampled camera and
/// an HDR one without MSAA, both with `Camera3d`'s default transmission steps. Needs a GPU, so
/// `#[ignore]`d and skipped by CI:
///
/// ```text
/// cargo test -p visuals water_pipelines_compile -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn water_pipelines_compile() {
    use std::{
        path::Path,
        time::{Duration, Instant},
    };

    use bevy::{
        asset::io::embedded::EmbeddedAssetRegistry,
        log::LogPlugin,
        render::{
            camera::RenderTarget,
            pipelined_rendering::PipelinedRenderingPlugin,
            render_resource::{
                CachedPipelineState, Extent3d, PipelineCache, PipelineDescriptor, ShaderDefVal,
                TextureDimension, TextureFormat, TextureUsages,
            },
            RenderApp,
        },
        window::ExitCondition,
        winit::WinitPlugin,
    };

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .build()
            .disable::<LogPlugin>()
            .disable::<WinitPlugin>()
            // keeps the render app in this app, so the pipeline cache is readable
            .disable::<PipelinedRenderingPlugin>()
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                close_when_requested: false,
            }),
    );
    app.add_plugins(assets::EmbedAssetsPlugin);
    // the coast's shaders as `CoastPlugin` and the terrain embed them; the rest of the coast
    // plugin needs the terrain
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

    let world = app.world_mut();
    let mut oceans = world.query_filtered::<&mut Visibility, With<Ocean>>();
    for mut visibility in oceans.iter_mut(world) {
        *visibility = Visibility::Inherited;
    }
    let mut target = Image::new_fill(
        Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::all(),
    );
    target.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT;
    let mut images = world.resource_mut::<Assets<Image>>();
    let targets = [images.add(target.clone()), images.add(target)];
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

    // a few frames queue every pipeline; then they compile in the background
    for _ in 0..10 {
        app.update();
    }
    let started = Instant::now();
    let mut errors = Vec::new();
    loop {
        app.update();
        let cache = app.sub_app(RenderApp).world().resource::<PipelineCache>();
        errors.clear();
        let mut pending = 0;
        for pipeline in cache.pipelines() {
            match &pipeline.state {
                CachedPipelineState::Ok(_) => {}
                CachedPipelineState::Err(error) => errors.push(error.to_string()),
                _ => pending += 1,
            }
        }
        if pending == 0 {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(120),
            "{pending} pipelines still compiling"
        );
    }
    assert!(
        errors.is_empty(),
        "pipeline errors:\n{}",
        errors.join("\n\n")
    );

    // the sea's pipelines are the ones specialized with the wave constants
    let cache = app.sub_app(RenderApp).world().resource::<PipelineCache>();
    let water = cache
        .pipelines()
        .filter(|pipeline| {
            matches!(
                &pipeline.descriptor,
                PipelineDescriptor::RenderPipelineDescriptor(descriptor)
                    if descriptor.vertex.shader_defs.iter().any(|def| {
                        matches!(def, ShaderDefVal::UInt(name, _) if name == "WAVE_COUNT")
                    }) && descriptor.fragment.as_ref().is_some_and(|fragment| {
                        fragment.shader_defs.iter().any(|def| {
                            matches!(def, ShaderDefVal::UInt(name, _) if name == "REFRACTION_STRENGTH")
                        })
                    })
            ) && matches!(pipeline.state, CachedPipelineState::Ok(_))
        })
        .count();
    println!("{water} water pipelines compiled");
    assert!(water >= 2, "one water pipeline per camera");
}
