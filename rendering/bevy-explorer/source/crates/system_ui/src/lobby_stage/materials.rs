use bevy::{
    asset::embedded_path,
    pbr::{ExtendedMaterial, MaterialExtension, MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::{
        mesh::MeshVertexBufferLayoutRef,
        render_resource::{
            AsBindGroup, CompareFunction, RenderPipelineDescriptor, ShaderRef,
            SpecializedMeshPipelineError,
        },
    },
};

fn shader(path: &std::path::Path) -> ShaderRef {
    ShaderRef::Path(format!("embedded://{}", path.display()).into())
}

/// Transparent stage pieces draw in this order whatever their distance: the floor, the shadow on
/// it, the mist behind the avatar, the dust in front of it, then the vignette over everything.
pub mod order {
    pub const FLOOR: f32 = 0.0;
    pub const BLOB: f32 = 1000.0;
    pub const MIST: f32 = 2000.0;
    pub const DUST: f32 = 3000.0;
    pub const VIGNETTE: f32 = 100_000.0;
}

pub type FloorMaterial = ExtendedMaterial<FloorExtension>;

#[derive(Asset, TypePath, AsBindGroup, Clone, Default)]
pub struct FloorExtension {
    #[uniform(100)]
    pub floor: FloorUniform,
}

impl MaterialExtension for FloorExtension {
    type Base = StandardMaterial;

    fn fragment_shader() -> ShaderRef {
        shader(&embedded_path!("lobby_floor.wgsl"))
    }
}

pub type RockMaterial = ExtendedMaterial<RockExtension>;

#[derive(Asset, TypePath, AsBindGroup, Clone, Default)]
pub struct RockExtension {
    #[uniform(100)]
    pub rock: RockUniform,
}

impl MaterialExtension for RockExtension {
    type Base = StandardMaterial;

    fn fragment_shader() -> ShaderRef {
        shader(&embedded_path!("lobby_rock.wgsl"))
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct MistMaterial {
    #[uniform(0)]
    pub mist: MistUniform,
}

impl Material for MistMaterial {
    fn fragment_shader() -> ShaderRef {
        shader(&embedded_path!("lobby_mist.wgsl"))
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn depth_bias(&self) -> f32 {
        order::MIST
    }

    fn specialize(
        _: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        _: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct DustMaterial {
    #[uniform(0)]
    pub dust: DustUniform,
}

impl Material for DustMaterial {
    fn vertex_shader() -> ShaderRef {
        shader(&embedded_path!("lobby_dust.wgsl"))
    }

    fn fragment_shader() -> ShaderRef {
        shader(&embedded_path!("lobby_dust.wgsl"))
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn depth_bias(&self) -> f32 {
        order::DUST
    }

    fn specialize(
        _: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.buffers = vec![layout
            .0
            .get_layout(&[Mesh::ATTRIBUTE_POSITION.at_shader_location(0)])?];
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct BlobMaterial {
    #[uniform(0)]
    pub color: Vec4,
    #[texture(1)]
    #[sampler(2)]
    pub shape: Handle<Image>,
}

impl Material for BlobMaterial {
    fn fragment_shader() -> ShaderRef {
        shader(&embedded_path!("lobby_blob.wgsl"))
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn depth_bias(&self) -> f32 {
        order::BLOB
    }

    fn specialize(
        _: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        _: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// Drawn over the whole frame whatever its mesh's position.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct VignetteMaterial {
    #[uniform(0)]
    pub vignette: VignetteUniform,
}

impl Material for VignetteMaterial {
    fn vertex_shader() -> ShaderRef {
        shader(&embedded_path!("lobby_vignette.wgsl"))
    }

    fn fragment_shader() -> ShaderRef {
        shader(&embedded_path!("lobby_vignette.wgsl"))
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn depth_bias(&self) -> f32 {
        order::VIGNETTE
    }

    fn specialize(
        _: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        _: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        if let Some(depth_stencil) = descriptor.depth_stencil.as_mut() {
            depth_stencil.depth_compare = CompareFunction::Always;
            depth_stencil.depth_write_enabled = false;
        }
        Ok(())
    }
}

pub use uniforms::{DustUniform, FloorUniform, MistUniform, RockUniform, VignetteUniform};

mod uniforms {
    #![expect(
        dead_code,
        reason = "the ShaderType derive emits a never-called check fn per field"
    )]

    use bevy::{
        math::{Vec2, Vec4},
        render::render_resource::ShaderType,
    };

    #[derive(Clone, Copy, Default, Debug, ShaderType)]
    pub struct FloorUniform {
        /// linear rgb added at the centre
        pub spot_color: Vec4,
        /// world-space xz direction the dissolve runs along (away from the camera)
        pub blend_axis: Vec4,
        pub spot_radius: f32,
        pub spot_softness: f32,
        /// along the axis: solid before the start, gone after the end
        pub blend_start: f32,
        pub blend_end: f32,
        pub blend_hardness: f32,
    }

    #[derive(Clone, Copy, Default, Debug, ShaderType)]
    pub struct RockUniform {
        /// linear rgb
        pub moss: Vec4,
        /// the moss ramp's input: the higher, the more rock shows on surfaces facing up
        pub moss_mask: f32,
    }

    #[derive(Clone, Copy, Default, Debug, ShaderType)]
    pub struct MistUniform {
        /// linear rgb, with the mist's opacity in alpha
        pub color: Vec4,
        /// uv per second: layer 1 in xy, layer 2 in zw
        pub speed: Vec4,
        pub scale: f32,
        pub threshold: f32,
        pub softness: f32,
        pub edge_softness: f32,
    }

    #[derive(Clone, Copy, Default, Debug, ShaderType)]
    pub struct DustUniform {
        /// linear rgba
        pub color: Vec4,
        /// in the stage's frame
        pub emitter_centre: Vec4,
        pub emitter_size: Vec4,
        /// a life in seconds (min, max), then the speed in metres per second (min, max)
        pub lifetime_speed: Vec4,
        /// metres across (min, max), then the drift's strength and cells per metre
        pub size_noise: Vec4,
        pub noise_scroll: f32,
        /// share of the disc's radius its edge takes to fade
        pub softness: f32,
    }

    #[derive(Clone, Copy, Default, Debug, ShaderType)]
    pub struct VignetteUniform {
        /// linear rgba
        pub color: Vec4,
        /// half width, reach above and reach below the centre, in frame units
        pub mask_radii: Vec4,
        /// in frame units from the bottom left
        pub mask_center: Vec2,
        pub intensity: f32,
        pub smoothness: f32,
        pub mask_softness: f32,
    }
}
