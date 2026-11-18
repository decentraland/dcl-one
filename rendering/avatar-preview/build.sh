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
repo_root=${DCL_ONE_ROOT:-$(git rev-parse --show-toplevel)}
out_root=${AVATAR_PREVIEW_OUT:-web}
start=$(date +%s)
build() {
  out=$1
  shift
  RUSTUP_TOOLCHAIN=nightly-2026-04-15 "${WASM_PACK:-wasm-pack}" build $mode --target web --out-dir "$out" "$@"
  cp "$repo_root/rendering/bevy-explorer/source/deploy/web/engine/content_cache.js" "$out/content_cache.js"
  rm -f "$out/.gitignore"
  # the build id the page keys its module requests on, so a rebuilt module is never served stale
  printf '{"build":"%s"}\n' "$(date +%s)" > "$out/build.json"
  ls -l "$out"/*.wasm
}
build "$out_root/pkg" "$@"
if [ "$gl" = yes ]; then
  build "$out_root/pkg-gl" "$@" -- --no-default-features --features webgl2
fi
echo "built in $(( $(date +%s) - start )) s"
