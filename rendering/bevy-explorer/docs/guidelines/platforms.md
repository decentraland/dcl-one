# Platforms

The engine builds three ways from one workspace. The architecture diagrams show where each
piece runs; this page is the list of things that differ and bite.

| | Native | Web | Headless |
|---|---|---|---|
| Binary | `decentra-bevy`, plus the `dcl_deno_ipc` sidecar and the `decentra-bevy-cef` helper | wasm in the engine worker, loaded by `deploy/web/engine/` | `headless`, behind the `headless` feature |
| Scenes run in | V8 isolates in the sidecar | one sandbox worker per scene | V8 isolates in the sidecar |
| HUD | react-web rendered by CEF into an engine texture | react-web as DOM over the canvas | none |
| Rendering | wgpu | WebGPU in a render worker, Chrome only | none |

## Keep the boundary clean

Make as much code platform-independent as possible, behind a clean boundary. Generic backend
functions go in the `platform` crate, which has one `native` and one `wasm` implementation of
the same signatures: `fetch`, `websocket`, config and scene-file IO, pointer lock, the async
lock, the default camera bundle. Code that needs one of those calls `platform::x` and never
sees a `cfg`. Backend functionality that only one caller needs lives next to that caller
(the livekit bridge in `comms`, the page-event dispatch), behind a `cfg` there, not in
`platform`. Either way the platform-specific part is a leaf; the logic that uses it is shared.

## Web

- **Chrome only.** Design against Chrome's feature set (multi-memory, JSPI, `Atomics.waitAsync`)
  and do not add cross-browser hedges.
- **No threads as you know them.** `TaskPool::spawn` runs on the JS event loop: no `Send`,
  no blocking, no `std::thread`, no busy-waiting. Use `web_time` for clocks. The engine worker
  runs the app world; the render world lives in its own worker.
- **The log level is compiled in.** `RUST_LOG` does nothing at runtime on wasm. A debug log you
  want to see needs a rebuild with the filter changed.
- **Engine switches are page params, not launch options.** A web-only knob is a `?param=` the
  page reads and hands to a wasm export, like `?guest=`. Adding it to `LaunchOptions` pollutes
  the native `--help` for something native can't use.
- **Derivatives in uniform control flow.** `dpdx`, `dpdy`, `fwidth` and `textureSample` inside
  a non-uniform `if` fail on web only. Hoist them above the branch.
- **Custom transparent materials must write the focus target.** The fork's transparent pass
  has a second output at `@location(1)` (`TRANSPARENT_FOCUS_OUTPUT`). A fragment shader that
  omits it fails pipeline creation on web and is silent on native.
- **16-bit textures lose precision** through the web sampler. Expect banding if you rely on it.
- **Build.** The wasm build is CI's "Build WASM package" step; to run locally you also need the
  sandbox worker bundle. `just wasm` does both and starts the page dev server and the bridge.
  If a stale dev server is already on the port it gets reused; kill it first. Build the wasm on
  Linux (CI does): built on Windows, `file!()` keeps backslashes and the fork's
  `embedded_asset!` panics at start (`Failed to find src_prefix "src"`).
- **Exit work runs before `Last`.** On `AppExit`, `close_rooms_on_app_exit` blocks in `Last`
  (`comms`), so on web a system in `Last` may never run again. Anything that must reach the page
  at exit, such as the benchmark stream's final bytes, runs in `Update`.
- **Two GPUs: Chrome takes the low-power one.** On a laptop with integrated and discrete GPUs,
  Chrome's WebGPU renders on the integrated one while native wgpu takes the discrete one: on one
  laptop the same web build ran much slower (measurements (current evidence in Notion)).
  Start Chrome with `--force-high-performance-gpu` to compare with native (the benchmark's browser
  leg does).
- **Rollout.** The web build ships to zone and org by a percentage walk; publishing any branch
  moves zone. Ask before publishing.

## Native

- Build and run with `just native-debug` or `just native-release`, with the client's own
  arguments after them (`--realm <url> --position x,y`). Both bundle the HUD when it is stale.
  A bare `cargo run` gives a login screen with no HUD because the bundle isn't built. Both
  binaries build: `cargo build --package dcl_deno_ipc` then `cargo build`. `just setup-cef`
  fetches the CEF framework once.
- **CEF `localStorage` is ephemeral.** Anything the native HUD must remember goes through
  `lib/prefs`, which the engine persists to disk. No native popups from the HUD: CEF's popup
  windows aren't composited into the texture.
- **Windows paths.** `Path::is_absolute` is not a containment check there; see the security
  page.
- **Windows builds.** v8's build script symlinks its source into the target dir when the two sit
  on different drives, and Windows refuses without Developer Mode: keep `CARGO_HOME` and
  `CARGO_TARGET_DIR` on one drive. `cef-dll-sys` needs Ninja on `PATH`; Visual Studio ships one
  under `Common7\IDE\CommonExtensions\Microsoft\CMake\Ninja`. A cold build at a high job count can
  run LLVM out of memory: limit it with `CARGO_BUILD_JOBS`, and keep tens of gigabytes free for a
  debug target dir (measurements (current evidence in Notion)).
