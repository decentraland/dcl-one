#!/usr/bin/env sh
# Builds the wasm module into web/pkg with the Explorer's toolchain. `./build.sh --no-opt`
# skips wasm-opt for quick iteration.
set -eu
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"
RUSTUP_TOOLCHAIN=nightly-2026-04-15 wasm-pack build --target web --out-dir web/pkg "$@"
rm -f web/pkg/.gitignore
ls -l web/pkg/*.wasm
