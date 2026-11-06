#!/usr/bin/env bash
#
# Install a clang new enough to build livekit's webrtc-sys on Linux. Copied from
# livekit/rust-sdks .github/scripts/install-clang.sh.
#
# webrtc-sys compiles against the hermetic libc++ shipped inside the libwebrtc
# artifact (use_custom_libcxx=true). That libc++ tracks LLVM trunk and uses
# builtins that only exist in a recent clang, so the distro clang on the hosted
# runners (Ubuntu 22: clang 14, Ubuntu 24: clang 18) fails deep inside <limits>
# and <span> instead of saying the compiler is too old. webrtc-sys/build.rs
# checks the version up front and reports the real floor, which it reads from
# the artifact's own __configuration/compiler.h.
#
# The distro package managers have nothing recent enough, so pull the official
# LLVM release tarball instead. If a runner ever rejects it for glibc reasons,
# LLVM_VERSION is the knob to turn.
#
# Prints the bin directory on stdout; everything else goes to stderr. Also
# exports CC/CXX via $GITHUB_ENV when running as a workflow step, so the cargo
# steps that follow pick it up. Workflows run it through
# .github/actions/bevy-explorer-clang, which keeps LLVM_ROOT in the Actions
# cache so only the first job after a version bump downloads the tarball.

set -euo pipefail

LLVM_VERSION="${LLVM_VERSION:-21.1.8}"
LLVM_ROOT="${LLVM_ROOT:-/opt/llvm-$LLVM_VERSION}"

# The SHA-256 of each official release tarball, as the llvm-project release
# assets report it. A new version needs its pins here.
case "$LLVM_VERSION/$(uname -m)" in
  21.1.8/x86_64)
    llvm_arch=X64; sha256=b3b7f2801d15d50736acea3c73982994d025b01c2f035b91ae3b49d1b575732b ;;
  21.1.8/aarch64 | 21.1.8/arm64)
    llvm_arch=ARM64; sha256=65ce0b329514e5643407db2d02a5bd34bf33d159055dafa82825c8385bd01993 ;;
  *) echo "install-clang.sh: no pinned LLVM $LLVM_VERSION release for $(uname -m)" >&2; exit 1 ;;
esac

# /opt needs root; the runner's tool cache does not.
if [ "$(id -u)" -eq 0 ] || [ -w "$(dirname "$LLVM_ROOT")" ]; then
  sudo=""
else
  sudo="sudo"
fi

if [ ! -x "$LLVM_ROOT/bin/clang++" ]; then
  release="LLVM-$LLVM_VERSION-Linux-$llvm_arch"
  url="https://github.com/llvm/llvm-project/releases/download/llvmorg-$LLVM_VERSION/$release.tar.xz"
  archive=$(mktemp --tmpdir "$release.XXXXXX")
  trap 'rm -f "$archive"' EXIT
  echo "install-clang.sh: fetching $url" >&2
  curl --fail --location --silent --show-error --output "$archive" "$url"
  echo "$sha256  $archive" | sha256sum --check --status \
    || { echo "install-clang.sh: $release.tar.xz does not match its pinned SHA-256" >&2; exit 1; }
  $sudo mkdir -p "$LLVM_ROOT"
  # Keep only the clang driver and its resource directory (builtin headers and
  # compiler-rt): 350 MB of the 11 GB release, and all that webrtc-sys and the
  # cc crate use. --strip-components=1 drops the $release/ prefix.
  $sudo tar -xJf "$archive" --strip-components=1 -C "$LLVM_ROOT" \
      "$release/bin/clang" "$release/bin/clang++" \
      "$release/bin/clang-${LLVM_VERSION%%.*}" "$release/lib/clang"
fi

"$LLVM_ROOT/bin/clang++" --version >&2

if [ -n "${GITHUB_ENV:-}" ]; then
  {
    echo "CC=$LLVM_ROOT/bin/clang"
    echo "CXX=$LLVM_ROOT/bin/clang++"
  } >> "$GITHUB_ENV"
fi

echo "$LLVM_ROOT/bin"
