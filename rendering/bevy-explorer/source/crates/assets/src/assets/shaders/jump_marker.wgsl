// The jump landing marker (avatar/src/jump_marker): a soft black blob projected down a box under
// the player onto the surface the depth prepass holds, so it follows slopes, steps and ledges.
// Unlit and transparent. The opacity comes from the material; the blob's scale is the box's.
// The pipeline draws the box's far faces with no depth test; without a depth prepass there is
// no surface to project onto and nothing is drawn.

#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    mesh_functions::get_local_from_world,
    prepass_utils,
    view_transformations::{depth_ndc_to_view_z, frag_coord_to_ndc, position_ndc_to_world},
}

struct JumpMarkerMaterial {
    // the blob's opacity at its centre
    opacity: f32,
}

@group(2) @binding(0)
var<uniform> material: JumpMarkerMaterial;

// the blob is opaque inside the inner radius and fades out to the outer one, as fractions of
// the box's half extent across
const INNER_RADIUS: f32 = 0.35;
const OUTER_RADIUS: f32 = 1.0;

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var alpha = 0.0;
    var surface_depth = in.position.z;

#ifdef DEPTH_PREPASS
    // the surface already drawn at this pixel, in the box's own space: a unit cube centred on 0
    surface_depth = prepass_utils::prepass_depth(in.position, 0u);
    let world_position = position_ndc_to_world(
        frag_coord_to_ndc(vec4(in.position.xy, surface_depth, 1.0)),
    );
    let local = (get_local_from_world(in.instance_index) * vec4(world_position, 1.0)).xyz;
    if all(abs(local) <= vec3(0.5)) {
        // 1 at the edge of the footprint along each axis across
        let radius = length(local.xz * 2.0);
        alpha = material.opacity * (1.0 - smoothstep(INNER_RADIUS, OUTER_RADIUS, radius));
    }
#endif

    if alpha <= 0.0 {
        discard;
    }

    var out: FragmentOutput;
    out.color = vec4<f32>(0.0, 0.0, 0.0, alpha);

#ifdef TRANSPARENT_FOCUS_OUTPUT
    // accumulate alpha-weighted view depth + coverage for depth of field, at the surface
    out.focus = vec4(
        -depth_ndc_to_view_z(surface_depth),
        1.0,
        0.0,
        out.color.a,
    );
#endif

    return out;
}
