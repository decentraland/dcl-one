# UUAV

A video/audio player for the Explorer built on a Rust core and FFmpeg 8.1,
with hardware-accelerated video decoding where the platform offers it and a
software decoder otherwise: VideoToolbox on macOS (Metal), D3D11VA on
Windows, VAAPI on Linux. Decoding runs **out of process**: FFmpeg never
shares the host's address space or GPU device, so a decoder crash cannot
take the engine down, and the decode process is **sandboxed** on every
platform (see [Sandboxing](#sandboxing)).

- `src/` (**uuav-core**) - the decode/playback core, linked into the helper
- `uuav-client/` - the library the host links (a plain `rlib`; its surface
  is the C ABI `uuav_*` entry points, called as ordinary Rust functions)
- `uuav-server/` - compiled to `uuav-helper(.exe)`, the process that hosts
  the core
- `uuav-ipc/` - the wire protocol and the native channels shared by both
- `dist/<target>/` - the committed runtime set (Git LFS): the helper, the
  FFmpeg libraries it links and their license texts, pinned by
  [`tooling/uuav/uuav-binaries.lock.json`](../../tooling/uuav/uuav-binaries.lock.json)

The workspace is self-contained and pins its own compiler
(`rust-toolchain.toml`, independent of the Explorer's): the helper is a
prebuilt process whose bytes must reproduce, while the client crate builds
on any stable compiler the host uses. Its crates keep their `uuav-*` names
rather than the `dcl-*` convention of `library/`
([ADR](../../docs/adr/2026-11-03-uuav-native-video.md)).

## How it works

The host initializes the runtime once with `uuav_init`: a probe texture
(an `id<MTLTexture>` on macOS, an `ID3D11Texture2D*` on Windows; ignored on
Linux, null accepted), the audio output format, three log callbacks and the
FFmpeg protocol whitelist. Everything else goes through per-player calls by
id. The client then:

- creates a duplex OS channel - a message-mode named pipe on Windows, an
  `AF_UNIX` socketpair on macOS and Linux - and on Linux a second
  socketpair that carries file descriptors;
- spawns `uuav-helper` **from the directory of the executable it is linked
  into** (resolved from one of its own symbols) and hands it its channel
  end by inheritance. A session token in the first message guards against a
  stale helper generation and the helper's ABI version must equal the
  client's;
- sends `Configure`: the helper creates its own GPU device on the same
  adapter (LUID on Windows, registry id on macOS; nothing on Linux) and
  runs the core init against it.

From then on the helper adapts IPC messages onto the core's C API:

- **Commands** (open/play/pause/seek/looping/rate) are fire-and-forget;
  only init-time calls block on replies.
- **State** is pushed back at ~50 Hz per player; every per-frame getter is
  served from that local cache (`uuav_player_current_time` extrapolates
  between snapshots), so getters never touch the wire. `MediaInfo` is sent
  once per open and again if the decoder changes (see below).
- **Video**: the helper drives the core's render events itself (~60 Hz),
  copies each presented frame into one of 3 cross-process shared slots and
  publishes the slot. On the host's render event (`uuav_get_render_callback`,
  called with the player id) the client copies the latest published slot
  into client-owned presentation planes and `uuav_player_get_video_texture`
  returns them: two `MTLTexture`s (plane 0 = Y `R8Unorm`, plane 1 = UV
  `RG8Unorm`) on macOS; one NV12 `ID3D11Texture2D` on Windows (`plane`
  ignored, the host creates R8/RG8 views over it); two tightly packed CPU
  buffers (Y at w x h, UV at w/2 x h/2) on Linux that the host uploads to
  its own textures. The pointers are stable across frames and survive
  resolution changes with a one-poll retire grace. `uuav_player_get_presented_video`
  returns the same pointers together with the size the planes were created
  for (the decoded frame's, even-rounded, which the stream header of
  `uuav_player_get_video_size` need not match) and a count of presented
  frames, so a host reads the planes at their own size and skips its upload
  while nothing new was presented. The host converts NV12 to RGB itself. A frame the client
  cannot present (a generation past 4096x4096 pixels, a failed wrap or
  copy) is reported once through the error callback and the player reads
  `UUAV_ERROR` until a frame presents or new media is opened.
- **Audio** crosses the boundary as messages: the helper pulls interleaved
  `f32` from the core every 10 ms and the client feeds a ~50 ms jitter ring
  that the host's audio callback drains with `uuav_player_read_audio`
  (never blocks, pads silence). UUAV opens no audio device.

### Decoding: hardware first, software otherwise

Every stream opens on the platform hardware decoder when the codec has one
and the device exists; otherwise, and when `UUAV_FORCE_SW_DECODE=1` is set
in the host's environment (inherited by the helper), it opens on the plain
`libavcodec` decoder with frame threading. A hardware decoder that fails
before its first frame - a profile the hardware rejects only surfaces from
the first packets - is replaced by a software decoder and the packets it
was fed are replayed, so the stream restarts from its parameter sets with no
visible gap. Software-decoded frames are converted to NV12 (`libswscale`)
and uploaded into the same presentation planes the hardware path blits
into, so the host sees one contract. `MediaInfo.decoder` names what is in
use (`videotoolbox`, `d3d11va`, `vaapi`, `software`); the warning callback
reports every fallback once.

On Linux the VAAPI surfaces are downloaded to system memory after decoding
(nothing shares them with the host), so hardware and software frames take
the same CPU path there; zero-copy DMA-BUF sharing is a follow-up.

### Crash recovery

The helper is watched by a recovery worker inside the client. If it dies,
playback self-heals with no host involvement: every command keeps a
per-player *desired state* (url, play/pause, looping, rate, resume
position) current, so the worker respawns the helper (backoff 0 s/1 s/3 s),
re-runs the init handshake, and rebuilds every player - reopen, resume
playback, seek back to where the clock was. During the outage players read
`UUAV_OPENING`, commands are absorbed into desired state, the video freezes
on the last presented frame (the presentation planes are client-owned and
never die), and audio pads silence. Public player ids stay stable across
helper generations. After three failed respawns the runtime parks
(`UUAV_ERROR`) until the next open/play re-arms it. Every death still
reports once through the error callback.

The helper never outlives the host: it watches the parent pid and exits
when the host goes away for any reason; `uuav_deinit` shuts it down
gracefully (≤1 s, then kill).

### Sandboxing

FFmpeg demuxes attacker-controlled media (URLs come from untrusted scenes),
so the helper runs with the least authority that still allows decoding and
outbound network access. On every platform a compromised decoder cannot
launch another process or persist files on disk; the mechanism is the
native one of each OS:

- **Windows: restricted at spawn, by the client** (`uuav-client/src/sandbox_windows.rs`).
  The helper is created suspended under a restricted low-integrity token,
  assigned to a single-process kill-on-close job object, given a process
  mitigation policy (DEP, ASLR, CFG, strict handle checks, heap
  terminate-on-corruption), and only then resumed. The low-IL helper cannot
  duplicate texture handles into the host, so the client pulls them out.
- **macOS: self-lockdown under Seatbelt** (`uuav-server/src/sandbox_macos.rs`,
  `uuav-server/helper.sb`). As the first act of `main` the helper applies a
  deny-by-default profile: system library reads, Metal/IOSurface/VideoToolbox
  user clients, outbound TCP/UDP plus the DNS and trustd brokers, and
  `mach-lookup` of exactly the one per-session service the client
  registered for IOSurface transfer. `process-exec`, `process-fork` and
  every file write are denied. The IOSurface channel authenticates its
  sender by the kernel audit trailer.
- **Linux: Landlock plus seccomp** (`uuav-server/src/sandbox_linux.rs`).
  As the first act of `main` the helper restricts the filesystem to
  read-only access to the system library and configuration trees and its
  own directory, with read-write only on `/dev/dri` (VAAPI) and
  `/dev/null`, then installs a seccomp filter that denies `execve`,
  `execveat`, `fork`, `vfork`, `clone` without `CLONE_THREAD`, `ptrace` and
  `process_vm_*` (`clone3` answers `ENOSYS` so glibc falls back to the
  inspectable `clone`). This is weaker than Seatbelt: devices, IPC and the
  network are not enumerated, and a kernel without Landlock (5.13+) keeps
  only the seccomp half; the helper's startup log line says which. The
  descriptor channel authenticates its sender by `SO_PASSCRED`.

**Fail closed** everywhere: if the sandbox fails to apply the helper exits
and the client's recovery surfaces `UUAV_ERROR`; there is no bypass switch.
The `file:` protocol works for local development because the client passes
`--allow-file-read` when the protocol whitelist contains `file`, which opens
broad *reads* only; shipped hosts never pass it.

Debugging a macOS denial: `log stream --style compact --predicate 'sender == "Sandbox"'`
and look for `uuav-helper(pid) deny(1) <operation> <name>` lines. On Linux,
`strace -f -e trace=openat,execve` on the helper shows `EACCES`/`EPERM`
from the two layers.

### Headless smoke test

`uuav-client/examples/smoke.rs` exercises the whole pipeline without a
host engine: it creates a real GPU probe (nothing on Linux), walks the C
ABI like the Explorer does, expects a stream to reach PLAYING with video
pointers and audible audio, then kills the helper and asserts playback
self-recovers. Build the runtime set, stage it next to the example and run:

```bash
bash build.sh                                                   # dist/<target>/
cargo build -p uuav-client --example smoke
python3 ../../tooling/uuav/stage-dist.py host .target/debug/examples
.target/debug/examples/smoke [media-url]                        # hardware decode
UUAV_FORCE_SW_DECODE=1 .target/debug/examples/smoke [media-url] # software decode
```

`gaps.rs` and `soak.rs` probe audio gaps and pacing the same way.

## Building

The FFmpeg prefix lives in `.third_party/ffmpeg` (`FFMPEG_DIR` in
`.cargo/config.toml`); `build.sh` links the helper against it and copies
the runtime libraries into `dist/<target>/` from the same prefix, so the
majors can never drift apart. FFmpeg is used as LGPL-3.0 shared libraries
(`--enable-version3`, required by the Apache-2.0 mbedTLS that provides TLS
on macOS and Linux); the license texts ship next to the binaries.

### macOS (universal: Apple Silicon + Intel)

The binaries are universal, but the build host must be Apple Silicon (the
x86_64 slice is cross-compiled). Prerequisites: Xcode or the Command Line
Tools, `rustup target add aarch64-apple-darwin x86_64-apple-darwin` (done by
`rust-toolchain.toml`), `brew install nasm pkg-config cmake`.

```bash
./scripts/build-ffmpeg-macos.sh   # FFmpeg n8.1 + mbedTLS 3.6, universal LGPL shared build
./build.sh                        # dist/macos-universal
```

`build.sh` merges the slices with `lipo`, copies the seven FFmpeg dylibs,
ad-hoc code-signs everything (mandatory on arm64) and runs
`tooling/uuav/doctor-libs-macos.sh`, which fails the build if any deployed
binary is thin. Dylib loading needs no `install_name_tool`: FFmpeg is
configured with `--install-name-dir='@rpath'` and `uuav-helper` carries an
`LC_RPATH @loader_path`, so the set resolves from the helper's directory.

### Windows (x86_64, mingw)

Prerequisites: MSYS2 with `mingw-w64-x86_64-gcc` on `PATH` (the linker is
`x86_64-w64-mingw32-gcc`; GNU ld is used because it can be told not to stamp
the PE header, which is what makes two builds byte-identical),
`rustup target add x86_64-pc-windows-gnu`.

```bash
bash ../../tooling/uuav/fetch-ffmpeg-windows.sh   # the hash-pinned BtbN LGPL shared build
bash build.sh                                     # dist/windows-x86_64
```

The five DLLs in the helper's import closure ship (`avcodec`, `avformat`,
`avutil`, `swresample`, `swscale`); the BtbN build links its support
libraries statically.

### Linux (x86_64)

Prerequisites: `apt-get install build-essential nasm pkg-config cmake meson
ninja-build libxml2-dev libdrm-dev zlib1g-dev patchelf libclang-dev`.

```bash
./scripts/build-ffmpeg-linux.sh   # FFmpeg n8.1 + mbedTLS 3.6 + libva 2.22, LGPL shared build
./build.sh                        # dist/linux-<arch>
```

Everything shipped carries a `$ORIGIN` RUNPATH; the remaining shared
dependencies are glibc, zlib, libxml2 and libdrm, present on every desktop
distribution. libva is built from source and shipped so the package does not
depend on the distribution's; the hardware driver (`iHD`, `i965`,
`radeonsi`, ...) is the distribution's, dlopened from its DRI directory. On
a machine without a usable driver (CI runners, containers) the core logs a
warning and decodes in software.

