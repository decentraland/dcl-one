// Cliffs and sand: standard lighting, the rock textured from the reference cliff and the sand
// vertex-coloured, with the shore wash on the sand.

#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::globals,
}
#import "embedded://visuals/coast_surf.wgsl"::coast_surf
#import "embedded://visuals/coast_profile.wgsl"::{coast_outside, coast_profile}
#import "embedded://shaders/simplex.wgsl"::simplex_noise_2d

#ifdef ENVIRONMENT_MAP
#import bevy_pbr::{ambient, clustered_forward::ClusterableObjectIndexRanges, environment_map, lighting, mesh_view_bindings::lights}
#import "embedded://shaders/sky_envmap_lookup.wgsl"::{sky_envmap_input, sky_envmap_level}

// the sky envmap, levelled towards the flat ambient light, is the only ambient light
// (see sky_envmap_lookup.wgsl)
override fn environment_map::compute_radiances(input: lighting::LayerLightingInput, clusterable_object_index_ranges: ptr<function, ClusterableObjectIndexRanges>, world_position: vec3<f32>, found_diffuse_indirect: bool) -> environment_map::EnvironmentMapRadiances {
    let upper = sky_envmap_input(input);
    let radiances = environment_map::compute_radiances(upper, clusterable_object_index_ranges, world_position, found_diffuse_indirect);
    return sky_envmap_level(radiances, input, upper, lights.ambient_color);
}

override fn ambient::ambient_light(world_position: vec4<f32>, world_normal: vec3<f32>, V: vec3<f32>, NdotV: f32, diffuse_color: vec3<f32>, specular_color: vec3<f32>, perceptual_roughness: f32, occlusion: vec3<f32>) -> vec3<f32> {
    return vec3(0.0);
}
#endif

@group(2) @binding(100) var<uniform> land_bounds: vec4<f32>;
@group(2) @binding(101) var rock_colour: texture_2d<f32>;
@group(2) @binding(102) var rock_colour_sampler: sampler;
@group(2) @binding(103) var rock_normal: texture_2d<f32>;
@group(2) @binding(104) var rock_normal_sampler: sampler;

// metres per repeat of the rock texture: the texel density of the reference cliff mesh's uvs
const ROCK_TEXTURE_METRES: f32 = 17.08;
// the reference rock material: its smoothness, and a moss colour (sRGB) that takes over as faces
// turn upward, fully from a normal y of MOSS_MASK
const ROCK_SMOOTHNESS: f32 = 0.082;
const MOSS_SRGB: vec3<f32> = vec3(0.649, 0.23631646, 0.16419701);
const MOSS_MASK: f32 = 0.752;
const MOSS_EDGE: f32 = 1.12;

struct Rock {
    albedo: vec3<f32>,
    normal: vec3<f32>,
}

// One projection of the triplanar rock: tangent `t` and image-up `b` span the plane, `a` is its
// axis, with t x b = a so the map's green channel points along b. Blends the map's normal with
// the face normal `n` (whiteout) and returns it in world space.
fn rock_plane(p: vec3<f32>, n: vec3<f32>, t: vec3<f32>, b: vec3<f32>, a: vec3<f32>) -> Rock {
    // image rows run down the texture, so v grows against b
    let uv = vec2(dot(p, t), -dot(p, b)) / ROCK_TEXTURE_METRES;
    let albedo = textureSample(rock_colour, rock_colour_sampler, uv).rgb;
    let detail = textureSample(rock_normal, rock_normal_sampler, uv).rgb * 2.0 - vec3(1.0);
    let blended = vec3(detail.xy + vec2(dot(n, t), dot(n, b)), detail.z * dot(n, a));
    return Rock(albedo, t * blended.x + b * blended.y + a * blended.z);
}

// The procedural cliff has no uvs: the rock texture is projected along each world axis and the
// three blended by the face normal `n`, mirrored on negative faces so the normals keep their
// handedness. Samples textures: call in uniform control flow.
fn rock(p: vec3<f32>, n: vec3<f32>) -> Rock {
    let s = select(vec3(-1.0), vec3(1.0), n >= vec3(0.0));
    let x = rock_plane(p, n, vec3(0.0, 0.0, -s.x), vec3(0.0, 1.0, 0.0), vec3(s.x, 0.0, 0.0));
    let y = rock_plane(p, n, vec3(s.y, 0.0, 0.0), vec3(0.0, 0.0, -1.0), vec3(0.0, s.y, 0.0));
    let z = rock_plane(p, n, vec3(s.z, 0.0, 0.0), vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, s.z));
    var w = pow(abs(n), vec3(4.0));
    w /= w.x + w.y + w.z;
    let colour = x.albedo * w.x + y.albedo * w.y + z.albedo * w.z;
    let moss = pow((MOSS_SRGB + 0.055) / 1.055, vec3(2.4));
    let albedo = mix(moss, colour, smoothstep(n.y, MOSS_EDGE, MOSS_MASK));
    return Rock(albedo, normalize(x.normal * w.x + y.normal * w.y + z.normal * w.z));
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr = pbr_input_from_standard_material(in, is_front);
    // derivatives must be taken in uniform control flow (WebGPU rejects them in the branch)
    let p = in.world_position.xz;
    let footprint = max(length(dpdx(p)), length(dpdy(p)));
    let face = normalize(select(-in.world_normal, in.world_normal, is_front));
    let stone = rock(in.world_position.xyz, face);
