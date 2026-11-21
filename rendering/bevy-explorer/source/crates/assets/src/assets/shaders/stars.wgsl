// Unity's Stars v2 and shooting stars (`SkyLut.hlsl`), added at screen resolution: after depth of
// field where the view has it, else before the transparent pass (see stars.rs). Each pixel of
// clear sky looks up the star cells around its view direction on a cube grid, so a star keeps
// one angular size everywhere. Unity draws the stars inside its sky shader, under the clouds;
// here the sky cube's alpha, its cloud cover, fades them. Unity adds them before its sky's last
// step decodes sRGB to linear, so each adds what that decode makes of it over the sky behind
// it. The output is blended additively, so pixels with no star are discarded.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import bevy_render::view::View

// Unity's `_DclStarsParams`, `_DclStarsParams2` and `_DclStarsParams3`, and its time
struct Stars {
    // brightness at the current day phase
    brightness: f32,
    twinkle: f32,
    patch_strength: f32,
    // shooting-star rate at the current day phase
    shooting_rate: f32,
    // cells along each edge of a cube face
    cells: f32,
    // star radius in radians
    radius: f32,
    // turns per second about the zenith
    rotation_speed: f32,
    twinkle_speed: f32,
    // the heights (the direction's y) the fade to the horizon starts and ends at
    horizon_fade: vec2<f32>,
    // seconds
    time: f32,
}

@group(0) @binding(0) var<uniform> view: View;
#ifdef MULTISAMPLED
@group(0) @binding(1) var depth_texture: texture_depth_multisampled_2d;
#else
@group(0) @binding(1) var depth_texture: texture_depth_2d;
#endif
// the sky cubemap: alpha is cloud cover
@group(0) @binding(2) var sky_texture: texture_cube<f32>;
@group(0) @binding(3) var sky_sampler: sampler;
@group(0) @binding(4) var<uniform> stars: Stars;
// the sky's horizon noise, reused for the dim patches
@group(0) @binding(5) var noise_texture: texture_2d<f32>;
@group(0) @binding(6) var noise_sampler: sampler;
#ifdef FOCUS
// depth of field's transparent coverage (g): imposters and other transparents
#ifdef MULTISAMPLED
@group(0) @binding(7) var focus_texture: texture_multisampled_2d<f32>;
#else
@group(0) @binding(7) var focus_texture: texture_2d<f32>;
#endif
#endif

const PI: f32 = 3.14159265;

// the shooting stars' cells over the sky (azimuth, elevation), the seconds between a cell's
// rolls and the share of them a streak lives for
const SHOOTING_GRID: vec2<f32> = vec2<f32>(12.0, 6.0);
const SHOOTING_PERIOD: f32 = 25.0;
const STREAK_LIFE: f32 = 1.2 / SHOOTING_PERIOD;

// Shader Graph's Colorspace Conversion node, RGB to Linear, which ends Unity's sky: the sRGB
// curve, continued past 1 on its power branch. As in stylized_sky.wgsl
fn rgb_to_linear(colour: vec3<f32>) -> vec3<f32> {
    let low = colour / 12.92;
    let high = pow(max(abs((colour + 0.055) / 1.055), vec3<f32>(1.192092896e-07)), vec3<f32>(2.4));
    return select(high, low, colour <= vec3<f32>(0.04045));
}

// Its inverse, for the sky cube, which holds the decoded sky
fn linear_to_rgb(colour: vec3<f32>) -> vec3<f32> {
    let low = colour * 12.92;
    let high = 1.055 * pow(colour, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, colour <= vec3<f32>(0.0031308));
}

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn hash2(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(hash(p), hash(p + 19.19));
}

