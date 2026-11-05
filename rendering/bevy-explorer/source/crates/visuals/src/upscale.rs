//! Render scale: the primary camera draws the world into the scene target, an image at a share of
//! the window's pixels, and the compositor, a 2D camera over the window, draws that image onto
//! the window with FSR 1.0's EASU and sharpens it, as Unity's URP always does (`m_UpscalingFilter`
//! FSR, `m_FsrSharpness`). EASU is `fsr_easu.wgsl`; bevy's contrast adaptive sharpening stands in
//! for FSR's RCAS. At scale 1 the compositor copies the pixels untouched and does not sharpen.
//!
//! The compositor is the window camera with the highest order, so bevy's UI (the scene UI and the
//! native HUD image) renders through it, at the window's full resolution.

use bevy::{
    asset::RenderAssetUsages,
    core_pipeline::contrast_adaptive_sharpening::ContrastAdaptiveSharpening,
    image::{BevyDefault as _, ImageSampler},
    math::FloatOrd,
    prelude::*,
    render::{
        camera::{ImageRenderTarget, RenderTarget, ScalingMode},
        mesh::MeshVertexBufferLayoutRef,
        render_asset::RenderAssetTransferPriority,
        render_resource::{
            AsBindGroup, Extent3d, RenderPipelineDescriptor, ShaderRef,
            SpecializedMeshPipelineError, TextureDimension, TextureFormat, TextureUsages,
        },
        view::RenderLayers,
    },
    sprite::{Material2d, Material2dKey, Material2dPlugin},
    window::PrimaryWindow,
};
use common::structs::{AppConfig, PrimaryCamera, SessionGraphics};

/// unity: UniversalRenderPipeline - High.asset m_FsrSharpness
pub const FSR_SHARPNESS: f32 = 0.92;

/// The compositor and its quad only: keeps them out of every other camera, and keeps 3D gizmos
/// out of the compositor's 2D view.
const COMPOSITOR_LAYER: RenderLayers = RenderLayers::layer(2);

/// The image the primary camera renders the world into, and the render scale it is sized for.
#[derive(Resource)]
pub struct SceneTarget {
    pub image: Handle<Image>,
    pub scale: f32,
}

impl FromWorld for SceneTarget {
    fn from_world(world: &mut World) -> Self {
        let mut image = Image::new_uninit(
            Extent3d::default(),
            TextureDimension::D2,
            TextureFormat::bevy_default(),
            RenderAssetUsages::all(),
        );
        image.texture_descriptor.usage =
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING;
        image.sampler = ImageSampler::linear();
        image.transfer_priority = RenderAssetTransferPriority::Immediate;
        Self {
            image: world.resource_mut::<Assets<Image>>().add(image),
            scale: 1.0,
        }
    }
}

/// The `Camera2d` (order 1) that draws the scene target and carries the UI. It is the only camera
/// drawing to the window, so it clears it: while its quad's pipeline is still compiling (at
/// startup, or on the first switch below scale 1) the window would otherwise show whatever was
/// there before under the UI.
#[derive(Component)]
pub struct Compositor;

/// The compositor quad's material: EASU from the scene target, or a plain copy when the scene
/// target is as large as the window.
#[derive(AsBindGroup, Asset, TypePath, Clone)]
#[bind_group_data(FsrMaterialKey)]
pub struct FsrMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub source: Handle<Image>,
    /// The scene target's size (xy) and the window's (zw), in physical pixels.
    #[uniform(2)]
    pub params: Vec4,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct FsrMaterialKey {
    plain_blit: bool,
}

impl From<&FsrMaterial> for FsrMaterialKey {
    fn from(material: &FsrMaterial) -> Self {
        Self {
            plain_blit: material.params.xy() == material.params.zw(),
        }
    }
}

impl Material2d for FsrMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://shaders/fsr_easu.wgsl".into()
    }

    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if key.bind_group_data.plain_blit {
            if let Some(fragment) = descriptor.fragment.as_mut() {
                fragment.shader_defs.push("PLAIN_BLIT".into());
            }
        }
        Ok(())
    }
}

