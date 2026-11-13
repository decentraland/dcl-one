// Vertex stage of the landscape trees, bushes and stones: the mesh as bevy draws it, with the
// leaves swaying and fluttering in the wind. The wind is worked out in the reference frame
// (x east, z north, so z is bevy's -z), with its noise and phases taken from world positions
// there. Every pass applies it, so depth, shadows and colour agree.
#import bevy_pbr::{mesh_functions, view_transformations::position_world_to_clip}
#ifdef PREPASS_PIPELINE
#import bevy_pbr::prepass_io::{Vertex, VertexOutput}
#import bevy_render::globals::Globals
// Shadow and depth passes use the prepass view layout, where the globals are binding 1.
@group(0) @binding(1) var<uniform> globals: Globals;
#else
#import bevy_pbr::forward_io::{Vertex, VertexOutput}
#import bevy_pbr::mesh_view_bindings::globals
#endif

// speed, strength, frequency, turbulence
@group(2) @binding(100) var<uniform> sway_settings: vec4<f32>;
// direction in the reference frame, in xyz
@group(2) @binding(101) var<uniform> wind_direction: vec4<f32>;
// flutter speed, strength, frequency; w is 1 while the surface sways
@group(2) @binding(102) var<uniform> flutter_settings: vec4<f32>;

const TAU: f32 = 6.28318;

fn hash(p_in: vec3<f32>) -> f32 {
    var p = fract(p_in * 0.3183099 + 0.1);
    p *= 17.0;
    return fract(p.x * p.y * p.z * (p.x + p.y + p.z));
}

fn noise(x: vec3<f32>) -> f32 {
    let i = floor(x);
    var f = fract(x);
    f = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(
            mix(hash(i + vec3(0.0, 0.0, 0.0)), hash(i + vec3(1.0, 0.0, 0.0)), f.x),
            mix(hash(i + vec3(0.0, 1.0, 0.0)), hash(i + vec3(1.0, 1.0, 0.0)), f.x),
            f.y
        ),
        mix(
            mix(hash(i + vec3(0.0, 0.0, 1.0)), hash(i + vec3(1.0, 0.0, 1.0)), f.x),
            mix(hash(i + vec3(0.0, 1.0, 1.0)), hash(i + vec3(1.0, 1.0, 1.0)), f.x),
            f.y
        ),
        f.z
    );
}

// Three octaves of noise for the turbulence.
fn fbm(p: vec3<f32>) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    for (var octave = 0; octave < 3; octave++) {
        value += amplitude * noise(p * frequency);
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    return value;
}

struct Sway {
    // added to the world position, in bevy's frame
    offset: vec3<f32>,
    // added to the local normal before it is normalised, in bevy's frame
    normal: vec3<f32>,
}

// `world` is bevy's world position, `local_y` the mesh's own height, `uv` the texture
// coordinate: the quads' corners flutter, their centres stay put.
fn sway(world: vec3<f32>, local_y: f32, uv: vec2<f32>, time: f32) -> Sway {
    var out: Sway;
    out.offset = vec3(0.0);
    out.normal = vec3(0.0);
    if flutter_settings.w == 0.0 {
        return out;
    }
    let position = vec3(world.x, world.y, -world.z);
    let phase_offset = hash(position * 0.1) * TAU;

    let wind_time = time * sway_settings.x;
    let direction = normalize(wind_direction.xyz);
    let phase = dot(position, direction) * sway_settings.z + wind_time;
    let wave = pow(sin(phase) * 0.5 + 0.5, 1.5);
    let turbulence = fbm(position * 0.5 + wind_time * 0.3) * sway_settings.w;
    let force = (wave + turbulence) * sway_settings.y;

    let flutter_time = time * flutter_settings.x;
    let flutter = sin(flutter_time * flutter_settings.z + phase_offset)
        * cos(flutter_time * flutter_settings.z * 0.7 + phase_offset * 1.3)
        * flutter_settings.y;

    let edge = saturate(length(uv - 0.5) * 2.0);
    let height = saturate(local_y * 0.5 + 0.5);

    var offset = vec3(direction.x * force * height, force * 0.2 * height, direction.z * force * height);
    let flutter_direction = vec3(sin(phase_offset), cos(phase_offset * 1.3), sin(phase_offset * 0.7));
    offset += flutter_direction * flutter * edge;

    out.offset = vec3(offset.x, offset.y, -offset.z);
    let normal = direction * force * 0.3;
    out.normal = vec3(normal.x, normal.y, -normal.z);
    return out;
}

fn vertex_uv(vertex: Vertex) -> vec2<f32> {
#ifdef VERTEX_UVS_A
    return vertex.uv;
#else
    // without coordinates the quads do not flutter
    return vec2(0.5);
#endif
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4(vertex.position, 1.0));
    let wind = sway(world.xyz, vertex.position.y, vertex_uv(vertex), globals.time);
    out.world_position = world + vec4(wind.offset, 0.0);
    out.position = position_world_to_clip(out.world_position.xyz);

#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif

#ifdef PREPASS_PIPELINE
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
#endif
#ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(normalize(vertex.normal + wind.normal), vertex.instance_index);
#endif
#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, vertex.instance_index);
#endif
#endif
#ifdef MOTION_VECTOR_PREPASS
    let previous_from_local = mesh_functions::get_previous_world_from_local(vertex.instance_index);
    let previous = mesh_functions::mesh_position_local_to_world(previous_from_local, vec4(vertex.position, 1.0));
    let previous_wind = sway(previous.xyz, vertex.position.y, vertex_uv(vertex), globals.time - globals.delta_time);
    out.previous_world_position = previous + vec4(previous_wind.offset, 0.0);
#endif
#else
#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(normalize(vertex.normal + wind.normal), vertex.instance_index);
#endif
#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, vertex.instance_index);
#endif
#endif

#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif
    return out;
}
