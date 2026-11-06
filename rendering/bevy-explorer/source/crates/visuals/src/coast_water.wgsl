// The sea: warped ripple slopes from two samples of one texture (ours: Ocean.mat has
// `_NormalStrength` 0), lit by the standard pipeline, with foam at the shore. Unity's Stylized
// Water 2 density, colours, refraction and surface foam, with the values of
// unity:Explorer/Assets/DCL/Landscape/Assets/Materials/Ocean.mat, from
// unity:Explorer/Library/PackageCache/decentraland.rendergraphs@*/Ocean/Shaders/:
// - Libraries/ForwardPass.hlsl: `GetWaterDensity`, the albedo, the surface foam and the final
//   blend with the scene;
// - Libraries/Features.hlsl: `RefractionOffset`, `ScreenEdgeMask` and `SampleOpaqueTexture`;
// - Libraries/Common.hlsl: `PackedUV` (the foam layers) and `TIME` (`_Direction`);
// - Libraries/Input.hlsl: `_FoamTiling` is a float2;
// - StylizedWater2_Standard.watershader: Ocean.mat's `_ADVANCED_SHADING` enables
//   `PHYSICAL_REFRACTION`.
// The depth below the surface is the coast profile's, not the depth buffer's.

#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::{globals, lights, view, view_transmission_texture, view_transmission_sampler},
    shadows,
}
#import "embedded://shaders/simplex.wgsl"::simplex_noise_3d
#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping::approximate_inverse_tone_mapping
#endif
#import "embedded://visuals/coast_surf.wgsl"::coast_surf
#import "embedded://visuals/coast_profile.wgsl"::coast_profile
#import "embedded://shaders/shadow_softening.wgsl"::soften_directional_shadow

// same softened sun shadows as scene materials (see bound_material.wgsl)
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

// coast_waves.wgsl's uniform
struct WaveUniform {
    land_bounds: vec4<f32>,
    fade: vec2<f32>,
    camera_xz: vec2<f32>,
    time: f32,
}

