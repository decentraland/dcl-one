# ADR: UUAV, the native video player for the Explorer

- Status: Accepted
- Date: 2026-11-03

## Context

The Explorer plays scene video through an in-process FFmpeg 6 software
decoder whose libraries come from Homebrew, apt or a download at CI time:
nothing shipped is reproducible, every frame crosses the CPU, and a demuxer
crash takes the engine down. UUAV decodes in hardware out of process,
inside a sandbox, with crash recovery.
Its sources now live at `library/uuav`; the Explorer-side integration
follows in a later change, once it shows frames on every desktop
platform, Windows included.

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
  tree or the tooling, `uuav-native.yml` on the `ci:uuav-native` label (PRs
  touching the UUAV inputs) or by hand, rebuilding all targets and running the reproducibility gates
  (same-runner determinism, byte reproduction on the pinned toolchain).
  Neither commits; relocking is a reviewed human step.
- The workspace pins its own Rust toolchain (`library/uuav/rust-toolchain.toml`),
  independent of the Explorer's: the helper's bytes must reproduce, the
  client crate builds on whatever stable compiler the host uses.

## Consequences

- Every change under `library/uuav/src`, `uuav-server` or `uuav-ipc` is a
  rebuild-and-relock event for the committed helpers; `uuav-client` edits
  that change the wire or the ABI version are too.
- The host ships `dist/<target>/` next to its executable; the helper is
  located as a sibling of the executable that links the client.
- Hardware decoding on Linux depends on the distribution's VA driver;
  without one the core decodes in software and says so.
