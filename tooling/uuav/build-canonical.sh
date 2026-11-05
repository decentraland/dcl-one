#!/bin/bash
#
# Canonical build: the shipped bytes must not depend on where the source was
# checked out, so HEAD is exported to one fixed directory (the canonical root)
# and built there. Two builds from two checkouts through this script are
# therefore comparable byte-for-byte, which is what repro-gate.sh relies on.

set -euo pipefail

case "$(uname -s)" in
Darwin) CANONICAL_ROOT="${UUAV_CANONICAL_ROOT:-/Users/Shared/build}" ;;
Linux) CANONICAL_ROOT="${UUAV_CANONICAL_ROOT:-/var/tmp/build}" ;;
*) CANONICAL_ROOT="${UUAV_CANONICAL_ROOT:-/c/build}" ;;
esac

REPO="$(git rev-parse --show-toplevel)"
UUAV="library/uuav"
TOOLING="tooling/uuav"
REV="$(git -C "$REPO" rev-parse HEAD)"

case "$(uname -s)" in
Darwin) TARGET="macos-universal" ;;
Linux) TARGET="linux-$(uname -m)" ;;
*) TARGET="windows-x86_64" ;;
esac

if ! git -C "$REPO" diff --quiet HEAD -- "$UUAV" "$TOOLING" ":(exclude)$UUAV/dist"; then
    echo "error: $UUAV or $TOOLING has uncommitted changes, and the build takes its source from HEAD - they would not be in it" >&2
    exit 1
fi

if [[ -L "$CANONICAL_ROOT" ]]; then
    echo "error: $CANONICAL_ROOT is a symlink; refusing to build shipped binaries through it" >&2
    exit 1
fi
if [[ -e "$CANONICAL_ROOT" && ! -O "$CANONICAL_ROOT" ]]; then
    echo "error: $CANONICAL_ROOT exists and is not owned by you; refusing to build shipped binaries in it" >&2
    exit 1
fi

FFMPEG_SRC="$REPO/$UUAV/.third_party/ffmpeg"
if [[ ! -d "$FFMPEG_SRC" ]]; then
    echo "error: $FFMPEG_SRC is missing; build it with $UUAV/scripts/build-ffmpeg-macos.sh (macOS) or build-ffmpeg-linux.sh (Linux), or fetch it with $TOOLING/fetch-ffmpeg-windows.sh (Windows) first" >&2
    exit 1
fi

rm -rf "$CANONICAL_ROOT"
mkdir -p "$CANONICAL_ROOT"

git -C "$REPO" -c core.autocrlf=false -c core.eol=lf archive "$REV" "$UUAV" "$TOOLING" |
    tar -x -C "$CANONICAL_ROOT"

CANONICAL_NATIVE="$CANONICAL_ROOT/$UUAV"
mkdir -p "$CANONICAL_NATIVE/.third_party"
cp -R "$FFMPEG_SRC" "$CANONICAL_NATIVE/.third_party/ffmpeg"
# the license texts build.sh ships come from the source checkouts
for src in .ffmpeg-src .mbedtls-src .libva-src; do
    if [[ -d "$REPO/$UUAV/$src" ]]; then
        mkdir -p "$CANONICAL_NATIVE/$src"
        for name in LICENSE.md COPYING.LGPLv3 LICENSE COPYING; do
            [[ -f "$REPO/$UUAV/$src/$name" ]] && cp "$REPO/$UUAV/$src/$name" "$CANONICAL_NATIVE/$src/$name"
        done
    fi
done

(cd "$CANONICAL_NATIVE" && bash build.sh)

DEPLOYED="$CANONICAL_NATIVE/dist/$TARGET"
DEST="$REPO/$UUAV/dist/$TARGET"
rm -rf "$DEST"
mkdir -p "$DEST"
cp -R "$DEPLOYED/." "$DEST/"

echo "built at $CANONICAL_ROOT from $REV, deployed to $DEST"
