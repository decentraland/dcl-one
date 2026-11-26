//! The star field and the shooting stars, drawn at screen resolution instead of being baked into
//! the sky cubemap and blurred with it. See `stars.wgsl`.
//!
//! Every view draws them before bloom, so they glow: the night stars are several times brighter
//! than white, and the glow is what shows their twinkle and the shooting stars' trails. Depth of
//! field, which runs after bloom, leaves them sharp: it keeps the sky (cleared depth) in focus
//! away from geometry. A view with depth of field draws them at the end of the main pass
//! and dims them behind transparent geometry by its coverage in DoF's transparent focus texture.
//! Any other view, which includes every MSAA view (depth of field yields to MSAA), has no such
//! coverage, so it draws them before the transparent pass, into the multisampled target, and
//! transparent geometry then covers them as it covers the sky.

use bevy::{
    core_pipeline::{
        core_3d::graph::{Core3d, Node3d},
        dof::TransparentFocusTexture,
        fullscreen_vertex_shader::fullscreen_shader_vertex_state,
    },
    ecs::{
        query::{Has, QueryItem},
        system::lifetimeless::Read,
    },
    image::BevyDefault as _,
    platform::collections::HashMap,
    prelude::*,
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::RenderAssets,
        render_graph::{
            NodeRunError, RenderGraphApp, RenderGraphContext, RenderLabel, ViewNode, ViewNodeRunner,
        },
        render_resource::{
            binding_types::{
                sampler, texture_2d, texture_2d_multisampled, texture_cube, texture_depth_2d,
                texture_depth_2d_multisampled, uniform_buffer,
            },
            BindGroupEntries, BindGroupLayout, BlendComponent, BlendFactor, BlendOperation,
            BlendState, CachedRenderPipelineId, ColorTargetState, ColorWrites, FragmentState,
            LoadOp, MultisampleState, Operations, PipelineCache, PrimitiveState,
            RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor,
            SamplerBindingType, ShaderStages, ShaderType, SpecializedRenderPipeline,
            SpecializedRenderPipelines, StoreOp, TextureFormat, TextureSampleType, TextureUsages,
            UniformBuffer,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        texture::GpuImage,
        view::{
            ExtractedView, Msaa, ViewDepthTexture, ViewTarget, ViewUniform, ViewUniformOffset,
            ViewUniforms,
        },
        Render, RenderApp, RenderSet,
    },
};
use bevy_atmosphere::{
    pipeline::AtmosphereImage,
    prelude::{AtmosphereCamera, AtmosphereModel},
};
use common::sky_preset::{evaluate_by_phase, STARS_V2};

use crate::stylized_sky::StylizedSky;

/// The star brightness the protocol gives a scene that sets none: a scene's value scales the
/// preset's stars from it.
const SCENE_STAR_BRIGHTNESS: f32 = 4.62;

pub struct StarsPlugin;

impl Plugin for StarsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StarsSettings>()
            .init_resource::<StarsNoise>()
            .add_plugins((
                ExtractResourcePlugin::<StarsSettings>::default(),
                ExtractResourcePlugin::<StarsNoise>::default(),
                ExtractComponentPlugin::<StarsView>::default(),
            ))
            .add_systems(Update, (update_stars_settings, bind_depth_texture));

        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .init_resource::<SpecializedRenderPipelines<StarsPipeline>>()
            .init_resource::<StarsUniformBuffer>()
            .add_systems(
                Render,
                (
                    prepare_stars_pipelines.in_set(RenderSet::Prepare),
                    prepare_stars_uniform.in_set(RenderSet::PrepareResources),
                ),
            )
            .add_render_graph_node::<ViewNodeRunner<StarsNode<true>>>(Core3d, StarsEarlyLabel)
            .add_render_graph_edges(
                Core3d,
                (
                    Node3d::MainTransmissivePass,
                    StarsEarlyLabel,
                    Node3d::MainTransparentPass,
                ),
            )
            .add_render_graph_node::<ViewNodeRunner<StarsNode<false>>>(Core3d, StarsLabel)
            .add_render_graph_edges(Core3d, (Node3d::EndMainPass, StarsLabel, Node3d::Bloom));
    }

    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp).init_resource::<StarsPipeline>();
    }
}

