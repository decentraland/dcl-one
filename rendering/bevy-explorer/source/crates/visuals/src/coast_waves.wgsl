// The sea's vertex stage: Unity's Stylized Water 2 wave sum
// (unity:Explorer/Library/PackageCache/decentraland.rendergraphs@*/Ocean/Shaders/Libraries/Waves.hlsl,
// values from unity:Explorer/Assets/DCL/Landscape/Assets/Materials/Ocean.mat) displaces the
// surface, fading out with distance from the camera. The constants come from coast/waves.rs, whose
// `wave_displacement` is the CPU twin of `waves` below.

#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
    forward_io::{Vertex, VertexOutput},
}

struct WaveUniform {
    land_bounds: vec4<f32>,
    fade: vec2<f32>,
    camera_xz: vec2<f32>,
    time: f32,
}

@group(2) @binding(100) var<uniform> wave: WaveUniform;

struct Waves {
    // unity x offset, height, unity z offset
    offset: vec3<f32>,
    // d(height)/d(unity x), d(height)/d(unity z)
    slope: vec2<f32>,
}

fn f(bits: u32) -> f32 {
    return bitcast<f32>(bits);
}

// `GetWaveInfo` at `p` (unity x/z), with the distance `fade` Waves.hlsl multiplies into the
// amplitudes on every iteration
fn waves(p: vec2<f32>, fade: f32) -> Waves {
    let count = #{WAVE_COUNT}u;
    let height = f(#{WAVE_HEIGHT}u);
    // `TIME_VERTEX * _WaveSpeed`, with TIME_VERTEX = time * -_Direction (Common.hlsl)
    let time = -vec2(f(#{WAVE_DIRECTION_X}u), f(#{WAVE_DIRECTION_Y}u)) * wave.time
        * f(#{WAVE_SPEED}u);
    let speed = vec4(1.2 * time.x, 1.375 * time.y, 1.1 * time.x, time.y);
    // `lerp(1, MAX_WAVE_COUNT, 1 / WAVE_COUNT)` divides two uints, so it is 1
    let steepness = 12.0 * f(#{WAVE_STEEPNESS}u);
    let dir1 = vec4(f(#{WAVE_DIR1_X}u), f(#{WAVE_DIR1_Y}u), f(#{WAVE_DIR1_Z}u), f(#{WAVE_DIR1_W}u));
    let dir2 = vec4(f(#{WAVE_DIR2_X}u), f(#{WAVE_DIR2_Y}u), f(#{WAVE_DIR2_Z}u), f(#{WAVE_DIR2_W}u));
    var amp = vec4(f(#{WAVE_AMP_X}u), f(#{WAVE_AMP_Y}u), f(#{WAVE_AMP_Z}u), f(#{WAVE_AMP_W}u));
    var freq = vec4(f(#{WAVE_FREQ_X}u), f(#{WAVE_FREQ_Y}u), f(#{WAVE_FREQ_Z}u), f(#{WAVE_FREQ_W}u))
        * (1.0 - f(#{WAVE_DISTANCE}u)) * 3.0;
    var out: Waves;
    for (var i = 0u; i <= count; i++) {
        freq *= 1.0 + f32(i) / f32(count);
        amp *= fade;
        let phase = freq * vec4(dot(dir1.xy, p), dot(dir1.zw, p), dot(dir2.xy, p), dot(dir2.zw, p))
            + speed;
        let c = cos(phase);
        // GerstnerOffset4
        let ab = steepness * dir1 * amp.xxyy;
        let cd = steepness * dir2 * amp.zzww;
        out.offset.x += dot(c, vec4(ab.xz, cd.xz));
        out.offset.z += dot(c, vec4(ab.yw, cd.yw));
        out.offset.y += dot(sin(phase), amp);
        // the analytic derivative of the height term: d/dp of amp * sin(freq * dot(dir, p))
        let k = c * amp * freq;
        out.slope += k.x * dir1.xy + k.y * dir1.zw + k.z * dir2.xy + k.w * dir2.zw;
    }
    let scale = height / f32(count);
    out.offset.y *= scale;
    out.slope *= scale;
    out.offset.x *= 0.01 * height;
    out.offset.z *= 0.01 * height;
    return out;
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    var world_position = mesh_functions::mesh_position_local_to_world(world_from_local,
        vec4(vertex.position, 1.0));
    // Waves.hlsl's `fadeFactor`
    let distance = length(wave.camera_xz - world_position.xz);
    let fade = saturate((wave.fade.y - distance) / (wave.fade.y - wave.fade.x));
    // bevy z is -unity z
    let w = waves(world_position.xz * vec2(1.0, -1.0), fade);
    world_position += vec4(w.offset.x, w.offset.y, -w.offset.z, 0.0);
    out.world_position = world_position;
    out.position = position_world_to_clip(world_position.xyz);
    out.world_normal = normalize(vec3(-w.slope.x, 1.0, w.slope.y));
#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex.instance_index, world_from_local[3]);
#endif
    return out;
}
