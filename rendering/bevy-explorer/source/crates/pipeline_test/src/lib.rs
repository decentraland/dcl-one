//! The setup the `#[ignore]`d pipeline-compile tests share (see recipes.md), and a wait for work
//! off the main thread. Only ever a dev-dependency.

#![forbid(unsafe_code)]

use std::{
    fmt::Debug,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use bevy::{
    app::PluginGroupBuilder,
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
        renderer::RenderDevice,
        settings::{WgpuLimits, WgpuSettings},
        RenderApp, RenderPlugin,
    },
    window::ExitCondition,
    winit::WinitPlugin,
};

use assets::EmbedAssetsPlugin;

/// An app with the default plugins, a renderer with no window and the embedded assets. Add the
/// plugins under test, then `finish` and `cleanup` it.
pub fn headless_app() -> App {
    let mut app = headless_app_without_embedded_assets();
    app.add_plugins(EmbedAssetsPlugin);
    app
}

/// Register `EmbedAssetsPlugin` after the plugins under test to exercise the client's asset
/// registration order. Shader dependency requests must wait until startup.
pub fn headless_app_without_embedded_assets() -> App {
    let mut app = App::new();
    app.add_plugins(headless_plugins());
    app
}

/// `headless_app` held to WebGPU's default limits, so a material that binds more than the web
/// allows fails here too, and with meshes batched on the CPU, as on the web: the GPU batching
/// needs more storage buffers than those limits.
pub fn web_limited_app() -> App {
    let mut app = App::new();
    app.add_plugins(
        headless_plugins()
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
    app.add_plugins(EmbedAssetsPlugin);
    app
}

fn headless_plugins() -> PluginGroupBuilder {
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
}

/// A small image for a camera to render to.
pub fn render_target(app: &mut App) -> Handle<Image> {
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
    app.world_mut().resource_mut::<Assets<Image>>().add(target)
}

/// Spawns two cameras at `from` looking at `looking_at`, rendering to a new `render_target`, and a
/// shadowing sun, so every pipeline variant a lit material is drawn with is queued:
/// - one with the depth and normal prepasses and a one-texel sky as its environment map;
/// - one with a depth prepass alone, as the client's camera has with the avatar outline off,
///   whose prepass has no fragment output.
pub fn spawn_prepass_cameras(app: &mut App, from: Vec3, looking_at: Vec3) {
    let target = render_target(app);
    let world = app.world_mut();
    let sky = sky_cube(&mut world.resource_mut::<Assets<Image>>());
    let transform = Transform::from_translation(from).looking_at(looking_at, Vec3::Y);
    world.spawn((
        Camera3d::default(),
        DepthPrepass,
        NormalPrepass,
        Camera {
            target: RenderTarget::Image(target.clone().into()),
            ..default()
        },
        transform,
        EnvironmentMapLight {
            diffuse_map: sky.clone(),
            specular_map: sky,
            intensity: 1.0,
            ..default()
        },
    ));
    world.spawn((
        Camera3d::default(),
        DepthPrepass,
        Camera {
            target: RenderTarget::Image(target.into()),
            order: 1,
            ..default()
        },
        transform,
    ));
    world.spawn((
        DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(1.0, 3.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
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

/// The errors the GPU device reports outside an error scope, from this call on: a bind group that
/// does not match its layout, a buffer bound too small, a shader module that fails validation.
/// Call it on a finished app, before the frames under test, and assert the list is empty after
/// them. Without it wgpu panics on the thread that reports the error.
pub fn gpu_errors(app: &App) -> Arc<Mutex<Vec<String>>> {
    let errors = Arc::new(Mutex::new(Vec::new()));
    let reported = errors.clone();
    app.world()
        .resource::<RenderDevice>()
        .wgpu_device()
        .on_uncaptured_error(Box::new(move |error: wgpu::Error| {
            reported.lock().unwrap().push(error.to_string());
        }));
    errors
}

/// Updates the app until every pipeline it queues has compiled, and returns how many did. Panics
/// on a pipeline error, or when one is still compiling after two minutes.
pub fn compile_pipelines(app: &mut App) -> usize {
    // a few frames queue every pipeline; then they compile in the background
    for _ in 0..10 {
        app.update();
    }
    let started = Instant::now();
    loop {
        app.update();
        let cache = app.sub_app(RenderApp).world().resource::<PipelineCache>();
        let (mut ok, mut pending, mut errors) = (0, 0, Vec::new());
        for pipeline in cache.pipelines() {
            match &pipeline.state {
                CachedPipelineState::Ok(_) => ok += 1,
                CachedPipelineState::Err(error) => errors.push(error.to_string()),
                _ => pending += 1,
            }
        }
        if pending == 0 {
            assert!(
                errors.is_empty(),
                "pipeline errors:\n{}",
                errors.join("\n\n")
            );
            println!("{ok} pipelines compiled");
            return ok;
        }
        assert!(
            started.elapsed() < Duration::from_secs(120),
            "{pending} pipelines still compiling"
        );
    }
}

/// How many compiled pipelines have the shader at `path` as their fragment stage. Panics when
/// that shader never loaded.
pub fn fragment_pipelines(app: &App, path: &str) -> usize {
    let shader = app
        .world()
        .resource::<AssetServer>()
        .get_handle::<Shader>(path)
        .unwrap_or_else(|| panic!("{path} never loaded"))
        .id();
    let cache = app.sub_app(RenderApp).world().resource::<PipelineCache>();
    cache
        .pipelines()
        .filter(|pipeline| {
            matches!(
                &pipeline.descriptor,
                PipelineDescriptor::RenderPipelineDescriptor(descriptor)
                    if descriptor.fragment.as_ref()
                        .is_some_and(|fragment| fragment.shader.id() == shader)
            ) && matches!(pipeline.state, CachedPipelineState::Ok(_))
        })
        .count()
}

/// How long `update_until` waits before it fails.
const DEADLINE: Duration = Duration::from_secs(30);

/// Updates `app` until `pending` lists nothing, and panics naming what `pending` still listed
/// once `DEADLINE` has passed.
///
/// Assets decode on the IO task pool, which every test in the process shares, so how many frames
/// a load takes depends on what the other tests are doing: the wait is measured on the clock, and
/// the thread sleeps between frames so the pool gets to run.
pub fn update_until<T: Debug>(app: &mut App, mut pending: impl FnMut(&mut App) -> Vec<T>) {
    let deadline = Instant::now() + DEADLINE;
    loop {
        app.update();
        let pending = pending(app);
        if pending.is_empty() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "still pending after {DEADLINE:?}: {pending:?}"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}
