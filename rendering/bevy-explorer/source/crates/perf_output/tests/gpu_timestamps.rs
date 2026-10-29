//! Whether two compute passes that hold no work still carry GPU timestamps, so that a pair of
//! them can bracket a frame's GPU work and give its duration.
//!
//! A bracket is a compute pass with a pass-level timestamp write: one at the start of the first
//! pass (`beginning_of_pass_write_index`), one at the end of the last (`end_of_pass_write_index`).
//! The test submits that first pass, a compute pass that does measurable work and the last pass,
//! resolves the two timestamps and reads them back. A slot no pass wrote reads back as 0 on DX12;
//! on Vulkan the resolve waits on it until wgpu times out, after about a minute, and then both
//! slots read 0. So the test asserts that:
//!
//! - both slots are non-zero, naming the pass whose slot is not;
//! - the second is greater than the first;
//! - the time between them, converted with `Queue::get_timestamp_period`, is over 0.05 ms (the
//!   work's time, which the empty passes alone do not reach) and under 1 s (a slot holding some
//!   other value would not land that close to the other one).
//!
//! Together these show that each empty pass wrote a non-zero timestamp, that the second is later
//! than the first and that the span between them is as long as the work needs and under a second.
//! A slot left holding a stale value that stays inside that range would pass: the test cannot tell
//! it from a written one.
//!
//! It needs a GPU with `TIMESTAMP_QUERY`, so it is `#[ignore]`d and CI skips it. Run it per
//! backend (`WGPU_BACKEND` is honoured by the instance the test creates):
//!
//! ```text
//! WGPU_BACKEND=dx12   cargo test -p perf_output --test gpu_timestamps -- --ignored --nocapture
//! WGPU_BACKEND=vulkan cargo test -p perf_output --test gpu_timestamps -- --ignored --nocapture
//! ```
//!
//! The bracket passes are empty here, the cheapest form. A backend that drops an encoder with no
//! work leaves its slot unwritten, which fails the non-zero assert; the fallback is one 1x1x1
//! dispatch of a shader that does nothing in each bracket pass.

use bevy::tasks::block_on;
use wgpu::{
    util::DeviceExt, BindGroupDescriptor, BindGroupEntry, Buffer, BufferDescriptor, BufferUsages,
    CommandBuffer, CommandEncoderDescriptor, ComputePassDescriptor, ComputePassTimestampWrites,
    ComputePipelineDescriptor, DeviceDescriptor, Features, InstanceDescriptor, Maintain, MapMode,
    PowerPreference, QuerySet, QuerySetDescriptor, QueryType, RequestAdapterOptions,
    ShaderModuleDescriptor, ShaderSource,
};

/// Workgroups of 64 invocations dispatched by the work pass.
const WORK_GROUPS: (u32, u32) = (64, 64);

/// A time between the two timestamps, in nanoseconds, at or under this means the work pass was not
/// timed: the dispatch runs 262144 invocations of 4096 dependent steps each.
const MIN_WORK_NS: f64 = 0.05e6;

/// A time between the two timestamps, in nanoseconds, at or over this means a slot holds something
/// other than this submission's timestamp: the work takes well under a millisecond.
const MAX_SPAN_NS: f64 = 1e9;

/// Every invocation runs a dependent integer loop the compiler cannot fold and stores the result,
/// so the pass takes a measurable time on any GPU.
const WORK_SHADER: &str = "
@group(0) @binding(0) var<storage, read_write> sink: array<u32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    var x = id.x * 747796405u + id.y;
    for (var i = 0u; i < 4096u; i = i + 1u) {
        x = x * 1664525u + 1013904223u;
        x = x ^ (x >> 13u);
    }
    sink[id.y * groups.x * 64u + id.x] = x;
}
";

/// A command buffer holding one compute pass that does nothing but write a timestamp.
fn bracket(
    device: &wgpu::Device,
    label: &str,
    query_set: &QuerySet,
    beginning_of_pass_write_index: Option<u32>,
    end_of_pass_write_index: Option<u32>,
) -> CommandBuffer {
    let mut encoder =
        device.create_command_encoder(&CommandEncoderDescriptor { label: Some(label) });
    drop(encoder.begin_compute_pass(&ComputePassDescriptor {
        label: Some(label),
        timestamp_writes: Some(ComputePassTimestampWrites {
            query_set,
            beginning_of_pass_write_index,
            end_of_pass_write_index,
        }),
    }));
    encoder.finish()
}

/// Reads `buffer` back (blocking, which this test alone may do) as little-endian `u64`s.
fn read_u64s(device: &wgpu::Device, buffer: &Buffer) -> Vec<u64> {
    let slice = buffer.slice(..);
    slice.map_async(MapMode::Read, |result| {
        result.expect("map the readback buffer")
    });
    device.poll(Maintain::Wait);
    let bytes = slice.get_mapped_range();
    let values = bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|chunk| u64::from_le_bytes(*chunk))
        .collect();
    drop(bytes);
    buffer.unmap();
    values
}

