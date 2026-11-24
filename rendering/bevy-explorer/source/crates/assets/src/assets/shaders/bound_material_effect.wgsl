#import bevy_pbr::{
    pbr_types::STANDARD_MATERIAL_FLAGS_EMISSIVE_TEXTURE_BIT,
    mesh_view_bindings::{view, globals},
    prepass_utils::{prepass_depth, prepass_normal},
    pbr_types,
    mesh_functions,
}
#import bevy_render::maths::PI

// an avatar being revealed bottom-up from its ghost (avatar/src/ghost.rs): the tag's low byte is
// the reveal height above the mesh origin (the feet), 0 for no reveal and 1..=255 spanning the
// hide-to-full range, and fragments above it are discarded. the encoding comes from
// scene_material::reveal_shader_defs: a pipeline importing this needs those defs, and one importing
// only the rest of this file (the ghost, shell texturing) gets by without them
#ifdef REVEAL_MESH_TAG_MASK
fn reveal_discard(mesh_tag: u32, world_y: f32, instance_index: u32) -> bool {
    let step = mesh_tag & #{REVEAL_MESH_TAG_MASK};
    if step == 0u {
        return false;
    }
    let hide = f32(#{REVEAL_HIDE_MILLIMETRES}) * 0.001;
    let full = f32(#{REVEAL_FULL_MILLIMETRES}) * 0.001;
    let reveal_height = hide + (full - hide) * f32(step - 1u) / f32(#{REVEAL_MESH_TAG_MASK} - 1u);
    let feet_y = mesh_functions::get_world_from_local(instance_index)[3].y;
    return world_y - feet_y > reveal_height;
}
#endif

#ifdef DEPTH_PREPASS
// the prepass's distance in front of the camera at this pixel, in metres
fn get_depth(pos: vec4<f32>, si: u32) -> f32 {
    let ndc = prepass_depth(pos, si);
    if view.clip_from_view[3].w == 1.0 {
        // orthographic: the depth is linear in the distance
        return (view.clip_from_view[3][2] - ndc) / view.clip_from_view[2][2];
    }
    return view.clip_from_view[3][2] / ndc;
}

// a neighbouring prepass pixel the dither discarded holds whatever lies behind the mesh, so it
// would read as an edge. the prepass ran the same dither on it: the same noise at its pixel, and
// a coverage that barely changes over a few pixels. predict the discard from this fragment's
// opacity and the neighbour's threshold, and use this fragment's value in its place
fn neighbour_dithered(position: vec4<f32>, offset: vec2<f32>, dither_opacity: f32) -> bool {
    // the prepass hashed the neighbour pixel's centre
    return dither_opacity < 1.0 && dither_opacity < dither_threshold(floor(position.xy + offset) + 0.5);
}

fn neighbour_depth(position: vec4<f32>, offset: vec2<f32>, fallback: f32, dither_opacity: f32, si: u32) -> f32 {
    if neighbour_dithered(position, offset, dither_opacity) {
        return fallback;
    }
    return get_depth(position + vec4(offset, 0.0, 0.0), si);
}

#ifdef NORMAL_PREPASS
// a neighbour on another surface (seen through the dither holes, or in front of this fragment)
// can't form a crease with it: the silhouette between them is the nearer surface's depth edge.
// surface_gap_m: the depth difference, in metres, beyond which the neighbour is another surface
fn neighbour_normal(position: vec4<f32>, offset: vec2<f32>, fallback: vec3<f32>, center_depth: f32, surface_gap_m: f32, dither_opacity: f32, si: u32) -> vec3<f32> {
    if neighbour_dithered(position, offset, dither_opacity) {
        return fallback;
    }
    let p = position + vec4(offset, 0.0, 0.0);
    if abs(get_depth(p, si) - center_depth) > surface_gap_m {
        return fallback;
    }
    return prepass_normal(p, si);
}
#endif
#endif

// draws a line on the fragments the prepass shows on an edge: a depth edge (the silhouette)
// becomes outline_rgb, and a crease multiplies the surface by crease.
// width_px: the line's width in pixels; below one pixel the line covers that fraction of the
// pixel, so a thin line fades instead of breaking up
// edge_px: how far behind this fragment, in pixel footprints (the metres a pixel spans at its
// depth), the mean of its diagonal neighbours must lie for a depth edge. A straight edge has half
// of them behind it, so it needs a gap of twice that: a few pixels' worth lines every overlap the
// eye reads as one (a chin over the neck, hair over the face), and stays clear of the depth a
// curved surface gains towards its own silhouette
// dither_opacity: this fragment's discard_dither opacity; 1 when it isn't dithered
fn apply_outline(position: vec4<f32>, color_in: vec4<f32>, outline_rgb: vec3<f32>, crease: vec3<f32>, width_px: f32, edge_px: f32, sample_index: u32, dither_opacity: f32) -> vec4<f32> {
    var out = color_in;
#ifdef DEPTH_PREPASS

    var edge1 = false;
    var edge2 = false;

    let center = get_depth(position, sample_index);
    let d = max(width_px, 1.0);
    let strength = saturate(width_px);
    // the metres a pixel spans at this depth: focal_px is the pixels a metre covers one metre
    // deep, and at any depth in an orthographic view
    let focal_px = view.clip_from_view[1][1] * view.viewport.w * 0.5;
    var pixel_m = 1.0 / focal_px;
    if view.clip_from_view[3].w != 1.0 {
        pixel_m *= center;
    }

    let pxpy = neighbour_depth(position, vec2(d, d), center, dither_opacity, sample_index);
    let pxmy = neighbour_depth(position, vec2(d, -d), center, dither_opacity, sample_index);
    let mxpy = neighbour_depth(position, vec2(-d, d), center, dither_opacity, sample_index);
    let mxmy = neighbour_depth(position, vec2(-d, -d), center, dither_opacity, sample_index);

    let expected = (pxpy + pxmy + mxpy + mxmy) / 4.0;

    if expected - center > edge_px * d * pixel_m {
        edge1 = true;
    }

#ifdef NORMAL_PREPASS
    let nd = 1.0;
    let nmid = prepass_normal(position, sample_index);
    // the gap a straight depth edge one pixel away needs
    let surface_gap_m = 2.0 * edge_px * nd * pixel_m;
    let npxpy = neighbour_normal(position, vec2(nd, nd), nmid, center, surface_gap_m, dither_opacity, sample_index);
    let npxmy = neighbour_normal(position, vec2(nd, -nd), nmid, center, surface_gap_m, dither_opacity, sample_index);
    let nmxpy = neighbour_normal(position, vec2(-nd, nd), nmid, center, surface_gap_m, dither_opacity, sample_index);
    let nmxmy = neighbour_normal(position, vec2(-nd, -nd), nmid, center, surface_gap_m, dither_opacity, sample_index);

    let nexpected = (npxpy + nmxmy) / 2.0;
    let nexpected2 = (npxmy + nmxpy) / 2.0;
    if length(nmid - nexpected) + length(nmid - nexpected2) > 0.4 {
        edge2 = true;
    }
#endif

    if edge1 {
        out = vec4<f32>(mix(out.rgb, outline_rgb, strength), out.a);
    } else if edge2 {
        out = vec4<f32>(mix(out.rgb, out.rgb * crease, strength), out.a);
    }
#endif

return out;
}

// the avatar outline's width in metres at and below OUTLINE_NEAREST_M from the camera
const OUTLINE_WIDTH_M: f32 = 0.002;
// the camera distance at and below which the outline has its full width
const OUTLINE_NEAREST_M: f32 = 0.5;
// the camera distance at and beyond which the outline has no width
const OUTLINE_FARTHEST_M: f32 = 100.0;
// the grey of the outline, taken times the base colour twice: a darker, more saturated tone of the
// surface it borders, not lit
const OUTLINE_COLOR: f32 = 0.6320754;
// the squared camera distance beyond which an avatar has no outline in a perspective view: 8 m
const OUTLINE_MAX_SQR_DISTANCE: f32 = 64.0;
// how far behind a fragment the mean of its four diagonal neighbours must lie for the line, in
// pixel footprints (the metres one step to the neighbours spans at the fragment's depth). A
// straight edge has half its neighbours behind it, so the surface behind needs a gap of three
// footprints: a chin over the neck, an ear or a lock of hair is lined, while a curved surface's
// own rise towards its silhouette stays below it
const OUTLINE_EDGE_PX: f32 = 1.5;

// the avatar outline: a darker, more saturated tone of the surface, base being its base colour,
// that thins with the mesh's distance from the camera and, in a perspective view, stops at the
// cutoff distance. Distances are to the mesh's origin, so a whole mesh gains or loses its outline
// at once. An orthographic view has no cutoff: its camera stands back however large it draws the
// avatar (the avatar preview). A pipeline importing this needs
// scene_material::avatar_style_shader_defs
#ifdef TOON_LIGHT_UNIT_LUX
fn toon_outline(position: vec4<f32>, color_in: vec4<f32>, base: vec3<f32>, instance_index: u32, view_z: f32, sample_index: u32, dither_opacity: f32) -> vec4<f32> {
    let to_camera = mesh_functions::get_world_from_local(instance_index)[3].xyz - view.world_position;
    let sqr_distance = dot(to_camera, to_camera);
    let perspective = view.clip_from_view[3].w != 1.0;
    // past the cutoff a perspective view has no line, and no prepass read
    if perspective && sqr_distance > OUTLINE_MAX_SQR_DISTANCE {
        return color_in;
    }
    // the width in metres falls from full at the nearest distance to none at the farthest, and
    // is projected at this fragment's depth. focal_px is the pixels a metre covers one metre
    // deep, and at any depth in an orthographic view
    let falloff = 1.0 - smoothstep(OUTLINE_NEAREST_M, OUTLINE_FARTHEST_M, sqrt(sqr_distance));
    let focal_px = view.clip_from_view[1][1] * view.viewport.w * 0.5;
    var width_px = OUTLINE_WIDTH_M * falloff * focal_px;
    if perspective {
        width_px /= view_z;
    }
    // the line is not lit: its colour is in the base colour's space, scaled by one light unit to
    // the pre-tonemap level of the lit body (toon.wgsl)
    let unit = bitcast<f32>(#{TOON_LIGHT_UNIT_LUX}u) * view.exposure / PI;
    let outline_rgb = OUTLINE_COLOR * base * base * unit;
    // a crease darkens the surface by the outline's grey
    let crease = vec3(OUTLINE_COLOR);
    return apply_outline(position, color_in, outline_rgb, crease, width_px, OUTLINE_EDGE_PX, sample_index, dither_opacity);
}
#endif

struct Dither {
    // what the pixel's threshold is tested against; 1 or more never discards
    opacity: f32,
    // how faded the fragment is; used to cap brightness so bloom doesn't fill the discarded regions
    transparency: f32,
}

// "interleaved gradient noise" magic numbers
fn dither_threshold(pixel: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(pixel * (1.0 * 5.588238), vec2(0.06711056, 0.00583715))));
}

// avatars fade as a whole rather than per fragment: the dither is taken at the point of their
// (fixed-size) capsule nearest the ray, so every fragment lands on the same opacity
const AVATAR_CAPSULE_RADIUS: f32 = 0.35;
const AVATAR_CAPSULE_HEIGHT: f32 = 2.0;

// the point of the segment a-b nearest the line through o along (unit) d
fn closest_point_on_segment_to_line(a: vec3<f32>, b: vec3<f32>, o: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    let ab = b - a;
    let ao = a - o;
    let abab = dot(ab, ab);
    let abd = dot(ab, d);
    let denom = abab - abd * abd;
    var t = 0.0;
    if denom > 1e-6 {
        t = clamp((abd * dot(d, ao) - dot(ab, ao)) / denom, 0.0, 1.0);
    }
    return a + ab * t;
}

// discards an avatar's fragments between the camera and the player
// base_opacity: the opacity the dither fades towards; 0 fades out fully
// capsule: dither by the avatar capsule standing at capsule_root instead of by the fragment
fn discard_dither(ndc_position: vec2<f32>, world_position: vec3<f32>, depth: f32, base_opacity: f32, capsule_root: vec3<f32>, capsule: bool) -> Dither {
    // player is left of the view forward by 0.25 * clamp(camera distance, 0, 3). we use half of that as our target
    let view_fwd = -view.world_from_view[2].xyz;
    let view_right = view.world_from_view[0].xyz;
    let view_up = view.world_from_view[1].xyz;
    let target_offset = depth * view_fwd - 0.125 * clamp(depth, 0.0, 3.0) * view_right; 

    let view_direction = normalize(target_offset);

    var position = world_position;
    var radius = 0.0;
    if capsule {
        position = closest_point_on_segment_to_line(
            capsule_root + vec3(0.0, AVATAR_CAPSULE_RADIUS, 0.0),
            capsule_root + vec3(0.0, AVATAR_CAPSULE_HEIGHT - AVATAR_CAPSULE_RADIUS, 0.0),
            view.world_position,
            view_direction,
        );
        radius = AVATAR_CAPSULE_RADIUS;
    }

    let view_to_frag = position - view.world_position;
    let projection_length = dot(view_to_frag, view_direction);

    if projection_length < depth + 0.35 { // 0.35 = collider radius
        let cone_distance = 
            // distance from ray to target
            max(0.0, length(position - (view.world_position.xyz + (view_direction * projection_length))) - radius)
            // shrink to cone, 0 radius at view position, 1 at target position
            / (projection_length / depth) 
            // expand cone at further camera distances
            / (1.0 + max(0.0, (projection_length - depth)) / 10.0);

        let opacity = cone_distance
            // add opacity
            + 0.5 
            // remove further from target
            - saturate((depth - projection_length) * 0.5) * 1.0 
            // add behind target
            + saturate(projection_length - depth) * 5.0;

        let result = Dither(max(0.1, base_opacity + opacity * (1.0 - base_opacity)), 1.0 - saturate(cone_distance));
        if result.opacity < dither_threshold(ndc_position) {
            discard;
        }
        return result;
    }

    return Dither(1.0, 0.0);
}

// the camera-to-player dither of a tagged mesh, for the scene material's passes and the face
// masks alike. Only avatars carry the cone-only tag and fade between the camera and the player;
// scene meshes stay opaque, as the player camera collides with them instead, and the
// no-dithering tag skips it.
// instance_index: the mesh, whose origin is the capsule's root
// A pipeline importing this needs scene_material::avatar_style_shader_defs
#ifdef NO_DITHERING_MESH_TAG
fn camera_dither(mesh_tag: u32, frag_coord: vec2<f32>, world_position: vec3<f32>, instance_index: u32, base_opacity: f32, capsule: bool) -> Dither {
    if (mesh_tag & #{NO_DITHERING_MESH_TAG}) != 0u || (mesh_tag & #{CONE_ONLY_DITHER_MESH_TAG}) == 0u {
        return Dither(1.0, 0.0);
    }
    return discard_dither(frag_coord, world_position, view.user_value, base_opacity, mesh_functions::get_world_from_local(instance_index)[3].xyz, capsule);
}
#endif