## Runtime deployment

Everything ships in one directory, next to the executable that links
`uuav-client` - that is where the client looks for the helper and where the
helper's libraries resolve from:

```
dist/macos-universal/          dist/windows-x86_64/       dist/linux-x86_64/
  uuav-helper                    uuav-helper.exe            uuav-helper
  libavcodec.62.dylib            avcodec-62.dll             libavcodec.so.62
  libavdevice.62.dylib           avformat-62.dll            libavdevice.so.62
  libavfilter.11.dylib           avutil-60.dll              libavfilter.so.11
  libavformat.62.dylib           swresample-6.dll           libavformat.so.62
  libavutil.60.dylib             swscale-9.dll              libavutil.so.60
  libswresample.6.dylib          LICENSE-ffmpeg.txt         libswresample.so.6
  libswscale.9.dylib                                        libswscale.so.9
  LICENSE-ffmpeg.txt                                        libva.so.2
  LICENSE-mbedtls.txt                                       libva-drm.so.2
                                                            LICENSE-*.txt
```

`tooling/uuav/stage-dist.py <target|host> <dir>` copies a set next to a
binary (a packaged app, a test binary, the smoke example) and refuses an
unfetched Git LFS pointer. The FFmpeg majors (avcodec **62**, avformat
**62**, avutil **60**, swresample **6**, swscale **9**) are tied to the
8.1 release and must match what the helper linked; keep `.third_party/ffmpeg`
and `ffmpeg-sys-next = "8.1.0"` in `Cargo.toml` in lockstep. If a library is
missing on a user machine the helper dies at load - watch for the "uuav
helper terminated" error with a loader exit code.

