// Stage rocks: moss over their texture, the rock showing through more the less a surface faces
// up past the moss mask.

#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}

struct LobbyRock {
    moss: vec4<f32>,
    moss_mask: f32,
}

@group(2) @binding(100) var<uniform> rock: LobbyRock;

// the ramp's upper edge: past straight up, so no surface's rock share reaches zero by the ramp alone
const MOSS_TOP: f32 = 1.12;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr = pbr_input_from_standard_material(in, is_front);
    // the share of rock: the mask's place on a ramp that starts at how much the surface faces up
    let up = normalize(in.world_normal).y;
    let rock_share = smoothstep(up, MOSS_TOP, rock.moss_mask);
    pbr.material.base_color = vec4(mix(rock.moss.rgb, pbr.material.base_color.rgb, rock_share), pbr.material.base_color.a);
    var out: FragmentOutput;
    out.color = main_pass_post_lighting_processing(pbr, apply_pbr_lighting(pbr));
    return out;
}
