#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_vertex_output,
    pbr_functions::main_pass_post_lighting_processing,
    mesh_view_bindings::{globals, lights, view},
    mesh_functions,
    shadows,
    view_transformations,
}
#import "embedded://shaders/simplex.wgsl"::simplex_noise_3d
#import "embedded://shaders/bound_material_effect.wgsl"::{camera_dither, reveal_discard, toon_outline}
#import "embedded://shaders/toon.wgsl"::facial_feature_lighting
#import "embedded://shaders/scene_bounds.wgsl"::{scene_bounds, scene_outside_amount, SCENE_DITHER_CAPSULE}
#import "embedded://shaders/shadow_softening.wgsl"::soften_directional_shadow

// same softened sun shadows as the avatar body (see bound_material.wgsl), so a face never stays
// shadowed where the body's shadow has faded out
override fn shadows::fetch_directional_shadow(light_id: u32, frag_position: vec4<f32>, surface_normal: vec3<f32>, view_z: f32) -> f32 {
    let base = shadows::fetch_directional_shadow(light_id, frag_position, surface_normal, view_z);
    // the far bound of the last cascade is the shadow distance for this light
    let light = &lights.directional_lights[light_id];
    let far = (*light).cascades[max((*light).num_cascades, 1u) - 1u].far_bound;
    let to_frag = frag_position.xyz - view.world_position;
    return soften_directional_shadow(base, far, dot(to_frag, to_frag));
}

#ifdef ENVIRONMENT_MAP
#import bevy_pbr::{ambient, clustered_forward::ClusterableObjectIndexRanges, environment_map, lighting}
#import "embedded://shaders/sky_envmap_lookup.wgsl"::{sky_envmap_input, sky_envmap_level, sky_envmap_light}

// the sky envmaps are the only ambient light: the Trilight ambient, and the sky reflections
// levelled towards the flat ambient light (see sky_envmap_lookup.wgsl)
override fn environment_map::compute_radiances(input: lighting::LayerLightingInput, clusterable_object_index_ranges: ptr<function, ClusterableObjectIndexRanges>, world_position: vec3<f32>, found_diffuse_indirect: bool) -> environment_map::EnvironmentMapRadiances {
    let upper = sky_envmap_input(input);
    let radiances = environment_map::compute_radiances(upper, clusterable_object_index_ranges, world_position, found_diffuse_indirect);
    return sky_envmap_level(radiances, input, upper, lights.ambient_color);
}

// the ambient diffuse at full strength, the reflection added on top (see sky_envmap_lookup.wgsl)
override fn environment_map::environment_map_light(input: ptr<function, lighting::LightingInput>, clusterable_object_index_ranges: ptr<function, ClusterableObjectIndexRanges>, found_diffuse_indirect: bool) -> environment_map::EnvironmentMapLight {
    return sky_envmap_light(input, clusterable_object_index_ranges, found_diffuse_indirect);
}

override fn ambient::ambient_light(world_position: vec4<f32>, world_normal: vec3<f32>, V: vec3<f32>, NdotV: f32, diffuse_color: vec3<f32>, specular_color: vec3<f32>, perceptual_roughness: f32, occlusion: vec3<f32>) -> vec3<f32> {
    return vec3(0.0);
}
#endif

struct MaskMaterial {
    color: vec4<f32>,
};

@group(2) @binding(0)
var<uniform> material: MaskMaterial;
@group(2) @binding(1)
var base_texture: texture_2d<f32>;
@group(2) @binding(2)
var base_sampler: sampler;
@group(2) @binding(3)
var mask_texture: texture_2d<f32>;
@group(2) @binding(4)
var mask_sampler: sampler;

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
#ifdef MULTISAMPLED
    @builtin(sample_index) sample_index: u32,
#endif
) -> FragmentOutput {
#ifndef MULTISAMPLED
    let sample_index = 0u;
#endif

    // a mask carries the tag of the body it is drawn on
    let mesh_tag = mesh_functions::get_tag(in.instance_index);

    if reveal_discard(mesh_tag, in.world_position.y, in.instance_index) {
        discard;
    }

    // the camera-to-player dither, the same call bound_material.wgsl makes for the body: by the
    // avatar's capsule, so a mask drops the pixels the head under it drops
    let dither = camera_dither(mesh_tag, in.position.xy, in.world_position.xyz, in.instance_index, scene_bounds.base_opacity, (scene_bounds.flags & SCENE_DITHER_CAPSULE) != 0u);
    let cap_brightness = dither.transparency;
    let dither_opacity = dither.opacity;

    var pbr_input = pbr_input_from_vertex_output(in, is_front, false);

    let world_position = pbr_input.world_position.xyz;
    // check bounds
    let outside_amt = scene_outside_amount(world_position);

    var noise = 0.05;
    if outside_amt > 0.00 {
        if outside_amt < scene_bounds.distance {
            noise = simplex_noise_3d(world_position * 2.0 + globals.time * vec3(0.2, 0.16, 0.24)) * 0.5 + 0.55;
            if noise < (outside_amt - 0.125) / 2.0 {
                discard;
            }
        } else if outside_amt > 0.05 {
            discard;
        }
    }

    let mask = textureSample(mask_texture, mask_sampler, in.uv);
    let base = textureSample(base_texture, base_sampler, in.uv);
    let color_amt = mask.r * mask.a;

    let color = mix(material.color, vec4<f32>(1.0), color_amt) * base;

    pbr_input.material.base_color = color;

    var out: FragmentOutput;
    // apply lighting: the face features' own, by the ambient light (toon.wgsl)
    out.color = facial_feature_lighting(pbr_input);

    if (mesh_tag & #{OUTLINE_MESH_TAG}) != 0u {
        // the avatar outline, in the tone of the mask's own colour
        out.color = toon_outline(
            in.position,
            out.color,
            color.rgb,
            in.instance_index,
            -view_transformations::depth_ndc_to_view_z(in.position.z),
            sample_index,
            dither_opacity,
        );
    }

    // apply in-shader post processing (fog, alpha-premultiply, and also tonemapping, debanding if the camera is non-hdr)
    // note this does not include fullscreen postprocessing effects like bloom.
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);

    // a fading mask is capped as the fading body is, so bloom doesn't fill the discarded pixels
    let cap_factor = max(max(out.color.r, out.color.g), max(out.color.b, 1.0));
    out.color = mix(out.color, vec4<f32>(out.color.rgb / cap_factor, out.color.a), saturate(cap_brightness * 2.0));

    return out;
}
