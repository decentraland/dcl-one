// Fragment stage of the landscape trees, bushes and stones when they are baked into an impostor:
// the surface's colour, cut-out, normal and roughness as the lit material reads them, composited
// into the bake buffer over what the pixel already holds.
#import bevy_pbr::{
    prepass_io::VertexOutput,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}
#import boimp::shared::{compose_over, pack_pbrinput, pack_props, passes_depth_check, unpack_props}

struct BakeDims {
    width: u32,
}

@group(3) @binding(0) var<storage, read_write> bake_buffer: array<vec2<u32>>;
@group(3) @binding(1) var<uniform> bake_dims: BakeDims;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) {
    // every texture sample happens here, before any branch
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
    if pbr_input.material.base_color.a <= 0.0 {
        discard;
    }

    let new_props = unpack_props(pack_pbrinput(pbr_input));
    let pixel = vec2<u32>(in.position.xy);
    let index = pixel.y * bake_dims.width + pixel.x;
    let existing = unpack_props(bake_buffer[index]);
    if !passes_depth_check(new_props.depth, existing) {
        discard;
    }
    bake_buffer[index] = pack_props(compose_over(existing, new_props));
}
