// The lens flare of the directional light (visuals/src/lens_flare.rs), added onto the frame before
// bloom. A full-screen pass: each pixel sums every flare shape over it, where Unity draws each
// shape as its own quad. The formulas are URP's data-driven lens flare:
// - placement of the elements and their random copies: LensFlareCommonSRP.cs
//   (ProcessLensFlareSRPElementsSingle, InternalGetFlareData0, GetLensFlareRayOffset),
//   unity:Explorer/Library/PackageCache/com.unity.render-pipelines.core@a4c9a9793789/Runtime/PostProcessing/LensFlareCommonSRP.cs
// - occlusion, the quad's placement, the circle falloff and the polygon SDF: LensFlareCommon.hlsl
//   (GetOcclusion, vert, ComputeCircle, ComputePolygon, ComputeRing), which
//   unity:Explorer/Library/PackageCache/com.unity.render-pipelines.universal@d7774c218bdf/Shaders/PostProcessing/LensFlareDataDriven.shader
//   includes from
//   unity:Explorer/Library/PackageCache/com.unity.render-pipelines.core@a4c9a9793789/Runtime/PostProcessing/Shaders/LensFlareCommon.hlsl
// The shapes' cut-off blocker is left out: every element's shapeCutOffSpeed is 0, so it never
// cuts. Unity's screen position runs from -1 to 1 with y down (flarePosScreen); so does
// `screen` here.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct FlareElement {
    tint: vec4<f32>,
    // half the size of the element's quad, in half screen heights
    size: vec2<f32>,
    position: f32,
    // degrees
    rotation: f32,
    intensity: f32,
    falloff: f32,
    edge_offset: f32,
    spread: f32,
    roundness: f32,
    scale_variation: f32,
    rotation_variation: f32,
    kind: u32,
    sides: u32,
    count: u32,
    auto_rotate: u32,
    modulate_by_light: u32,
}

struct LensFlare {
    light_color: vec3<f32>,
    // with the light's shape attenuation
    intensity: f32,
    // the light's position on screen, y up
    sun_ndc: vec2<f32>,
    // radius of the occlusion samples' disc
    occlusion_radius: f32,
    count: u32,
    // the random copies' tint gradient: colour, then time
    random_tint: array<vec4<f32>, 3>,
    elements: array<FlareElement, #{MAX_ELEMENTS}u>,
}

#ifdef MULTISAMPLED
@group(0) @binding(0) var depth_texture: texture_depth_multisampled_2d;
#else
@group(0) @binding(0) var depth_texture: texture_depth_2d;
#endif
@group(0) @binding(1) var<uniform> flare: LensFlare;

const PI: f32 = 3.14159265;

// Hash, unity:Explorer/Library/PackageCache/com.unity.render-pipelines.core@a4c9a9793789/ShaderLibrary/Random.hlsl
fn hash(seed: u32) -> f32 {
    var s = seed ^ 2747636419u;
    s = s * 2654435769u;
    s = s ^ (s >> 16u);
    s = s * 2654435769u;
    s = s ^ (s >> 16u);
    s = s * 2654435769u;
    return f32(s) / 4294967296.0;
}

// GetOcclusion: the share of samples in a disc around the light that see it. The light is at
// infinity, so Unity's test (the depth behind the sample is beyond the light's) passes only
// where nothing was drawn. Samples off screen count as hidden (allowOffScreen is off).
fn occlusion(sun: vec2<f32>, dims: vec2<f32>) -> f32 {
    var visible = 0.0;
    for (var i = 0u; i < #{OCCLUSION_SAMPLES}u; i += 1u) {
        // SampleDiskUniform(Hash(2i), Hash(2i + 1)), Sampling.hlsl
        let r = sqrt(hash(2u * i));
        let phi = 2.0 * PI * hash(2u * i + 1u);
        let uv = (sun + flare.occlusion_radius * r * vec2<f32>(cos(phi), sin(phi))) * 0.5 + 0.5;
        if all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0)) {
            let coord = vec2<i32>(min(uv * dims, dims - 1.0));
            if textureLoad(depth_texture, coord, 0) == 0.0 {
                visible += 1.0;
            }
        }
    }
    return visible / f32(#{OCCLUSION_SAMPLES}u);
}

// ComputeCircle, with `edge` and `falloff` as _FlareData3 holds them
fn circle(v: vec2<f32>, edge: f32, falloff: f32) -> f32 {
    let sdf = clamp((length(v) - 1.0) / (edge - 1.0), 0.0, 1.0);
    if sdf <= 0.0 {
        return 0.0;
    }
    return pow(sdf, falloff);
}

// ComputePolygon, with _FlareData4 as ProcessLensFlareSRPElementsSingle computes it
fn polygon(v: vec2<f32>, edge: f32, falloff: f32, sides: u32, roundness: f32) -> f32 {
    let inv_side = 1.0 / f32(sides);
    let r_cos = cos(PI * inv_side);
    let r = r_cos - r_cos * roundness;
    let an = 2.0 * PI * inv_side;
    let he = r * tan(0.5 * an);

    let bn = an * floor((atan2(v.y, v.x) + 0.5 * an) / an);
    let c = cos(bn);
    let s = sin(bn);
    let p = vec2<f32>(c * v.x + s * v.y, -s * v.x + c * v.y);
    let sdf = (length(p - vec2<f32>(r, clamp(p.y, -he, he))) * sign(p.x - r) - roundness) * edge;
    let shape = clamp(-sdf, 0.0, 1.0);
    if shape <= 0.0 {
        return 0.0;
    }
    return pow(shape, falloff);
}

