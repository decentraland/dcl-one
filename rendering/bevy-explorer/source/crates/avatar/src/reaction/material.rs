//! The one material every reaction draws with (reaction.wgsl), and the mesh of an emoji.

use bevy::{
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::{
        mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology},
        render_asset::RenderAssetUsages,
        render_resource::{
            AsBindGroup, CompareFunction, RenderPipelineDescriptor, ShaderRef,
            SpecializedMeshPipelineError,
        },
    },
};
use scene_runner::update_world::text_shape::TextQuad;

const SHADER: &str = "embedded://shaders/reaction.wgsl";

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct ReactionMaterial {
    /// The glyph atlas' bitmap page (colour glyphs), once it has one.
    #[texture(0)]
    #[sampler(1)]
    pub bitmap: Option<Handle<Image>>,
}

impl Material for ReactionMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    /// Blended meshes are drawn farthest first, sorted by the square of their distance from
    /// the camera less this bias, so the bias is in square metres. A reaction is sorted as
    /// nearer than any other blended mesh (an avatar's face is biased by 10,000) while it is
    /// within 100 m of the camera, so it is drawn after them and shows over them, as it does
    /// over what is solid. Other players' are within the 25 m they are drawn in; the local
    /// player's may be farther from a scene's camera, and are then sorted as any mesh is.
    fn depth_bias(&self) -> f32 {
        20_000.0
    }

    fn specialize(
        _: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
        ])?];
        // the vertex stage turns the quad to the camera
        descriptor.primitive.cull_mode = None;
        // a reaction shows whatever stands between it and the camera
        if let Some(depth_stencil) = descriptor.depth_stencil.as_mut() {
            depth_stencil.depth_compare = CompareFunction::Always;
        }
        Ok(())
    }
}

/// How opaque a particle is, as the byte of its mesh tag the shader reads: 0 for none of it,
/// 255 for all.
pub fn opacity_tag(opacity: f32) -> u32 {
    (opacity.clamp(0.0, 1.0) * 255.0).round() as u32
}

/// The mesh of an emoji from its glyphs' quads (text pixels, y down): fitted to the unit square
/// around their middle, keeping their shape, with uvs in atlas pixels. `None` for no quads, or
/// quads of no size.
pub fn emoji_mesh(quads: &[TextQuad]) -> Option<Mesh> {
    let min = quads.iter().map(|quad| quad.min).reduce(Vec2::min)?;
    let max = quads.iter().map(|quad| quad.max).reduce(Vec2::max)?;
    let middle = (min + max) * 0.5;
    let extent = (max - min).max_element();
    if extent <= 0.0 {
        return None;
    }

    let mut positions = Vec::with_capacity(quads.len() * 4);
    let mut uvs = Vec::with_capacity(quads.len() * 4);
    let mut indices = Vec::with_capacity(quads.len() * 6);
    for quad in quads {
        let base = positions.len() as u32;
        // up is the other way in the world; the atlas' v runs down as the text does
        let (left, right) = (
            (quad.min.x - middle.x) / extent,
            (quad.max.x - middle.x) / extent,
        );
        let (top, bottom) = (
            (middle.y - quad.min.y) / extent,
            (middle.y - quad.max.y) / extent,
        );
        positions.extend([
            [left, bottom, 0.0],
            [right, bottom, 0.0],
            [right, top, 0.0],
            [left, top, 0.0],
        ]);
        uvs.extend([
            [quad.uv_min.x, quad.uv_max.y],
            [quad.uv_max.x, quad.uv_max.y],
            [quad.uv_max.x, quad.uv_min.y],
            [quad.uv_min.x, quad.uv_min.y],
        ]);
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices)),
    )
}

#[cfg(test)]
mod tests {
    use bevy::render::mesh::VertexAttributeValues;

    use super::*;

    fn quad(min: Vec2, max: Vec2, uv_min: Vec2, uv_max: Vec2) -> TextQuad {
        TextQuad {
            min,
            max,
            z: 0.0,
            uv_min,
            uv_max,
            color: LinearRgba::WHITE,
        }
    }

    fn positions(mesh: &Mesh) -> Vec<[f32; 3]> {
        match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(values)) => values.clone(),
            _ => panic!("no positions"),
        }
    }

    fn uvs(mesh: &Mesh) -> Vec<[f32; 2]> {
        match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
            Some(VertexAttributeValues::Float32x2(values)) => values.clone(),
            _ => panic!("no uvs"),
        }
    }

    #[test]
    fn an_emoji_fills_the_unit_square_whatever_it_was_rastered_at() {
        // wider than tall, away from the origin
        let mesh = emoji_mesh(&[quad(
            Vec2::new(10.0, 4.0),
            Vec2::new(90.0, 64.0),
            Vec2::new(100.0, 200.0),
            Vec2::new(180.0, 260.0),
        )])
        .unwrap();
        let positions = positions(&mesh);
        assert_eq!(
            positions,
            [
                [-0.5, -0.375, 0.0],
                [0.5, -0.375, 0.0],
                [0.5, 0.375, 0.0],
                [-0.5, 0.375, 0.0],
            ]
        );
        // the glyph's top left in the atlas is the quad's top left in the world
        let uvs = uvs(&mesh);
        assert_eq!(uvs[3], [100.0, 200.0]);
        assert_eq!(uvs[1], [180.0, 260.0]);
    }

    #[test]
    fn an_emoji_of_several_glyphs_keeps_them_in_place() {
        let mesh = emoji_mesh(&[
            quad(Vec2::ZERO, Vec2::splat(40.0), Vec2::ZERO, Vec2::splat(40.0)),
            quad(
                Vec2::new(40.0, 0.0),
                Vec2::new(80.0, 40.0),
                Vec2::new(50.0, 0.0),
                Vec2::new(90.0, 40.0),
            ),
        ])
        .unwrap();
        let positions = positions(&mesh);
        assert_eq!(positions.len(), 8);
        // side by side across the square, half its height
        assert_eq!(positions[0], [-0.5, -0.25, 0.0]);
        assert_eq!(positions[6], [0.5, 0.25, 0.0]);
        assert_eq!(mesh.indices().unwrap().len(), 12);
    }

    #[test]
    fn nothing_to_draw_makes_no_mesh() {
        assert!(emoji_mesh(&[]).is_none());
        let point = Vec2::splat(3.0);
        assert!(emoji_mesh(&[quad(point, point, point, point)]).is_none());
    }

    #[test]
    fn the_opacity_tag_spans_the_byte() {
        assert_eq!(opacity_tag(0.0), 0);
        assert_eq!(opacity_tag(1.0), 255);
        assert_eq!(opacity_tag(0.5), 128);
        // out of range, and not a number
        assert_eq!(opacity_tag(-3.0), 0);
        assert_eq!(opacity_tag(7.0), 255);
        assert_eq!(opacity_tag(f32::NAN), 0);
        let steps = (0..=100).map(|step| opacity_tag(step as f32 / 100.0));
        assert!(steps.clone().zip(steps.skip(1)).all(|(a, b)| a <= b));
    }
}
