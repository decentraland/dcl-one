// The jump landing marker (avatar/src/jump_marker.rs): a soft black blob on a ground-aligned quad.
// Unlit and transparent. The opacity and the blob's scale follow the player's distance to the
// ground through two ramps each; the constants come from Rust as shader defs.

#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    view_transformations,
}

struct JumpMarkerMaterial {
    // the distance from the player down to the ground, in metres
    ground_distance: f32,
}

@group(2) @binding(0)
var<uniform> material: JumpMarkerMaterial;

// smoothstep over [lo, hi]; a degenerate range is a step at `lo`
fn ramp(lo: f32, hi: f32, x: f32) -> f32 {
    if hi <= lo {
        return select(0.0, 1.0, x >= lo);
    }
    return smoothstep(lo, hi, x);
}

// one over the first pair of distances, back to zero over the second
fn rise_and_fall(distances: vec4<f32>, x: f32) -> f32 {
    return ramp(distances.x, distances.y, x) - ramp(distances.z, distances.w, x);
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    let opacity_range = vec2<f32>(
        bitcast<f32>(#{OPACITY_RANGE_0}u),
        bitcast<f32>(#{OPACITY_RANGE_1}u),
    );
    let opacity_distances = vec4<f32>(
        bitcast<f32>(#{OPACITY_DISTANCES_0}u),
        bitcast<f32>(#{OPACITY_DISTANCES_1}u),
        bitcast<f32>(#{OPACITY_DISTANCES_2}u),
        bitcast<f32>(#{OPACITY_DISTANCES_3}u),
    );
    let scale_range = vec2<f32>(
        bitcast<f32>(#{HEIGHT_SCALE_RANGE_0}u),
        bitcast<f32>(#{HEIGHT_SCALE_RANGE_1}u),
    );
    let scale_distances = vec4<f32>(
        bitcast<f32>(#{HEIGHT_SCALE_DISTANCES_0}u),
        bitcast<f32>(#{HEIGHT_SCALE_DISTANCES_1}u),
        bitcast<f32>(#{HEIGHT_SCALE_DISTANCES_2}u),
        bitcast<f32>(#{HEIGHT_SCALE_DISTANCES_3}u),
    );
    let inner_radius = bitcast<f32>(#{INNER_RADIUS}u);
    let outer_radius = bitcast<f32>(#{OUTER_RADIUS}u);
    let fade_factor = bitcast<f32>(#{FADE_FACTOR}u);

    // the projector's fade factor scales the material's opacity
    let opacity = fade_factor * mix(opacity_range.x, opacity_range.y, rise_and_fall(opacity_distances, material.ground_distance));
    // small near the ground, growing with the distance
    let scale = mix(scale_range.x, scale_range.y, 1.0 - rise_and_fall(scale_distances, material.ground_distance));

    // the quad's uv, centred: 1 at the edge of the footprint along each axis
    let radius = saturate(length(in.uv * 2.0 - 1.0) / max(scale, 0.00001));
    let blob = 1.0 - smoothstep(inner_radius, outer_radius, radius);

    var out: FragmentOutput;
    out.color = vec4<f32>(0.0, 0.0, 0.0, opacity * blob);

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
