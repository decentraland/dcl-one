// Standard lighting with the softened sun shadows of the ground, grass and scene materials. With
// LANDSCAPE_STONE, the base colour first takes the ground's colour near the ground and moss on the
// faces turned up.
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::lights,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    shadows,
}
#import "embedded://shaders/shadow_softening.wgsl"::soften_directional_shadow

override fn shadows::fetch_directional_shadow(light_id: u32, frag_position: vec4<f32>, surface_normal: vec3<f32>, view_z: f32) -> f32 {
    let base = shadows::fetch_directional_shadow(light_id, frag_position, surface_normal, view_z);
    // far bound of the last cascade = the shadow distance for this light
    let light = &lights.directional_lights[light_id];
    let far = (*light).cascades[max((*light).num_cascades, 1u) - 1u].far_bound;
    return soften_directional_shadow(base, far, view_z);
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

#ifdef LANDSCAPE_STONE
// moss colour, linear, in rgb
@group(2) @binding(100) var<uniform> stone_moss: vec4<f32>;
// moss mask, terrain texture tiling and offset
@group(2) @binding(101) var<uniform> stone_terrain: vec4<f32>;
@group(2) @binding(102) var terrain_texture: texture_2d<f32>;
@group(2) @binding(103) var terrain_sampler: sampler;

// The terrain texture's coordinates before its tiling, per metre of world.
const TERRAIN_UV_PER_METRE: f32 = 0.1;

// Where the terrain texture lies over the world: u along +x and v along bevy +z, which is the
// reference's layout with its z negated and its v flipped (its v runs up the image, bevy's down).
fn terrain_uv(world: vec3<f32>) -> vec2<f32> {
    let tiling = stone_terrain.y;
    let offset = stone_terrain.z;
    return world.xz * (TERRAIN_UV_PER_METRE * tiling) + vec2(offset, 1.0 - offset);
}

// The stone's own colour over the ground's below world height 0.12 m (all ground from -0.04 m),
// then moss over that. `up` is the height of the surface normal before its normal map: a face
// turned up past the mask is all moss, and the moss thins over steeper faces. World height 0 is the
// scenes' ground level, so the stones on the flat land by them stand in the ground's colour; on
// the hills, above it, they keep their own.
fn stone_color(rock: vec3<f32>, ground: vec3<f32>, world_y: f32, up: f32) -> vec3<f32> {
    let foot = mix(ground, rock, smoothstep(-0.04, 0.12, world_y));
    // up is at most 1, so the edges stay ordered
    let bare = smoothstep(up, 1.12, stone_terrain.x);
    return mix(stone_moss.rgb, foot, bare);
}
#endif

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
#ifdef LANDSCAPE_STONE
    // sampled before any branch, for its implicit derivatives
    let ground_uv = terrain_uv(in.world_position.xyz);
    let ground = textureSample(terrain_texture, terrain_sampler, ground_uv).rgb;
#endif
    var pbr_input = pbr_input_from_standard_material(in, is_front);
#ifdef LANDSCAPE_STONE
    let base = pbr_input.material.base_color;
    pbr_input.material.base_color = vec4(
        stone_color(base.rgb, ground, in.world_position.y, pbr_input.world_normal.y),
        base.a,
    );
#endif
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
    var out: FragmentOutput;
    out.color = main_pass_post_lighting_processing(pbr_input, apply_pbr_lighting(pbr_input));
    return out;
}
