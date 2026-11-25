// A nametag (avatar/src/nametag): one mesh of quads, drawn back to front in index order. Each
// quad carries its kind in `shape.x`: a glyph from the distance-field atlas page, a colour
// glyph from the bitmap page, an icon, a rounded rectangle, or a voice bar. Unlit, blended and
// depth-tested without writing depth. The mesh is flat, in tag units around its anchor: the
// vertex stage turns it to the camera, sizes it to keep its size on screen and fades it with
// distance. Its colours are tonemapped with the rest of the frame.

#import bevy_pbr::{
    forward_io::FragmentOutput,
    mesh_functions,
    mesh_view_bindings::{view, globals},
    view_transformations,
}
#import "embedded://shaders/sdf_coverage.wgsl"::sdf_layer_alpha

@group(2) @binding(0) var sdf_texture: texture_2d<f32>;
@group(2) @binding(1) var sdf_sampler: sampler;
@group(2) @binding(2) var bitmap_texture: texture_2d<f32>;
@group(2) @binding(3) var bitmap_sampler: sampler;
@group(2) @binding(4) var verified_texture: texture_2d<f32>;
@group(2) @binding(5) var official_texture: texture_2d<f32>;
@group(2) @binding(6) var hushed_texture: texture_2d<f32>;
@group(2) @binding(7) var icon_sampler: sampler;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    // tag units, the anchor at the origin
    @location(0) position: vec3<f32>,
    // glyphs: atlas pixels; icons: 0..1; shapes: tag units from the shape's centre, y up
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    // x: kind; yzw by kind (see the fragment stage)
    @location(3) shape: vec4<f32>,
    // corner radii of a rounded rectangle: top right, bottom right, top left, bottom left
    @location(4) radii: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) shape: vec4<f32>,
    @location(3) radii: vec4<f32>,
    @location(4) opacity: f32,
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    // the anchor alone: the tag takes neither its player's turn nor its scale
    let anchor = mesh_functions::get_world_from_local(vertex.instance_index)[3].xyz;
    let range = distance(view.world_position, anchor);
    // a tag unit covers the same share of the view's height under any camera. Under a
    // perspective clip_from_view[1][1] is 1 / tan(fov / 2) and the view's height grows with the
    // distance, so the tag does too. Under an orthographic one (clip_from_view[3][3] is 1, where
    // a perspective's is 0) it is 2 / the view's height, whatever the distance
    let orthographic = view.clip_from_view[3][3] == 1.0;
    let depth = select(range, 1.0, orthographic);
    let scale = depth * f32(#{NAMETAG_SCALE_MILLIONTHS}) * 1e-6 / view.clip_from_view[1][1];
    let right = view.world_from_view[0].xyz;
    let up = view.world_from_view[1].xyz;
    let world_position = anchor + (right * vertex.position.x + up * vertex.position.y) * scale;
    out.position = view_transformations::position_world_to_clip(world_position);
    out.uv = vertex.uv;
    out.color = vertex.color;
    out.shape = vertex.shape;
    out.radii = vertex.radii;
    let fade_from = f32(#{NAMETAG_FADE_DISTANCE});
    out.opacity = saturate(1.0 - (range - fade_from) / (f32(#{NAMETAG_MAX_DISTANCE}) - fade_from));
    return out;
}

// signed distance to a box of half size `half_size` with a radius per corner, negative inside
fn rounded_box(p: vec2<f32>, half_size: vec2<f32>, radii: vec4<f32>) -> f32 {
    let side = select(radii.zw, radii.xy, p.x > 0.0);
    let radius = select(side.y, side.x, p.y > 0.0);
    let q = abs(p) - half_size + radius;
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2(0.0))) - radius;
}

// four rotated-grid taps: icons are drawn smaller than their texture and have no mips
fn icon_sample(icon: texture_2d<f32>, uv: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>) -> vec4<f32> {
    return (textureSampleGrad(icon, icon_sampler, uv + 0.125 * dx + 0.375 * dy, dx, dy)
        + textureSampleGrad(icon, icon_sampler, uv - 0.375 * dx + 0.125 * dy, dx, dy)
        + textureSampleGrad(icon, icon_sampler, uv + 0.375 * dx - 0.125 * dy, dx, dy)
        + textureSampleGrad(icon, icon_sampler, uv - 0.125 * dx - 0.375 * dy, dx, dy)) * 0.25;
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    // derivatives first, in uniform control flow
    let dx = dpdx(in.uv);
    let dy = dpdy(in.uv);
    // uv units per screen pixel, along the denser axis
    let px = max(max(length(dx), length(dy)), 1e-6);

    let kind = u32(in.shape.x + 0.5);
    var color = in.color;

    if kind == #{NAMETAG_KIND_GLYPH} {
        let dims = vec2<f32>(textureDimensions(sdf_texture));
        color.a *= sdf_layer_alpha(sdf_texture, sdf_sampler, f32(#{NAMETAG_SDF_SPREAD}), in.uv / dims, dx / dims, dy / dims, px, 0.0, 0.0);
    } else if kind == #{NAMETAG_KIND_BITMAP} {
        let dims = vec2<f32>(textureDimensions(bitmap_texture));
        // a colour glyph keeps its own colours: only the tag's opacity applies
        let bitmap = textureSampleGrad(bitmap_texture, bitmap_sampler, in.uv / dims, dx / dims, dy / dims);
        color = vec4(bitmap.rgb, bitmap.a * color.a);
    } else if kind == #{NAMETAG_KIND_ICON} {
        // shape.y: which icon
        let icon = u32(in.shape.y + 0.5);
        if icon == #{NAMETAG_ICON_VERIFIED} {
            color *= icon_sample(verified_texture, in.uv, dx, dy);
        } else if icon == #{NAMETAG_ICON_OFFICIAL} {
            color *= icon_sample(official_texture, in.uv, dx, dy);
        } else {
            color *= icon_sample(hushed_texture, in.uv, dx, dy);
        }
    } else if kind == #{NAMETAG_KIND_RECT} {
        // shape.yz: half size
        color.a *= saturate(0.5 - rounded_box(in.uv, in.shape.yz, in.radii) / px);
    } else {
        // a voice bar. shape.y: half width; shape.zw: the two heights it swings between, one
        // way per period
        let swing = abs(fract(globals.time / (2.0 * f32(#{NAMETAG_VOICE_PERIOD_MS}) * 0.001)) * 2.0 - 1.0);
        let height = mix(in.shape.z, in.shape.w, smoothstep(0.0, 1.0, swing));
        color.a *= saturate(0.5 - rounded_box(in.uv, vec2(in.shape.y, height * 0.5), in.radii) / px);
    }

    color.a *= in.opacity;

    var out: FragmentOutput;
    out.color = color;

#ifdef TRANSPARENT_FOCUS_OUTPUT
    // accumulate alpha-weighted view depth + coverage for depth of field
    out.focus = vec4(
        -view_transformations::depth_ndc_to_view_z(in.position.z),
        1.0,
        0.0,
        out.color.a,
    );
#endif

    return out;
}
