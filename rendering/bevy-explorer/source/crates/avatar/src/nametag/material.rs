//! The one material every nametag draws with (nametag.wgsl), and the quads a tag's mesh is
//! made of. A tag is a single mesh, so its quads paint in order within one draw: background,
//! then badges, then text, with no depth bias between them.

use bevy::{
    math::{Rect, Vec2, Vec4},
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::{
        mesh::{Indices, MeshVertexAttribute, MeshVertexBufferLayoutRef, PrimitiveTopology},
        render_asset::RenderAssetUsages,
        render_resource::{
            AsBindGroup, RenderPipelineDescriptor, ShaderDefVal, ShaderRef,
            SpecializedMeshPipelineError, VertexFormat,
        },
    },
};
use scene_runner::update_world::text_shape::GlyphAtlas;

use super::{
    layout::{Tail, TAIL_CLIP, TAIL_SIZE, TAIL_TIP_RADIUS},
    FADE_DISTANCE, MAX_DISTANCE, SCREEN_SCALE, UNIT,
};

const SHADER: &str = "embedded://shaders/nametag.wgsl";

const ATTRIBUTE_SHAPE: MeshVertexAttribute =
    MeshVertexAttribute::new("NametagShape", 0x6e74_6701, VertexFormat::Float32x4);
const ATTRIBUTE_RADII: MeshVertexAttribute =
    MeshVertexAttribute::new("NametagRadii", 0x6e74_6702, VertexFormat::Float32x4);

const KIND_GLYPH: u32 = 0;
const KIND_BITMAP: u32 = 1;
const KIND_ICON: u32 = 2;
const KIND_RECT: u32 = 3;
const KIND_VOICE: u32 = 4;

/// The tag's world size per tag unit, metre of distance and unit of `tan(fov / 2)`, in
/// millionths: shader definitions are whole numbers.
const SCALE_MILLIONTHS: u32 = (SCREEN_SCALE * UNIT * 1e6) as u32;
/// How long a voice bar takes from one of its heights to the other, in milliseconds.
const VOICE_PERIOD_MS: u32 = 300;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Verified = 0,
    Official = 1,
    Hushed = 2,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct NametagMaterial {
    /// The glyph atlas' distance-field page.
    #[texture(0)]
    #[sampler(1)]
    pub sdf: Handle<Image>,
    /// The glyph atlas' bitmap page (colour glyphs), once it has one.
    #[texture(2)]
    #[sampler(3)]
    pub bitmap: Option<Handle<Image>>,
    #[texture(4)]
    #[sampler(7)]
    pub verified: Handle<Image>,
    #[texture(5)]
    pub official: Handle<Image>,
    #[texture(6)]
    pub hushed: Handle<Image>,
}

impl Material for NametagMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
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
            Mesh::ATTRIBUTE_COLOR.at_shader_location(2),
            ATTRIBUTE_SHAPE.at_shader_location(3),
            ATTRIBUTE_RADII.at_shader_location(4),
        ])?];
        // the vertex stage turns a tag to the camera
        descriptor.primitive.cull_mode = None;

        let defs = [
            ("NAMETAG_KIND_GLYPH", KIND_GLYPH),
            ("NAMETAG_KIND_BITMAP", KIND_BITMAP),
            ("NAMETAG_KIND_ICON", KIND_ICON),
            ("NAMETAG_KIND_RECT", KIND_RECT),
            ("NAMETAG_ICON_VERIFIED", Icon::Verified as u32),
            ("NAMETAG_ICON_OFFICIAL", Icon::Official as u32),
            ("NAMETAG_SDF_SPREAD", GlyphAtlas::SPREAD),
            ("NAMETAG_SCALE_MILLIONTHS", SCALE_MILLIONTHS),
            ("NAMETAG_FADE_DISTANCE", FADE_DISTANCE as u32),
            ("NAMETAG_MAX_DISTANCE", MAX_DISTANCE as u32),
            ("NAMETAG_VOICE_PERIOD_MS", VOICE_PERIOD_MS),
        ]
        .map(|(name, value)| ShaderDefVal::UInt(name.to_owned(), value));
        descriptor.vertex.shader_defs.extend(defs.clone());
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader_defs.extend(defs);
        }
        Ok(())
    }
}

