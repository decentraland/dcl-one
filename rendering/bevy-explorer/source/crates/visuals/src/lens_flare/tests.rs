use bevy::render::camera::CameraProjection;

use super::*;

#[test]
fn flare_intensity_follows_the_preset_keys() {
    assert_eq!(flare_intensity(0.5, 0.0), 1.0);
    assert_eq!(flare_intensity(0.8, 0.0), 0.0);
    assert!((flare_intensity(0.0, 0.0) - 0.6).abs() < 1e-6);
    assert_eq!(flare_intensity(0.5, 1.0), 0.0);
}

#[test]
fn flare_set_switches_at_the_start_times() {
    assert!(core::ptr::eq(flare_set(0.19), NIGHT_FLARE.as_slice()));
    assert!(core::ptr::eq(flare_set(0.2), DAY_FLARE.as_slice()));
    assert!(core::ptr::eq(flare_set(0.8), NIGHT_FLARE.as_slice()));
}

fn camera(forward: Vec3) -> (Mat4, Vec3, Vec3) {
    let view = Transform::default().looking_to(forward, Vec3::Y);
    let projection = PerspectiveProjection::default().get_clip_from_view();
    let clip_from_world = projection * view.compute_matrix().inverse();
    (clip_from_world, *view.up(), *view.forward())
}

fn flare(toward_light: Vec3, forward: Vec3) -> Option<LensFlareUniform> {
    let settings = LensFlareSettings {
        toward_light,
        intensity: 1.0,
        ..default()
    };
    let (clip_from_world, up, forward) = camera(forward);
    flare_uniform(&settings, clip_from_world, up, forward, 16.0 / 9.0)
}

/// A light ahead of the camera shows its flare at its screen position; below the horizon, behind
/// the camera or off screen it shows none.
#[test]
fn flare_shows_only_for_a_light_in_view_above_the_horizon() {
    let up_ahead = Vec3::new(0.0, 0.2, -1.0).normalize();
    let shown = flare(up_ahead, Vec3::NEG_Z).expect("a light ahead shows its flare");
    // y down on screen: a light above the centre is at negative y
    assert!(shown.sun.x.abs() < 1e-5 && shown.sun.y < 0.0);
    assert!(shown.occlusion_radius > 0.0 && shown.occlusion_radius < 0.01);
    assert_eq!(shown.count as usize, quad_count(&DAY_FLARE));

    let below = Vec3::new(0.0, -0.2, -1.0).normalize();
    assert!(flare(below, Vec3::new(0.0, -0.2, -1.0)).is_none());
    assert!(flare(up_ahead, Vec3::Z).is_none());
    assert!(flare(up_ahead, Vec3::new(1.0, 0.0, -0.3)).is_none());
}

/// Every copy of every element is a quad, and each sits on the ray from the light through the
/// screen centre: the glow on the light, the others where their position puts them.
#[test]
fn quads_follow_the_ray_through_the_screen_centre() {
    let sun = Vec2::new(0.4, -0.3);
    let quads = flare_quads(&DAY_FLARE, sun, 16.0 / 9.0, Vec3::ONE, 1.0);
    assert_eq!(quads.len(), quad_count(&DAY_FLARE));
    assert_eq!(
        flare_quads(&NIGHT_FLARE, sun, 1.0, Vec3::ONE, 1.0).len(),
        quad_count(&NIGHT_FLARE)
    );

    // the glow, at position 0
    assert!(quads[0].centre.distance(sun) < 1e-6);
    assert_eq!(quads[0].kind, KIND_CIRCLE);
    // the full ring, at position 0.35: twice that along the ray towards the centre
    let ring = quads.iter().find(|q| q.kind == KIND_RING).unwrap();
    assert!(ring.centre.distance(sun * (1.0 - 0.7)) < 1e-6);
    // every copy lies on the line through the light and the centre
    for quad in &quads {
        assert!(quad.centre.perp_dot(sun).abs() < 1e-5, "{quad:?}");
        assert!(quad.size.min_element() > 0.0);
        assert!((quad.turn.length() - 1.0).abs() < 1e-5);
    }
    // the flare's intensity scales every quad's light
    let dim = flare_quads(&DAY_FLARE, sun, 16.0 / 9.0, Vec3::ONE, 0.5);
    for (bright, dim) in quads.iter().zip(&dim) {
        assert!((bright.color * 0.5 - dim.color).abs().max_element() < 1e-5);
    }
}

