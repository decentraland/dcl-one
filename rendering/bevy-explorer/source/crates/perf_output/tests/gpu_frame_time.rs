//! The GPU frame time end to end on this machine's adapter, in a renderer with no window: where
//! the device has timestamp queries the plugin measures the GPU work of the frame's render graph
//! and not the render world's CPU time; where it has none it logs one line and registers nothing.
//!
//! Both need a GPU, so they are `#[ignore]`d and CI skips them. `WGPU_BACKEND` picks the backend:
//!
//! ```text
//! WGPU_BACKEND=dx12   cargo test -p perf_output --test gpu_frame_time -- --ignored --nocapture
//! WGPU_BACKEND=vulkan cargo test -p perf_output --test gpu_frame_time -- --ignored --nocapture
//! ```

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use bevy::{
    diagnostic::DiagnosticsStore,
    log::{
        tracing::{self, field::Field, Event, Level, Subscriber},
        tracing_subscriber::{layer::Context, prelude::*, registry, Layer},
        LogPlugin,
    },
    prelude::*,
    render::{
        render_graph::{Node, NodeRunError, RenderGraph, RenderGraphContext, RenderLabel},
        renderer::{RenderContext, RenderDevice},
        settings::{RenderCreation, WgpuFeatures, WgpuSettings},
        Render, RenderApp, RenderPlugin, RenderSet,
    },
    window::ExitCondition,
    winit::WinitPlugin,
};
use common::gpu_time::GPU_FRAME_TIME;
use perf_output::gpu::GpuTimePlugin;

/// Held by each test while its renderer exists: two Vulkan devices created at once in one process
/// sometimes fail to create.
static ONE_RENDERER: Mutex<()> = Mutex::new(());

/// A renderer with no window, `work` if given and then the plugin, finished and ready to update.
fn app(settings: WgpuSettings, work: Option<WorkAndStallPlugin>) -> App {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .build()
            .disable::<LogPlugin>()
            .disable::<WinitPlugin>()
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                close_when_requested: false,
            })
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(settings),
                ..default()
            }),
    );
    if let Some(work) = work {
        app.add_plugins(work);
    }
    app.add_plugins(GpuTimePlugin);
    app.finish();
    app.cleanup();
    app
}

fn measurements(app: &App) -> Option<Vec<f64>> {
    app.world()
        .resource::<DiagnosticsStore>()
        .get(&GPU_FRAME_TIME)
        .map(|diagnostic| diagnostic.values().copied().collect())
}

/// Each invocation runs a dependent integer loop and stores the result (the work shader of
/// `tests/gpu_timestamps.rs`): about 0.24 ms over [`WORK_GROUPS`] on an RTX 5080 Laptop GPU.
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

/// Workgroups of 64 invocations the work node dispatches each frame.
const WORK_GROUPS: (u32, u32) = (64, 64);

/// A sample at or under this, in milliseconds, did not cover the work node's dispatch.
const MIN_WORK_MS: f64 = 0.05;

/// The render world sleeps this long each frame in `Prepare`, before the frame is submitted,
/// while the GPU has nothing to do. A sample that covers it is at least this long.
const CPU_STALL: Duration = Duration::from_millis(50);

/// A sample at or over this, in milliseconds, covers the render world's stall. Half the stall,
/// and far above the work node's dispatch even on a slow integrated GPU.
const MAX_WORK_MS: f64 = 25.0;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct WorkLabel;

/// A render-graph node that dispatches the work shader.
struct WorkNode {
    pipeline: wgpu::ComputePipeline,
    bind_group: wgpu::BindGroup,
}

impl Node for WorkNode {
    fn run<'w>(
        &self,
        _: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        _: &'w World,
    ) -> Result<(), NodeRunError> {
        let mut pass =
            render_context
                .command_encoder()
                .begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("work"),
                    timestamp_writes: None,
                });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.dispatch_workgroups(WORK_GROUPS.0, WORK_GROUPS.1, 1);
        Ok(())
    }
}

/// Every frame: [`CPU_STALL`] in the render world's `Prepare`, and a render-graph node that keeps
/// the GPU busy. Added before [`GpuTimePlugin`], like the app's own plugins that add nodes.
struct WorkAndStallPlugin;

impl Plugin for WorkAndStallPlugin {
    fn build(&self, app: &mut App) {
        app.sub_app_mut(RenderApp).add_systems(
            Render,
            (|| std::thread::sleep(CPU_STALL)).in_set(RenderSet::Prepare),
        );
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        let device = render_app.world().resource::<RenderDevice>().wgpu_device();
        let sink = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sink"),
            size: u64::from(WORK_GROUPS.0 * 64 * WORK_GROUPS.1 * 4),
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("work"),
            source: wgpu::ShaderSource::Wgsl(WORK_SHADER.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("work"),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("work"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: sink.as_entire_binding(),
            }],
        });
        render_app
            .world_mut()
            .resource_mut::<RenderGraph>()
            .add_node(
                WorkLabel,
                WorkNode {
                    pipeline,
                    bind_group,
                },
            );
    }
}

/// Every event logged on this thread under one target, with its level and message.
#[derive(Clone, Default)]
struct Lines(Arc<Mutex<Vec<(Level, String)>>>);

impl<S: Subscriber> Layer<S> for Lines {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        if event.metadata().target() != "gpu_time" {
            return;
        }
        struct Message(String);
        impl tracing::field::Visit for Message {
            fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.0 = format!("{value:?}");
                }
            }
        }
        let mut message = Message(String::new());
        event.record(&mut message);
        self.0
            .lock()
            .unwrap()
            .push((*event.metadata().level(), message.0));
    }
}

#[test]
#[ignore = "needs a GPU with TIMESTAMP_QUERY; run with --ignored"]
fn the_frame_s_gpu_work_is_measured_and_the_render_world_s_cpu_time_is_not() {
    let _renderer = ONE_RENDERER.lock().unwrap_or_else(|e| e.into_inner());
    let mut app = app(WgpuSettings::default(), Some(WorkAndStallPlugin));
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut values = Vec::new();
    while values.len() < 10 && Instant::now() < deadline {
        app.update();
        values = measurements(&app).expect("the path is registered with timestamp queries");
    }
    println!("{} samples (ms): {values:?}", values.len());
    assert!(values.len() >= 10, "only {} samples in 30 s", values.len());
    assert!(
        values.iter().all(|ms| *ms > MIN_WORK_MS),
        "a sample does not cover the work node's dispatch: {values:?}"
    );
    assert!(
        values.iter().all(|ms| *ms < MAX_WORK_MS),
        "a sample covers the render world's {CPU_STALL:?} stall: {values:?}"
    );
}

#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn an_adapter_without_timestamp_queries_logs_once_and_registers_nothing() {
    let _renderer = ONE_RENDERER.lock().unwrap_or_else(|e| e.into_inner());
    let lines = Lines::default();
    let subscriber = registry().with(lines.clone());
    tracing::subscriber::with_default(subscriber, || {
        let mut app = app(
            WgpuSettings {
                disabled_features: Some(WgpuFeatures::TIMESTAMP_QUERY),
                ..default()
            },
            None,
        );
        for _ in 0..30 {
            app.update();
        }
        assert_eq!(measurements(&app), None);
    });
    assert_eq!(
        *lines.0.lock().unwrap(),
        [(
            Level::INFO,
            "GPU frame time unavailable: the adapter has no timestamp queries".to_owned()
        )]
    );
}
