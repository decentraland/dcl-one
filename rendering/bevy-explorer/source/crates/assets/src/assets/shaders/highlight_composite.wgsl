// The hover glow, blended over the frame by its alpha outside the meshes themselves
// (scene_material/src/highlight). The glow is premultiplied, so the cut-out scales all of it.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var glow: texture_2d<f32>;
@group(0) @binding(1) var glow_sampler: sampler;
// opaque except where a highlighted mesh is, at the view's size
@group(0) @binding(2) var cut_out: texture_2d<f32>;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let outside = textureSample(cut_out, glow_sampler, in.uv).a;
    return textureSample(glow, glow_sampler, in.uv) * outside;
}