/// Physical size of the scene target for a window and scale, never below 1x1.
pub fn scene_target_size(window_physical: UVec2, render_scale: f32) -> UVec2 {
    (window_physical.as_vec2() * render_scale)
        .round()
        .as_uvec2()
        .max(UVec2::ONE)
}

/// The scene target's scale factor, which keeps its logical size equal to the window's, so the
/// camera's viewport, cursor rays and projected positions stay in window coordinates.
pub fn scene_target_scale_factor(window_scale_factor: f32, render_scale: f32) -> f32 {
    window_scale_factor * render_scale
}

pub struct UpscalePlugin;

impl Plugin for UpscalePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<FsrMaterial>::default())
            .init_resource::<SceneTarget>()
            .add_systems(Startup, spawn_compositor)
            .add_systems(Update, sync_scene_target);
    }
}

fn spawn_compositor(
    mut commands: Commands,
    scene_target: Res<SceneTarget>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<FsrMaterial>>,
) {
    commands.spawn((
        Compositor,
        Camera2d,
        Camera {
            order: 1,
            ..default()
        },
        // a unit quad fills the view at any window size
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed {
                width: 1.0,
                height: 1.0,
            },
            ..OrthographicProjection::default_2d()
        }),
        Msaa::Off,
        ContrastAdaptiveSharpening {
            enabled: false,
            sharpening_strength: FSR_SHARPNESS,
            denoise: false,
        },
        COMPOSITOR_LAYER,
    ));
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))),
        MeshMaterial2d(materials.add(FsrMaterial {
            source: scene_target.image.clone(),
            params: Vec4::ONE,
        })),
        COMPOSITOR_LAYER,
    ));
}

