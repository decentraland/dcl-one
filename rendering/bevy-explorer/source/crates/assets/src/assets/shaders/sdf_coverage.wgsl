// coverage of a glyph distance field: an atlas texture holding 0.5 at the ink edge and 0/1 at
// `spread` pixels outside/inside. Shared by the text shapes (text_sdf_common.wgsl) and the
// nametags (nametag.wgsl).

// signed distance in atlas pixels, positive inside the ink. explicit gradients: samples are
// taken inside non-uniform branches
fn sdf_distance(atlas: texture_2d<f32>, atlas_sampler: sampler, spread: f32, uv: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>) -> f32 {
    let sample = textureSampleGrad(atlas, atlas_sampler, uv, dx, dy).r;
    return (sample - 0.5) * 2.0 * spread;
}

// coverage of a distance, anti-aliased with a linear ramp `width` atlas pixels wide
fn coverage(distance: f32, width: f32) -> f32 {
    return saturate(distance / width + 0.5);
}

// anti-aliased coverage of the ink dilated by `dilate` atlas pixels, with the edge softened
// by a further `soft`. `px` is atlas pixels per screen pixel: the ramp is one screen pixel,
// and minified glyphs take four rotated-grid taps so thin strokes don't sparkle or vanish.
fn sdf_layer_alpha(atlas: texture_2d<f32>, atlas_sampler: sampler, spread: f32, uv: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>, px: f32, dilate: f32, soft: f32) -> f32 {
    let width = max(px, 1e-4) + soft;
    if px > 1.0 {
        let a = coverage(sdf_distance(atlas, atlas_sampler, spread, uv + 0.125 * dx + 0.375 * dy, dx, dy) + dilate, width)
            + coverage(sdf_distance(atlas, atlas_sampler, spread, uv - 0.375 * dx + 0.125 * dy, dx, dy) + dilate, width)
            + coverage(sdf_distance(atlas, atlas_sampler, spread, uv + 0.375 * dx - 0.125 * dy, dx, dy) + dilate, width)
            + coverage(sdf_distance(atlas, atlas_sampler, spread, uv - 0.125 * dx - 0.375 * dy, dx, dy) + dilate, width);
        return a * 0.25;
    }
    return coverage(sdf_distance(atlas, atlas_sampler, spread, uv, dx, dy) + dilate, width);
}
