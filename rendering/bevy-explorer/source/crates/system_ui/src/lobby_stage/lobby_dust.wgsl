// Floating dust. The mesh holds one quad per slot (dust.rs): position.xy is the corner, position.z
// the slot. Each slot lives a lifetime, then respawns at a new point of the emitter box; the time
// is the frame's, so the motes move without the mesh changing.
#import bevy_pbr::mesh_functions::get_world_from_local
#import bevy_pbr::mesh_view_bindings::{view, globals}

struct LobbyDust {
    // linear rgba
    color: vec4<f32>,
    // in the stage's frame
    emitter_centre: vec4<f32>,
    emitter_size: vec4<f32>,
    // seconds min/max, then metres per second min/max
    lifetime_speed: vec4<f32>,
    // metres across min/max, then the drift's strength and cells per metre
    size_noise: vec4<f32>,
    noise_scroll: f32,
    softness: f32,
}

@group(2) @binding(0) var<uniform> dust: LobbyDust;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) view_depth: f32,
}

// matches bevy_pbr::forward_io::FragmentOutput: WebGPU needs every target of the transparent pass
struct FragmentOutput {
    @location(0) color: vec4<f32>,
#ifdef TRANSPARENT_FOCUS_OUTPUT
    @location(1) focus: vec4<f32>,
#endif
}

// PCG hash, 0..1
fn random(n: u32) -> f32 {
    var h = n * 747796405u + 2891336453u;
    h = ((h >> ((h >> 28u) + 4u)) ^ h) * 277803737u;
    h = (h >> 22u) ^ h;
    return f32(h >> 8u) / 16777216.0;
}

fn hash3(p: vec3<i32>) -> f32 {
    let u = bitcast<vec3<u32>>(p);
    return random(u.x * 0x8da6b343u ^ u.y * 0xd8163841u ^ u.z * 0xcb1ab31fu);
}

fn value_noise(p: vec3<f32>) -> f32 {
    let i = vec3<i32>(floor(p));
    let f = fract(p);
    let s = f * f * (3.0 - 2.0 * f);
    let x00 = mix(hash3(i), hash3(i + vec3(1, 0, 0)), s.x);
    let x10 = mix(hash3(i + vec3(0, 1, 0)), hash3(i + vec3(1, 1, 0)), s.x);
    let x01 = mix(hash3(i + vec3(0, 0, 1)), hash3(i + vec3(1, 0, 1)), s.x);
    let x11 = mix(hash3(i + vec3(0, 1, 1)), hash3(i + vec3(1, 1, 1)), s.x);
    return mix(mix(x00, x10, s.y), mix(x01, x11, s.y), s.z);
}

// metres per second, each axis -strength..strength
fn drift(position: vec3<f32>, time: f32) -> vec3<f32> {
    let p = position * dust.size_noise.w + vec3(time * dust.noise_scroll);
    let axis = vec3(value_noise(p), value_noise(p + vec3(31.7)), value_noise(p + vec3(73.1)));
    return (axis * 2.0 - 1.0) * dust.size_noise.z;
}

fn fade(life: f32) -> f32 {
    return 1.0 - abs(life * 2.0 - 1.0);
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    let corner = vertex.position.xy;
    let slot = u32(vertex.position.z + 0.5);
    let seed = slot * 16u;
    let lifetime = mix(dust.lifetime_speed.x, dust.lifetime_speed.y, random(seed));
    let lives = globals.time / lifetime + random(seed + 1u);
    let life = fract(lives);
    let age = life * lifetime;
    let born = seed ^ (u32(floor(lives)) * 0x9e3779b9u);
    let offset = vec3(random(born + 2u), random(born + 3u), random(born + 4u)) - 0.5;
    let spawn = dust.emitter_centre.xyz + offset * dust.emitter_size.xyz;
    let speed = mix(dust.lifetime_speed.z, dust.lifetime_speed.w, random(born + 5u));
    let size = mix(dust.size_noise.x, dust.size_noise.y, random(born + 6u));
    let local = spawn + vec3(0.0, 0.0, speed * age) + drift(spawn, globals.time) * age;

    let centre = (get_world_from_local(vertex.instance_index) * vec4(local, 1.0)).xyz;
    let right = normalize(view.world_from_view[0].xyz);
    let up = normalize(view.world_from_view[1].xyz);
    let world_position = centre + (right * corner.x + up * corner.y) * size;

    var out: VertexOutput;
    out.position = view.clip_from_world * vec4(world_position, 1.0);
    out.uv = vec2(corner.x + 0.5, 0.5 - corner.y);
    out.color = vec4(dust.color.rgb, dust.color.a * fade(life));
    out.view_depth = -(view.view_from_world * vec4(world_position, 1.0)).z;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    let from_centre = length(in.uv - 0.5) * 2.0;
    let disc = 1.0 - smoothstep(1.0 - dust.softness, 1.0, from_centre);
    var out: FragmentOutput;
    out.color = vec4(in.color.rgb, in.color.a * disc);
#ifdef TRANSPARENT_FOCUS_OUTPUT
    out.focus = vec4(in.view_depth, 1.0, 0.0, out.color.a);
#endif
    return out;
}
