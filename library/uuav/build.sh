#!/bin/bash
# Builds uuav-helper with cargo against the FFmpeg prefix in .third_party/ffmpeg
# and assembles the runtime set under dist/<target>/ from the same prefix.
# `nix build .#uuav-dist` builds the same set on Linux and macOS.

set -euo pipefail

NATIVE_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$NATIVE_DIR"
FFMPEG="$NATIVE_DIR/.third_party/ffmpeg"

ship_licenses() {
    cp "$FFMPEG"/share/uuav/LICENSE-*.txt "$1/"
}

case "$(uname -s)" in
Darwin)
    TARGET="aarch64-apple-darwin"
    DEST="$NATIVE_DIR/dist/macos-arm64"
    if [[ ! -f "$FFMPEG/lib/libavutil.dylib" ]]; then
        echo "error: $FFMPEG is missing; link it with 'nix build .#uuav-ffmpeg -o library/uuav/.third_party/ffmpeg' from the repository root" >&2
        exit 1
    fi
    export MACOSX_DEPLOYMENT_TARGET=11.0
    cargo build --release -p uuav-server --target "$TARGET"
    rm -rf "$DEST"
    mkdir -p "$DEST"
    cp ".target/$TARGET/release/uuav-helper" "$DEST/"
    for lib in avcodec avdevice avfilter avformat avutil swresample swscale; do
        major=$(find "$FFMPEG/lib/" -name "lib$lib.*.dylib" | grep -E "lib$lib\.[0-9]+\.dylib$")
        cp -L "$major" "$DEST/"
    done
    ship_licenses "$DEST"
    chmod -R u+w "$DEST"
    codesign -f -s - "$DEST"/*.dylib "$DEST/uuav-helper"
    echo "Deployed to: $DEST"
    ;;
Linux)
    ARCH="$(uname -m)"
    TARGET="$ARCH-unknown-linux-gnu"
    DEST="$NATIVE_DIR/dist/linux-$ARCH"
    if [[ ! -f "$FFMPEG/lib/libavutil.so" ]]; then
        echo "error: $FFMPEG is missing; link it with 'nix build .#uuav-ffmpeg -o library/uuav/.third_party/ffmpeg' from the repository root" >&2
        exit 1
    fi
    cargo build --release -p uuav-server --target "$TARGET"
    rm -rf "$DEST"
    mkdir -p "$DEST"
    cp ".target/$TARGET/release/uuav-helper" "$DEST/"
    for lib in avcodec avdevice avfilter avformat avutil swresample swscale va va-drm; do
        major=$(find "$FFMPEG/lib/" -name "lib$lib.so.*" | grep -E "lib$lib\.so\.[0-9]+$")
        cp -L "$major" "$DEST/"
    done
    ship_licenses "$DEST"
    chmod -R u+w "$DEST"
    for file in "$DEST"/uuav-helper "$DEST"/lib*.so.*; do
        [[ $(patchelf --print-rpath "$file") == '$ORIGIN'* ]] || patchelf --set-rpath "\$ORIGIN:$(patchelf --print-rpath "$file")" "$file"
    done
    echo "Deployed to: $DEST"
    ;;
*)
    TARGET="x86_64-pc-windows-gnu"
    DEST="$NATIVE_DIR/dist/windows-x86_64"
    if [ ! -d "$FFMPEG/bin" ]; then
        echo "error: $FFMPEG/bin is missing; fetch the pinned BtbN LGPL shared build with tooling/uuav/fetch-ffmpeg-windows.sh" >&2
        exit 1
    fi
    if [ -z "${BINDGEN_EXTRA_CLANG_ARGS:-}" ]; then
        BINDGEN_EXTRA_CLANG_ARGS="$(bash "$NATIVE_DIR/../../tooling/uuav/bindgen-clang-args.sh")"
        export BINDGEN_EXTRA_CLANG_ARGS
    fi
    cargo build --release -p uuav-server --target "$TARGET"
    rm -rf "$DEST"
    mkdir -p "$DEST"
    cp ".target/$TARGET/release/uuav-helper.exe" "$DEST/"
    for lib in avcodec avformat avutil swresample swscale; do
        cp "$FFMPEG/bin/$lib"-*.dll "$DEST/"
    done
    cp "$FFMPEG/LICENSE.txt" "$DEST/LICENSE-ffmpeg.txt"
    echo "Deployed to: $DEST"
    ;;
esac
