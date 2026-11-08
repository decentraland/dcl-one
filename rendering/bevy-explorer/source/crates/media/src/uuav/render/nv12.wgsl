// NV12 (Y plane + interleaved CbCr plane, BT.709 limited range) to linear rgba, drawn as one
// fullscreen triangle into the player's target image. The target format is sRGB, so the
// linear output is encoded on store.

@group(0) @binding(0) var y_plane: texture_2d<f32>;
@group(0) @binding(1) var uv_plane: texture_2d<f32>;
@group(0) @binding(2) var plane_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    // (-1,-1) (3,-1) (-1,3): one triangle covering the clip space
    let x = f32(i32(index & 1u) * 4 - 1);
    let y = f32(i32(index >> 1u) * 4 - 1);
    var out: VertexOutput;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    // texture rows run top-down
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let y = textureSample(y_plane, plane_sampler, in.uv).r;
    let cbcr = textureSample(uv_plane, plane_sampler, in.uv).rg;

    // limited range: Y 16..235, CbCr 16..240 (in 8-bit terms)
    let luma = (y - 16.0 / 255.0) * (255.0 / 219.0);
    let cb = (cbcr.x - 128.0 / 255.0) * (255.0 / 224.0);
    let cr = (cbcr.y - 128.0 / 255.0) * (255.0 / 224.0);

    // BT.709
    let r = luma + 1.5748 * cr;
    let g = luma - 0.1873 * cb - 0.4681 * cr;
    let b = luma + 1.8556 * cb;
    let rgb = clamp(vec3<f32>(r, g, b), vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(srgb_to_linear(rgb), 1.0);
}