/// One quad of a tag, axis-aligned in tag units (y up). Its uv is affine in its position, so
/// a quad can be cut down to a clip box without changing what the rest of it draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub min: Vec2,
    pub max: Vec2,
    /// The uv at `min`, and its change per unit of x and of y.
    uv_min: Vec2,
    uv_x: Vec2,
    uv_y: Vec2,
    /// Linear rgba at the bottom and at the top.
    color_bottom: Vec4,
    color_top: Vec4,
    shape: Vec4,
    radii: Vec4,
}

fn linear(color: Color) -> Vec4 {
    Vec4::from_array(color.to_linear().to_f32_array())
}

impl Quad {
    /// A filled rectangle with every corner rounded by `radius`.
    pub fn rect(rect: Rect, radius: f32, color: Color) -> Self {
        Self::gradient(rect, radius, color, color)
    }

    /// A rounded rectangle shading from `bottom` to `top`.
    pub fn gradient(rect: Rect, radius: f32, bottom: Color, top: Color) -> Self {
        let half = rect.half_size();
        Self {
            min: rect.min,
            max: rect.max,
            uv_min: -half,
            uv_x: Vec2::X,
            uv_y: Vec2::Y,
            color_bottom: linear(bottom),
            color_top: linear(top),
            shape: Vec4::new(KIND_RECT as f32, half.x, half.y, 0.0),
            radii: Vec4::splat(radius.min(half.min_element())),
        }
    }

    pub fn icon(rect: Rect, icon: Icon, tint: Color) -> Self {
        let size = rect.size();
        Self {
            min: rect.min,
            max: rect.max,
            // the texture's v runs down
            uv_min: Vec2::new(0.0, 1.0),
            uv_x: Vec2::new(1.0 / size.x, 0.0),
            uv_y: Vec2::new(0.0, -1.0 / size.y),
            color_bottom: linear(tint),
            color_top: linear(tint),
            shape: Vec4::new(KIND_ICON as f32, icon as u32 as f32, 0.0, 0.0),
            radii: Vec4::ZERO,
        }
    }

    /// A glyph of the atlas: `uv_min` and `uv_max` are its top left and bottom right there, in
    /// pixels. `color` is linear.
    pub fn glyph(rect: Rect, uv_min: Vec2, uv_max: Vec2, bitmap: bool, color: LinearRgba) -> Self {
        let size = rect.size();
        let color = Vec4::from_array(color.to_f32_array());
        Self {
            min: rect.min,
            max: rect.max,
            uv_min: Vec2::new(uv_min.x, uv_max.y),
            uv_x: Vec2::new((uv_max.x - uv_min.x) / size.x, 0.0),
            uv_y: Vec2::new(0.0, (uv_min.y - uv_max.y) / size.y),
            color_bottom: color,
            color_top: color,
            shape: Vec4::new(
                if bitmap { KIND_BITMAP } else { KIND_GLYPH } as f32,
                0.0,
                0.0,
                0.0,
            ),
            radii: Vec4::ZERO,
        }
    }

    /// A voice bar centred in `rect`, swinging between `from` and `to` tall. The shader moves
    /// it, so a speaking tag's mesh is not rebuilt.
    pub fn voice_bar(rect: Rect, from: f32, to: f32, color: Color) -> Self {
        let half = rect.half_size();
        Self {
            shape: Vec4::new(KIND_VOICE as f32, half.x, from, to),
            radii: Vec4::splat(half.x),
            ..Self::rect(rect, 0.0, color)
        }
    }

