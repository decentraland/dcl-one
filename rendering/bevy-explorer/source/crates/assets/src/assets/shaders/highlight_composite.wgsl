// The hover glow's mask, blended over the frame by its alpha (scene_material/src/highlight).

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var mask: texture_2d<f32>;
@group(0) @binding(1) var mask_sampler: sampler;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return textureSample(mask, mask_sampler, in.uv);
}
