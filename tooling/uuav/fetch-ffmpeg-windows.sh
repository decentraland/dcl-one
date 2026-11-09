#!/usr/bin/env bash

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PIN="$REPO_ROOT/tooling/uuav/ffmpeg-windows.json"
PREFIX="${1:-$REPO_ROOT/library/uuav/.third_party/ffmpeg}"

for PYTHON in python3 python; do
    "$PYTHON" -c pass >/dev/null 2>&1 && break
done

read -r URL EXPECTED_SHA ASSET RELEASE_TAG < <(
    "$PYTHON" -c '
import json, sys
sys.stdout.reconfigure(newline="\n")
pin = json.load(open(sys.argv[1]))
print(pin["url"], pin["sha256"], pin["asset"], pin["release_tag"])' "$PIN"
)

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "Release: $RELEASE_TAG"
echo "Asset:   $ASSET"
curl --fail --location --show-error --silent --output "$WORK/ffmpeg.zip" "$URL"

ACTUAL_SHA="$(sha256sum "$WORK/ffmpeg.zip" | cut -d' ' -f1)"
if [[ "$ACTUAL_SHA" != "$EXPECTED_SHA" ]]; then
    echo "error: archive does not match $PIN - refusing to unpack." >&2
    echo "  expected sha256 $EXPECTED_SHA" >&2
    echo "  actual   sha256 $ACTUAL_SHA" >&2
    exit 1
fi
echo "sha256:  $ACTUAL_SHA (matches the pin)"

unzip -q "$WORK/ffmpeg.zip" -d "$WORK/unpacked"
SRC="$(find "$WORK/unpacked" -mindepth 1 -maxdepth 1 -type d)"

rm -rf "$PREFIX"
mkdir -p "$PREFIX"
cp -R "$SRC"/. "$PREFIX"/

echo "Unpacked into: $PREFIX"
