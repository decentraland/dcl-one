// The lobby stage's floor: the lit ground texture plus a soft coloured spot around the stage
// centre. It stays solid near the avatar and dissolves along a band, perpendicular to the view,
// that ends where the floor meets the backdrop on screen, so the backdrop shows through.

#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_functions::get_world_from_local,
    view_transformations::position_world_to_view,
}

struct LobbyFloor {
    spot_color: vec4<f32>,
    blend_axis: vec4<f32>,
    spot_radius: f32,
    spot_softness: f32,
    blend_start: f32,
    blend_end: f32,
    blend_hardness: f32,
}

@group(2) @binding(100) var<uniform> stage_floor: LobbyFloor;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    let pbr = pbr_input_from_standard_material(in, is_front);
    var color = apply_pbr_lighting(pbr);

    let origin = get_world_from_local(in.instance_index)[3].xz;
    let radial = length(in.world_position.xz - origin);
    let spot = 1.0 - smoothstep(stage_floor.spot_radius, stage_floor.spot_radius + stage_floor.spot_softness, radial);
    color = vec4(color.rgb + stage_floor.spot_color.rgb * spot, color.a);

    let along = dot(in.world_position.xyz, stage_floor.blend_axis.xyz);
    var blend = smoothstep(stage_floor.blend_start, stage_floor.blend_end, along);
    // hardness squeezes the gradient towards a step at the middle of the band
    blend = saturate((blend - 0.5) / max(1.0 - stage_floor.blend_hardness, 0.001) + 0.5);
    color.a = color.a * (1.0 - blend);

    var out: FragmentOutput;
    out.color = main_pass_post_lighting_processing(pbr, color);
#ifdef TRANSPARENT_FOCUS_OUTPUT
    out.focus = vec4(-position_world_to_view(in.world_position.xyz).z, 1.0, 0.0, out.color.a);
#endif
    return out;
}