    /// The bubble's tail: the lower corner of a square standing on its rounded tip, as much of
    /// it as shows below the tag.
    pub fn tail(tail: Tail, color: Color) -> Self {
        let clip = TAIL_CLIP * tail.scale;
        // the square's own axes: it stands on its bottom right corner
        let x_axis = Vec2::new(1.0, -1.0) * std::f32::consts::FRAC_1_SQRT_2 / tail.scale;
        let y_axis = Vec2::new(1.0, 1.0) * std::f32::consts::FRAC_1_SQRT_2 / tail.scale;
        let min = tail.origin - Vec2::new(clip.x * 0.5, clip.y);
        let from_centre = min - tail.origin;
        Self {
            min,
            max: tail.origin + Vec2::new(clip.x * 0.5, 0.0),
            uv_min: Vec2::new(from_centre.dot(x_axis), from_centre.dot(y_axis)),
            uv_x: Vec2::new(x_axis.x, y_axis.x),
            uv_y: Vec2::new(x_axis.y, y_axis.y),
            color_bottom: linear(color),
            color_top: linear(color),
            shape: Vec4::new(KIND_RECT as f32, TAIL_SIZE * 0.5, TAIL_SIZE * 0.5, 0.0),
            // top right, bottom right, top left, bottom left
            radii: Vec4::new(0.0, TAIL_TIP_RADIUS, 0.0, 0.0),
        }
    }

    /// The same quad shading from `bottom` to `top`.
    pub fn with_colors(mut self, bottom: Color, top: Color) -> Self {
        self.color_bottom = linear(bottom);
        self.color_top = linear(top);
        self
    }

    /// The part of the quad within `clip`, drawing what it drew there before.
    pub fn clipped(mut self, clip: Rect) -> Option<Self> {
        let min = self.min.max(clip.min);
        let max = self.max.min(clip.max);
        if min.x >= max.x || min.y >= max.y {
            return None;
        }
        let height = self.max.y - self.min.y;
        let color_at = |y: f32| {
            self.color_bottom
                .lerp(self.color_top, (y - self.min.y) / height)
        };
        (self.color_bottom, self.color_top) = (color_at(min.y), color_at(max.y));
        let moved = min - self.min;
        self.uv_min += self.uv_x * moved.x + self.uv_y * moved.y;
        self.min = min;
        self.max = max;
        Some(self)
    }

    fn uv_at(&self, position: Vec2) -> Vec2 {
        let offset = position - self.min;
        self.uv_min + self.uv_x * offset.x + self.uv_y * offset.y
    }
}