/// The day's full ring grows from its own size toward 20 times it as it moves off the screen's
/// centre; the partial ring's target is its own size, so it keeps it.
#[test]
fn the_full_ring_grows_away_from_the_centre() {
    let size_of = |sun: Vec2, kind: u32| {
        flare_quads(&DAY_FLARE, sun, 1.0, Vec3::ONE, 1.0)
            .into_iter()
            .find(|q| q.kind == kind)
            .unwrap()
            .size
    };
    let base = SCALE_SIZE * 4.2;
    let centred = size_of(Vec2::ZERO, KIND_RING);
    assert!((centred.x - base).abs() < 1e-6);
    // the ring sits at 0.3 of the light's offset: half way to the edge for a light at 1/0.6
    let off = size_of(Vec2::new(0.5 / 0.3, 0.0), KIND_RING);
    assert!(
        (off.x - (base + 0.5 * (20.0 * base - base))).abs() < 1e-5,
        "{off}"
    );
    let partial = size_of(Vec2::new(0.9, 0.0), KIND_PARTIAL_RING);
    assert!((partial.x - SCALE_SIZE * 3.93).abs() < 1e-6);
}

/// The light's colour tints only the elements that ask for it.
#[test]
fn light_colour_tints_only_modulated_elements() {
    let sun = Vec2::new(0.2, 0.1);
    let white = flare_quads(&DAY_FLARE, sun, 1.0, Vec3::ONE, 1.0);
    let red = flare_quads(&DAY_FLARE, sun, 1.0, Vec3::new(1.0, 0.0, 0.0), 1.0);
    let ring = white.iter().position(|q| q.kind == KIND_RING).unwrap();
    assert_eq!(white[ring].color, red[ring].color);
    assert_eq!(red[0].color.y, 0.0);
    assert!(white[0].color.y > 0.0);
}

