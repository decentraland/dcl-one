// A situational reaction (avatar/src/reaction): an emoji from the glyph atlas' bitmap page on a
// quad that faces the camera. Unlit and blended, with no depth test: it shows over whatever
// stands in front of it. The mesh is a unit square around its centre; the entity's transform
// gives the centre and, as its scale, the width in metres, and the low byte of its tag the
// opacity. Its colours are tonemapped with the rest of the frame.

#import bevy_pbr::{
    forward_io::FragmentOutput,
    mesh_functions,
    mesh_view_bindings::view,
    view_transformations,
}

@group(2) @binding(0) var bitmap_texture: texture_2d<f32>;
@group(2) @binding(1) var bitmap_sampler: sampler;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    // the unit square, the emoji's centre at the origin
    @location(0) position: vec3<f32>,
    // atlas pixels
    @location(1) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) opacity: f32,
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let centre = world_from_local[3].xyz;
    let size = length(world_from_local[0].xyz);
    let right = view.world_from_view[0].xyz;
    let up = view.world_from_view[1].xyz;
    let world_position = centre + (right * vertex.position.x + up * vertex.position.y) * size;
    out.position = view_transformations::position_world_to_clip(world_position);
    out.uv = vertex.uv;
    out.opacity = f32(mesh_functions::get_tag(vertex.instance_index) & 0xffu) / 255.0;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    // derivatives first, in uniform control flow
    let dx = dpdx(in.uv);
    let dy = dpdy(in.uv);
    let dims = vec2<f32>(textureDimensions(bitmap_texture));
    let bitmap = textureSampleGrad(bitmap_texture, bitmap_sampler, in.uv / dims, dx / dims, dy / dims);

    var out: FragmentOutput;
    out.color = vec4(bitmap.rgb, bitmap.a * in.opacity);

#ifdef TRANSPARENT_FOCUS_OUTPUT
    // accumulate alpha-weighted view depth + coverage for depth of field
    out.focus = vec4(
        -view_transformations::depth_ndc_to_view_z(in.position.z),
        1.0,
        0.0,
        out.color.a,
    );
#endif

    return out;
}