@group(2) @binding(100) var<uniform> wave: WaveUniform;
@group(2) @binding(101) var ripples: texture_2d<f32>;
@group(2) @binding(102) var ripple_sampler: sampler;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    let land_bounds = wave.land_bounds;
    var pbr = pbr_input_from_standard_material(in, is_front);
    // the material is transmissive only so that it draws after the opaque copy; the refraction
    // below replaces the standard transmission
    pbr.material.specular_transmission = 0.0;
    let p = in.world_position.xz;
    // long crossing swells carry smaller chop; the swell bends the chop's lookup so the two
    // layers don't read as rigid copies sliding over each other
    let swell_space = vec2(dot(p, vec2(0.939693, -0.342020)),
        dot(p, vec2(0.342020, 0.939693)));
    let swell = textureSample(ripples, ripple_sampler,
        swell_space * vec2(0.0073, 0.0091) + globals.time * vec2(0.007, -0.0045)).rg * 2.0 - 1.0;
    let chop_space = vec2(dot(p, vec2(0.529919, 0.848048)),
        dot(p, vec2(-0.848048, 0.529919)));
    let chop = textureSample(ripples, ripple_sampler,
        chop_space * vec2(0.02275, 0.02925) + swell * 0.72
        + globals.time * vec2(-0.005625, 0.0040625)).rg * 2.0 - 1.0;
    // rotate the sampled slopes back into world x/z
    let swell_slope = vec2(dot(swell, vec2(0.939693, 0.342020)),
        dot(swell, vec2(-0.342020, 0.939693)));
    let chop_slope = vec2(dot(chop, vec2(0.529919, -0.848048)),
        dot(chop, vec2(0.848048, 0.529919)));
    let chop_strength = clamp(0.16 + swell.x * 0.36 + swell.y * 0.22, 0.06, 0.28);
    let dx = dpdx(p);
    let dy = dpdy(p);
    let footprint = sqrt(max(dot(dx, dx), dot(dy, dy)));
    let unity = p * vec2(1.0, -1.0);
    let outside = max(max(land_bounds.xy - unity, unity - land_bounds.zw), vec2(0.0));
    // the waterline is at most 35 m out and the shore tint ends 18 m past it, so beyond 54 m
    // there is no foam or tint to compute
    var wash = vec4(0.0, 0.0, 0.0, 18.0);
    if dot(outside, outside) < 54.0 * 54.0 {
        wash = coast_surf(unity, land_bounds, globals.time, footprint);
    }
    // the sand below the waterline, 4 m down over 6 m in coast.rs, fading with depth
    let seabed = exp(-max(wash.w, 0.0) * (4.0 / 6.0) / 1.5);
    // calm the chop over the shallows, where the sand shows through
    let slope = swell_slope * 0.32 + chop_slope * chop_strength * (1.0 - seabed);
    pbr.N = normalize(vec3(-slope.x, 1.0, -slope.y));
    // tilt to the beach's slope at the waterline, so the water meets the sand's lighting: the
    // sand drops 1.7 m from coast.rs's shelf ring (ring 4) to the waterline
    let meet = 1.0 - smoothstep(0.0, 1.5, wash.w);
    if meet > 0.0 && wash.w > -3.0 {
        let edge = clamp(unity, land_bounds.xy, land_bounds.zw);
        let profile = coast_profile(edge);
        let shelf = profile.x + 7.5 + (profile.y - profile.x - 18.0) * 0.4;
        let beach = 1.7 / (profile.y - shelf);
        let o = normalize(unity - edge);
        let sand_normal = normalize(vec3(o.x * beach, 1.0, -o.y * beach));
        pbr.N = normalize(mix(pbr.N, sand_normal, meet));
    }
    // `GetWaterDensity` (ForwardPass.hlsl) with `_DepthHorizontal`, `_DepthVertical` and
    // `_DepthExp` (Ocean.mat). The vertical depth is the sand's, 4 m down over 6 m past the
    // waterline in coast.rs; the view depth is the eye-depth step from the surface to that sand
    // along the view ray
    let height_scalar = bitcast<f32>(#{WATER_DEPTH_HORIZONTAL}u);
    let view_depth_scalar = bitcast<f32>(#{WATER_DEPTH_VERTICAL}u);
    let exponential = bitcast<f32>(#{WATER_DEPTH_EXP}u);
    var vertical_depth = max(wash.w, 0.0) * (4.0 / 6.0);
    let forward = -view.world_from_view[2].xyz;
    let view_depth = vertical_depth / max(pbr.V.y, 0.001) * dot(-pbr.V, forward);
    // at grazing angles SW2 falls back to the view depth
    let grazing = pow(1.0 - saturate(dot(pbr.V, pbr.world_normal)), 64.0);
    vertical_depth = mix(vertical_depth, view_depth, grazing);
    let depth_attenuation = 1.0 - exp(-view_depth * view_depth_scalar * 0.1);
    let height_attenuation = saturate(mix(vertical_depth * height_scalar,
        1.0 - exp(-vertical_depth * height_scalar), exponential));
    let fog = max(depth_attenuation, height_attenuation);
    // `_ShallowColor` and `_BaseColor`, linear; both alphas are 1
    let shallow = vec3(0.1571, 0.8553, 1.1909);
    let base = vec3(0.0079, 0.0060, 0.0454);
    let albedo = mix(shallow, base, fog);
    // `RefractionOffset` with `PHYSICAL_REFRACTION` (Features.hlsl): the view ray refracted into
    // water, in view space, eased to the flat offset at the screen edges, times
    // `_RefractionStrength`. It is added to the screen position before the divide by w, so it
    // shrinks with distance. Unity's screen y points up, the texture's v down
    let clip = view.clip_from_world * vec4(in.world_position.xyz, 1.0);
    let uv = (in.position.xy - view.viewport.xy) / view.viewport.zw;
    let ray = refract(-pbr.V, pbr.N, 0.7501875);
    let view_refraction = (view.view_from_world * vec4(ray, 0.0)).xyz;
    let edge_length = length(view_refraction.xy);
    let edge = saturate((1.0 - abs(uv * 2.0 - 1.0)) / edge_length);
    let edge_mask = edge.x * edge.x * (3.0 - 2.0 * edge.x) * edge.y * edge.y * (3.0 - 2.0 * edge.y);
    let flat_offset = vec2(pbr.N.x, -pbr.N.z) * 0.5;
    let offset = mix(flat_offset, view_refraction.xy, edge_mask)
        * bitcast<f32>(#{REFRACTION_STRENGTH}u);
    let scene_uv = uv + vec2(offset.x, -offset.y) / clip.w;
    // `SampleOpaqueTexture`'s chromatic part: red and blue shift sideways by
    // `length(offset) * _RefractionChromaticAberration / w * CHROMASHIFT_SIZE`
    let shift = vec2(length(offset) * bitcast<f32>(#{REFRACTION_CHROMATIC}u) / clip.w * 0.05,
        0.0);
    let scene = vec3(
        background(scene_uv + shift).r,
        background(scene_uv).g,
        background(scene_uv - shift).b,
    );
    // surface foam: two scrolling layers (`PackedUV`) at `_FoamTiling` 0.01 and `_FoamSubTiling`
    // 0.5, moving at `_FoamSpeed` 0.15 and `_FoamSubSpeed` -0.25 along `TIME`, which runs against
    // `_Direction`; simplex noise stands in for SW2's foam texture
    let flow = -vec2(bitcast<f32>(#{WAVE_DIRECTION_X}u), bitcast<f32>(#{WAVE_DIRECTION_Y}u))
        * globals.time;
    let foam_uv = unity * 0.01 + flow * 0.15 * 0.01;
    let foam_sub_uv = unity * 0.005 + flow * (0.15 * -0.25 * 0.005);
    let foam_texture = saturate(saturate(simplex_noise_3d(vec3(foam_uv * 8.0, 0.0)))
        + saturate(simplex_noise_3d(vec3(foam_sub_uv * 8.0, 7.0))));
    // `_FoamClipping` 0.75 and `_FoamBaseAmount` 0.109; `_FoamWaveAmount` is 0, so wave height
    // adds none
    let foam_mask = 1.0 - 0.109;
    let sea_foam = smoothstep(foam_mask, foam_mask + 1.0, smoothstep(0.75, 1.0, foam_texture));
    // the shore wash is ours, on top of SW2's foam
    let foam = max(wash.x, sea_foam);
    let film = mix(albedo, vec3(0.09, 0.15, 0.19), wash.y * 0.35);
    // SW2 whitens the albedo by twice the foam
    let whitening = saturate(max(wash.x, sea_foam * 2.0));
    pbr.material.base_color = vec4(mix(film, vec3(0.72, 0.78, 0.78), whitening), 1.0);
    pbr.material.perceptual_roughness = mix(0.28, 0.62, whitening);
    // the lit surface over the refracted scene by density and foam (ForwardPass.hlsl's final
    // blend; there is no intersection term)
    var color = apply_pbr_lighting(pbr);
    color = vec4(mix(scene, color.rgb, saturate(fog + foam)), color.a);
    // the flat ocean plane lies under the wave grid that follows the camera, and the sea writes
    // no depth (WaterExtension::specialize), so the plane gives way wherever the grid covers it;
    // the waves never reach it (coast/tests.rs), and the check sits after every derivative
    let from_grid = abs(in.world_position.xz - wave.camera_xz);
    if in.world_position.y < bitcast<f32>(#{OCEAN_PLANE_TOP}u)
        && max(from_grid.x, from_grid.y) < bitcast<f32>(#{OCEAN_PLANE_HOLE}u) {
        discard;
    }
    var out: FragmentOutput;
    out.color = main_pass_post_lighting_processing(pbr, color);
    return out;
}

// the opaque scene behind the water, in the lighting's units
fn background(uv: vec2<f32>) -> vec3<f32> {
    var color = textureSampleLevel(view_transmission_texture, view_transmission_sampler, uv, 0.0);
#ifdef TONEMAP_IN_SHADER
    color = approximate_inverse_tone_mapping(color, view.color_grading);
#endif
    return color.rgb;
}