#ifdef VERTEX_COLORS
    let rock_weight = in.color.a;
#else
    let rock_weight = 0.0;
#endif
    pbr.material.base_color = vec4(mix(pbr.material.base_color.rgb, stone.albedo, rock_weight), 1.0);
    pbr.material.perceptual_roughness =
        mix(pbr.material.perceptual_roughness, 1.0 - ROCK_SMOOTHNESS, rock_weight);
    pbr.N = normalize(mix(pbr.N, stone.normal, rock_weight));
    if in.world_position.y < -16.2 {
        let fade = 1.0 - smoothstep(0.27, 0.8, footprint);
        let sand = max(pbr.N.y, 0.0);
        let unity = p * vec2(1.0, -1.0);
        // crest highlights and trough shadows vary independently over a few metres, averaging
        // to the plain profile
        let glint = 1.0 + simplex_noise_2d(p * 0.23) * 0.7;
        // wind ripples about 1 m apart along the beach: a gentle seaward face rising to a
        // sharp crest, a landward face that falls steeply then eases into a broad trough,
        // varying in spacing, shape and height along the shore
        var height = 0.5;
        if fade > 0.0 && sand > 0.0 {
            // across the beach from the cliff foot (coast.rs's ring 3, 12.3 m from the waterline
            // on average) to the waterline, so the bands run parallel to both; the meander calms
            // near the foot so bands don't cut into the rock, and the swash zone smooths out
            let shore = coast_profile(clamp(unity, land_bounds.xy, land_bounds.zw));
            let foot = shore.x + 5.7 + (shore.y - shore.x - 18.0) * 0.4;
            let out = coast_outside(unity, land_bounds);
            let across = (out - foot) / (shore.y - foot);
            let calm = smoothstep(0.0, 4.0, out - foot);
            let keep = 1.0 - smoothstep(-8.0, 0.0, out - shore.y);
            let phase = -across * 12.3
                + (simplex_noise_2d(p * 0.06) * 0.9 + simplex_noise_2d(p * 0.19) * 0.2) * calm;
            let band = fract(phase);
            let crest = 0.6 + simplex_noise_2d(p * 0.09) * 0.1;
            let amplitude = 0.75 + simplex_noise_2d(p * 0.045) * 0.45;
            var rise: f32;
            if band < crest {
                height = (band / crest) * (band / crest);
                rise = 2.0 * band / (crest * crest);
            } else {
                let t = (band - crest) / (1.0 - crest);
                height = (1.0 - t) * (1.0 - t);
                rise = -2.0 * (1.0 - t) / (1.0 - crest);
            }
            height = mix(0.5, height, amplitude * fade * sand * keep);
            // phase runs landward, so the surface climbs outward at -rise / 1 m per metre,
            // 0.1 m high
            let outward = unity - clamp(unity, land_bounds.xy, land_bounds.zw);
            if dot(outward, outward) > 0.0 {
                let peak = mix(1.0, glint, smoothstep(0.5, 0.9, height));
                let slope = -rise * 0.1 * amplitude * peak * fade * sand * keep;
                let o = normalize(outward);
                pbr.N = normalize(pbr.N - vec3(o.x, 0.0, -o.y) * slope);
            }
        }
        let tint = 1.0 + simplex_noise_2d(p * 0.05) * 0.06;
        pbr.material.perceptual_roughness = mix(pbr.material.perceptual_roughness, 0.7, sand);
        let wash = coast_surf(p * vec2(1.0, -1.0), land_bounds, globals.time, footprint);
        let shadow = simplex_noise_2d(p * 0.21 + 31.7) * 0.7;
        let trough = 1.0 - smoothstep(0.0, 0.35, height);
        let dry = pbr.material.base_color.rgb * tint
            * (0.93 + 0.1 * height + 0.14 * pow(max(height, 0.0), 8.0) * glint)
            * (1.0 - 0.08 * trough * shadow);
        let wet = dry * (1.0 - wash.z * 0.32);
        // meet the water's colour and gloss at the waterline, where coast_water.wgsl starts its
        // seabed
        let sink = smoothstep(-1.5, 0.0, wash.w);
        let submerged = mix(wet, vec3(0.295, 0.233, 0.126), sink);
        pbr.material.perceptual_roughness = mix(pbr.material.perceptual_roughness, 0.28, sink);
        let film = mix(submerged, vec3(0.09, 0.15, 0.19), wash.y * 0.35);
        pbr.material.base_color = vec4(mix(film, vec3(0.72, 0.78, 0.78), wash.x), 1.0);
    }
    var out: FragmentOutput;
    out.color = main_pass_post_lighting_processing(pbr, apply_pbr_lighting(pbr));
    return out;
}
