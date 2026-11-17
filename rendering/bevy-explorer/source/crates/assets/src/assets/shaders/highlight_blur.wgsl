// One axis of the hover glow's blur (scene_material/src/highlight): a 5-tap gaussian in the
// target's texels, read from the full-size mask through the linear filter. The kernel comes from
// `BLUR_OFFSETS` and `BLUR_WEIGHTS` as f32 bit patterns.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
// the blur's axis: (1, 0) across, (0, 1) down
@group(0) @binding(2) var<uniform> axis: vec2<f32>;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let offsets = vec3<f32>(
        bitcast<f32>(#{BLUR_OFFSET_0}u),
        bitcast<f32>(#{BLUR_OFFSET_1}u),
        bitcast<f32>(#{BLUR_OFFSET_2}u),
    );
    let weights = vec3<f32>(
        bitcast<f32>(#{BLUR_WEIGHT_0}u),
        bitcast<f32>(#{BLUR_WEIGHT_1}u),
        bitcast<f32>(#{BLUR_WEIGHT_2}u),
    );
    // one target texel in uv, from the fullscreen triangle's constant uv derivatives
    let step = axis * abs(vec2<f32>(dpdx(in.uv).x, dpdy(in.uv).y));

    var sum = textureSample(source, source_sampler, in.uv + step * offsets.x) * weights.x;
    sum += textureSample(source, source_sampler, in.uv + step * offsets.y) * weights.y;
    sum += textureSample(source, source_sampler, in.uv - step * offsets.y) * weights.y;
    sum += textureSample(source, source_sampler, in.uv + step * offsets.z) * weights.z;
    sum += textureSample(source, source_sampler, in.uv - step * offsets.z) * weights.z;
    return sum;
}
