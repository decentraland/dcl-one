// The empty-parcel terrain height under a point, for the shaders that stand on the terrain: the
// ground's vertices and the grass. It matches `common::terrain::height`, reading eased per-parcel
// step values from `terrain_steps`.

#import "embedded://visuals/terrain_params.wgsl"::terrain

@group(2) @binding(100) var terrain_steps: texture_2d<f32>;

const STEP_HEIGHT: f32 = 1.8;
const TEXEL_STEP: f32 = 10.0;
// Past the hill reach the hills flatten over this share of it, so no vertex drops abruptly as the
// player walks.
const REACH_FADE: f32 = 0.1;

fn modulo289(value: f32) -> f32 {
    return value - floor(value * (1.0 / 289.0)) * 289.0;
}

fn permute(value: f32) -> f32 {
    return modulo289((34.0 * value + 1.0) * value);
}

fn simplex_corner(i: vec2<f32>, offset: vec2<f32>, corner: vec2<f32>) -> f32 {
    let p = permute(permute(i.y + offset.y) + i.x + offset.x);
    var m = max(0.5 - dot(corner, corner), 0.0);
    m *= m;
    m *= m;
    let fractional = p * 0.024390243 - floor(p * 0.024390243);
    let x = 2.0 * fractional - 1.0;
    let h = abs(x) - 0.5;
    let a = x - floor(x + 0.5);
    m *= 1.7928429 - 0.85373473 * (a * a + h * h);
    return m * (a * corner.x + h * corner.y);
}

fn simplex(v: vec2<f32>) -> f32 {
    let c = vec4<f32>(0.21132487, 0.36602542, -0.57735026, 0.024390243);
    let i = floor(v + dot(v, c.yy));
    let x0 = v - i + dot(i, c.xx);
    var i1 = vec2<f32>(0.0, 1.0);
    if x0.x > x0.y {
        i1 = vec2<f32>(1.0, 0.0);
    }
    let x1 = x0 + c.xx - i1;
    let x2 = x0 + c.zz;
    let im = vec2<f32>(modulo289(i.x), modulo289(i.y));
    return 130.0 * (
        simplex_corner(im, vec2<f32>(0.0), x0)
        + simplex_corner(im, i1, x1)
        + simplex_corner(im, vec2<f32>(1.0), x2)
    );
}

fn mountain_noise(p: vec2<f32>) -> f32 {
    let offsets = array<vec2<f32>, 3>(
        vec2<f32>(-99974.82, -93748.33),
        vec2<f32>(-67502.3, -22190.19),
        vec2<f32>(77881.34, -61863.88),
    );
    var amplitude = 1.0;
    var frequency = 1.0;
    var height = 0.0;
    for (var octave = 0; octave < 3; octave++) {
        height += simplex((p + offsets[octave]) * 0.02 * frequency) * amplitude;
        amplitude *= 0.338;
        frequency *= 2.9;
    }
    return height * 3.0;
}

fn terrain_texel(parcel: vec2<i32>) -> f32 {
    let local = parcel - terrain.min;
    if any(local < vec2<i32>(0)) || any(local >= terrain.size) {
        return 0.0;
    }
    let steps = textureLoad(terrain_steps, local, 0).r;
    if steps < 0.0 {
        return 0.0;
    }
    return 255.0 - TEXEL_STEP * terrain.max_steps + TEXEL_STEP * steps;
}

// 1 within the hill reach of the player (square, like the ground rings), falling to 0 over
// REACH_FADE of it beyond (x and unity z)
fn hill_fade(p: vec2<f32>) -> f32 {
    let player = (vec2<f32>(terrain.player_parcel) + terrain.player_offset) * 16.0;
    let offset = abs(p - player);
    let reach = terrain.hill_reach;
    return 1.0 - smoothstep(reach, reach * (1.0 + REACH_FADE), max(offset.x, offset.y));
}

// x and unity z (= -bevy z). Within the hill reach, where the collider and the player stand, this
// is `common::terrain::height`.
fn terrain_height_unity(p: vec2<f32>) -> f32 {
    let fade = hill_fade(p);
    if terrain.max_steps <= 0.0 || fade <= 0.0 {
        return 0.0;
    }
    let uv_size = terrain.uv_size;
    let half = i32(uv_size * 0.5);
    let last = vec2<i32>(i32(uv_size) - 1);
    let pixel = ((p / 16.0 + uv_size * 0.5) / uv_size) * uv_size - 0.5;
    // unity's texture clamps to its edge
    let base0 = clamp(vec2<i32>(floor(pixel)), vec2<i32>(0), last) - half;
    let base1 = clamp(vec2<i32>(floor(pixel)) + 1, vec2<i32>(0), last) - half;
    let fraction = pixel - floor(pixel);
    let top = mix(
        terrain_texel(base0),
        terrain_texel(vec2<i32>(base1.x, base0.y)),
        fraction.x,
    );
    let bottom = mix(
        terrain_texel(vec2<i32>(base0.x, base1.y)),
        terrain_texel(base1),
        fraction.x,
    );
    let occupancy = mix(top, bottom, fraction.y) * (1.0 / 255.0);
    let floor_value = (255.0 - TEXEL_STEP * terrain.max_steps) / 255.0;
    if occupancy <= floor_value {
        return 0.0;
    }
    let normalized = (occupancy - floor_value) / (1.0 - floor_value);
    return max(
        normalized * (terrain.max_steps * STEP_HEIGHT)
            + mountain_noise(p) * clamp(normalized * 20.0, 0.0, 1.0),
        0.0,
    ) * fade;
}

// bevy world x/z
fn terrain_height(xz: vec2<f32>) -> f32 {
    return terrain_height_unity(vec2<f32>(xz.x, -xz.y));
}
