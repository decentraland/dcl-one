#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{SampleBias, alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing, calculate_tbn_mikktspace, apply_normal_mapping},
    pbr_bindings::{material, emissive_texture, emissive_sampler, normal_map_texture, normal_map_sampler},
    pbr_types::{STANDARD_MATERIAL_FLAGS_EMISSIVE_TEXTURE_BIT, STANDARD_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT, STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT},
    mesh_functions,
    mesh_view_bindings::{globals, view, lights},
    pbr_types,
    shadows,
    view_transformations,
}

#import "embedded://shaders/simplex.wgsl"::simplex_noise_3d
#import "embedded://shaders/bound_material_effect.wgsl"::{camera_dither, reveal_discard, toon_outline}
#import "embedded://shaders/toon.wgsl"::{facial_feature_lighting, toon_lighting}
#import "embedded://shaders/shadow_softening.wgsl"::soften_directional_shadow
#import "embedded://shaders/scene_bounds.wgsl"::{scene_bounds, scene_outside_amount, SCENE_DITHER_CAPSULE, SCENE_PLAIN_EMISSIVE, SCENE_ALPHA_TEXTURE}

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
#import "embedded://shaders/sky_envmap_lookup.wgsl"::{sky_envmap_input, sky_envmap_level, sky_envmap_light, sky_envmap_reflectivity, sky_envmap_metallic_reflectivity}

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
@group(2) @binding(102) var scene_alpha_texture: texture_2d<f32>;
@group(2) @binding(103) var scene_alpha_sampler: sampler;

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
#ifdef MULTISAMPLED
    @builtin(sample_index) sample_index: u32,