/// The mesh of `quads`, painted in order and drawn `scale` times their size, and how far it
/// reaches from the anchor. Positions are tag units. The uvs stay as they are: a shape's are in
/// tag units from its centre, so the whole tag grows alike.
pub fn quad_mesh(quads: &[Quad], scale: f32) -> (Mesh, f32) {
    let count = quads.len() * 4;
    let mut positions = Vec::with_capacity(count);
    let mut uvs = Vec::with_capacity(count);
    let mut colors = Vec::with_capacity(count);
    let mut shapes = Vec::with_capacity(count);
    let mut radii = Vec::with_capacity(count);
    let mut indices = Vec::with_capacity(quads.len() * 6);
    let mut reach = 0f32;

    for quad in quads {
        let base = positions.len() as u32;
        for (corner, color) in [
            (quad.min, quad.color_bottom),
            (Vec2::new(quad.max.x, quad.min.y), quad.color_bottom),
            (quad.max, quad.color_top),
            (Vec2::new(quad.min.x, quad.max.y), quad.color_top),
        ] {
            positions.push([corner.x * scale, corner.y * scale, 0.0]);
            uvs.push(quad.uv_at(corner).to_array());
            colors.push(color.to_array());
            shapes.push(quad.shape.to_array());
            radii.push(quad.radii.to_array());
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        reach = reach.max(quad.min.abs().max(quad.max.abs()).length() * scale);
    }

    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_attribute(ATTRIBUTE_SHAPE, shapes)
    .with_inserted_attribute(ATTRIBUTE_RADII, radii)
    .with_inserted_indices(Indices::U32(indices));

    (mesh, reach)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shader_is_given_the_scale_whole() {
        assert_eq!(SCALE_MILLIONTHS as f32 * 1e-6, SCREEN_SCALE * UNIT);
        assert_eq!(FADE_DISTANCE.fract(), 0.0);
        assert_eq!(MAX_DISTANCE.fract(), 0.0);
    }

    #[test]
    fn a_clipped_quad_draws_what_it_drew_there() {
        let quad = Quad::glyph(
            Rect::new(10.0, 20.0, 30.0, 60.0),
            Vec2::new(100.0, 200.0),
            Vec2::new(140.0, 280.0),
            false,
            LinearRgba::WHITE,
        );
        let clip = Rect::new(15.0, 0.0, 100.0, 40.0);
        let clipped = quad.clipped(clip).unwrap();
        assert_eq!(clipped.min, Vec2::new(15.0, 20.0));
        assert_eq!(clipped.max, Vec2::new(30.0, 40.0));
        for corner in [clipped.min, clipped.max, Vec2::new(20.0, 33.0)] {
            assert!(clipped.uv_at(corner).distance(quad.uv_at(corner)) < 1e-3);
        }
        // the atlas' v runs down: the quad's top left is the glyph's
        assert!(
            quad.uv_at(Vec2::new(10.0, 60.0))
                .distance(Vec2::new(100.0, 200.0))
                < 1e-3
        );

        assert!(quad.clipped(Rect::new(40.0, 0.0, 50.0, 100.0)).is_none());
        assert_eq!(quad.clipped(Rect::new(0.0, 0.0, 100.0, 100.0)), Some(quad));
    }

    #[test]
    fn a_clipped_gradient_keeps_its_colours_in_place() {
        let quad = Quad::gradient(
            Rect::new(0.0, 0.0, 10.0, 10.0),
            0.0,
            Color::BLACK,
            Color::WHITE,
        );
        let clipped = quad.clipped(Rect::new(0.0, 5.0, 10.0, 10.0)).unwrap();
        assert!((clipped.color_bottom.x - 0.5).abs() < 1e-6);
        assert_eq!(clipped.color_top, quad.color_top);
    }

    #[test]
    fn the_tail_stands_on_its_rounded_corner() {
        let tail = Tail {
            origin: Vec2::new(0.0, 10.0),
            scale: 0.5,
        };
        let quad = Quad::tail(tail, Color::WHITE);
        assert_eq!(quad.max, Vec2::new(10.0, 10.0));
        assert_eq!(quad.min, Vec2::new(-10.0, -4.0));
        // the square's centre is the origin
        assert!(quad.uv_at(tail.origin).length() < 1e-4);
        // straight down from it is the square's bottom right corner (x positive, y negative),
        // half a diagonal away at full scale
        let below = quad.uv_at(tail.origin - Vec2::Y * 0.5);
        assert!(below.distance(Vec2::new(1.0, -1.0) * std::f32::consts::FRAC_1_SQRT_2) < 1e-4);
    }

    #[test]
    fn quads_keep_their_order_in_the_mesh() {
        let quads = [
            Quad::rect(Rect::new(-5.0, 0.0, 5.0, 4.0), 2.0, Color::BLACK),
            Quad::rect(Rect::new(-1.0, 1.0, 20.0, 3.0), 1.0, Color::WHITE),
        ];
        let (mesh, reach) = quad_mesh(&quads, 1.0);
        assert_eq!(mesh.count_vertices(), 8);
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("no indices");
        };
        assert_eq!(indices[..6], [0, 1, 2, 0, 2, 3]);
        assert_eq!(indices[6..], [4, 5, 6, 4, 6, 7]);
        // the corner farthest from the anchor
        assert_eq!(reach, Vec2::new(20.0, 3.0).length());
    }
}