## CI verification of the shipped binaries

The committed binaries are pinned by a hash lock,
`tooling/uuav/uuav-binaries.lock.json`, and two workflows enforce it:

- **`uuav-verify.yml`** runs on every PR touching this tree, `tooling/uuav/`
  or the shared `tooling/native-lock/`. It re-hashes every shipped binary
  and every build input that feeds one (Rust source trees, manifests, the
  `Cargo.lock` closure of the shipping root, `.cargo/config.toml`, the FFmpeg
  builder scripts) against the lock, reads the FFmpeg configure line
  embedded in each shipped library and compares it with the recorded
  provenance, and requires new native binaries to be stored in Git LFS. It
  also formats, checks, tests and lints the FFmpeg-free crates (`uuav-ipc`,
  `uuav-client`) on Linux, macOS and Windows and asserts nothing in this
  workspace links `uuav-client`.
- **`uuav-native.yml`** runs on demand (`workflow_dispatch`, or the
  `ci:uuav-native` PR label). It rebuilds all three targets from pinned
  inputs - macOS and Linux FFmpeg from the source commits in the lock,
  Windows FFmpeg fetched as the hash-pinned BtbN release asset - lints and
  tests the whole workspace where the FFmpeg headers exist, then runs two
  gates: Gate A (`tooling/uuav/repro-gate.sh`) builds twice from two source
  trees through one canonical path (`tooling/uuav/build-canonical.sh`) and
  requires byte-identical cargo artifacts; Gate B
  (`tooling/native-lock/reproduces-lock.py`) compares the fresh build against
  the committed hashes, hard-failing only when the runner's toolchain
  matches the one pinned in the lock. The Linux job also runs the headless
  smoke in software mode. Nothing is committed.

