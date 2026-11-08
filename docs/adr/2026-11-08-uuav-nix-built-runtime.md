# ADR: the UUAV runtime set is built with Nix, not committed

- Status: Accepted
- Date: 2026-11-08
- Amends: [UUAV, the native video player](2026-11-03-uuav-native-video.md)

## Context

The runtime set (`uuav-helper`, the FFmpeg 8.1 and libva libraries it links,
license texts) was committed in Git LFS under `library/uuav/dist/<target>/`
and pinned by a hash lock: about 190 MB over three targets, of which the
helper built from this tree is under 2 MB and the rest is FFmpeg. Keeping it
honest took a hash verifier, a reproduction gate and two workflows, and every
helper change was a rebuild-and-relock round trip across three runners.

## Decision

- `infra/packages/uuav.nix` builds the set from pinned inputs: FFmpeg at the
  `n8.1` commit with a statically linked mbedTLS 3.6.7 and libxml2 (both from
  nixpkgs), libva (`libva-minimal`) on Linux, VideoToolbox on macOS; the
  helper with crane on the compiler `library/uuav/rust-toolchain.toml` names;
  `uuav-dist` assembles the helper, the major-version libraries and the
  license texts. `uuav-dist-windows` cross-builds the helper with mingw on
  Linux and takes the FFmpeg DLLs from the hash-pinned BtbN LGPL shared build
  (`tooling/uuav/ffmpeg-windows.json`).
- Nothing built is committed: `library/uuav/dist/` is ignored, the lock and
  `tooling/native-lock/` are gone. The pins are the inputs: `flake.lock`,
  the FFmpeg source hash, nixpkgs' mbedTLS, libxml2 and libva, the
  workspace `Cargo.lock`, the BtbN asset digest.
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
- Every input bump (nixpkgs, FFmpeg commit, BtbN asset) is a visible diff in
  `flake.lock`, `infra/packages/uuav.nix` or `tooling/uuav/ffmpeg-windows.json`.
