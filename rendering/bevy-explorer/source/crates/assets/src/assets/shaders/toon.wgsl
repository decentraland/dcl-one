// Toon (cel) shading for avatars, in this client's light units.
//
// - the sun (the brightest directional light) gives a half-Lambert, scaled down to half in its
//   cast shadow, that picks the base colour or one of two grey shades in two feathered steps;
// - the light colour is the sun in light units, saturated, and floored by the ambient light
//   times a gain, so avatars dim at night but stay visible. The ambient light is the sky
//   environment map's where the view has it, and the view's flat ambient light where it has not,
//   so a camera lit flat lights avatars flat;
// - a white rim light on the silhouette, in the same light colour.
// The eyes, eyebrows and mouth are lit apart (facial_feature_lighting): by the ambient light
// alone, times a gain, with no sun, no bands and no saturation.
// No point or spot light reaches the avatar (decisions.md).
//
// Two values are derived in Rust and carried as defs by scene_material::avatar_style_shader_defs,
// each float as its bit pattern read with bitcast<f32>: TOON_LIGHT_UNIT_LUX (the light unit,
// common::sky_preset::LIGHT_UNIT_LUX) and TOON_FACIAL_FLAT_AMBIENT_R/G/B.
// Avatars still CAST shadows (that is the shadow pass's job, not this shading).

#import bevy_pbr::{
    pbr_types::PbrInput,
    mesh_view_bindings::{lights, view},
    shadows::fetch_directional_shadow,
    mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT,
    mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT,
}
#import bevy_render::maths::PI

#ifdef ENVIRONMENT_MAP
#import bevy_pbr::{clustered_forward, clustered_forward::ClusterableObjectIndexRanges, environment_map, lighting}
#endif

// undoes bound_material.wgsl's x10 lift: an avatar material's emissive is already its emission
// in output units, where lit white is 1 (avatar::avatar_emission)
const EMISSIVE_SCALE: f32 = 0.1;

const LUMA: vec3<f32> = vec3<f32>(0.2126, 0.7152, 0.0722);

// the half-Lambert below which the base colour gives way to the shades
const BASE_COLOR_STEP: f32 = 0.2;
// the width of the base-to-shade transition below BASE_COLOR_STEP
const BASE_SHADE_FEATHER: f32 = 0.02;
// the half-Lambert below which the first shade gives way to the second
const SHADE_COLOR_STEP: f32 = 0.2;
// the width of the first-to-second shade transition below SHADE_COLOR_STEP
const SHADES_FEATHER: f32 = 0.05;
// the grey the lit base colour is multiplied by in the first shade
const FIRST_SHADE: f32 = 0.9490197;
// the grey of the second, darker shade
const SECOND_SHADE: f32 = 0.8000001;
// the gain on the ambient light that floors the light colour
const UNLIT_INTENSITY: f32 = 4.0;
// the least ambient light the floor takes, before UNLIT_INTENSITY: with no light at all an
// avatar is lit at a fifth of noon
const UNLIT_FLOOR: f32 = 0.05;
// the gain on the ambient light that lights the eyes, eyebrows and mouth
const FACIAL_AMBIENT_GAIN: f32 = 4.0;
// the rim light's width: the rim is the silhouette term to the power 2^(3 - 3 * RIM_POWER), so 0
// keeps it at the very edge and 1 spreads it over the body
const RIM_POWER: f32 = 0.3;
// the silhouette term below which there is no rim light; above it the rim rises to 1
const RIM_INSIDE_MASK: f32 = 0.15;
// the rim light's strength, as saturate(1 + RIM_MASK_LEVEL): a tenth
const RIM_MASK_LEVEL: f32 = -0.9;

// the shadow mask (1 in the shades) and the shades term (the first shade blending into the
// second) for a half-Lambert from n_dot_l and the sun's cast-shadow term (1 lit). A full shadow
// halves the half-Lambert the mask reads, so only surfaces already turning away from the sun fall
// into the shades; the shades blend on the unshadowed half-Lambert
fn toon_mask_and_shades(n_dot_l: f32, shadow: f32) -> vec2<f32> {
    let base_step = BASE_COLOR_STEP;
    let base_feather = BASE_SHADE_FEATHER;
    let shade_step = SHADE_COLOR_STEP;
    let shades_feather = SHADES_FEATHER;

    let half_lambert = 0.5 * n_dot_l + 0.5;
    let shadowed = half_lambert * saturate(shadow * 0.5 + 0.5);
    let mask = saturate(1.0 - (shadowed - (base_step - base_feather)) / base_feather);
    let second = saturate(1.0 - (half_lambert - (shade_step - shades_feather)) / shades_feather);
    let shades = mix(FIRST_SHADE, SECOND_SHADE, second);
    return vec2(mask, shades);
}