/// The outcome per backend, on an NVIDIA GeForce RTX 5080 Laptop GPU (Windows 11):
///
/// - DX12: passes with empty bracket passes (about 0.24 ms measured around the work, a period of
///   1 ns per tick); no dispatch needed.
/// - Vulkan: passes with empty bracket passes (about 0.24 ms, 1 ns per tick); no dispatch needed.
/// - Metal: not run here; the apple-m1 run of the fleet covers it.
/// - WebGPU: not run here; the browser run of the explorer covers it.
///
/// With the work pass cut to one workgroup of one loop step, the same two empty passes measure
/// 0.004 ms on DX12 and 0.018 ms on Vulkan and the test fails, so the time above 0.05 ms is the
/// work's and not the passes' own.
#[test]
#[ignore = "needs a GPU with TIMESTAMP_QUERY; run with --ignored"]
fn bracket_ticks_increase_on_this_adapter() {
    let instance = wgpu::Instance::new(&InstanceDescriptor::from_env_or_default());
    let adapter = block_on(instance.request_adapter(&RequestAdapterOptions {
        power_preference: PowerPreference::from_env().unwrap_or(PowerPreference::HighPerformance),
        ..Default::default()
    }))
    .expect("an adapter on the selected backend");
    let info = adapter.get_info();
    println!("adapter: {} ({:?})", info.name, info.backend);
    assert!(
        adapter.features().contains(Features::TIMESTAMP_QUERY),
        "{} ({:?}) does not report TIMESTAMP_QUERY",
        info.name,
        info.backend
    );
    let (device, queue) = block_on(adapter.request_device(
        &DeviceDescriptor {
            label: Some("gpu_timestamps"),
            required_features: Features::TIMESTAMP_QUERY,
            ..Default::default()
        },
        None,
    ))
    .expect("a device with TIMESTAMP_QUERY");

    let query_set = device.create_query_set(&QuerySetDescriptor {
        label: Some("bracket"),
        ty: QueryType::Timestamp,
        count: 2,
    });
    let resolved = device.create_buffer(&BufferDescriptor {
        label: Some("resolved"),
        size: 2 * 8,
        usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&BufferDescriptor {
        label: Some("readback"),
        size: 2 * 8,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let sink = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("sink"),
        contents: &vec![0u8; (WORK_GROUPS.0 * 64 * WORK_GROUPS.1 * 4) as usize],
        usage: BufferUsages::STORAGE,
    });
    let module = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("work"),
        source: ShaderSource::Wgsl(WORK_SHADER.into()),
    });
    let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("work"),
        layout: None,
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("work"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[BindGroupEntry {
            binding: 0,
            resource: sink.as_entire_binding(),
        }],
    });

    let first = bracket(&device, "first bracket", &query_set, Some(0), None);

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("work"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("work"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(WORK_GROUPS.0, WORK_GROUPS.1, 1);
    }
    let work = encoder.finish();

    let last = bracket(&device, "last bracket", &query_set, None, Some(1));

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("resolve"),
    });
    encoder.resolve_query_set(&query_set, 0..2, &resolved, 0);
    encoder.copy_buffer_to_buffer(&resolved, 0, &readback, 0, 2 * 8);
    let resolve = encoder.finish();

    queue.submit([first, work, last, resolve]);

    let ticks = read_u64s(&device, &readback);
    let (begin, end) = (ticks[0], ticks[1]);
    let period_ns = queue.get_timestamp_period();
    let elapsed_ns = end.wrapping_sub(begin) as f64 * period_ns as f64;
    println!(
        "begin {begin} end {end} ticks {} period {period_ns} ns/tick: {:.3} ms around the work",
        end.wrapping_sub(begin),
        elapsed_ns / 1e6
    );

    assert!(
        begin != 0,
        "the first bracket pass wrote no timestamp on {:?} (slot 0 reads 0, end {end})",
        info.backend
    );
    assert!(
        end != 0,
        "the last bracket pass wrote no timestamp on {:?} (slot 1 reads 0, begin {begin})",
        info.backend
    );
    assert!(
        end > begin,
        "the timestamps did not increase (begin {begin}, end {end}) on {:?}",
        info.backend
    );
    assert!(
        elapsed_ns > MIN_WORK_NS,
        "{:.3} ms around the work is not over {:.3} ms (period {period_ns} ns/tick)",
        elapsed_ns / 1e6,
        MIN_WORK_NS / 1e6
    );
    assert!(
        elapsed_ns < MAX_SPAN_NS,
        "{:.3} ms around the work is not under {:.3} ms (period {period_ns} ns/tick): a slot holds a value from outside this submission",
        elapsed_ns / 1e6,
        MAX_SPAN_NS / 1e6
    );
}