// A procedural stand-in for Unity's ring images (Ring_T_Flare, PartialRIng_T_Flare, not
// carried): ComputeRing's ring, |r - (1 - t)| - t, without its noise, fading linearly from its
// middle to its edges. The partial ring keeps the arc toward +x, as the image.
fn ring(v: vec2<f32>, partial: bool) -> f32 {
    let thickness = bitcast<f32>(#{RING_THICKNESS}u);
    let len = length(v);
    var shape = clamp((thickness - abs(len - (1.0 - thickness))) / thickness, 0.0, 1.0);
    if partial {
        shape *= clamp((v.x / max(len, 1e-4) - 0.5) * 2.0, 0.0, 1.0);
    }
    return shape;
}

// colorGradient.Evaluate: linear between the keys, clamped
fn random_tint(t: f32) -> vec3<f32> {
    let g = flare.random_tint;
    if t <= g[0].w {
        return g[0].rgb;
    }
    if t <= g[1].w {
        return mix(g[0].rgb, g[1].rgb, (t - g[0].w) / (g[1].w - g[0].w));
    }
    if t <= g[2].w {
        return mix(g[1].rgb, g[2].rgb, (t - g[1].w) / (g[2].w - g[1].w));
    }
    return g[2].rgb;
}

// One element's quad at `position` along the ray, its shape at `screen`.
// GetLensFlareRayOffset and InternalGetFlareData0 place and turn the quad; the inverse of
// LensFlareCommon.hlsl's vert finds the quad's texcoord under the pixel.
fn shape_at(
    element: FlareElement,
    screen: vec2<f32>,
    sun: vec2<f32>,
    aspect: f32,
    position: f32,
    size: vec2<f32>,
    rotation: f32,
    edge: f32,
    falloff: f32,
) -> f32 {
    if size.x <= 0.0 || size.y <= 0.0 {
        return 0.0;
    }
    // no angular offset, translation scale 1 and no position offset in these flares
    let ray = -sun * position;
    let centre = sun + ray;
    var angle = rotation + 180.0;
    if element.auto_rotate != 0u && length(ray) > 0.0 {
        let along = normalize(ray) * vec2<f32>(aspect, 1.0);
        angle -= degrees(atan2(along.y, along.x));
    }
    // localCos0, localSin0
    let c = cos(-radians(angle));
    let s = sin(-radians(angle));

    // vert: local = Rotate(quad * size, c, s); local.x *= height / width; + centre
    var d = screen - centre;
    d.x *= aspect;
    let quad = vec2<f32>(c * d.x + s * d.y, -s * d.x + c * d.y) / size;
    if any(abs(quad) > vec2<f32>(1.0)) {
        return 0.0;
    }
    // the texcoord's x is flipped (uv.x = 1 - uv.x), and the shapes read 2 uv - 1
    let v = vec2<f32>(-quad.x, quad.y);

    switch element.kind {
        case #{KIND_CIRCLE}u: {
            return circle(v, edge, falloff);
        }
        case #{KIND_POLYGON}u: {
            return polygon(v, edge, falloff, element.sides, element.roundness);
        }
        case #{KIND_RING}u: {
            return ring(v, false);
        }
        default: {
            return ring(v, true);
        }
    }
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(depth_texture));
    // flarePosScreen
    let sun = vec2<f32>(flare.sun_ndc.x, -flare.sun_ndc.y);
    let visible = occlusion(sun, dims);
    if visible <= 0.0 {
        discard;
    }
    let screen = in.uv * 2.0 - 1.0;
    // width / height, vLocalScreenRatio.x
    let aspect = dims.x / dims.y;
    let intensity_variation = bitcast<f32>(#{INTENSITY_VARIATION}u);
    let position_variation = bitcast<f32>(#{POSITION_VARIATION}u);

    var light = vec3<f32>(0.0);
    for (var e = 0u; e < flare.count; e += 1u) {
        let element = flare.elements[e];
        var color = element.tint.rgb * element.intensity;
        if element.modulate_by_light != 0u {
            color *= flare.light_color;
        }
        // _FlareData3
        let falloff = exp(mix(0.0, 4.0, clamp(1.0 - element.falloff, 0.0, 1.0)));
        var edge = clamp((1.0 - element.edge_offset) - 1e-6, 0.0, 1.0);
        if element.kind == #{KIND_POLYGON}u {
            edge = pow(edge + 1.0, 5.0);
        }

        if element.count <= 1u {
            light += color * shape_at(
                element, screen, sun, aspect, 2.0 * element.position, element.size,
                element.rotation, edge, falloff,
            );
            continue;
        }

        // the Random distribution: Random.Range draws in Unity's order from seed 0, with this
        // hash standing in for UnityEngine.Random
        let step = 2.0 * element.spread / f32(element.count - 1u);
        var position = 2.0 * element.position;
        for (var i = 0u; i < element.count; i += 1u) {
            let k = i * 6u;
            let local_intensity = (hash(k) * 2.0 - 1.0) * intensity_variation + 1.0;
            let size = element.size * (1.0 + element.scale_variation * (hash(k + 1u) * 2.0 - 1.0));
            let tint = random_tint(hash(k + 2u)) * local_intensity;
            // k + 3 offsets the copy sideways by positionVariation.y, which is 0
            let rotation = element.rotation
                + (hash(k + 4u) * 2.0 - 1.0) * PI * element.rotation_variation;
            light += color * tint * shape_at(
                element, screen, sun, aspect, position, size, rotation, edge, falloff,
            );
            position += step + 0.5 * step * (hash(k + 5u) * 2.0 - 1.0) * position_variation;
        }
    }

    return vec4<f32>(light * flare.intensity * visible, 0.0);
}
