//! The mask phase: each highlighted mesh drawn in its highlight colour, widened along its normals,
//! into the view's mask (see highlight_mask.wgsl).

use std::ops::Range;

use bevy::{
    ecs::system::{lifetimeless::SRes, SystemParamItem},
    math::FloatOrd,
    pbr::{
        setup_morph_and_skinning_defs, DrawMesh, MeshInstanceLookup, MeshPipeline, MeshPipelineKey,
        SetMeshBindGroup, SetMeshViewBindGroup, ViewKeyCache,
    },
    platform::collections::HashSet,
    prelude::*,
    render::{
        batching::{
            gpu_preprocessing::UntypedPhaseIndirectParametersBuffers, GetBatchData,
            GetFullBatchData,
        },
        mesh::{MeshVertexBufferLayoutRef, RenderMesh},
        render_asset::RenderAssets,
        render_phase::{
            CachedRenderPipelinePhaseItem, DrawFunctionId, DrawFunctions, PhaseItem,
            PhaseItemExtraIndex, RenderCommand, RenderCommandResult, SetItemPipeline,
            SortedPhaseItem, TrackedRenderPass, ViewSortedRenderPhases,
        },
        render_resource::{
            binding_types::uniform_buffer, BindGroup, BindGroupEntries, BindGroupLayout,
            BindGroupLayoutEntries, BlendState, CachedRenderPipelineId, ColorTargetState,
            ColorWrites, DynamicUniformBuffer, FragmentState, MultisampleState, PipelineCache,
            PrimitiveState, RenderPipelineDescriptor, ShaderDefVal, ShaderStages,
            SpecializedMeshPipeline, SpecializedMeshPipelineError, SpecializedMeshPipelines,
            TextureFormat, VertexState,
        },
        renderer::{RenderDevice, RenderQueue},
        sync_world::MainEntity,
        view::{ExtractedView, RenderVisibleEntities, RetainedViewEntity},
        Extract,
    },
};
use nonmax::NonMaxU32;

use common::structs::PrimaryCamera;

use super::Highlight;

/// The masks the silhouettes are drawn into and the glow is blurred into.
pub const MASK_FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;

/// The view key bits that choose the mesh view bind group layout.
const VIEW_LAYOUT_BITS: MeshPipelineKey = MeshPipelineKey::DEPTH_PREPASS
    .union(MeshPipelineKey::NORMAL_PREPASS)
    .union(MeshPipelineKey::MOTION_VECTOR_PREPASS)
    .union(MeshPipelineKey::DEFERRED_PREPASS)
    .union(MeshPipelineKey::OIT_ENABLED);

/// One highlighted mesh in one view.
pub struct HighlightMaskItem {
    sort_key: FloatOrd,
    entity: (Entity, MainEntity),
    pipeline: CachedRenderPipelineId,
    draw_function: DrawFunctionId,
    batch_range: Range<u32>,
    extra_index: PhaseItemExtraIndex,
    indexed: bool,
    /// the item's entry in `HighlightItemUniforms`
    uniform_offset: u32,
}

impl PhaseItem for HighlightMaskItem {
    fn entity(&self) -> Entity {
        self.entity.0
    }

    fn main_entity(&self) -> MainEntity {
        self.entity.1
    }

    fn draw_function(&self) -> DrawFunctionId {
        self.draw_function
    }

    fn batch_range(&self) -> &Range<u32> {
        &self.batch_range
    }

    fn batch_range_mut(&mut self) -> &mut Range<u32> {
        &mut self.batch_range
    }

    fn extra_index(&self) -> PhaseItemExtraIndex {
        self.extra_index.clone()
    }

    fn batch_range_and_extra_index_mut(&mut self) -> (&mut Range<u32>, &mut PhaseItemExtraIndex) {
        (&mut self.batch_range, &mut self.extra_index)
    }
}

impl SortedPhaseItem for HighlightMaskItem {
    type SortKey = FloatOrd;

    fn sort_key(&self) -> Self::SortKey {
        self.sort_key
    }

    fn indexed(&self) -> bool {
        self.indexed
    }
}

impl CachedRenderPipelinePhaseItem for HighlightMaskItem {
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.pipeline
    }
}

/// The mesh pipeline's batch data with batching off: a batch draws with its first item's
/// highlight uniform, so items that share a mesh and material must not merge.
pub struct HighlightBatch;

impl GetBatchData for HighlightBatch {
    type Param = <MeshPipeline as GetBatchData>::Param;
    type CompareData = ();
    type BufferData = <MeshPipeline as GetBatchData>::BufferData;

    fn get_batch_data(
        param: &SystemParamItem<Self::Param>,
        query_item: (Entity, MainEntity),
    ) -> Option<(Self::BufferData, Option<Self::CompareData>)> {
        MeshPipeline::get_batch_data(param, query_item).map(|(data, _)| (data, None))
    }
}

impl GetFullBatchData for HighlightBatch {
    type BufferInputData = <MeshPipeline as GetFullBatchData>::BufferInputData;

    fn get_binned_batch_data(
        param: &SystemParamItem<Self::Param>,
        query_item: MainEntity,
    ) -> Option<Self::BufferData> {
        MeshPipeline::get_binned_batch_data(param, query_item)
    }

