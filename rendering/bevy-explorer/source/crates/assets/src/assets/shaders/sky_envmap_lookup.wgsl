// Lookups into the sky environment maps (visuals/src/sky_envmap.rs), shared by the pbr material
// shaders. Each overrides environment_map::compute_radiances to level what it samples with
// sky_envmap_level, and overrides ambient::ambient_light to return nothing.
//
// The diffuse map is the Trilight ambient (sky_preset::ambient_trilight), a gradient over the
// surface normal written on the CPU, sampled with the normal as is and passed through: it is the
// only diffuse ambient light.
//
// The reflection map is the sky above the horizon and the ground below it (sky_envmap.wgsl).
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
// puts the reflection on top of the ambient diffuse at full strength.
//
// The overrides themselves stay in each top-level shader: naga_oil only applies an override
// declared there, and only the override can call bevy's original compute_radiances.

#import bevy_pbr::{clustered_forward::ClusterableObjectIndexRanges, environment_map, lighting, mesh_view_bindings::lights}

// `radiances` as sampled for `input`; `ambient` is the lights uniform's ambient_color
fn sky_envmap_level(radiances: environment_map::EnvironmentMapRadiances, input: lighting::LayerLightingInput, ambient: vec4<f32>) -> environment_map::EnvironmentMapRadiances {
    return environment_map::EnvironmentMapRadiances(
        radiances.irradiance,
        sky_envmap_ambient(radiances.radiance, ambient, input.perceptual_roughness),
    );
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

// the share of a dielectric's light that is diffuse: one minus its 4% reflectance
const DIELECTRIC_DIFFUSE: f32 = 0.96;

// The reflectivity of the fragment being lit, for a material with a metallic value: the 4% of a
// dielectric rising to 1 for a metal, whatever its colour (`sky_envmap_metallic_reflectivity`).
// A shader sets it before lighting; negative, the default, leaves the F0 reading above.
var<private> sky_envmap_reflectivity: f32 = -1.0;

fn sky_envmap_metallic_reflectivity(metallic: f32) -> f32 {
    return 1.0 - DIELECTRIC_DIFFUSE * (1.0 - metallic);
}

// The environment light: the ambient diffuse at full strength, with the reflection added on
// top. bevy's own split takes the reflection's share out of the diffuse, which on a smooth
// surface seen at a grazing angle is most of it: wherever the sun does not reach, the smooth
// patches of a floor's roughness map read as dark, or as wet with the sky.
// The reflectivity that widens the grazing term is `sky_envmap_reflectivity` where the shader
// sets it, else the brightest channel of F0: 0.04 for a dielectric, close to the albedo for a
// metal.
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
    let reflectivity = select(max(F0.r, max(F0.g, F0.b)), sky_envmap_reflectivity, sky_envmap_reflectivity >= 0.0);
    let grazing = saturate(1.0 - layer.perceptual_roughness + reflectivity);
    let fresnel = pow(1.0 - saturate(layer.NdotV), 4.0);
    let surface_reduction = 1.0 / (layer.roughness * layer.roughness + 1.0);

    if (!found_diffuse_indirect) {
        out.diffuse = (*input).diffuse_color * DIELECTRIC_DIFFUSE * radiances.irradiance;
    } else {
        out.diffuse = vec3(0.0);
    }
    out.specular = surface_reduction * mix(F0, vec3(grazing), fresnel) * radiances.radiance;

#ifdef STANDARD_MATERIAL_CLEARCOAT
    environment_map::environment_map_light_clearcoat(&out, input, clusterable_object_index_ranges, found_diffuse_indirect);
#endif

    return out;
}
