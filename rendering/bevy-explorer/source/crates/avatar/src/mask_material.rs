use bevy::{
    prelude::*,
    reflect::TypePath,
    render::{
        render_resource::{AsBindGroup, ShaderRef},
        storage::ShaderStorageBuffer,
    },
};
use scene_material::{reveal_shader_defs, SceneBound, SceneBoundData, SceneBounds};

pub struct MaskMaterialPlugin;

impl Plugin for MaskMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<MaskMaterial>::default());
    }
}

impl Material for MaskMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://shaders/mask_material.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn depth_bias(&self) -> f32 {
        10000.0
    }

    fn specialize(
        _: &bevy::pbr::MaterialPipeline<Self>,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _: &bevy::render::mesh::MeshVertexBufferLayoutRef,
        _: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        // mask_material.wgsl imports reveal_discard
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader_defs.extend(reveal_shader_defs());
        }
        Ok(())
    }
}

mod decl {
    #![allow(dead_code)]
    use bevy::{math::Vec4, render::render_resource::ShaderType};

    #[derive(ShaderType, Debug, Clone)]
    pub struct MaskData {
        pub(super) color: Vec4,
    }
}
use decl::*;

// This is the struct that will be passed to your shader
#[derive(AsBindGroup, Asset, Debug, Clone, TypePath)]
pub struct MaskMaterial {
    #[uniform(0)]
    pub mask_data: MaskData,
    #[texture(1)]
    #[sampler(2)]
    pub base_texture: Handle<Image>,
    #[texture(3)]
    #[sampler(4)]
    pub mask_texture: Handle<Image>,
    #[uniform(100)]
    pub bounds: SceneBoundData,
    #[cfg_attr(
        not(feature = "webgl2"),
        storage(101, read_only, visibility(fragment))
    )]
    pub cells: Handle<ShaderStorageBuffer>,
}

impl MaskMaterial {
    pub fn new(
        color: Color,
        base_texture: Handle<Image>,
        mask_texture: Handle<Image>,
        bounds: &SceneBounds,
        distance: f32,
    ) -> Self {
        let bounds = SceneBound::new(bounds, distance);
        Self {
            mask_data: MaskData {
                color: color.to_linear().to_vec4(),
            },
            bounds: bounds.data,
            cells: bounds.cells,
            base_texture,
            mask_texture,
        }
    }
}
