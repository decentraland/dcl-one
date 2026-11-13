//! The material the landscape's impostors draw with: boimp's impostor, with its bindings and its
//! vertex stage, drawn cut out in the main pass only.
//!
//! boimp's own material is blended: drawn in the depth prepass, an impostor writes its box's front
//! face there, and its fragment stage then hides it behind that depth. Kept out of the prepass,
//! the impostor's depth check meets the scene's depth only, so it can be cut out and drawn in
//! the binned opaque passes, where instances sharing a material batch into one draw. Ambient
//! occlusion, which reads the prepass, sees what stands behind an impostor; depth of field reads
//! the main pass's depth, where an impostor leaves its box's nearest face.

use bevy::{
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::{
        mesh::MeshVertexBufferLayoutRef,
        render_resource::{
            AsBindGroup, RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError,
        },
    },
};
use boimp::{
    oct_coords::GRID_MASK,
    render::{
        INDEXED_FLAG, INDEXED_V2_FLAG, RENDER_MULTISAMPLE_FLAG, V2_HAS_HIGH_BYTE_FLAG,
        V2_HAS_HIGH_NIBBLE_FLAG, V2_IDX10S_FLAG, V2_V3_FLAG, VERTEX_HANDLE,
    },
    GridMode, Imposter, ImposterData,
};

/// Texels covered less than this are cut out (the impostor shader's `CUTOFF`).
pub(crate) const IMPOSTOR_CUTOFF: f32 = 0.5;

/// An impostor as boimp loads it, with the same bindings.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
#[bind_group_data(TreeImpostorKey)]
pub(crate) struct TreeImpostor {
    #[uniform(0)]
    pub data: ImposterData,
    #[texture(1, dimension = "2d", sample_type = "u_int")]
    pub pixels: Handle<Image>,
    #[texture(2, dimension = "2d", sample_type = "u_int")]
    pub indices: Handle<Image>,
    #[texture(3, dimension = "2d", sample_type = "u_int")]
    pub indices_hi: Handle<Image>,
    #[texture(4, dimension = "2d", sample_type = "u_int")]
    pub depth_palette: Handle<Image>,
}

impl From<&Imposter> for TreeImpostor {
    fn from(imposter: &Imposter) -> Self {
        Self {
            data: imposter.data,
            pixels: imposter.pixels.clone(),
            indices: imposter.indices.clone(),
            indices_hi: imposter.indices_hi.clone(),
            depth_palette: imposter.depth_palette.clone(),
        }
    }
}

/// The stored format and grid of an impostor, which pick its shader variant.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct TreeImpostorKey(u32);

impl From<&TreeImpostor> for TreeImpostorKey {
    fn from(impostor: &TreeImpostor) -> Self {
        Self(impostor.data.flags)
    }
}

/// The main pass only: no depth prepass and no shadows (boimp's stage has no shadow pass).
pub(crate) fn tree_impostor_plugin() -> MaterialPlugin<TreeImpostor> {
    MaterialPlugin {
        prepass_enabled: false,
        shadows_enabled: false,
        ..default()
    }
}

impl Material for TreeImpostor {
    fn vertex_shader() -> ShaderRef {
        VERTEX_HANDLE.into()
    }

    fn fragment_shader() -> ShaderRef {
        super::tree_impostor_shader()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Mask(IMPOSTOR_CUTOFF)
    }

    /// boimp's variants: its grid, and the format the impostor is stored in.
    fn specialize(
        _: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let flags = key.bind_group_data.0;
        let grid = match flags & GRID_MASK {
            mode if mode == GridMode::Hemispherical.as_flags() => "GRID_HEMISPHERICAL",
            mode if mode == GridMode::Spherical.as_flags() => "GRID_SPHERICAL",
            _ => "GRID_HORIZONTAL",
        };
        let mut defs = vec![grid];
        if flags & RENDER_MULTISAMPLE_FLAG != 0 {
            defs.push("MATERIAL_MULTISAMPLE");
        }
        if flags & INDEXED_FLAG != 0 {
            defs.push("INDEXED_PIXELS");
        }
        if flags & INDEXED_V2_FLAG != 0 {
            defs.push("INDEXED_V2");
            if flags & V2_V3_FLAG != 0 {
                defs.push("INDEXED_V3");
                if flags & V2_HAS_HIGH_NIBBLE_FLAG != 0 {
                    defs.push("INDEXED_V3_12");
                }
            } else if flags & V2_IDX10S_FLAG != 0 {
                defs.push("INDEXED_V2_10S");
            } else if flags & V2_HAS_HIGH_NIBBLE_FLAG != 0 {
                defs.push("INDEXED_V2_12");
            } else if flags & V2_HAS_HIGH_BYTE_FLAG != 0 {
                defs.push("INDEXED_V2_14");
            }
        }
        descriptor.vertex.shader_defs.push(grid.into());
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment
                .shader_defs
                .extend(defs.into_iter().map(Into::into));
        }
        Ok(())
    }
}