pub use settings::StarsSettings;

mod settings {
    #![expect(
        dead_code,
        reason = "the ShaderType derive emits a never-called check fn per field"
    )]

    use super::*;

    /// The star field's and the shooting stars' parameters at the current day phase, and the
    /// time that drives them.
    #[derive(Resource, Clone, Copy, ExtractResource, ShaderType)]
    pub struct StarsSettings {
        /// brightness at the current day phase
        pub brightness: f32,
        pub twinkle: f32,
        pub patch_strength: f32,
        /// shooting-star rate at the current day phase
        pub shooting_rate: f32,
        /// cells along each edge of a cube face
        pub cells: f32,
        /// star radius in radians
        pub radius: f32,
        /// turns per second about the zenith
        pub rotation_speed: f32,
        pub twinkle_speed: f32,
        /// the heights (the direction's y) the fade to the horizon starts and ends at
        pub horizon_fade: Vec2,
        /// seconds
        pub time: f32,
    }
}

impl Default for StarsSettings {
    fn default() -> Self {
        Self {
            brightness: 0.0,
            twinkle: STARS_V2.twinkle,
            patch_strength: STARS_V2.patch_strength,
            shooting_rate: 0.0,
            cells: STARS_V2.density,
            radius: STARS_V2.size.to_radians(),
            rotation_speed: STARS_V2.rotation_speed,
            twinkle_speed: STARS_V2.twinkle_speed,
            horizon_fade: STARS_V2.horizon_fade,
            time: 0.0,
        }
    }
}

/// The sky's horizon noise, which the star pass reads for its dim patches.
#[derive(Resource, Clone, Default, ExtractResource)]
struct StarsNoise(Handle<Image>);

/// The stars' brightness and the shooting-star rate at a day phase. A scene's star brightness
/// scales the stars from the protocol's default, and dims the shooting stars with them without
/// ever raising their rate. Nothing is `shown` while a scene shows a panorama.
fn star_levels(phase: f32, scene_brightness: Option<f32>, shown: bool) -> (f32, f32) {
    if !shown {
        return (0.0, 0.0);
    }
    let visibility = evaluate_by_phase(STARS_V2.brightness_by_phase, phase);
    let scene_scale = scene_brightness.map_or(1.0, |brightness| {
        brightness.max(0.0) / SCENE_STAR_BRIGHTNESS
    });
    (
        STARS_V2.brightness * visibility * scene_scale,
        STARS_V2.shooting_rate * visibility * scene_scale.min(1.0),
    )
}

/// The stars come out with the sky's day phase.
fn update_stars_settings(
    environment: Res<crate::environment::SceneEnvironment>,
    atmosphere: Res<AtmosphereModel>,
    time: Res<Time>,
    mut settings: ResMut<StarsSettings>,
    mut noise: ResMut<StarsNoise>,
) {
    let Some(sky) = atmosphere.to_ref::<StylizedSky>() else {
        return;
    };
    // a scene that hides the sun hides its discs, not the stars
    (settings.brightness, settings.shooting_rate) = star_levels(
        sky.phase,
        environment.star_brightness,
        environment.texture.is_none(),
    );
    settings.time = time.elapsed_secs();
    if noise.0 != sky.noise {
        noise.0 = sky.noise.clone();
    }
}

/// The star pass reads the view's depth texture, which bevy only makes bindable on request.
fn bind_depth_texture(mut cameras: Query<&mut Camera3d, With<AtmosphereCamera>>) {
    for mut camera in cameras.iter_mut() {
        let usages = TextureUsages::from(camera.depth_texture_usages);
        if !usages.contains(TextureUsages::TEXTURE_BINDING) {
            camera.depth_texture_usages = (usages | TextureUsages::TEXTURE_BINDING).into();
        }
    }
}

