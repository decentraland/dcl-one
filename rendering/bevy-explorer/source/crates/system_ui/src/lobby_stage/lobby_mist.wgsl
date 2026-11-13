// Ground mist on a flat quad behind the avatar: two layers of drifting value noise thresholded
// into wisps, with a soft elliptical mask so the quad's border never shows. Unlit.

#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::globals,
    view_transformations::position_world_to_view,
}

struct LobbyMist {
    color: vec4<f32>,
    speed: vec4<f32>,
    scale: f32,
    threshold: f32,
    softness: f32,
    edge_softness: f32,
}

@group(2) @binding(0) var<uniform> mist: LobbyMist;

fn hash(p_in: vec2<f32>) -> f32 {
    var p = fract(p_in * vec2(123.34, 456.21));
    p = p + dot(p, p + 45.32);
    return fract(p.x * p.y);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    var f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    let a = hash(i);
    let b = hash(i + vec2(1.0, 0.0));
    let c = hash(i + vec2(0.0, 1.0));
    let d = hash(i + vec2(1.0, 1.0));
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

fn fbm(p_in: vec2<f32>) -> f32 {
    var p = p_in;
    var value = 0.0;
    var amplitude = 0.5;
    for (var i = 0; i < 3; i++) {
        value += value_noise(p) * amplitude;
        p = p * 2.1 + 17.0;
        amplitude *= 0.5;
    }
    return value;
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    let time = globals.time;
    let p = in.uv * mist.scale;
    let layer1 = fbm(p + mist.speed.xy * time);
    let layer2 = fbm(p * 1.7 + mist.speed.zw * time + 31.0);
    let density = smoothstep(mist.threshold, mist.threshold + mist.softness, layer1 * 0.6 + layer2 * 0.4);
    let centred = (in.uv - 0.5) * 2.0;
    let edge = 1.0 - smoothstep(1.0 - mist.edge_softness, 1.0, length(centred));

    var out: FragmentOutput;
    out.color = vec4(mist.color.rgb, mist.color.a * density * edge);
#ifdef TRANSPARENT_FOCUS_OUTPUT
    out.focus = vec4(-position_world_to_view(in.world_position.xyz).z, 1.0, 0.0, out.color.a);
#endif
    return out;
}
