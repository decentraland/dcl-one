//! A windowless renderer for the shader compile tests: a camera with the prepasses and a shadowing
//! light, so every pipeline variant the materials need is queued, and a wait until none is
//! pending. The device is held to WebGPU's default limits, so a material that binds more textures
//! than the web allows fails here too; meshes are batched on the CPU, as on the web, since the GPU
//! batching needs more storage buffers than those limits.

use std::time::{Duration, Instant};

use bevy::{
    asset::RenderAssetUsages,
    core_pipeline::prepass::{DepthPrepass, NormalPrepass},
    log::LogPlugin,
    pbr::{environment_map::EnvironmentMapLight, PbrPlugin},
    prelude::*,
    render::{
        camera::RenderTarget,
        pipelined_rendering::PipelinedRenderingPlugin,
        render_resource::{
            CachedPipelineState, Extent3d, PipelineCache, PipelineDescriptor, TextureDimension,
            TextureFormat, TextureUsages, TextureViewDescriptor, TextureViewDimension,
        },
        settings::{WgpuLimits, WgpuSettings},
        RenderApp, RenderPlugin,
    },
    window::ExitCondition,
    winit::WinitPlugin,
};

use crate::landscape_textures::LandscapeTexturesPlugin;

/// An app rendering off-screen, with the embedded shaders and the landscape textures.
pub(crate) fn app() -> App {
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
            })
            .set(PbrPlugin {
                use_gpu_instance_buffer_builder: false,
                ..default()
            })
            .set(RenderPlugin {
                render_creation: WgpuSettings {
                    constrained_limits: Some(WgpuLimits::default()),
                    ..default()
                }
                .into(),
                ..default()
            }),
    );
    app.add_plugins((assets::EmbedAssetsPlugin, LandscapeTexturesPlugin));
    app
}

/// A one-texel cube map, standing in for the sky's environment map.
fn sky_cube(images: &mut Assets<Image>) -> Handle<Image> {
    let mut cube = Image::new_fill(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        &[128, 160, 200, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    );
    cube.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    images.add(cube)
}

/// Spawns the camera, with the sky's environment map, and light, then updates until no pipeline is pending, and fails on any
/// pipeline error or if no pipeline compiled with each of `fragment_shaders` (file names).
pub(crate) fn assert_pipelines_compile(
    app: &mut App,
    from: Vec3,
    looking_at: Vec3,
    fragment_shaders: &[&str],
) {
    let world = app.world_mut();
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
    let target = images.add(target);
    let sky = sky_cube(&mut images);
    world.spawn((
        Camera3d::default(),
        DepthPrepass,
        NormalPrepass,
        Camera {
            target: RenderTarget::Image(target.into()),
            ..default()
        },
        Transform::from_translation(from).looking_at(looking_at, Vec3::Y),
        EnvironmentMapLight {
            diffuse_map: sky.clone(),
            specular_map: sky,
            intensity: 1.0,
            ..default()
        },
    ));
    world.spawn((
        DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(1.0, 3.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // a few frames load the textures and queue the pipelines; then they compile in the background
    for _ in 0..10 {
        app.update();
    }
    let server = app.world().resource::<AssetServer>().clone();
    let started = Instant::now();
    let mut ok;
    let mut errors = Vec::new();
    let mut missing: Vec<&str>;
    loop {
        app.update();
        let cache = app.sub_app(RenderApp).world().resource::<PipelineCache>();
        ok = 0;
        errors.clear();
        missing = fragment_shaders.to_vec();
        let mut pending = 0;
        for pipeline in cache.pipelines() {
            match &pipeline.state {
                CachedPipelineState::Ok(_) => ok += 1,
                CachedPipelineState::Err(error) => errors.push(error.to_string()),
                _ => pending += 1,
            }
            let PipelineDescriptor::RenderPipelineDescriptor(descriptor) = &pipeline.descriptor
            else {
                continue;
            };
            let shader = descriptor
                .fragment
                .as_ref()
                .map(|fragment| &fragment.shader);
            let Some(path) = shader.and_then(|shader| server.get_path(shader.id())) else {
                continue;
            };
            if matches!(pipeline.state, CachedPipelineState::Ok(_)) {
                missing.retain(|name| !path.path().ends_with(name));
            }
        }
        // done once nothing is pending and every named shader compiled, or something failed
        if pending == 0 && (missing.is_empty() || !errors.is_empty()) {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(120),
            "{pending} pipelines still compiling, {ok} compiled, none yet with {missing:?}"
        );
    }
    println!("{ok} pipelines compiled");
    assert!(
        errors.is_empty(),
        "pipeline errors:\n{}",
        errors.join("\n\n")
    );
}