    fn get_index_and_compare_data(
        param: &SystemParamItem<Self::Param>,
        query_item: (Entity, MainEntity),
    ) -> Option<(NonMaxU32, Option<Self::CompareData>)> {
        MeshPipeline::get_index_and_compare_data(param, query_item).map(|(index, _)| (index, None))
    }

    fn get_binned_index(
        param: &SystemParamItem<Self::Param>,
        query_item: (Entity, MainEntity),
    ) -> Option<NonMaxU32> {
        MeshPipeline::get_binned_index(param, query_item)
    }

    fn write_batch_indirect_parameters_metadata(
        indexed: bool,
        base_output_index: u32,
        batch_set_index: Option<NonMaxU32>,
        indirect_parameters_buffers: &mut UntypedPhaseIndirectParametersBuffers,
        indirect_parameters_offset: u32,
    ) {
        MeshPipeline::write_batch_indirect_parameters_metadata(
            indexed,
            base_output_index,
            batch_set_index,
            indirect_parameters_buffers,
            indirect_parameters_offset,
        );
    }
}

mod uniform {
    // the ShaderType derive's layout check function is never called
    #![allow(dead_code)]

    use bevy::{math::Vec4, render::render_resource::ShaderType};

    /// see highlight_mask.wgsl
    #[derive(ShaderType, Clone, Copy, Default)]
    pub struct HighlightItemUniform {
        pub color: Vec4,
        pub width: f32,
    }
}
use uniform::HighlightItemUniform;

/// The items' colours and widths for this frame. The first entry, at offset 0, is the punch-out
/// draw's: no colour and no widening.
#[derive(Resource, Default)]
pub struct HighlightItemUniforms {
    buffer: DynamicUniformBuffer<HighlightItemUniform>,
    bind_group: Option<BindGroup>,
}

const PUNCH_OFFSET: u32 = 0;

/// Draws a mask item in its highlight.
pub type DrawHighlightMask = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetHighlightBindGroup<2, false>,
    DrawMesh,
);

/// Draws a mask item unwidened in no colour, clearing the mask where the mesh itself is.
pub type DrawHighlightPunch = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetHighlightBindGroup<2, true>,
    DrawMesh,
);

pub struct SetHighlightBindGroup<const I: usize, const PUNCH: bool>;

impl<const I: usize, const PUNCH: bool> RenderCommand<HighlightMaskItem>
    for SetHighlightBindGroup<I, PUNCH>
{
    type Param = SRes<HighlightItemUniforms>;
    type ViewQuery = ();
    type ItemQuery = ();

    fn render<'w>(
        item: &HighlightMaskItem,
        _: (),
        _: Option<()>,
        uniforms: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(bind_group) = uniforms.into_inner().bind_group.as_ref() else {
            return RenderCommandResult::Failure("highlight item bind group missing");
        };
        let offset = if PUNCH {
            PUNCH_OFFSET
        } else {
            item.uniform_offset
        };
        pass.set_bind_group(I, bind_group, &[offset]);
        RenderCommandResult::Success
    }
}

#[derive(Resource)]
pub struct HighlightMaskPipeline {
    mesh_pipeline: MeshPipeline,
    item_layout: BindGroupLayout,
    shader: Handle<Shader>,
}

impl FromWorld for HighlightMaskPipeline {
    fn from_world(world: &mut World) -> Self {
        let item_layout = world.resource::<RenderDevice>().create_bind_group_layout(
            Some("highlight item layout"),
            &BindGroupLayoutEntries::single(
                ShaderStages::VERTEX_FRAGMENT,
                uniform_buffer::<HighlightItemUniform>(true),
            ),
        );
        Self {
            mesh_pipeline: world.resource::<MeshPipeline>().clone(),
            item_layout,
            shader: world
                .resource::<AssetServer>()
                .load("embedded://shaders/highlight_mask.wgsl"),
        }
    }
}

impl SpecializedMeshPipeline for HighlightMaskPipeline {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut shader_defs: Vec<ShaderDefVal> = vec!["VERTEX_POSITIONS".into()];
        let mut vertex_attributes = vec![Mesh::ATTRIBUTE_POSITION.at_shader_location(0)];
        // without normals the silhouette is drawn unwidened
        if layout.0.contains(Mesh::ATTRIBUTE_NORMAL) {
            shader_defs.push("VERTEX_NORMALS".into());
            vertex_attributes.push(Mesh::ATTRIBUTE_NORMAL.at_shader_location(1));
        }
        // the same skinning, morph and mesh bind group layout as the mesh pipeline's
        let mesh_layout = setup_morph_and_skinning_defs(
            &self.mesh_pipeline.mesh_layouts,
            layout,
            6,
            &key,
            &mut shader_defs,
            &mut vertex_attributes,
            self.mesh_pipeline.skins_use_uniform_buffers,
        );
        if let Some(batch_size) = self.mesh_pipeline.per_object_buffer_batch_size {
            shader_defs.push(ShaderDefVal::UInt(
                "PER_OBJECT_BUFFER_BATCH_SIZE".into(),
                batch_size,
            ));
        }

