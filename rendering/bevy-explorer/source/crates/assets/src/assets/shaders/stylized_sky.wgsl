// The stylized sky: the sky colour from a lookup by day phase and elevation, with a scene's own sky colours blended over it, then the sun or the moon as a
// disc in a halo, under three strips of cloud. The lookup follows Unity's SkyLut.hlsl, the disc
// its SunDisc.hlsl, the halo its SunRadiance sub-graph and the clouds its CloudsV2.hlsl. Like
// Unity's GenesisSky graph, everything is composited on the authored values and the result is
// decoded from sRGB to linear as the last step.
//
// Two entry points share `sky_at`. The skybox material (`SKY_MATERIAL`) draws the sky on the
// screen per pixel, as Unity's skybox does, so the clouds keep the strips' detail. The compute
// entry draws the same sky into the atmosphere cube, which the reflections, the ambient and the
// star pass read.

struct StylizedSky {
    tick: u32,
    // seconds
    time: f32,
    // 0 and 1 night, 0.25 sunrise, 0.5 day, 0.75 sunset
    phase: f32,
    body_direction: vec3<f32>,
    disc_opacity: f32,
    disc_color: vec3<f32>,
    disc_size: f32,
    // the halo around the disc: radius, intensity
    halo: vec2<f32>,
    // where the crescent's mask sits off the disc's centre, in the body's own frame
    mask_offset: vec2<f32>,
    // scales every cloud strip's opacity
    cloud_opacity: f32,
    // how much the body behind a cloud lights it through
    backlight_weight: f32,
    cloud_lit: vec3<f32>,
    scene_sun_visible: f32,
    cloud_shadow: vec3<f32>,
    // the size of the circle that cuts the moon's disc to a crescent; 0 leaves a disc whole
    mask_size: f32,
    // strength, tiling U, tiling V, speed
    horizon_noise: vec4<f32>,
    // scene-authored linear sky colours; alpha flags whether each band is overridden
    scene_zenith: vec4<f32>,
    scene_horizon: vec4<f32>,
    scene_nadir: vec4<f32>,
    scene_rim: vec4<f32>,
    // ramp knee, highlight strength, highlight threshold, highlight falloff
    cloud_params: vec4<f32>,
    // zenith fade start and end, occlusion start and end
    cloud_fades: vec4<f32>,
    // per strip: stretch and offset down the dome, repeats and offset around it
    cloud_layers: array<vec4<f32>, 3>,
    // per strip: strength, opacity, flow at the phase, scroll in turns
    cloud_looks: array<vec4<f32>, 3>,
}

// the lookup's columns: elevation, the horizon to the zenith. Its rows: night, sunrise, day,
// sunset, night; filtering between rows blends neighbouring phases. The cloud texture has one
// layer a strip, back to front: each a turn of the sky wide, repeating around it. R the shaded
// cloud, G its backlit look, B the growth order, A the mask
#ifdef SKY_MATERIAL
#import bevy_pbr::forward_io::VertexOutput

@group(2) @binding(0) var<uniform> sky: StylizedSky;
@group(2) @binding(1) var lut_texture: texture_2d<f32>;
@group(2) @binding(2) var lut_sampler: sampler;
@group(2) @binding(3) var noise_texture: texture_2d<f32>;
@group(2) @binding(4) var noise_sampler: sampler;
@group(2) @binding(5) var cloud_texture: texture_2d_array<f32>;
@group(2) @binding(6) var cloud_sampler: sampler;
// a scene's equirectangular image, shown instead of the sky. Its centre column faces -Z, its
// top row is the zenith
@group(2) @binding(7) var panorama_texture: texture_2d<f32>;
@group(2) @binding(8) var panorama_sampler: sampler;
#else
@group(0) @binding(0) var<uniform> sky: StylizedSky;
@group(0) @binding(1) var lut_texture: texture_2d<f32>;
@group(0) @binding(2) var lut_sampler: sampler;
@group(0) @binding(3) var noise_texture: texture_2d<f32>;
@group(0) @binding(4) var noise_sampler: sampler;
@group(0) @binding(5) var cloud_texture: texture_2d_array<f32>;
@group(0) @binding(6) var cloud_sampler: sampler;

@group(1) @binding(0)
var image: texture_storage_2d_array<rgba16float, write>;
#endif

const TAU: f32 = 6.283185307179586;
const LUT_ROWS: f32 = 5.0;
// degrees of elevation the sky dome's V spans, from the zenith down
const DOME_SPAN_DEG: f32 = 113.6;