/// The flare pipelines compile on this machine's adapter, and the node draws with them, in a
/// renderer with no window: a multisampled camera, an HDR one without MSAA and one with depth of
/// field (so with the transparent focus texture), all with `LensFlare`, facing a directional
/// light at noon. Needs a GPU, so `#[ignore]`d and skipped by CI:
///
/// ```text
/// cargo test -p visuals lens_flare_pipelines_compile -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn lens_flare_pipelines_compile() {
    use bevy::{core_pipeline::dof::DepthOfField, render::camera::RenderTarget};
    use pipeline_test::{compile_pipelines, headless_app, render_target};

    let mut app = headless_app();
    app.add_plugins((
        bevy::pbr::wireframe::WireframePlugin::default(),
        LensFlarePlugin,
    ));
    // the light ahead of the cameras and up, at noon
    let toward_light = Vec3::new(0.0, 0.2, -1.0).normalize();
    app.insert_resource(SceneGlobalLight {
        dir_color: Color::WHITE,
        dir_direction: -toward_light,
        ..default()
    });
    app.insert_resource(TimeOfDay {
        time: 12.0 * 3600.0,
    });
    app.finish();
    app.cleanup();

    // the wireframe pass, which resolves the MSAA frame again, runs before the flare
    let graph = app.sub_app(RenderApp).world().resource::<RenderGraph>();
    let flare_inputs: Vec<_> = graph
        .get_sub_graph(Core3d)
        .unwrap()
        .iter_node_inputs(LensFlareLabel)
        .unwrap()
        .map(|(_, input)| input.label)
        .collect();
    assert!(
        flare_inputs.contains(&Node3d::Wireframe.intern()),
        "the flare comes after the wireframe pass"
    );

    let targets = [
        render_target(&mut app),
        render_target(&mut app),
        render_target(&mut app),
    ];
    let world = app.world_mut();
    world.spawn((
        DirectionalLight::default(),
        Transform::default().looking_to(-toward_light, Vec3::Y),
    ));
    for (target, hdr, msaa, dof) in [
        (&targets[0], false, Msaa::Sample4, false),
        (&targets[1], true, Msaa::Off, false),
        (&targets[2], true, Msaa::Off, true),
    ] {
        let mut camera = world.spawn((
            Camera3d::default(),
            Camera {
                target: RenderTarget::Image(target.clone().into()),
                hdr,
                ..default()
            },
            msaa,
            LensFlare,
            Transform::default().looking_to(Vec3::NEG_Z, Vec3::Y),
        ));
        if dof {
            camera.insert(DepthOfField::default());
        }
    }

    compile_pipelines(&mut app);
    for _ in 0..5 {
        app.update();
    }

    let render_world = app.sub_app_mut(RenderApp).world_mut();
    let keys: Vec<LensFlarePipelineKey> = render_world
        .query::<&LensFlarePipelineId>()
        .iter(render_world)
        .map(|id| id.key)
        .collect();
    assert_eq!(keys.len(), 3, "one flare pipeline per camera");
    assert!(
        keys.iter().any(|key| key.focus),
        "the depth of field camera reads the focus texture: {keys:?}"
    );
    let drawing = render_world
        .query::<&LensFlareUniformOffset>()
        .iter(render_world)
        .count();
    assert_eq!(drawing, 3, "every camera draws the flare");

    // turning the flare off (the setting removes `LensFlare`) stops it drawing: the render
    // world's views keep their components between frames, so the extraction has to remove the
    // marker itself
    let main_world = app.world_mut();
    let cameras: Vec<Entity> = main_world
        .query_filtered::<Entity, With<LensFlare>>()
        .iter(main_world)
        .collect();
    for camera in cameras {
        main_world.entity_mut(camera).remove::<LensFlare>();
    }
    for _ in 0..3 {
        app.update();
    }
    let render_world = app.sub_app_mut(RenderApp).world_mut();
    let flaring = render_world
        .query_filtered::<(), With<LensFlareView>>()
        .iter(render_world)
        .count();
    assert_eq!(flaring, 0, "no view flares once LensFlare is removed");
}

/// What stands between the camera and the light in [`lens_flare_renders`].
#[derive(Clone, Copy, PartialEq)]
enum Blocker {
    None,
    /// an opaque wall, which writes depth
    Opaque,
    /// a transparent sheet, which writes no depth but covers the focus texture
    Transparent,
}