// One hashed star per occupied cell of the cube face `d` falls on, checked over the 3x3
// neighbourhood so a star near a cell edge is whole.
fn star_field(d: vec3<f32>, time: f32) -> f32 {
    let cells = max(stars.cells, 1.0);
    let radius = max(stars.radius, 1e-4);

    // the face, the direction's place on it and the face's axes, which turn a cell back into a
    // direction
    let a = abs(d);
    var face_uv: vec2<f32>;
    var face_index: f32;
    var axis_u: vec3<f32>;
    var axis_v: vec3<f32>;
    var axis_n: vec3<f32>;
    if a.y >= a.x && a.y >= a.z {
        face_index = select(1.0, 0.0, d.y > 0.0);
        axis_n = vec3<f32>(0.0, sign(d.y), 0.0);
        axis_u = vec3<f32>(1.0, 0.0, 0.0);
        axis_v = vec3<f32>(0.0, 0.0, 1.0);
        face_uv = d.xz / a.y;
    } else if a.x >= a.z {
        face_index = select(3.0, 2.0, d.x > 0.0);
        axis_n = vec3<f32>(sign(d.x), 0.0, 0.0);
        axis_u = vec3<f32>(0.0, 0.0, 1.0);
        axis_v = vec3<f32>(0.0, 1.0, 0.0);
        face_uv = d.zy / a.x;
    } else {
        face_index = select(5.0, 4.0, d.z > 0.0);
        axis_n = vec3<f32>(0.0, 0.0, sign(d.z));
        axis_u = vec3<f32>(1.0, 0.0, 0.0);
        axis_v = vec3<f32>(0.0, 1.0, 0.0);
        face_uv = d.xy / a.z;
    }

    let base_cell = floor((face_uv * 0.5 + 0.5) * cells);
    var sum = 0.0;
    for (var j = -1.0; j <= 1.0; j += 1.0) {
        for (var i = -1.0; i <= 1.0; i += 1.0) {
            let cell = base_cell + vec2<f32>(i, j);
            if any(cell < vec2<f32>(0.0)) || any(cell >= vec2<f32>(cells)) {
                continue;
            }
            let seed = cell + face_index * 131.0;
            if hash(seed + 7.7) < 0.45 {
                continue;
            }
            let star_uv = (cell + hash2(seed)) / cells * 2.0 - 1.0;
            let star_dir = normalize(axis_n + axis_u * star_uv.x + axis_v * star_uv.y);

            // the chord is the angle for small angles, and well conditioned where acos(dot) is
            // not. Full within a quarter of the radius, gone at the radius
            let angle = length(d - star_dir);
            let core = 1.0 - smoothstep(radius * 0.25, radius, angle);
            if core <= 0.0 {
                continue;
            }
            let magnitude = hash(seed + 3.3);
            let brightness = 0.25 + 0.75 * magnitude * magnitude * magnitude;
            let phase = hash(seed + 5.5) * 6.2832;
            let twinkle = 1.0 + stars.twinkle * 0.5 * sin(time * stars.twinkle_speed * (2.0 + 3.0 * magnitude) + phase);
            sum += core * brightness * twinkle;
        }
    }
    return sum;
}

