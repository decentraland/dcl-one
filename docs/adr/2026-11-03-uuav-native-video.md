# ADR: UUAV, the native video player for the Explorer

- Status: Accepted
- Date: 2026-11-03

## Context

The Explorer used to play scene video through an in-process FFmpeg 6
software decoder whose libraries came from Homebrew, apt or a download at
CI time: nothing shipped was reproducible, every frame crossed the CPU, and
a demuxer crash took the engine down. UUAV, the player built for the Unity
Explorer, decodes in hardware out of process, inside a sandbox, with crash
recovery. Its sources live at `library/uuav` and the Explorer plays every
scene video and audio stream through it (`crates/media/src/uuav`).

## Decision

- `library/uuav` is a self-contained Cargo workspace: `uuav-core` (decode
  core), `uuav-ipc` (wire protocol and channels), `uuav-server` (the
  `uuav-helper` process) and `uuav-client` (the library the host links). The
  crates keep their `uuav-*` names, an exception to the `dcl-*` rule of the
  [layout ADR](2026-10-19-organizing-library-services-folders.md), like
  `library/vendor/rusty_enet`: the `uuav_*` entry points are its public ABI.
- The helper is the only FFmpeg linker. `uuav-client` links no FFmpeg, so
  the host's dependency graph never carries `ffmpeg-sys-next` (whose
  `links = "ffmpeg"` forbids two versions in one graph).
- Decoding is hardware first (VideoToolbox, D3D11VA, VAAPI) with a software
  `libavcodec` fallback on every platform, selected at open, on a
  hardware failure before the first frame (packets replayed), or by
  `UUAV_FORCE_SW_DECODE`. Every frame leaves as NV12; the host converts.
- Linux is a first-class target: `AF_UNIX` socketpairs for control, sealed
  memfds passed over `SCM_RIGHTS` for frames (CPU planes the host uploads),
  Landlock plus seccomp as the sandbox. VAAPI surfaces are downloaded to
  system memory; DMA-BUF zero-copy is a follow-up.
- The runtime set (`uuav-helper`, the FFmpeg and libva libraries, license
  texts) is committed under `library/uuav/dist/<target>/` in Git LFS and
  pinned by `tooling/uuav/uuav-binaries.lock.json`, verified by
  `tooling/native-lock/verify-binaries.py`. FFmpeg is LGPL-3.0 shared
  (`--enable-version3`, required by the statically linked Apache-2.0
  mbedTLS); Windows uses the hash-pinned BtbN LGPL shared build, macOS and
  Linux build FFmpeg from the pinned source commit.
- Two workflows enforce the lock: `uuav-verify.yml` on every change to the
  tree or the tooling, `uuav-native.yml` on the `ci:uuav-native` label or
  by hand, rebuilding all targets and running the reproducibility gates
  (same-runner determinism, byte reproduction on the pinned toolchain).
  Neither commits; relocking is a reviewed human step.
- The workspace pins its own Rust toolchain (`library/uuav/rust-toolchain.toml`),
  independent of the Explorer's: the helper's bytes must reproduce, the
  client crate builds on whatever stable compiler the host uses.
- The Explorer links `uuav-client` (its `uuav` feature, on by default) and
  has no other video decoder: `ffmpeg-next` is gone from its graph. The
  main world drives each player and polls its state and clock; kira pulls
  the audio through a blocking decoder paced by the helper's media clock;
  the render world turns the presented NV12 planes into the player's
  target image before the gpu images are prepared. macOS wraps the client's
  Metal planes without a copy; Linux uploads the shared CPU planes every
  frame.
- Sources stream directly: a scene content file resolves to its content
  server url and only http(s) urls are opened, so the sandboxed helper never
  needs file access and nothing is downloaded first.

## Consequences

- Every change under `library/uuav/src`, `uuav-server` or `uuav-ipc` is a
  rebuild-and-relock event for the committed helpers; `uuav-client` edits
  that change the wire or the ABI version are too.
- The host ships `dist/<target>/` next to its executable; the helper is
  located as a sibling of the executable that links the client.
- Hardware decoding on Linux depends on the distribution's VA driver;
  without one the core decodes in software and says so.
- Playback is checked end to end, after unity-explorer's UUAV PlayMode
  suite: the Explorer's e2e `video` scene (`apps/bevy-explorer/e2e/scenes/video`)
  plays a committed fixture and its spec asserts the player's events, the
  running clock and the presented frame through the engine, on the web
  app in the e2e workflow and on the Linux standalone, in software decode,
  in the scenes workflow. Hardware decode on Linux and macOS, and any
  frame on Windows, have no CI run.
- The Explorer's CI, packaging and test runs fetch `library/uuav/dist/**`
  from LFS and stage it next to the binaries (`tooling/uuav/stage-dist.py`);
  the FFmpeg packages the runners used to install are gone.
- Windows shows no video frames yet: the client presents into a D3D11 NV12
  texture on its own device while the renderer runs on Vulkan or DX12. The
  path (a shared NT handle into a DX12 renderer, which pins the backend;
  Vulkan external memory; or a CPU readback as the software fallback) is an
  open evaluation, marked `TODO(uuav-windows)` in
  `crates/media/src/uuav/platform_windows.rs`. Audio-only sources and the
  audio of a video play.
- Not carried over yet: the playback rate (`AVCommand` has no rate, as
  before) and DMA-BUF zero-copy on Linux.