#endif
) -> FragmentOutput {

    // Lookup the tag for the given mesh
    let mesh_tag = mesh_functions::get_tag(in.instance_index);

    if reveal_discard(mesh_tag, in.world_position.y, in.instance_index) {
        discard;
    }

#ifdef INVERTED_SCALE
    let is_front_m = !is_front;
#else
    let is_front_m = is_front;
#endif

    let dither = camera_dither(mesh_tag, in.position.xy, in.world_position.xyz, in.instance_index, scene_bounds.base_opacity, (scene_bounds.flags & SCENE_DITHER_CAPSULE) != 0u);
    let cap_brightness = dither.transparency;
    let dither_opacity = dither.opacity;

    // generate a PbrInput struct from the StandardMaterial bindings
    var pbr_input = pbr_input_from_standard_material(in, is_front_m);
    var out: FragmentOutput;

    // the prepass holds the geometric normal (the avatar outline finds its edges in it), so a
    // normal read from it has no normal map applied
#ifdef LOAD_PREPASS_NORMALS
#ifdef VERTEX_UVS
#ifdef VERTEX_TANGENTS
#ifdef STANDARD_MATERIAL_NORMAL_MAP
    {
#ifdef STANDARD_MATERIAL_NORMAL_MAP_UV_B
        let normal_uv = (material.uv_transform * vec3(in.uv_b, 1.0)).xy;
#else
        let normal_uv = (material.uv_transform * vec3(in.uv, 1.0)).xy;
#endif
        let Nt = textureSampleBias(normal_map_texture, normal_map_sampler, normal_uv, view.mip_bias).rgb;
        let TBN = calculate_tbn_mikktspace(pbr_input.world_normal, in.world_tangent);
        let double_sided = (material.flags & STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u;
        pbr_input.N = apply_normal_mapping(material.flags, TBN, double_sided, is_front_m, Nt);
    }
#endif
#endif
#endif
#endif

#ifndef MULTISAMPLED
    let sample_index = 0u;
#endif

#ifdef VERTEX_UVS
    // the alpha texture's brightness scales the alpha, not its own alpha channel
    if (scene_bounds.flags & SCENE_ALPHA_TEXTURE) != 0u {
        let alpha_uv = (material.uv_transform * vec3(in.uv, 1.0)).xy;
        let alpha_texel = textureSampleBias(scene_alpha_texture, scene_alpha_sampler, alpha_uv, view.mip_bias);
        pbr_input.material.base_color.a *= saturate(length(alpha_texel.rgb));
    }
#endif

    // apply emmissive multiplier
    // dcl uses default 2.0 intensity. we also override bevy_pbr base emissive rules so that 
    // - if emissive texture is supplied but color is not, we use the texture (bevy by default multiplies emissive color and emissive texture, so color must be white to pass the texture through)
    // - if emissive color (== gltf emissive_intensity == dcl pbr emissive_color * emissive_intensity) is supplied but emissive texture is not, we use emissive color * base color
    // emissive color | emissive texture  | result
    // 0                no                  0
    // x                no                  x * base color
    // 0                t                   2 * t
    // x != 0           t                   x * t
    // an SDK material (SCENE_PLAIN_EMISSIVE) differs in two rows: x with no texture stays x,
    // and it binds no emissive texture when x is 0, so 0 with a texture is 0
    var emissive: vec3<f32> = material.emissive.rgb;
#ifdef VERTEX_UVS
    if ((material.flags & STANDARD_MATERIAL_FLAGS_EMISSIVE_TEXTURE_BIT) != 0u) {
        if dot(emissive, emissive) == 0.0 {
            emissive = vec3(2.0);
        }
        var bias: SampleBias;
        bias.mip_bias = view.mip_bias;
        emissive = vec3<f32>(emissive * textureSampleBias(
            emissive_texture, 
            emissive_sampler,
#ifdef STANDARD_MATERIAL_EMISSIVE_UV_B
#ifdef VERTEX_UVS_B
            (material.uv_transform * vec3(in.uv_b, 1.0)).xy,
#else
            (material.uv_transform * vec3(in.uv, 1.0)).xy,
#endif
#else
            (material.uv_transform * vec3(in.uv, 1.0)).xy,
#endif
            bias.mip_bias,
        ).rgb);
    } else {
        // emissive is set, no emissive texture, use base color texture as emissive texture (only if present)
        if ((material.flags & STANDARD_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT) != 0u && (scene_bounds.flags & SCENE_PLAIN_EMISSIVE) == 0u) {
            emissive = emissive * pbr_input.material.base_color.rgb;
        }
    }
#endif
    // a glTF emissive factor of 1 is five times as bright as a white surface under a light of
    // intensity 1; use 0 for auto-exposure weight (alpha channel)
    pbr_input.material.emissive = vec4(emissive * 5.0, 0.0);

    let world_position = pbr_input.world_position.xyz;
    // check bounds
    let outside_amt = scene_outside_amount(world_position);

    var noise = 0.05;
    var should_discard = false;
    if outside_amt > 0.00 {
        if outside_amt < scene_bounds.distance {
            noise = simplex_noise_3d(world_position * 2.0 + globals.time * vec3(0.2, 0.16, 0.24)) * 0.5 + 0.55;
            if noise < (outside_amt - 0.125) / 2.0 {
                should_discard = true;
            }
        } else if outside_amt > 0.05 {
            should_discard = true;
        }
    }

    if should_discard && ((mesh_tag & #{SHOW_OUTSIDE_BOUNDS_MESH_TAG}) == 0) {
        discard;
    }

    // alpha discard
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

    if (scene_bounds.flags & SCENE_PLAIN_EMISSIVE) != 0u {
        // the lighting scales the emissive light by the alpha, on top of the blend
        let alpha = max(pbr_input.material.base_color.a, 0.0001);
        pbr_input.material.emissive = vec4(pbr_input.material.emissive.rgb / alpha, 0.0);
        // premultiplied: the alpha thins the surface's colour, not its highlights
        if (material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_RESERVED_BITS) == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_PREMULTIPLIED {
            pbr_input.material.base_color = vec4(pbr_input.material.base_color.rgb * pbr_input.material.base_color.a, pbr_input.material.base_color.a);
        }
    }

#ifdef ENVIRONMENT_MAP
    sky_envmap_reflectivity = sky_envmap_metallic_reflectivity(pbr_input.material.metallic);
#endif

    // apply lighting
    if (pbr_input.material.flags & bevy_pbr::pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        if (mesh_tag & #{FACIAL_FEATURE_MESH_TAG}) != 0u {
            // the eyes, eyebrows and mouth without a mask texture: the face features' own
            // light model, by the ambient light (toon.wgsl)
            out.color = facial_feature_lighting(pbr_input);
        } else if (mesh_tag & #{TOON_MESH_TAG}) != 0u {
            // avatars: cel shading, a light model of its own (toon.wgsl) lit by the sun and
            // the ambient light; the sun's cast shadow pushes surfaces towards the shades
            out.color = toon_lighting(pbr_input);
        } else {
            out.color = apply_pbr_lighting(pbr_input);
        }
    } else {
        out.color = pbr_input.material.base_color;
    }

    if should_discard {
        out.color.a = out.color.a * 0.5;
        out.color.r = 4.0;
    } else {
        if noise < outside_amt / 2.0 {
            out.color = mix(out.color, vec4(10.0, 1.0, 0.0, 1.0), (outside_amt / 2.0 - noise) / 0.125);
        }
    }

    let view_z = -view_transformations::depth_ndc_to_view_z(in.position.z);

    if (mesh_tag & #{OUTLINE_MESH_TAG}) != 0u {
        // the avatar outline
        out.color = toon_outline(
            in.position,
            out.color,
            pbr_input.material.base_color.rgb,
            in.instance_index,
            view_z,
            sample_index,
            dither_opacity,
        );
    }

    // apply in-shader post processing (fog, alpha-premultiply, and also tonemapping, debanding if the camera is non-hdr)
    // note this does not include fullscreen postprocessing effects like bloom.
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);

    let cap_factor = max(max(out.color.r, out.color.g), max(out.color.b, 1.0));
    out.color = mix(out.color, vec4<f32>(out.color.rgb / cap_factor, out.color.a), saturate(cap_brightness * 2.0));

    if out.color.a < 0.001 {
        // avoid writing to the depth buffer for alpha-blend materials with low alpha
        discard;
    }

#ifdef TRANSPARENT_FOCUS_OUTPUT
    // accumulate alpha-weighted view depth + coverage for depth of field
    out.focus = vec4(
        view_z,
        1.0,
        0.0,
        out.color.a,
    );
#endif

    return out;
}
