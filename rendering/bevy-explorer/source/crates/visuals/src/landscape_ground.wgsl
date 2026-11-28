// The ground's colour: a splat map tiled over the world
// weights a ground texture (red) against a sand texture (green). The weights are used as stored,
// not normalised; the splat's blue and alpha channels are empty.

#ifdef GROUND_SPLAT_METRES

@group(2) @binding(102) var ground_splat: texture_2d<f32>;
@group(2) @binding(103) var ground_splat_sampler: sampler;
@group(2) @binding(104) var ground_colour: texture_2d<f32>;
@group(2) @binding(105) var ground_colour_sampler: sampler;
@group(2) @binding(106) var sand_colour: texture_2d<f32>;
@group(2) @binding(107) var sand_colour_sampler: sampler;

// the ground and sand textures repeat this many times over an 8192 m span
const DETAIL_SPAN: f32 = 8192.0;
const GROUND_TILING: f32 = 2048.0;
const SAND_TILING: f32 = 4096.0;

// `world_xz` is the bevy world position's xz. Every texture is laid with u along +x and v along
// bevy +z: with unity's z negated and its v flipped (unity's v runs up the image, bevy's down),
// that puts each texel on the same spot of the world as the reference. Takes implicit
// derivatives, so call it in uniform control flow, before any discard or branch. Returns the
// linear base colour; the ground has no detail normal, it is lit by the terrain's normal alone.
fn ground_albedo(world_xz: vec2<f32>) -> vec3<f32> {
    let splat = textureSample(
        ground_splat,
        ground_splat_sampler,
        world_xz / f32(#{GROUND_SPLAT_METRES}),
    ).rg;
    let ground = textureSample(
        ground_colour,
        ground_colour_sampler,
        world_xz * (GROUND_TILING / DETAIL_SPAN),
    ).rgb;
    let sand = textureSample(
        sand_colour,
        sand_colour_sampler,
        world_xz * (SAND_TILING / DETAIL_SPAN),
    ).rgb;
    return ground * splat.r + sand * splat.g;
}

// The colour of the grass, flower or reed rooted at `world_xz`: the
// ground and sand colours under the splat's weights, with all three textures laid at the splat's
// tiling and read at their full level. Stretched that far the colours change over metres, so the
// instances of a patch share its colour. Takes no derivatives.
fn scatter_colour(world_xz: vec2<f32>) -> vec3<f32> {
    let uv = world_xz / f32(#{GROUND_SPLAT_METRES});
    let splat = textureSampleLevel(ground_splat, ground_splat_sampler, uv, 0.0).rg;
    let ground = textureSampleLevel(ground_colour, ground_colour_sampler, uv, 0.0).rgb;
    let sand = textureSampleLevel(sand_colour, sand_colour_sampler, uv, 0.0).rgb;
    return ground * splat.r + sand * splat.g;
}

#endif