        Ok(RenderPipelineDescriptor {
            label: Some("highlight mask pipeline".into()),
            layout: vec![
                self.mesh_pipeline.get_view_layout(key.into()).clone(),
                mesh_layout,
                self.item_layout.clone(),
            ],
            push_constant_ranges: Vec::new(),
            vertex: VertexState {
                shader: self.shader.clone(),
                shader_defs: shader_defs.clone(),
                entry_point: "vertex".into(),
                buffers: vec![layout.0.get_layout(&vertex_attributes)?],
            },
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                shader_defs,
                entry_point: "fragment".into(),
                targets: vec![Some(ColorTargetState {
                    format: MASK_FORMAT,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: key.primitive_topology(),
                // the silhouette is the same from either side
                cull_mode: None,
                ..default()
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            zero_initialize_workgroup_memory: false,
        })
    }
}

/// A mask phase for the player's camera only: the glow is feedback for the player, so scene
/// texture cameras (mirrors and the like) never show it.
pub fn extract_highlight_phases(
    mut phases: ResMut<ViewSortedRenderPhases<HighlightMaskItem>>,
    cameras: Extract<Query<(Entity, &Camera), With<PrimaryCamera>>>,
) {
    let mut live = HashSet::new();
    for (entity, camera) in &cameras {
        if !camera.is_active {
            continue;
        }
        let view = RetainedViewEntity::new(entity.into(), None, 0);
        phases.insert_or_clear(view);
        live.insert(view);
    }
    phases.retain(|view, _| live.contains(view));
}

#[allow(clippy::too_many_arguments)]
pub fn queue_highlight_meshes(
    draw_functions: Res<DrawFunctions<HighlightMaskItem>>,
    mask_pipeline: Res<HighlightMaskPipeline>,
    mut pipelines: ResMut<SpecializedMeshPipelines<HighlightMaskPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    render_meshes: Res<RenderAssets<RenderMesh>>,
    mesh_instances: MeshInstanceLookup,
    view_key_cache: Res<ViewKeyCache>,
    mut phases: ResMut<ViewSortedRenderPhases<HighlightMaskItem>>,
    mut uniforms: ResMut<HighlightItemUniforms>,
    views: Query<(&ExtractedView, &RenderVisibleEntities)>,
    highlighted: Query<&Highlight>,
) {
    uniforms.buffer.clear();
    if highlighted.is_empty() {
        return;
    }
    let punch_offset = uniforms.buffer.push(&HighlightItemUniform::default());
    debug_assert_eq!(punch_offset, PUNCH_OFFSET);
    let draw_function = draw_functions.read().id::<DrawHighlightMask>();

    for (view, visible) in &views {
        let Some(phase) = phases.get_mut(&view.retained_view_entity) else {
            continue;
        };
        let Some(view_key) = view_key_cache.get(&view.retained_view_entity) else {
            continue;
        };
        let view_key = MeshPipelineKey::from_msaa_samples(view_key.msaa_samples())
            | view_key.intersection(VIEW_LAYOUT_BITS);
        let rangefinder = view.rangefinder3d();

        // the view's own visible meshes, so its layers and frustum apply
        for (entity, main_entity) in visible.iter::<Mesh3d>() {
            let Ok(highlight) = highlighted.get(*entity) else {
                continue;
            };
            let Some(mesh_instance) = mesh_instances.render_mesh_queue_data(*entity, *main_entity)
            else {
                continue;
            };
            let Some(mesh) = render_meshes.get(mesh_instance.mesh_asset_id) else {
                continue;
            };
            let key = view_key | MeshPipelineKey::from_bits_retain(mesh.key_bits.bits());
            let pipeline =
                match pipelines.specialize(&pipeline_cache, &mask_pipeline, key, &mesh.layout) {
                    Ok(pipeline) => pipeline,
                    Err(err) => {
                        error!("highlight mask pipeline: {err}");
                        continue;
                    }
                };
            let uniform_offset = uniforms.buffer.push(&HighlightItemUniform {
                color: highlight.color.to_vec4(),
                width: highlight.width,
            });
            phase.add(HighlightMaskItem {
                sort_key: FloatOrd(rangefinder.distance_translation(&mesh_instance.translation)),
                entity: (*entity, *main_entity),
                pipeline,
                draw_function,
                batch_range: 0..1,
                extra_index: PhaseItemExtraIndex::None,
                indexed: mesh.indexed(),
                uniform_offset,
            });
        }
    }
}

pub fn prepare_highlight_bind_group(
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    mask_pipeline: Res<HighlightMaskPipeline>,
    uniforms: ResMut<HighlightItemUniforms>,
) {
    let uniforms = uniforms.into_inner();
    uniforms.bind_group = None;
    if uniforms.buffer.is_empty() {
        return;
    }
    uniforms.buffer.write_buffer(&device, &queue);
    if let Some(binding) = uniforms.buffer.binding() {
        uniforms.bind_group = Some(device.create_bind_group(
            Some("highlight item bind group"),
            &mask_pipeline.item_layout,
            &BindGroupEntries::single(binding),
        ));
    }
}