// How far the lookup's elevation is pushed up near the horizon by a tiling noise, so the bright
// band shows soft silhouettes of distant clouds. `azimuth` is in turns.
fn horizon_noise(azimuth: f32, elevation_deg: f32) -> f32 {
    let strength = sky.horizon_noise.x;
    let v_dome = (90.0 - elevation_deg) / DOME_SPAN_DEG;
    // only below ~22 degrees of elevation, full at the horizon
    let weight = strength * saturate((v_dome - 0.6) / 0.3);
    if weight <= 0.0 {
        return 0.0;
    }
    // a whole repeat count, so the strip meets itself at the azimuth wrap
    let tiling_u = max(round(sky.horizon_noise.y), 1.0);
    let uv = vec2<f32>(azimuth * tiling_u + sky.time * sky.horizon_noise.w, v_dome * sky.horizon_noise.z);
    // Unity's V runs up the image; a compute shader has no derivatives, so the top level
    return textureSampleLevel(noise_texture, noise_sampler, vec2<f32>(uv.x, 1.0 - uv.y), 0.0).r * weight;
}

// The lookup's colour toward `dir`. Under the horizon the elevation clamps to 0: the horizon
// colour. `azimuth` is in turns.
fn sky_color(dir: vec3<f32>, azimuth: f32, elevation_deg: f32) -> vec3<f32> {
    let elevation = saturate(dir.y + horizon_noise(azimuth, elevation_deg));
    let row = sky.phase * (LUT_ROWS - 1.0) / LUT_ROWS + 0.5 / LUT_ROWS;
    return textureSampleLevel(lut_texture, lut_sampler, vec2<f32>(elevation, row), 0.0).rgb;
}

// A cloud texel's colour between the shadowed and the lit colour, through a midtone at the knee.
fn cloud_ramp(t: f32) -> vec3<f32> {
    let knee = sky.cloud_params.x;
    let mid = mix(sky.cloud_shadow, sky.cloud_lit, 0.35);
    if t < knee {
        return mix(sky.cloud_shadow, mid, saturate(t / max(knee, 1e-3)));
    }
    return mix(mid, sky.cloud_lit, saturate((t - knee) / max(1.0 - knee, 1e-3)));
}

// The cloud strips toward `dir`, back to front: their colour, premultiplied, and their alpha.
// `azimuth` is in turns.
fn clouds(dir: vec3<f32>, azimuth: f32, elevation_deg: f32) -> vec4<f32> {
    // each strip hangs on a band of the sky dome, whose V is 0 at the zenith and 1 at its rim
    // under the horizon
    let v_dome = (90.0 - elevation_deg) / DOME_SPAN_DEG;

    // the body behind a cloud drives its shading toward the backlit look and adds a rim; only
    // the drawn body's own side lights
    let threshold = sky.cloud_params.z;
    let facing = saturate((dot(dir, sky.body_direction) - threshold) / max(1.0 - threshold, 1e-3));
    let backlight = pow(facing, max(sky.cloud_params.w, 1e-3)) * sky.backlight_weight;

    // into the horizon haze, hidden below the horizon, and out toward the zenith, where the
    // strips pinch. The zenith edges are kept apart: smoothstep with equal edges divides by zero
    let zenith_fade = 1.0 - smoothstep(sky.cloud_fades.x, max(sky.cloud_fades.y, sky.cloud_fades.x + 1e-4), dir.y);
    let fade = smoothstep(-0.35, -0.05, dir.y) * zenith_fade * sky.cloud_opacity;

    var color = vec3<f32>(0.0);
    var alpha = 0.0;
    for (var index = 0; index < 3; index++) {
        let layer = sky.cloud_layers[index];
        let look = sky.cloud_looks[index];
        let v_tex = (v_dome + layer.y - 0.5) / max(layer.x, 1e-3) + 0.5;
        let uv = vec2<f32>(azimuth * layer.z + layer.w - look.w, saturate(1.0 - v_tex));
        // Unity's V runs up the image; a compute shader has no derivatives, so the top level
        let strip = textureSampleLevel(cloud_texture, cloud_sampler, vec2<f32>(uv.x, 1.0 - uv.y), index, 0.0);

        // the strips fade to black where the mask fades; undone, soft edges keep the cloud's
        // colour and draw no dark outline
        let shade = saturate(strip.rgb / max(strip.a, 1e-3));
        var layer_color = cloud_ramp(mix(shade.r, shade.g, backlight)) * look.x;
        layer_color += shade.g * backlight * sky.cloud_params.y * sky.cloud_lit;

        // texels whose growth order is under the flow are there
        let layer_alpha = strip.a * saturate((look.z * 1.3 - shade.b) / 0.3) * fade * look.y;

        color = color * (1.0 - layer_alpha) + layer_color * layer_alpha;
        alpha = alpha + layer_alpha * (1.0 - alpha);
    }
    return vec4<f32>(color, alpha);
}

