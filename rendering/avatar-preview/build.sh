#!/usr/bin/env sh
# Builds the wasm module into web/pkg with the Explorer's toolchain.
#   ./build.sh          the shipping build: size-first release profile and wasm-opt (minutes)
#   ./build.sh --fast   the iteration build: tuned dev profile, no wasm-opt (about a minute, larger)
# Other arguments go to wasm-pack.
set -eu
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"
mode=--release
for arg in "$@"; do
  case "$arg" in
    --fast) mode=--dev ;;
    *) set -- "$@" "$arg" ;;
  esac
  shift
done
start=$(date +%s)
RUSTUP_TOOLCHAIN=nightly-2026-04-15 wasm-pack build $mode --target web --out-dir web/pkg "$@"
rm -f web/pkg/.gitignore
# the build id the page keys its module requests on, so a rebuilt module is never served stale
printf '{"build":"%s"}\n' "$(date +%s)" > web/pkg/build.json
ls -l web/pkg/*.wasm
echo "built in $(( $(date +%s) - start )) s"
