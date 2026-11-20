// The lens flare of the directional light (visuals/src/lens_flare.rs), added onto the frame before
// bloom. One instanced draw: each instance is one element copy's quad, placed on the CPU. The
// vertex stage tests how much of the light is hidden and turns the quad; the fragment stage draws
// the element's shape. Screen positions run from -1 to 1 with y down, as the CPU places them.

struct FlareQuad {
    centre: vec2<f32>,
    // half the quad's size, in half screen heights
    size: vec2<f32>,
    // cosine and sine of the quad's angle
    turn: vec2<f32>,
    edge: f32,
    falloff: f32,
    color: vec3<f32>,
    roundness: f32,
    kind: u32,
    sides: u32,
}

struct LensFlare {
    // the light's position on screen
    sun: vec2<f32>,
    // radius of the occlusion samples' disc
    occlusion_radius: f32,
    // width over height
    aspect: f32,
    count: u32,
    quads: array<FlareQuad, #{MAX_QUADS}u>,
}

#ifdef MULTISAMPLED
@group(0) @binding(0) var depth_texture: texture_depth_multisampled_2d;
#else
@group(0) @binding(0) var depth_texture: texture_depth_2d;
#endif
@group(0) @binding(1) var<uniform> flare: LensFlare;
#ifdef FOCUS
// depth of field's transparent focus texture: the transparent surfaces' coverage in g. It is
// single-sampled, and depth of field yields to MSAA, so FOCUS never meets MULTISAMPLED
@group(0) @binding(2) var focus_texture: texture_2d<f32>;
#endif

const PI: f32 = 3.14159265;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    // the quad's own coordinates, -1 to 1 across it
    @location(0) quad: vec2<f32>,
    @location(1) @interpolate(flat) color: vec3<f32>,
    @location(2) @interpolate(flat) edge: f32,
    @location(3) @interpolate(flat) falloff: f32,
    @location(4) @interpolate(flat) roundness: f32,
    @location(5) @interpolate(flat) kind: u32,
    @location(6) @interpolate(flat) sides: u32,
}

// a random value in [0, 1) for `seed`, as lens_flare.rs's `hash`
fn hash(seed: u32) -> f32 {
    var s = seed ^ 2747636419u;
    s = s * 2654435769u;
    s = s ^ (s >> 16u);
    s = s * 2654435769u;
    s = s ^ (s >> 16u);
    s = s * 2654435769u;
    return f32(s) / 4294967296.0;
}

// The share of samples in a disc around the light that see it. The light is at infinity, so a
// sample sees it only where nothing opaque was drawn, and through the share of it no transparent
// surface covers. Samples off screen count as hidden.
fn visibility(dims: vec2<f32>) -> f32 {
    var visible = 0.0;
    for (var i = 0u; i < #{OCCLUSION_SAMPLES}u; i += 1u) {
        // uniform over the disc
        let r = sqrt(hash(2u * i));
        let phi = 2.0 * PI * hash(2u * i + 1u);
        let uv = (flare.sun + flare.occlusion_radius * r * vec2<f32>(cos(phi), sin(phi))) * 0.5
            + 0.5;
        if all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0)) {
            let coord = vec2<i32>(min(uv * dims, dims - 1.0));
            if textureLoad(depth_texture, coord, 0) == 0.0 {
#ifdef FOCUS
                visible += 1.0 - clamp(textureLoad(focus_texture, coord, 0).g, 0.0, 1.0);
#else
                visible += 1.0;
#endif
            }
        }
    }
    return visible / f32(#{OCCLUSION_SAMPLES}u);
}

@vertex
fn vertex(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    // two triangles
    var corners = array<vec2<f32>, 6>(
        vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(1.0, 1.0),
        vec2(-1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0),
    );
    let corner = corners[vertex_index];
    let quad = flare.quads[instance_index];

    var out: VertexOutput;
    out.quad = corner;
    out.edge = quad.edge;
    out.falloff = quad.falloff;
    out.roundness = quad.roundness;
    out.kind = quad.kind;
    out.sides = quad.sides;

    let visible = visibility(vec2<f32>(textureDimensions(depth_texture)));
    if visible <= 0.0 {
        // behind the near plane: the quad is clipped away
        out.position = vec4<f32>(0.0, 0.0, -1.0, 1.0);
        out.color = vec3<f32>(0.0);
        return out;
    }
    out.color = quad.color * visible;

    // turned, then squeezed to the screen's aspect, then moved to its centre
    let local = corner * quad.size;
    let c = quad.turn.x;
    let s = quad.turn.y;
    let turned = vec2<f32>(c * local.x - s * local.y, s * local.x + c * local.y);
    let screen = quad.centre + vec2<f32>(turned.x / flare.aspect, turned.y);
    out.position = vec4<f32>(screen.x, -screen.y, 0.0, 1.0);
    return out;
}

// a disc fading from `edge` to its rim
fn circle(v: vec2<f32>, edge: f32, falloff: f32) -> f32 {
    let sdf = clamp((length(v) - 1.0) / (edge - 1.0), 0.0, 1.0);
    if sdf <= 0.0 {
        return 0.0;
    }
    return pow(sdf, falloff);
}

// a regular polygon with rounded corners, fading over `1 / edge` inside its outline
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

// a bump of `height` at `peak`, `inner` wide inside it and `outer` wide outside
fn band(x: f32, peak: f32, height: f32, inner: f32, outer: f32) -> f32 {
    let d = (x - peak) / select(outer, inner, x < peak);
    return height * exp(-0.5 * d * d);
}

// The rings as bands of colour by radius. The ring: red, green, then blue outward, around two
// thirds of the quad out
fn ring(v: vec2<f32>) -> vec3<f32> {
    let r = length(v);
    return vec3<f32>(
        band(r, 0.66, 0.099, 0.033, 0.033),
        band(r, 0.70, 0.087, 0.035, 0.035),
        band(r, 0.735, 0.081, 0.035, 0.035),
    );
}

// The partial ring: a thin arc toward +x near the quad's rim, red outside and violet inside, fading
// over about 40 degrees either side
fn partial_ring(v: vec2<f32>) -> vec3<f32> {
    let r = length(v);
    let angle = abs(atan2(v.y, v.x)) / radians(34.0);
    let arc = exp(-angle * angle * angle * angle);
    return arc * vec3<f32>(
        band(r, 0.92, 0.47, 0.029, 0.016),
        band(r, 0.905, 0.04, 0.026, 0.026),
        band(r, 0.885, 0.185, 0.038, 0.028),
    );
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // the shapes are mirrored across the quad
    let v = vec2<f32>(-in.quad.x, in.quad.y);
    var shape: vec3<f32>;
    switch in.kind {
        case #{KIND_CIRCLE}u: {
            shape = vec3<f32>(circle(v, in.edge, in.falloff));
        }
        case #{KIND_POLYGON}u: {
            shape = vec3<f32>(polygon(v, in.edge, in.falloff, in.sides, in.roundness));
        }
        case #{KIND_RING}u: {
            shape = ring(v);
        }
        default: {
            shape = partial_ring(v);
        }
    }
    return vec4<f32>(in.color * shape, 0.0);
}