// The sky split into cells; every cell rolls once per period whether it fires a streak this
// round, and most never do. Streaks run mostly sideways with a slight downward slant and stay
// inside their cell. `sky` is (azimuth, elevation) in 0..1.
fn shooting_stars(sky: vec2<f32>, time: f32, rate: f32) -> f32 {
    let cell_f = sky * SHOOTING_GRID;
    let cell = floor(cell_f);
    let f = cell_f - cell;

    let h1 = hash(cell);
    let h2 = hash(cell + 17.3);
    let h3 = hash(cell + 41.7);

    let cycle = fract(time / SHOOTING_PERIOD + h1);
    if cycle > STREAK_LIFE {
        return 0.0;
    }
    let round_index = floor(time / SHOOTING_PERIOD + h1);
    let fires = step(1.0 - rate * 0.15, hash(cell + round_index * 3.1));

    let p = cycle / STREAK_LIFE;
    let to_right = select(-1.0, 1.0, h3 > 0.5);
    let dir = normalize(vec2<f32>(to_right * (0.85 + 0.15 * h3), -(0.15 + 0.3 * h2)));
    let start = vec2<f32>(select(0.9 - 0.2 * h2, 0.1 + 0.2 * h2, to_right > 0.0), 0.55 + 0.3 * h3);
    let head = start + dir * p * 0.6;
    let tail = head - dir * 0.25 * (1.0 - p);

    let pa = f - tail;
    let ba = head - tail;
    let along = saturate(dot(pa, ba) / max(dot(ba, ba), 1e-5));
    let dist = length(pa - ba * along);

    let streak = (1.0 - smoothstep(0.0, 0.012, dist)) * along;
    return streak * sin(p * PI) * fires;
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let coord = vec2<i32>(in.position.xy);

    // the view direction of this pixel
    let ndc = vec2<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    let world = view.world_from_clip * vec4<f32>(ndc, 1.0, 1.0);
    let direction = normalize(world.xyz / world.w - view.world_position);
    // Unity's +Z is -Z here: this is its direction for the pixel, so the field is Unity's
    let sky_dir = vec3<f32>(direction.x, direction.y, -direction.z);

    // azimuth in turns and elevation in 0..1. Straight up and down have no azimuth, and
    // atan2(0, 0) is undefined
    let pole = sky_dir.x == 0.0 && sky_dir.z == 0.0;
    let azimuth = select(atan2(sky_dir.x, sky_dir.z) / (2.0 * PI), 0.0, pole);
    let elevation = asin(clamp(sky_dir.y, -1.0, 1.0)) / PI + 0.5;

    // The dim patches drift over the noise. u jumps by the repeat count at the azimuth wrap;
    // removing whole periods from the derivative keeps the sample continuous there. The
    // derivatives are taken here, before any pixel is discarded.
    let patch_uv = vec2<f32>(azimuth * 3.0 + stars.time * 0.002, elevation * 3.0);
    var patch_dx = dpdx(patch_uv);
    var patch_dy = dpdy(patch_uv);
    patch_dx.x -= round(patch_dx.x / 3.0) * 3.0;
    patch_dy.x -= round(patch_dy.x / 3.0) * 3.0;

    // only where nothing was drawn
    if (stars.brightness <= 0.0 && stars.shooting_rate <= 0.0) || textureLoad(depth_texture, coord, 0) != 0.0 {
        discard;
    }

    // behind clouds and transparent geometry, and towards the horizon
    let sky = textureSampleLevel(sky_texture, sky_sampler, direction, 0.0);
    var visible = 1.0 - clamp(sky.a, 0.0, 1.0);
#ifdef FOCUS
    visible *= 1.0 - clamp(textureLoad(focus_texture, coord, 0).g, 0.0, 1.0);
#endif
    visible *= smoothstep(stars.horizon_fade.x, max(stars.horizon_fade.y, stars.horizon_fade.x + 1e-4), sky_dir.y);
    if visible <= 0.0 {
        discard;
    }

    // the field turns slowly about the zenith
    let turn = stars.time * stars.rotation_speed * 2.0 * PI;
    let s = sin(turn);
    let c = cos(turn);
    let turned = vec3<f32>(sky_dir.x * c - sky_dir.z * s, sky_dir.y, sky_dir.x * s + sky_dir.z * c);
    var star = star_field(turned, stars.time);

    // Unity's V runs up the image
    let patch_noise = textureSampleGrad(
        noise_texture,
        noise_sampler,
        vec2<f32>(patch_uv.x, 1.0 - patch_uv.y),
        vec2<f32>(patch_dx.x, -patch_dx.y),
        vec2<f32>(patch_dy.x, -patch_dy.y),
    ).r;
    star *= mix(1.0, mix(0.02, 1.0, patch_noise * patch_noise * patch_noise), stars.patch_strength);

    var shooting = 0.0;
    if stars.shooting_rate > 0.0 {
        shooting = shooting_stars(vec2<f32>(azimuth + 0.5, elevation), stars.time, stars.shooting_rate) * 3.0;
    }

    let light = (star * stars.brightness + shooting) * visible;
    if light <= 0.0 {
        discard;
    }
    let behind = max(sky.rgb, vec3<f32>(0.0));
    return vec4<f32>(rgb_to_linear(linear_to_rgb(behind) + light) - behind, 0.0);
}
