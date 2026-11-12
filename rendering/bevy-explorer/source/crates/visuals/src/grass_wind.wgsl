// Grass blades, flowers and reeds. Each vertex stands on the terrain under its instance's root,
// leans with the wind by its height above the root, and shrinks toward the root with distance
// from the camera, so far instances need no blending or discard. The blades take the ground's
// colour under them; flowers and reeds keep their own, blended toward the ground at the root.
//
// Mesh: uv is the instance's root (x/z from the parcel's centre), uv_b its height share (0 at the
// root, 1 at the top) and its part (grass/geometry.rs PART_*). To the fragment, uv carries the
// root's world x/z and uv_b the height share and the part plus a per-instance random.

#import bevy_pbr::{
    mesh_functions,
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
}
#import "embedded://visuals/terrain_height.wgsl"::terrain_height

#ifdef PREPASS_PIPELINE
    #import bevy_pbr::prepass_io::{Vertex, VertexOutput}
    #import bevy_render::globals::Globals
    // the prepass view layout has the globals at binding 1, not the forward layout's 11
    @group(0) @binding(1) var<uniform> globals: Globals;
#else
    #import bevy_pbr::forward_io::{Vertex, VertexOutput, FragmentOutput}
    #import bevy_pbr::{mesh_view_bindings::{globals, lights}, shadows}
    #import "embedded://shaders/shadow_softening.wgsl"::soften_directional_shadow
    #import bevy_pbr::pbr_fragment::pbr_input_from_standard_material
    #import bevy_pbr::pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing}
    #import "embedded://visuals/landscape_ground.wgsl"::ground_albedo_normal
#ifdef ENVIRONMENT_MAP
    #import bevy_pbr::{ambient, clustered_forward::ClusterableObjectIndexRanges, environment_map, lighting}
    #import "embedded://shaders/sky_envmap_lookup.wgsl"::{sky_envmap_input, sky_envmap_level}
#endif
#endif

// x: 1 while the landscape sways, 0 while it is held still
@group(2) @binding(108) var<uniform> wind_scale: vec4<f32>;

// The wind, as unity-explorer's grass material sets it.
// (1, 0, 1) in unity x/z, normalised, with unity's z negated
const WIND_DIRECTION: vec2<f32> = vec2(0.70710677, -0.70710677);
const WIND_STRENGTH: f32 = 0.128;
const WIND_SPEED: f32 = 3.44;
const AMBIENT_STRENGTH: f32 = 0.2;
const GUST_FREQUENCY: f32 = 0.33;
const GUST_SPEED: f32 = 5.0;
const GUST_STRENGTH: f32 = 0.055;
const SWINGING: f32 = 0.194;
const FLUTTER: f32 = 0.559;
// instances shrink away between these distances from the camera, in metres (grass.rs FADE_END)
const FADE: vec2<f32> = vec2(50.0, 100.0);

// How this shader reads those values (its own choices):
// radians of sway phase per metre along the wind, so neighbours sway out of step as a wave
// crosses the field
const SWAY_WAVENUMBER: f32 = 0.35;
// the flutter is this many times faster than the sway, and this share of the wind's strength
const FLUTTER_RATE: f32 = 4.0;
const FLUTTER_SHARE: f32 = 0.25;
const TAU: f32 = 6.2831855;

fn hash_u32(value: u32) -> u32 {
    var x = value;
    x ^= x >> 16u;
    x *= 0x7feb352du;
    x ^= x >> 15u;
    x *= 0x846ca68bu;
    x ^= x >> 16u;
    return x;
}

// 0 to 1 for an instance, from its root (bevy world x/z)
fn instance_random(root: vec2<f32>) -> f32 {
    let bits = bitcast<vec2<u32>>(root);
    return f32(hash_u32(bits.x ^ hash_u32(bits.y)) >> 8u) * (1.0 / 16777216.0);
}

// Horizontal lean, in metres per metre of height, of an instance rooted at `root` (bevy x/z).
// The ambient sway travels downwind and leans the grass with the wind, back toward upwind only
// by SWINGING; gusts are fronts GUST_SPEED m/s fast, GUST_FREQUENCY a second, pushing further
// downwind; the flutter is a quick shake across the wind.
fn lean(root: vec2<f32>, random: f32, time: f32) -> vec2<f32> {
    let along = dot(root, WIND_DIRECTION);
    let sway = sin(time * WIND_SPEED - along * SWAY_WAVENUMBER + random * TAU);
    let swing = mix(sway * 0.5 + 0.5, sway, SWINGING);
    let front = max(sin(TAU * GUST_FREQUENCY * (time - along / GUST_SPEED)), 0.0);
    let push = WIND_STRENGTH * (AMBIENT_STRENGTH + (1.0 - AMBIENT_STRENGTH) * swing)
        + GUST_STRENGTH * front * front;
    let flutter = sin(time * WIND_SPEED * FLUTTER_RATE + random * TAU * 7.0)
        * FLUTTER * WIND_STRENGTH * FLUTTER_SHARE;
    let across = vec2(-WIND_DIRECTION.y, WIND_DIRECTION.x);
    return (WIND_DIRECTION * push + across * flutter) * wind_scale.x;
}