A target marked `pending` in the lock (no binaries committed yet) is built
and gated like the others but not verified; the first relock promotes it.

### Relocking after a deliberate rebuild

`uuav-native.yml` hands back, per target, the rebuilt runtime set and the
`toolchain-<os>.txt` its "Record the toolchain actually used" step wrote.
Relocking is the human step that commits both:

1. Copy the files from that run's `uuav-<target>` artifact into
   `dist/<target>/` (macOS can be built and relocked locally on Apple
   Silicon: `scripts/build-ffmpeg-macos.sh`, then
   `tooling/uuav/build-canonical.sh`, and write `toolchain-macos.txt` with
   the same commands as the workflow step).
2. Relock that target, passing the toolchain the build recorded:

```
python3 tooling/native-lock/verify-binaries.py --lock tooling/uuav/uuav-binaries.lock.json --update --only macos-universal --toolchain toolchain-macos.txt
```

3. Check `git lfs ls-files` lists every binary, and that
   `python3 tooling/native-lock/verify-binaries.py --lock tooling/uuav/uuav-binaries.lock.json`
   passes offline before committing.

`--toolchain` is what makes the relock honest about who built the bytes.
Without it, `--update` refuses on any host whose pinned components differ
from `targets.<t>.rust.toolchain` - including the host that just bumped
`rust-toolchain.toml`. `--update` rewrites only machine-derived fields; it
never touches the upstream pins (FFmpeg, mbedTLS and libva tags and commits,
the BtbN release asset and its sha256), which are deliberate human moves.

Two things about the round trip are easy to get wrong:

- **Gate B needs a second run.** The round that produces the binaries
  compares them against the lock as it stands *before* the relock, so it
  still skips. It becomes a real byte comparison on the next labelled run,
  after the relocked lock and the new binaries are pushed.
- **`library/uuav` must not move between the build and the relock.** The
  binaries come from the commit CI built; `source_digest` is taken from the
  working tree at relock time.

The lock digests the source files as raw bytes, so `.gitattributes` forces
`eol=lf` across this tree.

## For the host

What the Explorer integration (a later change) relies on:

- Link `uuav-client` as a path dependency and call the `uuav_*` functions;
  `uuav_init` once with a probe texture from the render device, players
  per scene entity. Callbacks fire on the client's IO and recovery threads,
  never on the caller's.
- Call `uuav_get_render_callback()(player_id)` once per frame on the
  render thread, then `uuav_player_get_video_texture` for the planes; on
  Windows the client copies on its own D3D11 device's immediate context, so
  that call must stay on one thread.
- Run an audio output and pull `uuav_player_read_audio` from its callback
  at the sample rate and channel count passed to `uuav_init`.
- Ship `dist/<target>/` next to the executable (`stage-dist.py`), including
  the license texts.
