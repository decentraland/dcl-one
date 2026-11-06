// The floor under the avatar: invisible except for the avatar's shadow and, beneath it, a faint
// glow around the feet, as the Unity preview renderer's ShadowCatcher and GlowCatcher draw them.
// Written as straight alpha, so on a transparent canvas the host page shows through.

#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings::{lights, view},
    shadows::fetch_directional_shadow,
}

// x: the darkest the shadow gets (0..1), y: metres where the shadow has faded out,
// z: the glow's strength (1 is the Unity renderer's), w: the glow's radius in metres
@group(2) @binding(0) var<uniform> catcher: vec4<f32>;

const GLOW_COLOR: vec3<f32> = vec3<f32>(0.9255, 0.9725, 0.9765);
const GLOW_CENTRE: f32 = 0.03;

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
    let distance = length(in.world_position.xz);
    let shadow_fade = 1.0 - smoothstep(0.6, 1.0, distance / catcher.y);
    let shadow = (1.0 - lit) * catcher.x * shadow_fade;

    // two ramps: the centre's strength down to 5% of it at 0.45 of the radius, then to nothing
    // at 0.78
    let r = distance / max(catcher.w, 0.01);
    let inner = mix(GLOW_CENTRE, GLOW_CENTRE * 0.05, smoothstep(0.0, 0.45, r));
    let glow = select(mix(GLOW_CENTRE * 0.05, 0.0, smoothstep(0.45, 0.78, r)), inner, r < 0.45)
        * catcher.z;

    // the shadow drawn over the glow
    let alpha = shadow + glow * (1.0 - shadow);
    let colour = GLOW_COLOR * glow * (1.0 - shadow) / max(alpha, 1e-4);
    return vec4<f32>(colour, alpha);
}