// The sun's or the moon's disc toward `dir`: Unity's SunDisc_float without the haze and the
// second sun, which the preset leaves off.
fn disc(dir: vec3<f32>) -> vec3<f32> {
    // Unity's own vectors, so its maths applies as written: its +Z is -Z here, its light
    // direction runs from the body and its view direction from the sky to the eye
    let light = vec3<f32>(-sky.body_direction.x, -sky.body_direction.y, sky.body_direction.z);
    let view = vec3<f32>(-dir.x, -dir.y, dir.z);
    let body = step(acos(clamp(dot(light, view), -1.0, 1.0)), sky.disc_size * 0.3);

    // the crescent: a second circle, off the centre in the body's own frame so it keeps one
    // shape wherever the body is, cuts a hole. Size 0 leaves the disc whole
    let up_hint = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(light.y) > 0.999);
    let right = normalize(cross(up_hint, light));
    let up = cross(light, right);
    let mask_centre = normalize(light + right * sky.mask_offset.x + up * sky.mask_offset.y);
    let mask = step(sky.mask_size * sky.mask_size, acos(clamp(dot(mask_centre, view), -1.0, 1.0)));

    return body * sky.disc_color * sky.disc_opacity * sky.disc_opacity * mask;
}

// Shader Graph's Colorspace Conversion node, RGB to Linear: the sRGB curve, continued past 1 on
// its power branch.
fn rgb_to_linear(colour: vec3<f32>) -> vec3<f32> {
    let low = colour / 12.92;
    let high = pow(max(abs((colour + 0.055) / 1.055), vec3<f32>(1.192092896e-07)), vec3<f32>(2.4));
    return select(high, low, colour <= vec3<f32>(0.04045));
}

// The glow around the body toward `dir`: Unity's SunRadiance sub-graph. Its strength is the
// intensity times (1 + radius - angle), kept to 0 to 1: it falls off linearly with the angle from
// the body and is gone at 1 + radius radians. The graph's colour is a vec4, so its length counts
// an alpha of 1.
fn halo(dir: vec3<f32>) -> vec3<f32> {
    let angle = acos(clamp(dot(sky.body_direction, dir), -1.0, 1.0));
    return saturate((1.0 - angle + sky.halo.x) * sky.halo.y) * normalize(vec4<f32>(sky.disc_color, 1.0)).rgb;
}

// The sky toward `ray`, a unit vector: its colour, and how much the clouds hide of what lies
// behind them.
fn sky_at(ray: vec3<f32>) -> vec4<f32> {
    // Unity's +Z is -Z here. Straight up and down have no azimuth, and atan2(0, 0) is undefined
    var azimuth = 0.0;
    if ray.x != 0.0 || ray.z != 0.0 {
        azimuth = atan2(ray.x, -ray.z) / TAU;
    }
    let elevation_deg = degrees(asin(clamp(ray.y, -1.0, 1.0)));
    var color = sky_color(ray, azimuth, elevation_deg);

    // Each missing band retains the lookup's colour in that direction.
    // Authored colours also feed ambient lighting when an image replaces the visible sky.
    let scene_bands = max(max(sky.scene_zenith.a, sky.scene_horizon.a), max(sky.scene_nadir.a, sky.scene_rim.a));
    if scene_bands > 0.0 {
        let pole_weight = smoothstep(0.0, 1.0, abs(ray.y));
        let pole = select(sky.scene_nadir, sky.scene_zenith, ray.y >= 0.0);
        let horizon_weight = (1.0 - pole_weight) * sky.scene_horizon.a;
        let authored_pole_weight = pole_weight * pole.a;
        color = color * (1.0 - horizon_weight - authored_pole_weight)
            + sky.scene_horizon.rgb * horizon_weight + pole.rgb * authored_pole_weight;
        let rim_weight = (1.0 - smoothstep(0.0, 0.08, abs(ray.y))) * sky.scene_rim.a;
        color = mix(color, sky.scene_rim.rgb, rim_weight);
    }

    // Alpha is how much the clouds hide of what lies behind them, the stars and the disc: only
    // mostly opaque cloud does, or soft edges would wear a dark ring. The edges are kept apart,
    // as above
    let cloud = clouds(ray, azimuth, elevation_deg);
    let occlusion = smoothstep(sky.cloud_fades.z, max(sky.cloud_fades.w, sky.cloud_fades.z + 1e-4), cloud.a);

    // The body goes on the sky under the clouds, in the order of the edges of Unity's
    // GenesisSky.shadergraph with its stylized keyword on:
    //   SkyLut -> Blend (Screen; Blend = Sun Radiance, Opacity = Sun Opacity)
    //          -> SunComposite.Base (Blend = SunAndMoon * saturate(1 - Occlusion),
    //                                Opacity = saturate(1 - Occlusion))
    //          -> CloudsV2Composite.Base
    // So the halo is added once, screened onto the sky at the disc's opacity, and no cloud
    // occlusion masks it: the clouds only cover it. The disc is added (SunComposite, stylized)
    // and the occlusion counts twice, in what is blended and in how much of the blend shows.
    if sky.scene_sun_visible > 0.0 {
        let screened = 1.0 - (1.0 - halo(ray)) * (1.0 - color);
        color = mix(color, screened, sky.disc_opacity);
        let shown = 1.0 - occlusion;
        color += disc(ray) * shown * shown;
    }

    // the clouds over the sky and the body, so a soft cloud edge dims the sun behind it
    color = color * (1.0 - cloud.a) + cloud.rgb;

    // GenesisSky then decodes the whole sky from sRGB to linear (Colorspace Conversion, RGB to
    // Linear). The rim it screens on before is off with the lookup
    color = rgb_to_linear(color);

    return vec4<f32>(color, occlusion);
}