- **Any change under `rendering/bevy-explorer/source` runs the whole engine CI**: clippy, the
  native test suite on Linux, Windows and macOS, the headless smoke test and the web build. Only
  changes confined to the packaging folder `apps/bevy-explorer` narrow the selection
  (`tooling/bevy-ci.py` at the dcl-one root). The legs run in parallel and the run takes around
  40 minutes end to end.

## GPU timestamps

The benchmark's GPU time (`gpu/frame_time`, see [GPU time](benchmark.md#gpu-time)) is the
difference of two timestamps that empty compute passes write with `timestamp_writes`, which needs
`wgpu::Features::TIMESTAMP_QUERY` on the render device. The engine sets no `WgpuSettings` of its
own (the web render worker uses the default too), so Bevy's default `Functionality` priority
requests every feature the adapter reports, and the feature is on wherever the backend exposes
it. Without it the engine logs one info line under `gpu_time` and has no GPU time. In the fork's
wgpu (`robtfm/wgpu` `e25f3d4`, 24.0.5):

| Backend | `TIMESTAMP_QUERY` | Tick period | Checked on hardware |
|---|---|---|---|
| DX12 (Windows) | always | the queue's timestamp frequency | Windows, RTX 5080 Laptop GPU: the GPU tests, and the packaged release standalone with FXC and `--no_gltf` |
| Vulkan (Windows, Linux) | always | the device's `timestampPeriod` limit | Windows, the same GPU: the GPU tests and the packaged release standalone, which picks Vulkan by itself; Linux not |
| Metal (macOS) | when the device samples counters at stage boundaries (macOS 11 or newer) | fixed: 1 ns, 83.333 ns when the device name starts with "Intel" | not checked |
| WebGPU (Chrome) | when the browser's adapter lists `timestamp-query` | 1 ns (WebGPU reports nanoseconds) | Chrome on Windows (RTX 5080 Laptop GPU) |

- **DX12 and Vulkan** set the feature for every device: `wgpu-hal/src/dx12/adapter.rs` lists it
  among the features "always available on d3d12", and `wgpu-hal/src/vulkan/adapter.rs` sets it
  without reading the queue's timestamp bits.
- **DX12 in the packaged Windows folder** does not reach a frame, with the GPU time off too. The
  folder holds CEF's `dxcompiler.dll` and `dxil.dll`, and Bevy picks the DXC shader compiler
  whenever both sit in the working directory (`bevy_render/src/settings.rs`); wgpu's own
  indirect-validation pipeline then fails (`Validator version in metadata (1.9) is not supported;
  maximum: (1.7)`) and the device is lost at creation. `WGPU_DX12_COMPILER=fxc` starts it, and
  the full scene then runs out of descriptors (`Unable to allocate descriptors`); `--no_gltf`
  keeps the scene small enough to run. Vulkan, which wgpu picks by itself on the RTX 5080 laptop,
  is unaffected.
- **Metal.** A compute pass's `timestamp_writes` become the encoder's start and end samples
  (`wgpu-hal/src/metal/command.rs`), which stage-boundary sampling covers
  (`wgpu-hal/src/metal/adapter.rs`). Whether an empty pass writes its samples is not checked.
  wgpu does not convert Metal's GPU timestamps to CPU time; it hard-codes the period, and its own
  comment names Apple silicon (M1, M2) and AMD for the 1 ns. Not checked here.
- **WebGPU.** `wgpu/src/backend/webgpu.rs` maps the feature to `timestamp-query` and passes the
  pass `timestampWrites` through.
- **Chrome quantises timestamps** unless its "WebGPU Developer Features" flag is on
  (`chrome://flags/#enable-webgpu-developer-features`;
  [Chrome's WebGPU 121 notes](https://developer.chrome.com/blog/new-in-webgpu-121) give 100 µs).
  The benchmark runner does not set it. In the checked Chrome every web GPU value is a multiple of
  65.536 µs (measurements (current evidence in Notion)). That is Chrome's behaviour, not wgpu's.
- **Headless** has no render app, so it has no GPU time.

## Headless

- The headless binary is the production authoritative scene server and the local preview
  server. It runs the same `ScenePlugin` and scene loop with a narrower plugin set and no
  render app. `source/docs/headless-sdk-preview.md` and `source/docs/PRESENCE_ISOLATION.md`
  cover the server-side contracts. Resolve scene-response isolation proposals
  and their review status through authorized Notion context; proposals require
  a user verdict before implementation.
- **It is feature-gated.** `--features headless --bin headless`. Running the wrong bin through a
  pipe exits 0 and tells you nothing.
- **Missing resources only show at runtime.** The headless plugin set skips `InputManagerPlugin`
  among others, so a system that takes a `Res<>` from a skipped plugin compiles and panics on
  first run. Run the headless binary against a scene before pushing anything that touches
  shared systems.
- Every scene-facing capability needs the headless question answered: see the security page.