// World position of a vertex `offset` from its instance's root at `root` (bevy x/z) on the
// ground at height `ground`: shrunk with distance, then leant by its height, keeping its
// distance from the root.
fn placed(root: vec2<f32>, ground: f32, offset: vec3<f32>, random: f32, time: f32) -> vec3<f32> {
    let base = vec3(root.x, ground, root.y);
    let size = 1.0 - smoothstep(FADE.x, FADE.y, distance(base, view.world_position));
    let rest = offset * size;
    let tilt = lean(root, random, time);
    let height = rest.y * inverseSqrt(1.0 + dot(tilt, tilt));
    return base + vec3(rest.x + tilt.x * rest.y, height, rest.z + tilt.y * rest.y);
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let root = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4(vertex.uv.x, 0.0, vertex.uv.y, 1.0),
    ).xyz;
    let offset = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4(vertex.position, 1.0),
    ).xyz - root;
    let random = instance_random(root.xz);
    let ground = root.y + terrain_height(root.xz);
    let world = placed(root.xz, ground, offset, random, globals.time);

    out.world_position = vec4(world, 1.0);
    out.position = position_world_to_clip(world);
    out.uv = root.xz;
    // the part is whole, the random below it a quarter clear of each whole number, so
    // interpolation never moves the part
    out.uv_b = vec2(vertex.uv_b.x, vertex.uv_b.y + 0.25 + 0.5 * random);

#ifdef PREPASS_PIPELINE
#ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
    out.world_normal = vec3(0.0, 1.0, 0.0);
#endif
#ifdef MOTION_VECTOR_PREPASS
    out.previous_world_position = vec4(
        placed(root.xz, ground, offset, random, globals.time - globals.delta_time),
        1.0,
    );
#endif
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
#endif
#else
    // lit as the ground under them
    out.world_normal = vec3(0.0, 1.0, 0.0);
#endif

#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif
    return out;
}

#ifndef PREPASS_PIPELINE

// the softened sun shadows of the scene materials, trees and ground (see shadow_softening.wgsl)
override fn shadows::fetch_directional_shadow(light_id: u32, frag_position: vec4<f32>, surface_normal: vec3<f32>, view_z: f32) -> f32 {
    let base = shadows::fetch_directional_shadow(light_id, frag_position, surface_normal, view_z);
    // far bound of the last cascade = the shadow distance for this light
    let light = &lights.directional_lights[light_id];
    let far = (*light).cascades[max((*light).num_cascades, 1u) - 1u].far_bound;
    return soften_directional_shadow(base, far, view_z);
}

#ifdef ENVIRONMENT_MAP
// the sky envmap, levelled towards the flat ambient light, is the only ambient light, as on the
// ground (see sky_envmap_lookup.wgsl)
override fn environment_map::compute_radiances(input: lighting::LayerLightingInput, clusterable_object_index_ranges: ptr<function, ClusterableObjectIndexRanges>, world_position: vec3<f32>, found_diffuse_indirect: bool) -> environment_map::EnvironmentMapRadiances {
    let upper = sky_envmap_input(input);
    let radiances = environment_map::compute_radiances(upper, clusterable_object_index_ranges, world_position, found_diffuse_indirect);
    return sky_envmap_level(radiances, input, upper, lights.ambient_color);
}

override fn ambient::ambient_light(world_position: vec4<f32>, world_normal: vec3<f32>, V: vec3<f32>, NdotV: f32, diffuse_color: vec3<f32>, specular_color: vec3<f32>, perceptual_roughness: f32, occlusion: vec3<f32>) -> vec3<f32> {
    return vec3(0.0);
}
#endif

// The colours, as unity-explorer's grass and flower materials set them.
// a blade is darkened toward its root by this share
const BLADE_DARKENING: f32 = 0.487;
// a flower's or reed's colour blends this far toward the ground's at its root, fading out by
// this share of its height
const FLOWER_COLOUR_MAP_STRENGTH: f32 = 0.241;
const FLOWER_COLOUR_MAP_HEIGHT: f32 = 0.178;
// each instance leans toward this colour by up to its alpha, by its random
const HUE_VARIATION: vec4<f32> = vec4(1.0, 0.5, 0.0, 0.1);

// The flower and reed parts' own colours (linear), lookalikes of their own.
fn part_colour(part: u32) -> vec3<f32> {
    switch part {
        case 2u: { return vec3(0.80, 0.80, 0.74); }
        case 3u: { return vec3(0.90, 0.55, 0.04); }
        case 4u: { return vec3(0.24, 0.09, 0.50); }
        case 5u: { return vec3(0.10, 0.03, 0.22); }
        case 6u: { return vec3(0.16, 0.07, 0.025); }
        default: { return vec3(0.07, 0.19, 0.03); }
    }
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    // sampled first, in uniform control flow. The root is the same over an instance, so this
    // reads the textures' finest level: one colour per instance, from the ground under it.
    let ground = ground_albedo_normal(in.uv).albedo;

    let height = in.uv_b.x;
    let part = u32(in.uv_b.y);
    let random = (fract(in.uv_b.y) - 0.25) * 2.0;
    var albedo: vec3<f32>;
    if part == 0u {
        albedo = ground * mix(1.0 - BLADE_DARKENING, 1.0, height);
    } else {
        let colour_map = FLOWER_COLOUR_MAP_STRENGTH
            * (1.0 - smoothstep(0.0, FLOWER_COLOUR_MAP_HEIGHT, height));
        albedo = mix(part_colour(part), ground, colour_map);
    }
    albedo = mix(albedo, HUE_VARIATION.rgb, HUE_VARIATION.a * random);

    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = vec4(albedo, 1.0);
    pbr_input.N = vec3(0.0, 1.0, 0.0);
    pbr_input.world_normal = vec3(0.0, 1.0, 0.0);

    var out: FragmentOutput;
    out.color = main_pass_post_lighting_processing(pbr_input, apply_pbr_lighting(pbr_input));
    return out;
}

#endif