#ifdef SKY_MATERIAL
// From https://alex.vlachos.com/graphics/Alex_Vlachos_Advanced_VR_Rendering_GDC2015.pdf, as
// bevy_atmosphere's skybox dithers: Iestyn's RGB dither, centred so it does not brighten
fn dither(frag_coord: vec2<f32>) -> vec3<f32> {
    let dither = fract(vec3<f32>(dot(vec2<f32>(171.0, 231.0), frag_coord)) / vec3<f32>(103.0, 71.0, 97.0));
    return (dither - 0.5) / 255.0;
}

// The skybox mesh's normals run from the camera through each corner, so the interpolated normal
// is the view direction
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let ray = normalize(in.world_normal);
#ifdef PANORAMA
    let uv = vec2<f32>(atan2(ray.x, -ray.z) / TAU + 0.5, acos(clamp(ray.y, -1.0, 1.0)) / 3.14159265359);
    let color = textureSample(panorama_texture, panorama_sampler, uv).rgb;
#else
    let color = sky_at(ray).rgb;
#endif
    return vec4<f32>(color + dither(in.position.xy), 1.0);
}
#else
@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) original_invocation_id: vec3<u32>, @builtin(num_workgroups) num_workgroups: vec3<u32>) {
    let size = textureDimensions(image).x;
    let scale = f32(size) / 2f;

    // dither pattern for updates
    let tick = sky.tick & 63;

    var UPDATE_OFFSETS_X: array<u32,64> = array<u32,64>(
        0u, 4u, 4u, 0u, 2u, 6u, 6u, 2u, 2u, 6u, 6u, 2u, 0u, 4u, 4u, 0u,
        1u, 5u, 5u, 1u, 3u, 7u, 7u, 3u, 3u, 7u, 7u, 3u, 1u, 5u, 5u, 1u,
        1u, 5u, 5u, 1u, 3u, 7u, 7u, 3u, 3u, 7u, 7u, 3u, 1u, 5u, 5u, 1u,
        0u, 4u, 4u, 0u, 2u, 6u, 6u, 2u, 2u, 6u, 6u, 2u, 0u, 4u, 4u, 0u
    );

    var UPDATE_OFFSETS_Y: array<u32,64> = array<u32,64>(
        0u, 4u, 0u, 4u, 2u, 6u, 2u, 6u, 0u, 4u, 0u, 4u, 2u, 6u, 2u, 6u,
        1u, 5u, 1u, 5u, 3u, 7u, 3u, 7u, 1u, 5u, 1u, 5u, 3u, 7u, 3u, 7u,
        4u, 0u, 4u, 0u, 6u, 2u, 6u, 2u, 4u, 0u, 4u, 0u, 6u, 2u, 6u, 2u,
        5u, 1u, 5u, 1u, 7u, 3u, 7u, 3u, 5u, 1u, 5u, 1u, 7u, 3u, 7u, 3u
    );

    let invocation_id = original_invocation_id * vec3(8, 8, 1) + vec3(UPDATE_OFFSETS_X[tick], UPDATE_OFFSETS_Y[tick], 0);

    let dir = vec2<f32>((f32(invocation_id.x) / scale) - 1f, (f32(invocation_id.y) / scale) - 1f);

    var ray: vec3<f32>;

    switch invocation_id.z {
        case 0u {
            ray = vec3<f32>(1f, -dir.y, -dir.x); // +X
        }
        case 1u {
            ray = vec3<f32>(-1f, -dir.y, dir.x);// -X
        }
        case 2u {
            ray = vec3<f32>(dir.x, 1f, dir.y); // +Y
        }
        case 3u {
            ray = vec3<f32>(dir.x, -1f, -dir.y);// -Y
        }
        case 4u {
            ray = vec3<f32>(dir.x, -dir.y, 1f); // +Z
        }
        default: {
            ray = vec3<f32>(-dir.x, -dir.y, -1f);// -Z
        }
    }

    textureStore(image, vec2<i32>(invocation_id.xy), i32(invocation_id.z), sky_at(normalize(ray)));
}
#endif
