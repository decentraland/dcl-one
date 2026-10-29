#!/usr/bin/env bash
# Regenerate apps/ui3/src/play/engine/generated from the Explorer's Rust structs
# (crates/system_api_types, ts-rs) whenever those structs change.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
source_dir="$root/rendering/bevy-explorer/source"
[ -f "$source_dir/Cargo.toml" ] || { echo "$source_dir/Cargo.toml is missing" >&2; exit 1; }
out="$root/apps/ui3/src/play/engine/generated"
rm -rf "$out"
mkdir -p "$out"
TS_RS_EXPORT_DIR="$out" cargo test --manifest-path "$source_dir/Cargo.toml" -p system_api_types export_ "$@"
cd "$out"
: > index.ts
for f in $(ls ./*.ts | grep -v '^\./index\.ts$' | LC_ALL=C sort); do
  f="${f#./}"
  echo "export * from './${f%.ts}'" >> index.ts
done
echo "wrote $(ls | wc -l | tr -d ' ') files to $out"
