// The ghost avatar: the hologram of the base body shown while an avatar's files load
// (avatar/src/ghost.rs). Unlit and transparent: a fresnel rim with banded brightness, scanlines
// scrolling up the body, a flicker and a horizontal glitch jitter, all procedural. A reveal plane
// clips it, so it appears bottom-up, then hands over to the wearables bottom-up
// (bound_material_effect.wgsl's reveal_discard clips those to the same plane). The body is
// skinned, so the vertex stage skins like bevy's mesh.wgsl.

#import bevy_pbr::{
    forward_io::{Vertex, VertexOutput, FragmentOutput},
    mesh_functions,
    mesh_view_bindings::{view, globals},
    skinning,
    view_transformations,
}
#import "embedded://shaders/bound_material_effect.wgsl"::discard_dither

struct GhostMaterial {
    // the player's name colour
    color: vec4<f32>,
    // x: the reveal height above the feet, in metres; y: 1 shows the ghost below that height,
    // -1 above it; z: 1 fades the ghost near the camera like an avatar
    reveal: vec4<f32>,
}

@group(2) @binding(0)
var<uniform> material: GhostMaterial;

// --- baked params (edit + rebuild to tune) ---
const FRESNEL_POWER: f32 = 1.0;
// coarse bands of rim brightness up the body, per metre
const FRESNEL_BAND_DENSITY: f32 = 4.0;
// the glow, relative to the brightness avatars are lit at (toon.wgsl)
const EMISSION_INTENSITY: f32 = 3.0;
const BRIGHTNESS: f32 = 2000.0;
// scanlines per metre, and how fast they scroll (negative: downwards)
const SCANLINE_TILING: f32 = 5.0;
const SCANLINE_SPEED: f32 = -1.0;
const SCANLINE_ALPHA: f32 = 0.5;
const SCANLINE_NOISE_MIN: f32 = 0.4;
const FLICKER_SPEED: f32 = 0.8;
// horizontal glitch: bands per metre, how often they change, how many shift and by how far
const GLITCH_DENSITY: f32 = 12.0;
const GLITCH_SPEED: f32 = 6.0;
const GLITCH_THRESHOLD: f32 = 0.85;
const GLITCH_INTENSITY: f32 = 0.03;
// the camera-to-player dither's base opacity for avatars (avatar/src/lib.rs)
const DITHER_BASE_OPACITY: f32 = 0.5;

fn hash(x: f32) -> f32 {
    return fract(sin(x) * 43758.5453);
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let mesh_world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
#ifdef SKINNED
    let world_from_local = skinning::skin_model(vertex.joint_indices, vertex.joint_weights, vertex.instance_index);
#else
    let world_from_local = mesh_world_from_local;
#endif
    var world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));

    // the mesh node's origin is at the feet
    let height = world_position.y - mesh_world_from_local[3].y;
    let band = floor(height * GLITCH_DENSITY);
    let seed = hash(band * 127.1 + floor(globals.time * GLITCH_SPEED));
    world_position.x += step(GLITCH_THRESHOLD, seed) * (seed - 0.5) * GLITCH_INTENSITY;

    out.world_position = world_position;
    out.position = view_transformations::position_world_to_clip(world_position.xyz);
#ifdef VERTEX_NORMALS
#ifdef SKINNED
    out.world_normal = skinning::skin_normals(world_from_local, vertex.normal);
#else
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#endif
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif
    return out;
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    let origin = mesh_functions::get_world_from_local(in.instance_index)[3].xyz;
    let height = in.world_position.y - origin.y;

    if material.reveal.y * (material.reveal.x - height) < 0.0 {
        discard;
    }

    var faded = 0.0;
    if material.reveal.z != 0.0 {
        let dither = discard_dither(in.position.xy, in.world_position.xyz, view.user_value, false, DITHER_BASE_OPACITY, origin, true);
        faded = dither.transparency;
    }

    let view_dir = normalize(view.world_position - in.world_position.xyz);
    let normal = normalize(in.world_normal);
    let base_fresnel = pow(1.0 - saturate(dot(view_dir, normal)), FRESNEL_POWER);

    let band_brightness = hash(floor(height * FRESNEL_BAND_DENSITY) * 91.7);
    let banded_fresnel = base_fresnel * band_brightness * band_brightness;

    // each scanline keeps its own intensity as it scrolls
    let scan = height * SCANLINE_TILING + globals.time * SCANLINE_SPEED;
    let scan_noise = hash(floor(height * SCANLINE_TILING) * 153.7);
    let scanline = smoothstep(0.3, 0.7, fract(scan))
        * SCANLINE_ALPHA * (SCANLINE_NOISE_MIN + scan_noise * (1.0 - SCANLINE_NOISE_MIN));

    let flicker = hash(floor(globals.time * FLICKER_SPEED) * 127.1);
    let intensity = EMISSION_INTENSITY * (0.9 + flicker * 0.1) * BRIGHTNESS * view.exposure;

    var out: FragmentOutput;
    let glow = base_fresnel + banded_fresnel + scanline;
    let alpha = saturate(base_fresnel + banded_fresnel * 0.5 + scanline * 0.5) * material.color.a;
    // dim the glow as the dither fades the ghost, so bloom doesn't fill the discarded pixels
    out.color = vec4<f32>(material.color.rgb * glow * intensity * (1.0 - faded), alpha);

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
