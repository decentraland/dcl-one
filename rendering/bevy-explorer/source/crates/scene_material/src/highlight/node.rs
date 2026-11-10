//! The glow from the mask: blurred between its two masks, cut out where the meshes themselves are, and blended
//! over the frame with no depth test (highlight_blur.wgsl, highlight_composite.wgsl).

use bevy::{
    core_pipeline::fullscreen_vertex_shader::fullscreen_shader_vertex_state,
    ecs::{query::QueryItem, system::lifetimeless::Read},
    image::BevyDefault as _,
    prelude::*,
    render::{
        render_graph::{NodeRunError, RenderGraphContext, RenderLabel, ViewNode},
        render_phase::{DrawFunctions, PhaseItem, ViewSortedRenderPhases},
        render_resource::{
            binding_types::{sampler, texture_2d, uniform_buffer_sized},
            BindGroup, BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries, BlendState,
            Buffer, BufferInitDescriptor, BufferUsages, CachedRenderPipelineId, ColorTargetState,
            ColorWrites, Extent3d, FilterMode, FragmentState, LoadOp, MultisampleState, Operations,
            PipelineCache, PrimitiveState, RenderPassColorAttachment, RenderPassDescriptor,
            RenderPipeline, RenderPipelineDescriptor, Sampler, SamplerBindingType,
            SamplerDescriptor, ShaderDefVal, ShaderStages, StoreOp, TextureDescriptor,
            TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureView,
        },
        renderer::{RenderContext, RenderDevice},
        texture::{CachedTexture, TextureCache},
        view::{ExtractedView, ViewTarget},
    },
};

use super::{
    phase::{DrawHighlightPunch, HighlightMaskItem, MASK_FORMAT},
    BLUR_ITERATIONS, BLUR_OFFSETS, BLUR_WEIGHTS,
};

/// The view's mask and the blur's intermediate target, at the view's size.
#[derive(Component)]
pub struct HighlightTextures {
    mask_a: CachedTexture,
    mask_b: CachedTexture,
}