/// Sizes the scene target for the window and render scale, and points the primary camera, the
/// compositor's material and its sharpening at it.
#[allow(clippy::too_many_arguments)]
fn sync_scene_target(
    windows: Query<&Window, With<PrimaryWindow>>,
    config: Res<AppConfig>,
    session: Res<SessionGraphics>,
    mut scene_target: ResMut<SceneTarget>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<FsrMaterial>>,
    mut primary: Query<&mut Camera, (With<PrimaryCamera>, Without<Compositor>)>,
    mut compositor: Query<(&mut Camera, &mut ContrastAdaptiveSharpening), With<Compositor>>,
    quads: Query<&MeshMaterial2d<FsrMaterial>>,
) {
    let (Ok(window), Ok(mut primary)) = (windows.single(), primary.single_mut()) else {
        return;
    };
    let render_scale = session
        .render_scale(&config)
        .clamp(SessionGraphics::MIN_RENDER_SCALE, 1.0);
    let size = scene_target_size(window.physical_size(), render_scale);
    let output = window.physical_size().max(UVec2::ONE);
    let target = ImageRenderTarget {
        handle: scene_target.image.clone(),
        scale_factor: FloatOrd(scene_target_scale_factor(
            window.scale_factor(),
            render_scale,
        )),
    };

    let retarget = !matches!(&primary.target, RenderTarget::Image(current) if *current == target);
    let resize = images
        .get(&scene_target.image)
        .is_some_and(|image| image.size() != size);
    if retarget || resize {
        // touched either way: the image's change event is what makes bevy recompute the camera's
        // target size and scale factor
        if let Some(image) = images.get_mut(&scene_target.image) {
            image.texture_descriptor.size = Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            };
        }
        primary.target = RenderTarget::Image(target);
    }
    if scene_target.scale != render_scale {
        scene_target.scale = render_scale;
    }

    let params = Vec4::new(
        size.x as f32,
        size.y as f32,
        output.x as f32,
        output.y as f32,
    );
    for quad in &quads {
        // a retarget may give the image a new GPU texture, and the material's bind group only
        // follows when the material changes too: `params` may not (a scale factor change at the
        // same physical size), so the material is touched on every retarget
        let stale =
            retarget || resize || materials.get(&quad.0).is_some_and(|m| m.params != params);
        if stale {
            if let Some(material) = materials.get_mut(&quad.0) {
                material.params = params;
            }
        }
    }
    let sharpen = size != output;
    for (mut camera, mut cas) in &mut compositor {
        if camera.is_active != primary.is_active {
            camera.is_active = primary.is_active;
        }
        if cas.enabled != sharpen {
            cas.enabled = sharpen;
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::window::WindowResolution;

    use super::*;

    #[test]
    fn scene_target_size_rounds_and_clamps() {
        assert_eq!(
            scene_target_size(UVec2::new(1920, 1080), 0.5),
            UVec2::new(960, 540)
        );
        assert_eq!(
            scene_target_size(UVec2::new(1919, 1079), 0.5),
            UVec2::new(960, 540)
        );
        assert_eq!(scene_target_size(UVec2::ZERO, 0.5), UVec2::ONE);
        assert_eq!(
            scene_target_size(UVec2::new(1920, 1080), 1.0),
            UVec2::new(1920, 1080)
        );
    }

    #[test]
    fn logical_size_is_preserved() {
        assert!((scene_target_scale_factor(2.0, 0.5) - 1.0).abs() < 1e-6);
        assert!((scene_target_scale_factor(1.0, 1.0) - 1.0).abs() < 1e-6);
    }

    /// An 800x600 window at half scale draws the world at 400x300 into the scene target; a window
    /// minimised to nothing leaves a 1x1 target rather than an empty texture.
    #[test]
    fn scene_target_follows_the_window() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_asset::<FsrMaterial>()
            .init_resource::<SceneTarget>()
            .insert_resource(AppConfig::default())
            .insert_resource(SessionGraphics {
                render_scale: Some(0.5),
                ..default()
            })
            .add_systems(Update, sync_scene_target);
        let window = app
            .world_mut()
            .spawn((
                Window {
                    resolution: WindowResolution::new(800.0, 600.0).with_scale_factor_override(1.0),
                    ..default()
                },
                PrimaryWindow,
            ))
            .id();
        let camera = app
            .world_mut()
            .spawn((Camera::default(), PrimaryCamera::default()))
            .id();

        app.update();
        let image = app.world().resource::<SceneTarget>().image.clone();
        let size = |app: &App| {
            app.world()
                .resource::<Assets<Image>>()
                .get(&image)
                .unwrap()
                .size()
        };
        assert_eq!(size(&app), UVec2::new(400, 300));
        match &app.world().get::<Camera>(camera).unwrap().target {
            RenderTarget::Image(target) => {
                assert_eq!(target.handle, image);
                assert_eq!(target.scale_factor.0, 0.5);
            }
            other => panic!("primary camera renders to {other:?}"),
        }
        assert_eq!(app.world().resource::<SceneTarget>().scale, 0.5);

        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set_physical_resolution(0, 0);
        app.update();
        assert_eq!(size(&app), UVec2::ONE);
    }

    /// The compositor's pipelines compile on this machine's adapter, in a renderer with no
    /// window: one 2D camera draws a 64x64 source onto a 64x64 image (the plain copy), another
    /// onto a 128x128 image (EASU, with the sharpening after it). Needs a GPU, so `#[ignore]`d
    /// and skipped by CI:
    ///
    /// ```text
    /// cargo test -p visuals fsr_material_compiles -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "needs a GPU; run with --ignored"]
    fn fsr_material_compiles() {
        use std::time::{Duration, Instant};

        use bevy::{
            log::LogPlugin,
            render::view::screenshot::{Screenshot, ScreenshotCaptured},
            render::{
                pipelined_rendering::PipelinedRenderingPlugin,
                render_resource::{CachedPipelineState, PipelineCache, PipelineDescriptor},
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
        app.add_plugins((assets::EmbedAssetsPlugin, UpscalePlugin))
            .insert_resource(AppConfig::default())
            .init_resource::<SessionGraphics>();
        app.finish();
        app.cleanup();

        let world = app.world_mut();
        let extent = |size: u32| Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        };
        // a per-pixel pattern, so an offset or a flip in the copy shows
        let pattern: Vec<u8> = (0..64u8)
            .flat_map(|y| (0..64u8).flat_map(move |x| [x, y, x ^ y, 255]))
            .collect();
        let mut images = world.resource_mut::<Assets<Image>>();
        let source = images.add(Image::new(
            extent(64),
            TextureDimension::D2,
            pattern.clone(),
            TextureFormat::bevy_default(),
            RenderAssetUsages::all(),
        ));
        // blank outputs, so a compositor that never draws fails the check
        let output = |size: u32| {
            let mut image = Image::new_fill(
                extent(size),
                TextureDimension::D2,
                &[0, 0, 0, 0],
                TextureFormat::bevy_default(),
                RenderAssetUsages::all(),
            );
            image.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT;
            image
        };
        let outputs = [images.add(output(64)), images.add(output(128))];
        let mesh = world
            .resource_mut::<Assets<Mesh>>()
            .add(Rectangle::new(1.0, 1.0));
        for (layer, output, size) in [(6, &outputs[0], 64.0), (7, &outputs[1], 128.0)] {
            let material = world
                .resource_mut::<Assets<FsrMaterial>>()
                .add(FsrMaterial {
                    source: source.clone(),
                    params: Vec4::new(64.0, 64.0, size, size),
                });
            world.spawn((
                Camera2d,
                Camera {
                    target: RenderTarget::Image(output.clone().into()),
                    ..default()
                },
                Projection::Orthographic(OrthographicProjection {
                    scaling_mode: ScalingMode::Fixed {
                        width: 1.0,
                        height: 1.0,
                    },
                    ..OrthographicProjection::default_2d()
                }),
                Msaa::Off,
                ContrastAdaptiveSharpening {
                    enabled: size != 64.0,
                    sharpening_strength: FSR_SHARPNESS,
                    denoise: false,
                },
                RenderLayers::layer(layer),
            ));
            world.spawn((
                Mesh2d(mesh.clone()),
                MeshMaterial2d(material),
                RenderLayers::layer(layer),
            ));
        }

        // a few frames queue every pipeline; then they compile in the background, and once they
        // are ready a few more frames draw with them
        for _ in 0..10 {
            app.update();
        }
        let started = Instant::now();
        let mut ok;
        let mut errors = Vec::new();
        loop {
            app.update();
            let cache = app.sub_app(RenderApp).world().resource::<PipelineCache>();
            ok = 0;
            errors.clear();
            let mut pending = 0;
            for pipeline in cache.pipelines() {
                match &pipeline.state {
                    CachedPipelineState::Ok(_) => ok += 1,
                    CachedPipelineState::Err(error) => errors.push(error.to_string()),
                    _ => pending += 1,
                }
            }
            if pending == 0 {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(60),
                "{pending} pipelines still compiling"
            );
        }
        // the plain copy leaves every pixel as it was
        let copied = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = copied.clone();
        app.world_mut()
            .spawn(Screenshot::image(outputs[0].clone()))
            .observe(move |trigger: Trigger<ScreenshotCaptured>| {
                *sink.lock().unwrap() = Some(trigger.0.clone());
            });
        for _ in 0..5 {
            app.update();
        }
        println!("{ok} pipelines compiled");
        assert!(
            errors.is_empty(),
            "pipeline errors:\n{}",
            errors.join("\n\n")
        );

        let shader = app
            .world()
            .resource::<AssetServer>()
            .load::<Shader>("embedded://shaders/fsr_easu.wgsl")
            .id();
        let render_world = app.sub_app(RenderApp).world();
        let plain_blit: Vec<bool> = render_world
            .resource::<PipelineCache>()
            .pipelines()
            .filter_map(|pipeline| match &pipeline.descriptor {
                PipelineDescriptor::RenderPipelineDescriptor(descriptor) => {
                    descriptor.fragment.as_ref()
                }
                _ => None,
            })
            .filter(|fragment| fragment.shader.id() == shader)
            .map(|fragment| fragment.shader_defs.contains(&"PLAIN_BLIT".into()))
            .collect();
        assert_eq!(plain_blit.len(), 2, "one compositor pipeline per camera");
        assert!(plain_blit.contains(&true) && plain_blit.contains(&false));

        let copied = copied
            .lock()
            .unwrap()
            .take()
            .expect("the copy was captured");
        assert!(
            copied.data.as_deref() == Some(pattern.as_slice()),
            "the plain copy changed pixels"
        );
    }
}
