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
  hidden and the window is focused; on the web, the page has focus (`document.hasFocus()`, so a
  click on the HUD keeps sampling) and the viewport is active (not a background tab).
  `sampling.status` is `sampling` or `paused`, and `sampling.pausedReason` says why:
  `not_in_world`, `loading_screen`, `window_unfocused` or, on the web, `viewport_inactive`.
  Paused time never counts, and the frame on which sampling resumes is dropped, since its delta
  spans the pause.
- **Snapshots.** Every 15 s of sampled time one `PerformanceSnapshot` is taken: the CPU frame
  times (`Time<Real>` deltas, unclamped) and the GPU frame times (`gpu/frame_time`, where the
  adapter has timestamp queries) as a summary each: count, min, mean, p1 to p99, max, and the
  frames over the hiccup threshold with their time sum and their excess over the threshold.
  `inProgressSample.sampledSeconds` counts up to the next one. The GPU readings of the first four
  sampled frames after a resume are dropped too: a frame's GPU time is read back frames later,
  so those can still be the pause's. The snapshot also carries the
  engine memory and its peak during the sample, the scenes' JavaScript heaps summed, the nearby
  player count, the entity count and, with the profiler on, the top 30 scopes of the sample (its
  sampled frames only; the Profiler section's table is untouched).
- **HUD main thread (web).** The React HUD's cost on the page's main thread over the page's
  last window, one second, or several summed when the engine's frame spanned them: the HUD
  root's React commits (count and their time) and, where the browser has
  Long Animation Frame timing (Chromium), the page's frames over 50 ms: their count, the HUD
  bundle's script time in them and their style and layout time. The HUD bundle is the whole of
  `assets/` (the HUD's own chunks and its vendor, shell and crypto chunks); on the Vite dev
  server, its module URLs. The browser attributes a long frame's scripts of 5 ms or more, so the
  HUD script time counts those scripts within frames over 50 ms. The commits are reported only
  by a HUD built with `HUD_REACT_PROFILING=1` in the environment (`react-web`, `npm run build`,
  or `make web` in `apps/bevy-explorer`, which passes it through), which is React's profiling
  build, slower on every commit than the production build the players load; otherwise their two
  cells are `-`. React measures every commit, in a long frame or not, so the commit time is a
  separate measure and can exceed the HUD script time. `-` on native and before the first
  window. A long frame counts in the window its entry arrives in, after the frame ends; one that
  began before the meter's install or while the tab was hidden is not counted. A snapshot
  carries the windows of its sampled time summed (`snapshot.hudMainThreadCost`, `null` when none
  arrived).
- **Hiccup threshold.** The larger of 50 ms and two target frame times: 50 ms at 60 FPS and
  above, 66.7 ms at a 30 FPS cap, 50 ms with vsync or uncapped. The target is the session
  override (`/session_graphics fps`), else the configured one.
- **Frame time breakdown.** Where the previous frame's wall time went, smoothed: the main world's
  schedules (`First` through `Last`), the scene loop until its deadline, the frame limiter's sleep
  (native), the render world's CPU time from `ExtractCommands` on (extraction and the swapchain wait
  excluded), the swapchain wait itself
  (vsync shows here; on the web, the render worker's wait for the browser's next animation
  frame) and the GPU time. The parts are measured separately and overlap in part
  (the render world runs in parallel with the next main-world frame), so they do not sum to the
  frame. The same numbers are the diagnostics `perf/main_world_schedules_ms`,
  `perf/scene_runtime_loop_ms`, `perf/frame_limiter_sleep_ms`, `perf/render_world_cpu_ms`,
  `perf/swapchain_present_wait_ms` and `gpu/frame_time`.
- **Bottleneck.** Which stage bounds the frame rate, from the smoothed values with a 20 %
  margin: `frame_rate_limiter` when the frame's pacing wait is a fifth of the frame or more (the
  limiter's sleep natively; on the web, where the scene loop never sleeps, the app loop's time
  outside the app's update, `pipelined/main_update` minus the main world's schedules: its frame
  timer, the cap, then the animation frame that runs the next update);
  `swapchain_present_wait` when the present wait takes a fifth of the frame or more (vsync; on
  the web the render worker's wait for the browser's next animation frame,
  `pipelined/wait_frame`); otherwise `main_thread_cpu`, `render_thread_cpu` or `gpu`, whichever
  of CPU busy time and GPU time is the longer by the margin, or `cpu_and_gpu_balanced` when they
  are within it. CPU busy time is the frame without its pacing wait and, on the web, without the
  handoff wait (`pipelined/wait_handoff`, the app blocked until the render worker takes the
  frame); it includes the main thread's wait for the render world, so the frame is
  `render_thread_cpu` when the render world's CPU time is the longer, by the margin, of it and
  the main world's schedules. Null until a GPU time exists.
