// Dust motes: camera-facing quads drawn as soft discs, tinted by each mote's vertex colour, so
// no texture is needed. Unlit.

#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    view_transformations::position_world_to_view,
}

struct LobbyDust {
    color: vec4<f32>,
    softness: f32,
}

@group(2) @binding(0) var<uniform> dust: LobbyDust;

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    let from_centre = length(in.uv - 0.5) * 2.0;
    let disc = 1.0 - smoothstep(1.0 - dust.softness, 1.0, from_centre);
#ifdef VERTEX_COLORS
    let tint = in.color * dust.color;
#else
    let tint = dust.color;
#endif
    var out: FragmentOutput;
    out.color = vec4(tint.rgb, tint.a * disc);
#ifdef TRANSPARENT_FOCUS_OUTPUT
    out.focus = vec4(-position_world_to_view(in.world_position.xyz).z, 1.0, 0.0, out.color.a);
#endif
    return out;
}
