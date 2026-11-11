// Lookups into the sky environment maps (visuals/src/sky_envmap.rs), shared by the pbr material
// shaders. Each overrides environment_map::compute_radiances to sample with sky_envmap_input and
// level the result with sky_envmap_level, and overrides ambient::ambient_light to return nothing.
//
// The diffuse map is Unity's Trilight ambient (sky_preset::ambient_trilight), a gradient over the
// surface normal written on the CPU, sampled with the normal as is and passed through: it is the
// only diffuse ambient light.
//
// The reflection map is the sky, of which only the upper hemisphere is generated. Directions
// below the horizon read their mirror image above it, standing in for the sky reflected back up
// off the ground, tinted towards the ground and blurred: see sky_envmap_ground.wgsl for the curve.
// Reflections are levelled towards a target: the flat ambient light, which the overrides zero
// out and pass in here instead.
// - above the target, the brightest channel is compressed towards the target's in log space,
//   keeping the hue: brightest * (target / brightest)^compression, scaled by roughness so
//   mirrors still match the drawn sky.
// - below it, the target is a floor: an even mix of the ambient light as a plain per-channel
//   minimum, and a lift of the brightest channel up to the target's, keeping the envmap's hue.
// The compression rides in the ambient light's alpha, negated (the uniform holds alpha *
// brightness; cameras with their own ambient light have a positive alpha, and no compression).
//
// Each also overrides environment_map::environment_map_light with sky_envmap_light, which
// combines the two the way Unity does.
//
// The overrides themselves stay in each top-level shader: naga_oil only applies an override
// declared there, and only the override can call bevy's original compute_radiances.

#import bevy_pbr::{clustered_forward::ClusterableObjectIndexRanges, environment_map, lighting}
#import "embedded://shaders/sky_envmap_ground.wgsl"::{sky_envmap_ground_shininess, sky_envmap_ground_tint}

// the input to sample the envmaps with in place of `input`: the normal as is for the ambient
// map; for the reflection map, bevy samples along R exactly at zero roughness, so R is set to
// where it would have sampled, mirrored up
fn sky_envmap_input(input: lighting::LayerLightingInput) -> lighting::LayerLightingInput {
    let direction = environment_map::radiance_sample_direction(input.N, input.R, input.roughness);
    return lighting::LayerLightingInput(
        input.N,
        sky_envmap_upper(direction),
        input.NdotV,
        sky_envmap_perceptual_roughness(direction, input.perceptual_roughness),
        0.0,
    );
}

// `radiances` as sampled with `upper` (from sky_envmap_input) in place of `input`; `ambient` is
// the lights uniform's ambient_color
fn sky_envmap_level(radiances: environment_map::EnvironmentMapRadiances, input: lighting::LayerLightingInput, upper: lighting::LayerLightingInput, ambient: vec4<f32>) -> environment_map::EnvironmentMapRadiances {
    let direction = environment_map::radiance_sample_direction(input.N, input.R, input.roughness);
    return environment_map::EnvironmentMapRadiances(
        radiances.irradiance,
        sky_envmap_ambient(radiances.radiance * sky_envmap_ground_tint(direction), ambient, upper.perceptual_roughness),
    );
}

fn sky_envmap_upper(direction: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(direction.x, abs(direction.y), direction.z);
}

fn sky_envmap_perceptual_roughness(direction: vec3<f32>, perceptual_roughness: f32) -> f32 {
    return 1.0 - (1.0 - perceptual_roughness) * sky_envmap_ground_shininess(direction);
}

const FLOOR_HUE_WEIGHT: f32 = 0.5;

// `ambient` is the lights uniform's ambient_color; `compression_weight` scales its compression
// (the perceptual roughness of the reflection)
fn sky_envmap_ambient(color: vec3<f32>, ambient: vec4<f32>, compression_weight: f32) -> vec3<f32> {
    let goal = ambient.rgb;
    let brightest = max(color.r, max(color.g, color.b));
    let goal_brightest = max(goal.r, max(goal.g, goal.b));

    var levelled = color;
    if brightest > goal_brightest && goal_brightest > 0.0 {
        let compression = saturate(-ambient.a) * compression_weight;
        levelled = color * pow(goal_brightest / brightest, compression);
    }

    let plain = max(levelled, goal);
    var hue = levelled;
    if brightest <= 0.0 {
        // no hue to keep
        hue = goal;
    } else if brightest < goal_brightest {
        hue = color * (goal_brightest / brightest);
    }
    return mix(plain, hue, FLOOR_HUE_WEIGHT);
}

// unity: one minus kDielectricSpec.a, the share of a dielectric's light that is diffuse
const UNITY_DIELECTRIC_DIFFUSE: f32 = 0.96;

// The environment light as Unity's scene shader combines it (Scene_GlobalIllumination.hlsl,
// GlobalIllumination and URP's EnvironmentBRDF): the ambient diffuse at full strength, with the
// reflection added on top. bevy's own split takes the reflection's share out of the diffuse,
// which on a smooth surface seen at a grazing angle is most of it: wherever the sun does not
// reach, the smooth patches of a floor's roughness map read as dark, or as wet with the sky.
// Unity's reflectivity is 1 - 0.96 (1 - metallic); bevy's input carries F0 instead, whose
// brightest channel is exactly that for a dielectric and close to it for a metal.
// `compute_radiances` here is the top-level shader's override.
fn sky_envmap_light(input: ptr<function, lighting::LightingInput>, clusterable_object_index_ranges: ptr<function, ClusterableObjectIndexRanges>, found_diffuse_indirect: bool) -> environment_map::EnvironmentMapLight {
    let layer = (*input).layers[lighting::LAYER_BASE];
    var out: environment_map::EnvironmentMapLight;

    let radiances = environment_map::compute_radiances(layer, clusterable_object_index_ranges, (*input).P, found_diffuse_indirect);
    if (all(radiances.irradiance == vec3(0.0)) && all(radiances.radiance == vec3(0.0))) {
        out.diffuse = vec3(0.0);
        out.specular = vec3(0.0);
        return out;
    }

    let F0 = (*input).F0_;
    let reflectivity = max(F0.r, max(F0.g, F0.b));
    let grazing = saturate(1.0 - layer.perceptual_roughness + reflectivity);
    let fresnel = pow(1.0 - saturate(layer.NdotV), 4.0);
    let surface_reduction = 1.0 / (layer.roughness * layer.roughness + 1.0);

    if (!found_diffuse_indirect) {
        out.diffuse = (*input).diffuse_color * UNITY_DIELECTRIC_DIFFUSE * radiances.irradiance;
    } else {
        out.diffuse = vec3(0.0);
    }
    out.specular = surface_reduction * mix(F0, vec3(grazing), fresnel) * radiances.radiance;

#ifdef STANDARD_MATERIAL_CLEARCOAT
    environment_map::environment_map_light_clearcoat(&out, input, clusterable_object_index_ranges, found_diffuse_indirect);
#endif

    return out;
}