- **System.** On Linux and Windows, Bevy's system information: system and process CPU usage, the OS, the CPU,
  its cores and the RAM.

### Render (`/render_state`)

On native, Bevy's render diagnostics: every render pass's CPU time, and on Vulkan and DX12 its
GPU time too (Metal and WebGPU record CPU time only for pass spans), plus the adapter's name,
backend and driver, and the pipeline statistics (vertex and fragment shader invocations) where the
adapter supports them. The GPU frame time of the whole frame is the explorer's own measurement
(`benchmark.md`, *GPU time*) and works on every backend with timestamp queries.

### Profiler (`/profile_top [count] [--sort <metric>]`, `/profile_enable on|off|reset`)

The in-app scope profiler. A `profiler` build (the cargo feature, in `default`, so every native
build; the web build is without it) carries Bevy's tracing spans (Bevy's `trace` feature): one
per system run (named after the system, e.g. `scene_runner::run_scene_loop`), per schedule run
and per render-graph node, plus the explorer's own in-process spans (`av-thread`), which every
build has. Scene JavaScript runs in its own process on native (`dcl_deno_ipc`), so its
`js startup` and `js update` spans are not recorded here. A `tracing` layer in every build times
the spans there are, but records nothing until the table is enabled: **Enable profiler** in the
section, `/profile_enable on` in the console, or the benchmark autopilot for its measured phase.
Enabled, it costs two clock reads per span and an update of the thread's own map per exit (the
fold merges the threads' maps once a frame, so no exit waits on another thread), tens of
microseconds a frame on native. A span that lives across frames is one call, recorded when it
ends. `/profile_top` reports `profilerBuild`; without the feature the empty table says so, and
the rows are the explorer's own spans and Bevy's unconditional ones (`present_frames`,
`entity_sync`). The web build leaves the feature off because the engine's tracing layer there
(`tracing_wasm`) marks every span on the page's performance timeline, which nothing clears. With
the feature, Bevy's `trace` also prefixes a native session-log line logged inside a system with
its spans (`schedule{name=Update}:system{name=...}:`) and installs a span-trace panic hook.

A row is one scope since the last reset: its kind (`ecs_system`, `schedule`, `render_graph_node`,
`bevy_internal`, `explorer_span`), call count, self time, inclusive time, longest call and the
smoothed self time per frame (an exponential moving average over 60 frames). A snapshot's top
scopes are kept apart from this table, so neither a snapshot nor the sampler's pauses touch it.
Self time excludes
the child scopes entered on the same thread: a schedule run on the main thread whose systems run
on the task pool keeps their time, since the systems are not its children on that thread. Sort by
`self-time` (default), `inclusive-time`, `max-single-call` or `call-count`.

The section also shows the allocations per frame, count and bytes, from the counting allocator a
`profiler` build wraps its allocator in (`perf/allocation_count_per_frame`,
`perf/allocated_bytes_per_frame`; a reallocation counts once, with the bytes it grew by): the
first place to look for per-frame churn. The wrapper costs two shared atomic adds per
allocation on every thread, which a build without the feature does not pay.

### Memory and limits (`/debug_memory`)

The engine's memory reading and its pressure tiers (`common::memory`), the peak during the
current sample, the scenes' JavaScript heaps summed (used, limit, external bytes and the
number of reporting scenes; on the web only where the browser has `performance.measureUserAgentSpecificMemory()`),
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
heap sum, the entity count and the sample count; then the top 30 scopes by self time over every
frame of the phase, focused or not (the profiler's table, reset when the phase starts). The report
shows them as *Hiccups per run*, *Peak memory* and *Top scopes* (`benchmark.md`, *Reading the
results*).
