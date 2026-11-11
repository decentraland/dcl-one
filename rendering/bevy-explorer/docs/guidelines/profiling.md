# Profiling

What the explorer measures about itself, where to read it and how to go deeper. The code is
`source/src/perf/` (the sampler, the frame-time breakdown, the scope profiler, the issue log, the
benchmark summary), the shared types `source/crates/common/src/perf.rs`, the debug panel sections
`source/src/debug_panel.rs`. The benchmark (`benchmark.md`) is the A/B measurement of pull
requests; this page is the live measurement of one running client.

## Readings in the debug panel

Open the [runtime debug panel](running.md#runtime-debug-panel) and expand a section. Every
reading is also a console command that returns JSON, so a driver or the e2e suite reads the same
numbers.

### Performance (`/performance_state`)

- **Sampling.** Frames are sampled only while the player is in world, the loading screen is
  hidden and the window is focused. `sampling.status` is `sampling` or `paused`, and
  `sampling.pausedReason` says why: `not_in_world`, `loading_screen` or `window_unfocused`.
  Paused time never counts, and the frame on which sampling resumes is dropped, since its delta
  spans the pause.
- **Snapshots.** Every 15 s of sampled time one `PerformanceSnapshot` is taken: the CPU frame
  times (`Time<Real>` deltas, unclamped) and the GPU frame times (`gpu/frame_time`, where the
  adapter has timestamp queries) as a summary each: count, min, mean, p1 to p99, max, and the
  frames over the hiccup threshold with their time sum and their excess over the threshold.
  `inProgressSample.sampledSeconds` counts up to the next one. The snapshot also carries the
  engine memory and its peak during the sample, the scenes' JavaScript heaps summed, the nearby
  player count, the entity count and, with the profiler on, the top 30 scopes.
- **Hiccup threshold.** The larger of 50 ms and two target frame times: 50 ms at 60 FPS and
  above, 66.7 ms at a 30 FPS cap, 50 ms with vsync or uncapped. The target is the session
  override (`/session_graphics fps`), else the configured one.
- **Frame time breakdown.** Where the previous frame's wall time went, smoothed: the main world's
  schedules (`First` through `Last`), the scene loop until its deadline, the frame limiter's sleep
  (native), the render world's CPU time without the swapchain wait, the swapchain wait itself
  (vsync shows here) and the GPU time. The parts are measured separately and overlap in part
  (the render world runs in parallel with the next main-world frame), so they do not sum to the
  frame. The same numbers are the diagnostics `perf/main_world_schedules_ms`,
  `perf/scene_runtime_loop_ms`, `perf/frame_limiter_sleep_ms`, `perf/render_world_cpu_ms`,
  `perf/swapchain_present_wait_ms` and `gpu/frame_time`.
- **Bottleneck.** Which stage bounds the frame rate, from the smoothed values with a 20 %
  margin: `balanced` when the limiter sleeps a fifth of the frame or more, or when CPU busy time
  (frame minus sleep) and GPU time are within 20 % of each other; `present_wait` when the
  swapchain wait takes a fifth of the frame or more (vsync); otherwise `main_thread_cpu` or
  `gpu`, whichever is the longer by the margin. Null until a GPU time exists.
- **System.** On Linux and Windows, Bevy's system information: system and process CPU usage, the OS, the CPU,
  its cores and the RAM.

### Render (`/render_state`)

On native, Bevy's render diagnostics: every render pass's CPU time, and on Vulkan and DX12 its
GPU time too (Metal and WebGPU record CPU time only for pass spans), plus the adapter's name,
backend and driver, and the pipeline statistics (vertex and fragment shader invocations) where the
adapter supports them. The GPU frame time of the whole frame is the explorer's own measurement
(`benchmark.md`, *GPU time*) and works on every backend with timestamp queries.

### Profiler (`/profile_top [count] [sort]`, `/profile_enable on|off|reset`)

The in-app scope profiler. Every build carries Bevy's tracing spans (the `trace` feature is on):
one per system run, per schedule run and per render-graph node, plus the explorer's own
(`run_scene_loop`, `scene entity lifecycle`, `gltf containers`, `text shapes`, `imposter bake`,
`avatar processing`, `js update`, ...). A `tracing` layer in every build times them, but records
nothing until the table is enabled: **Enable profiler** in the section, `/profile_enable on` in
the console, or the benchmark autopilot for its measured phase. Enabled, it costs two clock reads
per span and a map update per exit, tens of microseconds a frame on native.

A row is one scope since the last reset: its kind (`ecs_system`, `schedule`, `render_graph_node`,
`bevy_internal`, `explorer_span`), call count, self time, inclusive time, longest call and the
smoothed self time per frame (an exponential moving average over 60 frames). Self time excludes
the child scopes entered on the same thread: a schedule run on the main thread whose systems run
on the task pool keeps their time, since the systems are not its children on that thread. Sort by
`self-time` (default), `inclusive-time`, `max-single-call` or `call-count`.

The section also shows the allocations per frame, count and bytes, from the counting allocator
the binaries wrap theirs in (`perf/allocs_count_per_frame`, `perf/alloc_bytes_per_frame`): the
first place to look for per-frame churn.

### Memory and limits (`/debug_memory`)

The engine's memory reading and its pressure tiers (`common::memory`), the peak during the
current sample, the scenes' JavaScript heaps summed (used, limit, external bytes and the engine
count; on the web only where the browser has `performance.measureUserAgentSpecificMemory()`),
and the current parcel scene's visible textures and meshes with their CPU-side bytes.
**Simulate Warning pressure** and **Simulate Full pressure** set the session limit from the
current use so the pressure tiers and the cache trimming they drive can be exercised;
**Stop simulating** restores the platform's capacity.

## The issue signal

While sampling, a frame of a second or more, or a completed sample whose mean frame rate is under
15 FPS, logs one line at warn level under the target `perf`, with the frame-time breakdown and the
bottleneck. One per sample. A warning is not a benchmark exception; an e2e run greps its log for
`[perf]` to catch a regression.

## Tracy

For a timeline of every span, run a build with the `tracy` feature, which adds Tracy's wire layer
to the same spans the in-app profiler reads:

```sh
# start the Tracy client (https://github.com/wolfpld/tracy, the version bevy's tracing-tracy pins) first
just native-tracy --realm <realm>
```

Tracy shows the main thread's schedules and systems, the task pool's systems, the render world's
nodes and, on Vulkan and DX12, GPU zones per pass (`bevy_render`'s `tracy_gpu`); each frame ends
at Bevy's frame mark after present. `trace_tracy_memory` (a Bevy feature, add it to the `tracy`
feature list to use it) attributes allocations. For a Chrome-format trace instead, build with
Bevy's `trace_chrome` and set `TRACE_CHROME=<file>`; open it in `chrome://tracing`.

## The benchmark's numbers

The autopilot's CSV holds every frame's CPU and GPU time (`benchmark.md`, *Files*). When the
measured phase ends the engine logs two `perf_summary` lines: the CPU and GPU p99 and hiccup
counts over the whole phase with the threshold in effect, the peak engine memory, the JavaScript
heap sum, the entity count and the sample count; then the top 30 scopes by self time, summed over
the phase's snapshots. The report shows them as *Hiccups per run*, *Peak memory* and *Top scopes*
(`benchmark.md`, *Reading the results*).
