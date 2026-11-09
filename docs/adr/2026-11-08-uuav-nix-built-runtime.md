# ADR: the UUAV runtime set is built with Nix, not committed

- Status: Accepted
- Date: 2026-11-08
- Amends: [UUAV, the native video player](2026-11-03-uuav-native-video.md)

## Context

The runtime set (`uuav-helper`, the FFmpeg and libva libraries it links,
license texts) was committed in Git LFS under `library/uuav/dist/<target>/`
and pinned by a hash lock: about 190 MB over three targets, of which the
helper built from this tree is under 2 MB and the rest is FFmpeg. Keeping it
honest took a hash verifier, a reproduction gate and two workflows, and every
helper change was a rebuild-and-relock round trip across three runners.

## Decision

- `infra/packages/uuav.nix` builds the set from pinned inputs: FFmpeg at the
  `n9.0.2` commit, configured from `--disable-everything` up with the
  decoders, demuxers, parsers, protocols, bitstream filters and hardware
  decoders scene media needs, with libdav1d (static, BSD-2-Clause) as the
  AV1 software decoder, and without libavfilter, libavdevice, libxml2 or a
  TLS backend ([the helper serves http(s)](2026-11-08-uuav-http-in-the-helper.md)),
  libva (`libva-minimal`) on Linux, VideoToolbox on macOS; the helper with
  crane on the compiler
  `library/uuav/rust-toolchain.toml` names, linking libavcodec, libavformat,
  libavutil, libswresample and libswscale only; `uuav-dist` assembles the
  helper, the major-version libraries and the license texts.
  `uuav-dist-windows` cross-builds the same FFmpeg configuration for mingw
  (D3D11VA for hardware decode) and the helper on Linux. The three sets are
  LGPL-2.1.
- Nothing built is committed: `library/uuav/dist/` is ignored, the lock and
  `tooling/native-lock/` are gone. The pins are the inputs: `flake.lock`,
  the FFmpeg source hash, patch and component lists, nixpkgs' libva,
  the workspace `Cargo.lock`; the BtbN asset digest
  (`tooling/uuav/ffmpeg-windows.json`) pins only the native Windows cargo
  path.
- Linux and macOS developers run `nix build .#uuav-dist`; `nix build
  .#uuav-ffmpeg -o library/uuav/.third_party/ffmpeg` gives cargo its FFmpeg
  prefix for iteration with `build.sh`. Windows keeps
  `tooling/uuav/fetch-ffmpeg-windows.sh` and `build.sh`.
- `uuav.yml` restores the Nix store snapshot (`cache-nix-action`, saved from
  `main`), builds the set, the smoke harness and the checks (format, clippy,
  tests) on Linux and macOS, runs the smoke in software decode on Linux,
  cross-builds the Windows set there, and on a Windows runner runs the cargo
  checks and the smoke against that cross-built set. Every run uploads the
  sets as artifacts; the Windows set ships from the Nix build only.
- macOS ships `arm64` only; no universal Explorer exists. The macOS set
  links system libraries only (libiconv relinked to `/usr/lib`) and the
  build fails on a Nix store reference.
- The Linux set resolves glibc, zlib and libdrm from the Nix store. A
  distribution-independent set is the step that packages the helper with
  the Explorer, when that lands.

## Consequences

- Helper and FFmpeg changes are ordinary pull requests; CI builds what the
  cache lacks and nothing is relocked by hand.
- The LFS objects stay in history; new checkouts fetch none of them.
- Every input bump (nixpkgs, FFmpeg commit or component list, BtbN asset
  for the native Windows path) is a visible diff in `flake.lock`,
  `infra/packages/uuav.nix` or `tooling/uuav/ffmpeg-windows.json`.
- The set carries what scene media uses and nothing else: a format outside
  the lists fails to open instead of decoding, and adding one is a list
  entry whose CI build is the proof.
- `dash` (`.mpd`) streams are not served: that demuxer needs libxml2, which
  is not built; scene media is HLS and progressive files. Adding it is the
  library plus a list entry.
- AV1 decodes in software through libdav1d where the GPU has no AV1
  decoder: FFmpeg's `av1` decoder carries hwaccel hooks only. The hardware
  lookup picks the decoder by hwaccel rather than registry order, so a
  capable GPU still gets the native decoder's D3D11VA, VAAPI or
  VideoToolbox path.
- FFmpeg's `http` protocol stays linked as the `rtsp` demuxer's dependency;
  no url reaches it ([the helper serves http(s)](2026-11-08-uuav-http-in-the-helper.md)).