// the lit multiplier on the base colour under the light colour: the shade colours are taken from
// the already lit base colour and lit again, so the shades darken faster than the light dims
fn toon_diffuse(n_dot_l: f32, shadow: f32, light: vec3<f32>) -> vec3<f32> {
    let mask_and_shades = toon_mask_and_shades(n_dot_l, shadow);
    return light * mix(vec3(1.0), light * mask_and_shades.y, mask_and_shades.x);
}

// the light colour: the sun in light units, saturated so a bright sun never lifts avatars past
// white, and floored by the ambient light times a gain and by a minimum so they stay visible
fn toon_light(sun_lux: vec3<f32>, ambient: vec3<f32>) -> vec3<f32> {
    let unlit = UNLIT_INTENSITY;
    let unlit_floor = UNLIT_FLOOR;
    let default_light = saturate(max(ambient, vec3(unlit_floor)) * unlit);
    return max(default_light, saturate(sun_lux / bitcast<f32>(#{TOON_LIGHT_UNIT_LUX}u)));
}

// the ambient light that floors the light colour, from the diffuse irradiance of a white surface
// facing up and down: the mean irradiance over every direction, approximated by the mean of the
// two, or the light from below where that is brighter
fn toon_ambient(up: vec3<f32>, down: vec3<f32>) -> vec3<f32> {
    return max(0.5 * (up + down), down);
}

// a view's flat ambient light (the lights uniform's ambient_color: colour times brightness) in
// light units. The pbr path lights a white surface by the flat ambient at that value, and by a
// sun facing it at its illuminance over pi, so one light unit of flat ambient is the unit's lux
// over pi
fn toon_flat_ambient(ambient_color: vec3<f32>) -> vec3<f32> {
    return max(ambient_color, vec3(0.0)) * PI / bitcast<f32>(#{TOON_LIGHT_UNIT_LUX}u);
}

#ifdef ENVIRONMENT_MAP
// the diffuse irradiance the pbr path's environment map lookup gives a surface facing `normal`
// at `world_position`: the sky envmap's Trilight ambient, through the top-level shader's
// compute_radiances override (sky_envmap_lookup.wgsl). Only its irradiance is read, so the
// reflection sample is dead code
fn toon_irradiance(normal: vec3<f32>, world_position: vec3<f32>, clusterable_object_index_ranges: ptr<function, ClusterableObjectIndexRanges>) -> vec3<f32> {
    let input = lighting::LayerLightingInput(normal, normal, 1.0, 1.0, 1.0);
    return environment_map::compute_radiances(input, clusterable_object_index_ranges, world_position, false).irradiance;
}
#endif

// the emission, in output units (see toon_lighting)
fn toon_emission(pbr_input: PbrInput) -> vec3<f32> {
    // emissive alpha is the exposure weight, as in bevy_pbr: bound_material
    // writes 0, so the emission is already in output units and must not be
    // scaled down by the view exposure (~0.001) like the lit term.
    let emissive_exposure = mix(1.0, view.exposure, pbr_input.material.emissive.a);
    return pbr_input.material.emissive.rgb * emissive_exposure * EMISSIVE_SCALE;
}

// the eyes, eyebrows and mouth: the base colour under the ambient light a surface facing the
// fragment's normal receives, in light units, times a gain. No sun, no bands and no saturation,
// so a face feature is as bright as the ambient makes it: brighter than the body under a bright
// sky, darker at night. The ambient is the sky environment map's irradiance, as toon_lighting
// reads it, where the view has it
fn facial_feature_lighting(pbr_input: PbrInput) -> vec4<f32> {
    let base_color = pbr_input.material.base_color;
    let unit = bitcast<f32>(#{TOON_LIGHT_UNIT_LUX}u) * view.exposure / PI;
#ifdef ENVIRONMENT_MAP
    let world_position = pbr_input.world_position;
    let view_z = dot(vec4<f32>(
        view.view_from_world[0].z,
        view.view_from_world[1].z,
        view.view_from_world[2].z,
        view.view_from_world[3].z
    ), world_position);
    let cluster_index = clustered_forward::fragment_cluster_index(pbr_input.frag_coord.xy, view_z, pbr_input.is_orthographic);
    var clusterable_object_index_ranges = clustered_forward::unpack_clusterable_object_index_ranges(cluster_index);
    let ambient = toon_irradiance(pbr_input.N, world_position.xyz, &clusterable_object_index_ranges) * view.exposure / unit;
#else
    // a view without the sky: its flat ambient is set to light the body flat, far above any sky,
    // so the face features take the noon sky's horizon instead
    let ambient = vec3(
        bitcast<f32>(#{TOON_FACIAL_FLAT_AMBIENT_R}u),
        bitcast<f32>(#{TOON_FACIAL_FLAT_AMBIENT_G}u),
        bitcast<f32>(#{TOON_FACIAL_FLAT_AMBIENT_B}u),
    );
#endif
    let light = max(ambient, vec3(0.0)) * FACIAL_AMBIENT_GAIN;
    let color = base_color.rgb * light * unit + toon_emission(pbr_input);
    return vec4<f32>(color, base_color.a);
}

fn toon_lighting(pbr_input: PbrInput) -> vec4<f32> {
    let base_color = pbr_input.material.base_color;
    // the bands take the vertex normal, so a normal map does not break up the cel steps, and the
    // rim the normal-mapped one
    let vertex_n = pbr_input.world_normal;
    let n = pbr_input.N;
    let v = pbr_input.V;
    let world_position = pbr_input.world_position;
    // a light colour of 1 in this client's pre-tonemap units: a white pbr surface facing a sun of
    // one light unit
    let unit = bitcast<f32>(#{TOON_LIGHT_UNIT_LUX}u) * view.exposure / PI;

    let view_z = dot(vec4<f32>(
        view.view_from_world[0].z,
        view.view_from_world[1].z,
        view.view_from_world[2].z,
        view.view_from_world[3].z
    ), world_position);

    // the sun is the brightest directional light. A black light keeps its direction for the
    // bands, and with no light at all they are lit from between the camera's up and back with no
    // colour, leaving the ambient floor
    var sun_lux = vec3(0.0);
    var sun_luma: f32 = 0.0;
    var sun = 0u;
    var l = normalize(view.world_from_view[1].xyz + view.world_from_view[2].xyz);
    let n_lights = min(lights.n_directional_lights, 4u);
    if n_lights > 0u {
        l = lights.directional_lights[0].direction_to_light;
    }
    for (var i = 0u; i < n_lights; i += 1u) {
        let light = &lights.directional_lights[i];
        let luma = dot((*light).color.rgb, LUMA);
        if luma > sun_luma {
            sun_luma = luma;
            sun = i;
            sun_lux = (*light).color.rgb;
            l = (*light).direction_to_light;
        }
    }

    var shadow: f32 = 1.0;
    if sun_luma > 0.0
        && (lights.directional_lights[sun].flags & DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u
        && (pbr_input.flags & MESH_FLAGS_SHADOW_RECEIVER_BIT) != 0u {
        shadow = fetch_directional_shadow(sun, world_position, vertex_n, view_z);
    }

    // the ambient light in light units. With the sky environment map it is the diffuse
    // irradiance the pbr path's envmap lookup gives a white surface facing up and down here,
    // before the dielectric factor (0.96) the world's diffuse takes: avatars take the whole
    // irradiance (decisions.md). A scene's ambient override and the ambient brightness setting
    // reach avatars as they reach the world; the flat ambient light is not read, being there
    // only the level the sky reflections are drawn towards. Without the environment map the
    // flat ambient light is the view's ambient, as it is for the world, so a camera lit flat
    // (the avatar preview) lights avatars flat
#ifdef ENVIRONMENT_MAP
    let cluster_index = clustered_forward::fragment_cluster_index(pbr_input.frag_coord.xy, view_z, pbr_input.is_orthographic);
    var clusterable_object_index_ranges = clustered_forward::unpack_clusterable_object_index_ranges(cluster_index);
    let up = toon_irradiance(vec3(0.0, 1.0, 0.0), world_position.xyz, &clusterable_object_index_ranges);
    let down = toon_irradiance(vec3(0.0, -1.0, 0.0), world_position.xyz, &clusterable_object_index_ranges);
    let ambient = toon_ambient(up * view.exposure / unit, down * view.exposure / unit);
#else
    let ambient = toon_flat_ambient(lights.ambient_color.rgb);
#endif
    let light = toon_light(sun_lux, ambient);

    let diffuse = toon_diffuse(dot(vertex_n, l), shadow, light);

    // a white rim light on the silhouette, where the surface turns away from the view, in the
    // light colour
    let rim_inside = RIM_INSIDE_MASK;
    let rim_area = abs(1.0 - dot(n, v));
    let rim_power = pow(rim_area, exp2(mix(3.0, 0.0, RIM_POWER)));
    let rim = saturate((rim_power - rim_inside) / (1.0 - rim_inside))
        * saturate(1.0 + RIM_MASK_LEVEL);

    // the rim adds on top of the lit colours
    var color = (base_color.rgb * diffuse + rim * light) * unit;
    color += toon_emission(pbr_input);

    return vec4<f32>(color, base_color.a);
}