/// Renders the flare headless into a `FRAME` square black frame, the light up and to the right of the
/// view centre, and returns the frame.
fn render_flare(blocker: Blocker) -> bevy::image::Image {
    use bevy::{
        asset::RenderAssetUsages,
        render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
    };
    use bevy::{
        core_pipeline::{dof::DepthOfField, tonemapping::Tonemapping},
        render::{
            camera::RenderTarget,
            view::screenshot::{Screenshot, ScreenshotCaptured},
        },
    };
    use pipeline_test::{compile_pipelines, headless_app};

    #[derive(Resource, Default)]
    struct Captured(Option<bevy::image::Image>);

    let mut app = headless_app();
    app.add_plugins(LensFlarePlugin)
        .init_resource::<Captured>()
        .insert_resource(SceneGlobalLight {
            dir_color: Color::WHITE,
            dir_direction: -toward_test_light(),
            ..default()
        })
        .insert_resource(TimeOfDay {
            time: 12.0 * 3600.0,
        });
    app.finish();
    app.cleanup();

    let mut image = Image::new_fill(
        Extent3d {
            width: FRAME,
            height: FRAME,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::all(),
    );
    image.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    let world = app.world_mut();
    let black = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: Color::BLACK,
            unlit: true,
            alpha_mode: if blocker == Blocker::Transparent {
                AlphaMode::Blend
            } else {
                AlphaMode::Opaque
            },
            ..default()
        });
    let wall = world
        .resource_mut::<Assets<Mesh>>()
        .add(Rectangle::new(20.0, 20.0));
    if blocker != Blocker::None {
        world.spawn((
            Mesh3d(wall),
            MeshMaterial3d(black),
            Transform::from_xyz(0.0, 0.0, -5.0),
        ));
    }
    let mut camera = world.spawn((
        Camera3d::default(),
        Camera {
            target: RenderTarget::Image(target.clone().into()),
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        Msaa::Off,
        Tonemapping::None,
        LensFlare,
        Transform::default().looking_to(Vec3::NEG_Z, Vec3::Y),
    ));
    if blocker == Blocker::Transparent {
        // the transparent focus texture comes with depth of field
        camera.insert(DepthOfField::default());
    }

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
    loop {
        app.update();
        if let Some(image) = app.world_mut().resource_mut::<Captured>().0.take() {
            return image;
        }
        assert!(start.elapsed().as_secs() < 30, "GPU capture timed out");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// The side of [`render_flare`]'s frame, in pixels.
const FRAME: u32 = 256;

fn toward_test_light() -> Vec3 {
    Vec3::new(0.15, 0.15, -1.0).normalize()
}

/// The flare draws on the light's pixel and across the screen, and hides behind an opaque wall
/// and, through the focus texture, behind a transparent sheet. Needs a GPU, so `#[ignore]`d and
/// skipped by CI:
///
/// ```text
/// cargo test -p visuals lens_flare_renders -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn lens_flare_renders() {
    let rgba = |image: bevy::image::Image| {
        image
            .try_into_dynamic()
            .expect("captured RGBA image")
            .to_rgba8()
    };

    let open = rgba(render_flare(Blocker::None));
    if let Some(path) = std::env::var_os("DCL_LENS_FLARE_PNG") {
        open.save(path).unwrap();
    }
    // where the light is: its direction through bevy's default projection
    let (clip_from_world, _, _) = camera(Vec3::NEG_Z);
    let clip = clip_from_world * toward_test_light().extend(0.0);
    let ndc = clip.xy() / clip.w;
    let half = 0.5 * FRAME as f32;
    let light = UVec2::new(((ndc.x + 1.0) * half) as u32, ((1.0 - ndc.y) * half) as u32);
    // the glow: full on the light, warm around it (its tint is orange; the rings are faint)
    let on = open.get_pixel(light.x, light.y).0;
    let beside = open.get_pixel(light.x + 24, light.y).0;
    println!("light at {light}: {on:?}, 24 px right: {beside:?}");
    assert!(on[0] >= 250, "the glow sits on the light: {on:?}");
    assert!(beside[0] > beside[2] + 40, "the glow is warm: {beside:?}");
    // the same distance on the far side of the screen centre is darker
    let mirrored = open.get_pixel(FRAME - 1 - light.x, FRAME - 1 - light.y).0;
    assert!(mirrored[0] < on[0], "{mirrored:?}");
    // the ghosts reach the lower left half, away from the light (y runs down)
    let away: u32 = open
        .enumerate_pixels()
        .filter(|(x, y, _)| x < y)
        .map(|(_, _, p)| p.0[0] as u32 + p.0[1] as u32 + p.0[2] as u32)
        .sum();
    assert!(away > 0, "no flare light away from the light");

    for blocker in [Blocker::Opaque, Blocker::Transparent] {
        let hidden = rgba(render_flare(blocker));
        let light: u32 = hidden
            .pixels()
            .map(|p| p.0[0] as u32 + p.0[1] as u32 + p.0[2] as u32)
            .sum();
        assert_eq!(light, 0, "the flare shows through the wall");
    }
}
