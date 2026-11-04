#!/bin/bash
# Verifies a macOS UUAV dist directory: every dylib and the helper must be
# a universal (arm64 + x86_64) Mach-O binary. An unfetched Git LFS pointer
# or a thin slice fails the check. Usage: doctor-libs-macos.sh <dist-dir>

set -euo pipefail

DIR="${1:?usage: doctor-libs-macos.sh <dist-dir>}"
status=0
for file in "$DIR"/*.dylib "$DIR"/uuav-helper; do
    [[ -e "$file" ]] || continue
    name="$(basename "$file")"
    if ! archs="$(lipo -archs "$file" 2> /dev/null)"; then
        echo "error: $name is not a Mach-O binary (an unfetched LFS pointer?)" >&2
        status=1
        continue
    fi
    if ! lipo "$file" -verify_arch arm64 x86_64 2> /dev/null; then
        echo "error: $name is not universal: [$archs]" >&2
        status=1
        continue
    fi
    printf '  universal  %-28s [%s]\n' "$name" "$archs"
done
if [[ "$status" -ne 0 ]]; then
    echo "error: $DIR is not a complete universal set" >&2
    exit 1
fi
echo "All binaries in $DIR are universal."
