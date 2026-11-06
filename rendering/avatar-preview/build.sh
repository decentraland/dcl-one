#!/usr/bin/env sh
# Builds the wasm modules with the Explorer's toolchain: web/pkg for WebGPU and web/pkg-gl for
# WebGL2 (the engine holds one backend per module; the page loads the one the browser supports).
#   ./build.sh          the shipping builds: size-first release profile and wasm-opt (minutes)
#   ./build.sh --fast   the iteration builds: tuned dev profile, no wasm-opt (about a minute, larger)
#   --webgpu-only       skips the WebGL2 module, for quicker iteration on WebGPU
# Other arguments go to wasm-pack.
set -eu
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"
mode=--release
gl=yes
for arg in "$@"; do
  case "$arg" in
    --fast) mode=--dev ;;
    --webgpu-only) gl=no ;;
    *) set -- "$@" "$arg" ;;
  esac
  shift
done
start=$(date +%s)
build() {
  out=$1
  shift
  RUSTUP_TOOLCHAIN=nightly-2026-04-15 wasm-pack build $mode --target web --out-dir "$out" "$@"
  rm -f "$out/.gitignore"
  # the build id the page keys its module requests on, so a rebuilt module is never served stale
  printf '{"build":"%s"}\n' "$(date +%s)" > "$out/build.json"
  ls -l "$out"/*.wasm
}
build web/pkg "$@"
if [ "$gl" = yes ]; then
  build web/pkg-gl "$@" -- --no-default-features --features webgl2
fi
echo "built in $(( $(date +%s) - start )) s"
