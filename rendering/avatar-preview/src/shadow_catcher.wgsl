// A floor that is invisible except where a shadow falls on it: black with the shadow's
// strength as alpha, so on a transparent canvas the host page shows the avatar's shadow.

#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings::{lights, view},
    shadows::fetch_directional_shadow,
}

// x: the darkest the shadow gets (0..1), y: metres from the centre where the floor fades out
@group(2) @binding(0) var<uniform> catcher: vec4<f32>;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let view_z = dot(vec4<f32>(
        view.view_from_world[0].z,
        view.view_from_world[1].z,
        view.view_from_world[2].z,
        view.view_from_world[3].z,
    ), in.world_position);
    var lit = 1.0;
    if lights.n_directional_lights > 0u {
        lit = fetch_directional_shadow(0u, in.world_position, normalize(in.world_normal), view_z);
    }
    let fade = 1.0 - smoothstep(0.55, 1.0, length(in.world_position.xz) / catcher.y);
    return vec4<f32>(0.0, 0.0, 0.0, (1.0 - lit) * catcher.x * fade);
}