/// A view that draws the sky, and so the stars.
#[derive(Component, Clone)]
struct StarsView;

impl ExtractComponent for StarsView {
    type QueryData = &'static AtmosphereCamera;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(_: QueryItem<'_, Self::QueryData>) -> Option<Self> {
        Some(Self)
    }
}

#[derive(Resource, Default)]
struct StarsUniformBuffer(UniformBuffer<StarsSettings>);

fn prepare_stars_uniform(
    settings: Res<StarsSettings>,
    mut buffer: ResMut<StarsUniformBuffer>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    buffer.0.set(*settings);
    buffer.0.write_buffer(&device, &queue);
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
struct StarsPipelineKey {
    samples: u32,
    /// the view has depth of field, and so a transparent focus texture to read coverage from
    focus: bool,
    hdr: bool,
}

#[derive(Resource)]
struct StarsPipeline {
    shader: Handle<Shader>,
    layouts: HashMap<(u32, bool), BindGroupLayout>,
}

impl FromWorld for StarsPipeline {
    fn from_world(world: &mut World) -> Self {
        let shader = world
            .resource::<AssetServer>()
            .load("embedded://shaders/stars.wgsl");
        let device = world.resource::<RenderDevice>();

        let mut layouts = HashMap::new();
        for samples in [1, 2, 4, 8] {
            let multisampled = samples > 1;
            for focus in [false, true] {
                let mut entries = vec![
                    uniform_buffer::<ViewUniform>(true).build(0, ShaderStages::FRAGMENT),
                    if multisampled {
                        texture_depth_2d_multisampled()
                    } else {
                        texture_depth_2d()
                    }
                    .build(1, ShaderStages::FRAGMENT),
                    texture_cube(TextureSampleType::Float { filterable: true })
                        .build(2, ShaderStages::FRAGMENT),
                    sampler(SamplerBindingType::Filtering).build(3, ShaderStages::FRAGMENT),
                    uniform_buffer::<StarsSettings>(false).build(4, ShaderStages::FRAGMENT),
                    // the horizon noise
                    texture_2d(TextureSampleType::Float { filterable: true })
                        .build(5, ShaderStages::FRAGMENT),
                    sampler(SamplerBindingType::Filtering).build(6, ShaderStages::FRAGMENT),
                ];
                if focus {
                    entries.push(
                        if multisampled {
                            texture_2d_multisampled(TextureSampleType::Float { filterable: false })
                        } else {
                            texture_2d(TextureSampleType::Float { filterable: false })
                        }
                        .build(7, ShaderStages::FRAGMENT),
                    );
                }
                layouts.insert(
                    (samples, focus),
                    device.create_bind_group_layout(Some("stars bind group layout"), &entries),
                );
            }
        }

        Self { shader, layouts }
    }
}

impl SpecializedRenderPipeline for StarsPipeline {
    type Key = StarsPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let mut shader_defs = Vec::new();
        if key.samples > 1 {
            shader_defs.push("MULTISAMPLED".into());
        }
        if key.focus {
            shader_defs.push("FOCUS".into());
        }
        RenderPipelineDescriptor {
            label: Some("stars pipeline".into()),
            layout: vec![self.layouts[&(key.samples, key.focus)].clone()],
            push_constant_ranges: Vec::new(),
            vertex: fullscreen_shader_vertex_state(),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState {
                // without the focus texture the stars draw into the main pass's (multisampled)
                // target; with it, onto the resolved frame at the end of the main pass
                count: if key.focus { 1 } else { key.samples },
                ..default()
            },
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                shader_defs,
                entry_point: "fragment".into(),
                targets: vec![Some(ColorTargetState {
                    format: if key.hdr {
                        ViewTarget::TEXTURE_FORMAT_HDR
                    } else {
                        TextureFormat::bevy_default()
                    },
                    // stars only add light to the frame already there
                    blend: Some(BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::One,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent {
                            src_factor: BlendFactor::Zero,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Add,
                        },
                    }),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            zero_initialize_workgroup_memory: false,
        }
    }
}

#[derive(Component)]
struct StarsPipelineId {
    id: CachedRenderPipelineId,
    key: StarsPipelineKey,
}

fn prepare_stars_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<StarsPipeline>>,
    pipeline: Res<StarsPipeline>,
    views: Query<(Entity, &ExtractedView, &Msaa, Has<TransparentFocusTexture>), With<StarsView>>,
) {
    for (entity, view, msaa, focus) in views.iter() {
        let key = StarsPipelineKey {
            samples: msaa.samples(),
            focus,
            hdr: view.hdr,
        };
        let id = pipelines.specialize(&pipeline_cache, &pipeline, key);
        commands.entity(entity).insert(StarsPipelineId { id, key });
    }
}

/// The stars at the end of the main pass, for views with depth of field's transparent focus
/// texture.
#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct StarsLabel;

/// The stars before the transparent pass, for views without it.
#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct StarsEarlyLabel;

/// Draws the stars of the views whose place is `EARLY` (see the module docs): a view draws them
/// once, in one of the two nodes.
#[derive(Default)]
struct StarsNode<const EARLY: bool>;

impl<const EARLY: bool> ViewNode for StarsNode<EARLY> {
    type ViewQuery = (
        Read<ViewUniformOffset>,
        Read<ViewTarget>,
        Read<ViewDepthTexture>,
        Read<StarsPipelineId>,
        Option<Read<TransparentFocusTexture>>,
    );

    fn run<'w>(
        &self,
        _: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (view_uniform_offset, view_target, depth, pipeline_id, focus): QueryItem<
            'w,
            Self::ViewQuery,
        >,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        // nothing to draw by day, or in this node for a view that draws them in the other
        let settings = world.resource::<StarsSettings>();
        if (settings.brightness <= 0.0 && settings.shooting_rate <= 0.0)
            || EARLY == pipeline_id.key.focus
        {
            return Ok(());
        }
        let pipeline = world.resource::<StarsPipeline>();
        let images = world.resource::<RenderAssets<GpuImage>>();
        let (
            Some(render_pipeline),
            Some(view_binding),
            Some(stars_binding),
            Some(sky),
            Some(noise),
        ) = (
            world
                .resource::<PipelineCache>()
                .get_render_pipeline(pipeline_id.id),
            world.resource::<ViewUniforms>().uniforms.binding(),
            world.resource::<StarsUniformBuffer>().0.binding(),
            images.get(&world.resource::<AtmosphereImage>().handle),
            // the stars wait for the noise to load
            images.get(&world.resource::<StarsNoise>().0),
        )
        else {
            return Ok(());
        };
        let layout = &pipeline.layouts[&(pipeline_id.key.samples, pipeline_id.key.focus)];

        let device = render_context.render_device();
        let bind_group = match (pipeline_id.key.focus, focus) {
            (true, Some(focus)) => device.create_bind_group(
                Some("stars bind group"),
                layout,
                &BindGroupEntries::sequential((
                    view_binding,
                    depth.view(),
                    &sky.texture_view,
                    &sky.sampler,
                    stars_binding,
                    &noise.texture_view,
                    &noise.sampler,
                    &focus.texture.default_view,
                )),
            ),
            _ => device.create_bind_group(
                Some("stars bind group"),
                layout,
                &BindGroupEntries::sequential((
                    view_binding,
                    depth.view(),
                    &sky.texture_view,
                    &sky.sampler,
                    stars_binding,
                    &noise.texture_view,
                    &noise.sampler,
                )),
            ),
        };

        let color_attachment = if EARLY {
            // the main pass's target, multisampled under MSAA and resolved at the end of the
            // pass; the main passes have drawn into it already, so it loads
            view_target.get_color_attachment()
        } else {
            // drawn straight onto the resolved frame, no copy
            RenderPassColorAttachment {
                view: view_target.main_texture_view(),
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
            }
        };
        let mut pass = render_context
            .command_encoder()
            .begin_render_pass(&RenderPassDescriptor {
                label: Some(if EARLY { "stars early" } else { "stars" }),
                color_attachments: &[Some(color_attachment)],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        pass.set_pipeline(render_pipeline);
        pass.set_bind_group(0, &bind_group, &[view_uniform_offset.offset]);
        pass.draw(0..3, 0..1);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn stars_follow_the_phase() {
        let (brightness, shooting_rate) = star_levels(0.0, None, true);
        assert!(close(brightness, 3.0 * 3.0), "{brightness}");
        assert!(close(shooting_rate, 0.4 * 3.0), "{shooting_rate}");
        assert_eq!(star_levels(0.5, None, true), (0.0, 0.0));
        let (brightness, _) = star_levels(0.25, None, true);
        assert!(close(brightness, 3.0 * 0.1), "{brightness}");
    }

    #[test]
    fn scene_star_brightness_scales_from_the_protocol_default() {
        let preset = star_levels(0.0, None, true);
        assert_eq!(star_levels(0.0, Some(4.62), true), preset);
        let (brightness, _) = star_levels(0.0, Some(2.31), true);
        assert!(close(brightness, preset.0 / 2.0), "{brightness}");
        // no brightness hides the shooting stars too, and more of it does not add any
        assert_eq!(star_levels(0.0, Some(0.0), true), (0.0, 0.0));
        let (brightness, shooting_rate) = star_levels(0.0, Some(9.24), true);
        assert!(close(brightness, preset.0 * 2.0), "{brightness}");
        assert_eq!(shooting_rate, preset.1);
    }

    #[test]
    fn stars_hidden_under_a_panorama() {
        assert_eq!(star_levels(0.0, None, false), (0.0, 0.0));
        assert_eq!(star_levels(0.0, Some(9.0), false), (0.0, 0.0));
    }

    /// Every variant of the star pipeline compiles, and each of the pass's three places (a plain
    /// view, a multisampled one and one with depth of field) lights stars on a clear night sky
    /// and none once a scene sets their brightness to nothing. Under depth of field the stars
    /// still bloom: the glow lights pixels around them.
    #[test]
    #[ignore = "needs a GPU; run with --ignored"]
    fn stars_pipelines_compile() {
        use crate::stylized_sky::test_support::{draw_sky, sky_test_app, start_sky};
        use bevy::{
            asset::RenderAssetUsages,
            core_pipeline::{
                dof::{DepthOfField, DepthOfFieldMode},
                tonemapping::Tonemapping,
            },
            render::{
                camera::RenderTarget,
                render_resource::{Extent3d, TextureDimension},
                view::screenshot::{Screenshot, ScreenshotCaptured},
            },
        };
        use pipeline_test::{compile_pipelines, fragment_pipelines};

        #[derive(Resource, Default)]
        struct Captured(Option<Image>);

        /// The brightest channel of each pixel of the view drawn to `target`.
        fn capture(app: &mut App, target: &Handle<Image>) -> Vec<u8> {
            app.world_mut().resource_mut::<Captured>().0 = None;
            app.world_mut()
                .spawn(Screenshot::image(target.clone()))
                .observe(
                    |event: Trigger<ScreenshotCaptured>, mut captured: ResMut<Captured>| {
                        captured.0 = Some(event.event().0.clone())
                    },
                );
            let start = std::time::Instant::now();
            loop {
                app.update();
                if let Some(image) = app.world_mut().resource_mut::<Captured>().0.take() {
                    let rgba = image
                        .try_into_dynamic()
                        .expect("captured RGBA image")
                        .to_rgba8();
                    return rgba
                        .pixels()
                        .map(|pixel| pixel.0[0].max(pixel.0[1]).max(pixel.0[2]))
                        .collect();
                }
                assert!(start.elapsed().as_secs() < 30, "GPU screenshot timed out");
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }

        let mut app = sky_test_app(64);
        app.init_resource::<crate::environment::SceneEnvironment>()
            .init_resource::<Captured>();
        app.add_plugins(StarsPlugin);
        start_sky(&mut app);

        // each view looks straight up, a pixel about a twelfth of a degree across
        let views = [
            ("plain", Msaa::Off, false, false),
            ("multisampled", Msaa::Sample4, false, false),
            ("depth of field", Msaa::Off, true, false),
            ("depth of field and bloom", Msaa::Off, true, true),
        ]
        .map(|(label, msaa, depth_of_field, bloom)| {
            let mut target = Image::new_fill(
                Extent3d {
                    width: 512,
                    height: 512,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &[0, 0, 0, 0],
                TextureFormat::Bgra8UnormSrgb,
                RenderAssetUsages::all(),
            );
            target.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT;
            let target = app.world_mut().resource_mut::<Assets<Image>>().add(target);
            let mut camera = app.world_mut().spawn((
                Camera3d::default(),
                Tonemapping::None,
                Camera {
                    target: RenderTarget::Image(target.clone().into()),
                    hdr: depth_of_field,
                    ..default()
                },
                msaa,
                AtmosphereCamera::default(),
                Transform::default().looking_to(Vec3::Y, Vec3::Z),
            ));
            if depth_of_field {
                // the client's low setting
                camera.insert(DepthOfField {
                    mode: DepthOfFieldMode::Gaussian,
                    focal_distance: 50.0,
                    aperture_f_stops: 0.15,
                    max_depth: 250.0,
                    max_circle_of_confusion_diameter: 20.0,
                    sensor_height: 0.06,
                });
            }
            if bloom {
                // the client's bloom setting, which thresholds at 1
                camera.insert(platform::scatter_bloom(platform::BLOOM_SCATTER));
            }
            (label, target)
        });

        // midnight, with every variant queued beside the ones the views use
        draw_sky(&mut app, |sky| sky.phase = 0.0);
        let mut keys = 0;
        app.sub_app_mut(RenderApp).world_mut().resource_scope(
            |world, mut pipelines: Mut<SpecializedRenderPipelines<StarsPipeline>>| {
                for samples in [1, 4] {
                    for focus in [false, true] {
                        for hdr in [false, true] {
                            pipelines.specialize(
                                world.resource::<PipelineCache>(),
                                world.resource::<StarsPipeline>(),
                                StarsPipelineKey {
                                    samples,
                                    focus,
                                    hdr,
                                },
                            );
                            keys += 1;
                        }
                    }
                }
            },
        );
        compile_pipelines(&mut app);
        assert_eq!(
            fragment_pipelines(&app, "embedded://shaders/stars.wgsl"),
            keys
        );

        let night = views.clone().map(|(_, target)| capture(&mut app, &target));
        app.world_mut()
            .resource_mut::<crate::environment::SceneEnvironment>()
            .star_brightness = Some(0.0);
        for _ in 0..3 {
            app.update();
        }
        let mut lit = Vec::new();
        for ((label, target), night) in views.into_iter().zip(night) {
            // pixel by pixel, as the default sky has its disc overhead
            let bare = capture(&mut app, &target);
            let stars = night
                .iter()
                .zip(&bare)
                .filter(|(&night, &bare)| night > bare.saturating_add(40))
                .count();
            // and any light the stars add, which counts their glow
            let glow = night
                .iter()
                .zip(&bare)
                .filter(|(&night, &bare)| night > bare.saturating_add(8))
                .count();
            println!("{label}: {stars} star pixels, {glow} with their glow");
            assert!(stars > 20, "{label}: {stars} star pixels");
            lit.push(glow);
        }
        // the glow around each star lights pixels the sharp star does not: about half as many
        // again under the client's bloom, where the views without it agree within a few pixels
        assert!(
            lit[3] > lit[2] * 5 / 4,
            "bloom lit {} pixels around the stars against {} without it",
            lit[3],
            lit[2]
        );
    }
}
