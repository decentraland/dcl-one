// Fragment stage of the landscape's tree and bush impostors: boimp's impostor fragment, cut out
// at half coverage and drawn in the main pass only (no depth prepass, no shadows), so the depth
// check below meets the scene's depth and never the impostor's own box.
#import bevy_pbr::{
    forward_io::FragmentOutput,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::{STANDARD_MATERIAL_FLAGS_UNLIT_BIT, STANDARD_MATERIAL_FLAGS_FOG_ENABLED_BIT},
    view_transformations::{direction_view_to_world, position_view_to_world, position_world_to_clip},
}
#import boimp::shared::{ImposterVertexOut, unpack_pbrinput, weighted_props}
#import boimp::bindings::{imposter_data, sample_positions_from_camera_dir, sample_uvs_unbounded, sample_tile_material}

const CUTOFF: f32 = 0.5;

@fragment
fn fragment(in: ImposterVertexOut) -> FragmentOutput {
    var out: FragmentOutput;

    let inv_rot = mat3x3(
        in.inverse_rotation_0c,
        in.inverse_rotation_1c,
        in.inverse_rotation_2c,
    );

    let camera_world_position = position_view_to_world(vec3<f32>(0.0));
#ifdef VIEW_PROJECTION_ORTHOGRAPHIC
    let back_vec = direction_view_to_world(vec3<f32>(0.0, 0.0, 1.0));
#else
    let back_vec = camera_world_position - in.world_position;
#endif
    // the grid covers no view from below the horizon: hold the horizon's
#ifdef GRID_HEMISPHERICAL
    let back = normalize(vec3<f32>(back_vec.x, max(back_vec.y, 0.0), back_vec.z));
#else
    let back = normalize(back_vec);
#endif

    // every texel read happens here, before any branch
    let samples = sample_positions_from_camera_dir(back * inv_rot);
    let uv_a = sample_uvs_unbounded(in.base_world_position, in.world_position, inv_rot, samples.tile_indices[0], in.content_half_extent);
    let uv_b = sample_uvs_unbounded(in.base_world_position, in.world_position, inv_rot, samples.tile_indices[1], in.content_half_extent);
    let props_a = sample_tile_material(uv_a, samples.tile_indices[0], vec2(0.0));
    let props_b = sample_tile_material(uv_b, samples.tile_indices[1], vec2(0.0));
#ifndef GRID_HORIZONTAL
    let uv_c = sample_uvs_unbounded(in.base_world_position, in.world_position, inv_rot, samples.tile_indices[2], in.content_half_extent);
    let props_c = sample_tile_material(uv_c, samples.tile_indices[2], vec2(0.0));
#endif

    let weights = samples.tile_weights;
    let props_ab = weighted_props(props_a, props_b, weights.x / max(weights.x + weights.y, 0.0001));
#ifndef GRID_HORIZONTAL
    let props = weighted_props(props_ab, props_c, (weights.x + weights.y) / (weights.x + weights.y + weights.z));
#else
    let props = props_ab;
#endif

    if props.rgba.a < CUTOFF {
        discard;
    }

#ifdef DEPTH_PREPASS
    // the stored depth is the surface's offset from the impostor's centre along the view
    let existing_depth_ndc = bevy_pbr::prepass_utils::prepass_depth(in.position, 0u);
    let surface_ndc = position_world_to_clip(in.base_world_position + back * props.depth * imposter_data.center_and_scale.w);
    if surface_ndc.z / surface_ndc.w < existing_depth_ndc {
        discard;
    }
#endif

    var pbr_input = unpack_pbrinput(props, in.position);
    pbr_input.N = inv_rot * normalize(pbr_input.N);
    pbr_input.world_normal = pbr_input.N;
    pbr_input.material.base_color.a = 1.0;

    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }
    pbr_input.material.flags |= STANDARD_MATERIAL_FLAGS_FOG_ENABLED_BIT;
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
