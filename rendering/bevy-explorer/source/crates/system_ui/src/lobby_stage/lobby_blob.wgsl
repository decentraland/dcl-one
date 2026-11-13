// The soft contact shadow under the avatar: the shape map's grey value is its coverage. Unlit.

#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    view_transformations::position_world_to_view,
}

@group(2) @binding(0) var<uniform> color: vec4<f32>;
@group(2) @binding(1) var shape_texture: texture_2d<f32>;
@group(2) @binding(2) var shape_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    let shape = textureSample(shape_texture, shape_sampler, in.uv).r;
    var out: FragmentOutput;
    out.color = vec4(color.rgb, color.a * shape);
#ifdef TRANSPARENT_FOCUS_OUTPUT
    out.focus = vec4(-position_world_to_view(in.world_position.xyz).z, 1.0, 0.0, out.color.a);
#endif
    return out;
}
