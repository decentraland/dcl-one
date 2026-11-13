// Full-frame vignette for the lobby stage, sparing an oval around the avatar. The vertex stage
// writes clip-space positions straight from the unit quad, so the quad covers the frame of
// whichever camera draws it, wherever the mesh is.

#import bevy_pbr::forward_io::FragmentOutput

struct Vertex {
    @location(0) position: vec3<f32>,
}

struct FrameOutput {
    @builtin(position) position: vec4<f32>,
    // frame units from the bottom left
    @location(0) uv: vec2<f32>,
}

struct LobbyVignette {
    color: vec4<f32>,
    mask_radii: vec4<f32>,
    mask_center: vec2<f32>,
    intensity: f32,
    smoothness: f32,
    mask_softness: f32,
}

@group(2) @binding(0) var<uniform> vignette: LobbyVignette;

// 1 / the distance from the frame's centre to a corner, so the ramp reaches 1 exactly there
const INV_CORNER_DISTANCE: f32 = 0.70710678;

@vertex
fn vertex(vertex: Vertex) -> FrameOutput {
    var out: FrameOutput;
    // the unit rectangle spans -0.5..0.5; doubled it fills clip space. Depth is ignored.
    out.position = vec4(vertex.position.xy * 2.0, 0.5, 1.0);
    out.uv = vertex.position.xy + 0.5;
    return out;
}

@fragment
fn fragment(in: FrameOutput) -> FragmentOutput {
    // 0 at the centre of the frame, 1 at its corners; the ellipse follows the frame's shape
    let centred = (in.uv - 0.5) * 2.0;
    let from_centre = length(centred) * INV_CORNER_DISTANCE;
    var darkening = smoothstep(1.0 - vignette.smoothness, 1.0, from_centre);
    // the oval around the avatar: 1 inside, fading to 0 across the softness band, so the sky
    // above the avatar keeps its brightness while the sides and the bottom still darken
    let offset = in.uv - vignette.mask_center;
    let radius_y = select(vignette.mask_radii.z, vignette.mask_radii.y, offset.y > 0.0);
    let mask_distance = length(offset / vec2(vignette.mask_radii.x, radius_y));
    let spared = 1.0 - smoothstep(1.0 - vignette.mask_softness, 1.0, mask_distance);
    darkening *= 1.0 - spared;

    var out: FragmentOutput;
    out.color = vec4(vignette.color.rgb, vignette.color.a * vignette.intensity * darkening);
#ifdef TRANSPARENT_FOCUS_OUTPUT
    out.focus = vec4(0.0);
#endif
    return out;
}