/// Masks for the views with something highlighted; none for the rest.
pub fn prepare_highlight_textures(
    mut commands: Commands,
    mut texture_cache: ResMut<TextureCache>,
    device: Res<RenderDevice>,
    phases: Res<ViewSortedRenderPhases<HighlightMaskItem>>,
    views: Query<(Entity, &ExtractedView, Has<HighlightTextures>), With<ViewTarget>>,
) {
    for (entity, view, has_textures) in &views {
        if phases
            .get(&view.retained_view_entity)
            .is_none_or(|phase| phase.items.is_empty())
        {
            if has_textures {
                commands.entity(entity).remove::<HighlightTextures>();
            }
            continue;
        }
        let size = Extent3d {
            width: view.viewport.z.max(1),
            height: view.viewport.w.max(1),
            depth_or_array_layers: 1,
        };
        let mut mask = |label| {
            texture_cache.get(
                &device,
                TextureDescriptor {
                    label: Some(label),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: MASK_FORMAT,
                    usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
            )
        };
        let textures = HighlightTextures {
            mask_a: mask("highlight mask a"),
            mask_b: mask("highlight mask b"),
        };
        commands.entity(entity).insert(textures);
    }
}

/// The blur and composite pipelines, which depend on nothing but the frame's format.
#[derive(Resource)]
pub struct HighlightPostPipelines {
    blur_layout: BindGroupLayout,
    composite_layout: BindGroupLayout,
    sampler: Sampler,
    /// one texel along the blur's axis, as a `vec2<f32>` padded to 16 bytes
    horizontal: Buffer,
    vertical: Buffer,
    blur_replace: CachedRenderPipelineId,
    blur_blend: CachedRenderPipelineId,
    composite: CachedRenderPipelineId,
    composite_hdr: CachedRenderPipelineId,
}

/// The blur kernel, as the bit patterns `highlight_blur.wgsl` reads back with `bitcast<f32>`.
fn blur_shader_defs() -> Vec<ShaderDefVal> {
    let defs = |name: &str, values: [f32; 3]| {
        values
            .into_iter()
            .enumerate()
            .map(move |(i, value)| ShaderDefVal::UInt(format!("{name}_{i}"), value.to_bits()))
            .collect::<Vec<_>>()
    };
    [
        defs("BLUR_OFFSET", BLUR_OFFSETS),
        defs("BLUR_WEIGHT", BLUR_WEIGHTS),
    ]
    .concat()
}

impl FromWorld for HighlightPostPipelines {
    fn from_world(world: &mut World) -> Self {
        let device = world.resource::<RenderDevice>();
        let blur_layout = device.create_bind_group_layout(
            Some("highlight blur layout"),
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    uniform_buffer_sized(false, None),
                ),
            ),
        );
        let composite_layout = device.create_bind_group_layout(
            Some("highlight composite layout"),
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                ),
            ),
        );
        // the kernel's offsets fall between texels, so the linear filter blends each pair
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("highlight sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..default()
        });
        let axis = |label, axis: [f32; 4]| {
            device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some(label),
                contents: &axis.map(f32::to_le_bytes).concat(),
                usage: BufferUsages::UNIFORM,
            })
        };
        let horizontal = axis("highlight blur horizontal", [1.0, 0.0, 0.0, 0.0]);
        let vertical = axis("highlight blur vertical", [0.0, 1.0, 0.0, 0.0]);

        let asset_server = world.resource::<AssetServer>();
        let blur_shader = asset_server.load("embedded://shaders/highlight_blur.wgsl");
        let composite_shader = asset_server.load("embedded://shaders/highlight_composite.wgsl");
        let fullscreen = |label: &'static str,
                          layout: &BindGroupLayout,
                          shader: &Handle<Shader>,
                          shader_defs: Vec<ShaderDefVal>,
                          format: TextureFormat,
                          blend: BlendState| RenderPipelineDescriptor {
            label: Some(label.into()),
            layout: vec![layout.clone()],
            push_constant_ranges: Vec::new(),
            vertex: fullscreen_shader_vertex_state(),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                shader: shader.clone(),
                shader_defs,
                entry_point: "fragment".into(),
                targets: vec![Some(ColorTargetState {
                    format,
                    blend: Some(blend),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            zero_initialize_workgroup_memory: false,
        };
        let blur = |blend| {
            fullscreen(
                "highlight blur pipeline",
                &blur_layout,
                &blur_shader,
                blur_shader_defs(),
                MASK_FORMAT,
                blend,
            )
        };
        let composite = |format| {
            fullscreen(
                "highlight composite pipeline",
                &composite_layout,
                &composite_shader,
                Vec::new(),
                format,
                BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            )
        };
        // the blurred mask is premultiplied: it starts transparent black, so a texel half covered
        // holds half the colour. Straight alpha blending would scale that again and darken the edge.
        let (blur_replace, blur_blend, composite, composite_hdr) = (
            blur(BlendState::REPLACE),
            blur(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            composite(TextureFormat::bevy_default()),
            composite(ViewTarget::TEXTURE_FORMAT_HDR),
        );

        let cache = world.resource::<PipelineCache>();
        Self {
            blur_replace: cache.queue_render_pipeline(blur_replace),
            blur_blend: cache.queue_render_pipeline(blur_blend),
            composite: cache.queue_render_pipeline(composite),
            composite_hdr: cache.queue_render_pipeline(composite_hdr),
            blur_layout,
            composite_layout,
            sampler,
            horizontal,
            vertical,
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct HighlightLabel;

#[derive(Default)]
pub struct HighlightNode;

impl ViewNode for HighlightNode {
    type ViewQuery = (
        Read<ExtractedView>,
        Read<ViewTarget>,
        Option<Read<HighlightTextures>>,
    );

    fn run<'w>(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (view, view_target, textures): QueryItem<'w, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let Some(textures) = textures else {
            return Ok(());
        };
        let Some(phase) = world
            .resource::<ViewSortedRenderPhases<HighlightMaskItem>>()
            .get(&view.retained_view_entity)
            .filter(|phase| !phase.items.is_empty())
        else {
            return Ok(());
        };
        let pipelines = world.resource::<HighlightPostPipelines>();
        let cache = world.resource::<PipelineCache>();
        let composite = match view.hdr {
            true => pipelines.composite_hdr,
            false => pipelines.composite,
        };
        let (Some(blur_replace), Some(blur_blend), Some(composite)) = (
            cache.get_render_pipeline(pipelines.blur_replace),
            cache.get_render_pipeline(pipelines.blur_blend),
            cache.get_render_pipeline(composite),
        ) else {
            return Ok(());
        };
        let view_entity = graph.view_entity();
        let (mask_a, mask_b) = (&textures.mask_a.default_view, &textures.mask_b.default_view);

        let device = render_context.render_device();
        let blur_group = |source: &TextureView, axis: &Buffer| {
            device.create_bind_group(
                Some("highlight blur"),
                &pipelines.blur_layout,
                &BindGroupEntries::sequential((
                    source,
                    &pipelines.sampler,
                    axis.as_entire_binding(),
                )),
            )
        };
        // (target, across, down) for even and odd iterations, each reading the other mask
        let ping_pong = [
            (
                mask_b,
                blur_group(mask_a, &pipelines.horizontal),
                blur_group(mask_a, &pipelines.vertical),
            ),
            (
                mask_a,
                blur_group(mask_b, &pipelines.horizontal),
                blur_group(mask_b, &pipelines.vertical),
            ),
        ];
        let glow = device.create_bind_group(
            Some("highlight composite"),
            &pipelines.composite_layout,
            &BindGroupEntries::sequential((mask_a, &pipelines.sampler)),
        );

        // 1: the widened silhouettes
        {
            let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("highlight mask"),
                color_attachments: &[Some(attachment(mask_a, true))],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if let Err(err) = phase.render(&mut pass, world, view_entity) {
                error!("highlight mask: {err:?}");
            }
        }

        // 2: unity's sequence (RenderFeature_ObjectHighlight.RenderPass_DrawObjects): each iteration
        // blurs its source across into the other mask, then blurs the same source down over that.
        // The two axes are not chained, so the glow spreads in a cross rather than a full gaussian,
        // and an even count ends back in a.
        for i in 0..BLUR_ITERATIONS as usize {
            let (target, across, down) = &ping_pong[i % 2];
            fullscreen_pass(
                render_context,
                "highlight blur horizontal",
                target,
                true,
                blur_replace,
                across,
                None,
            );
            fullscreen_pass(
                render_context,
                "highlight blur vertical",
                target,
                false,
                blur_blend,
                down,
                None,
            );
        }

        // 3: the meshes again, unwidened in no colour, so only the glow around them is left
        {
            let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("highlight punch"),
                color_attachments: &[Some(attachment(mask_a, false))],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            let draw_functions = world.resource::<DrawFunctions<HighlightMaskItem>>();
            let mut draw_functions = draw_functions.write();
            draw_functions.prepare(world);
            let punch = draw_functions.id::<DrawHighlightPunch>();
            if let Some(draw) = draw_functions.get_mut(punch) {
                for item in phase.items.iter() {
                    if item.batch_range().is_empty() {
                        continue;
                    }
                    if let Err(err) = draw.draw(world, &mut pass, view_entity, item) {
                        error!("highlight punch: {err:?}");
                    }
                }
            }
        }

        // 4: the glow over the frame, through anything in front of it
        fullscreen_pass(
            render_context,
            "highlight composite",
            view_target.main_texture_view(),
            false,
            composite,
            &glow,
            Some(view.viewport),
        );

        Ok(())
    }
}

/// `clear` starts the target transparent, otherwise it keeps what is there
fn attachment(view: &TextureView, clear: bool) -> RenderPassColorAttachment<'_> {
    RenderPassColorAttachment {
        view,
        resolve_target: None,
        ops: Operations {
            load: match clear {
                true => LoadOp::Clear(default()),
                false => LoadOp::Load,
            },
            store: StoreOp::Store,
        },
    }
}

fn fullscreen_pass(
    render_context: &mut RenderContext,
    label: &str,
    target: &TextureView,
    clear: bool,
    pipeline: &RenderPipeline,
    bind_group: &BindGroup,
    viewport: Option<UVec4>,
) {
    let mut pass = render_context
        .command_encoder()
        .begin_render_pass(&RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(attachment(target, clear))],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
    if let Some(viewport) = viewport {
        let viewport = viewport.as_vec4();
        pass.set_viewport(viewport.x, viewport.y, viewport.z, viewport.w, 0.0, 1.0);
    }
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    pass.draw(0..3, 0..1);
}
