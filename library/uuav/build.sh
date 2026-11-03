#!/bin/bash
# Builds uuav-helper for this host and assembles the shipped set under
# dist/<target>/: the helper, the FFmpeg runtime libraries it links (taken
# from the same .third_party/ffmpeg it linked against, so the majors can
# never drift apart) and the license texts of what ships. The host links
# uuav-client as a plain Rust library, so nothing else is produced here.
#
# Targets: macos-universal (built on Apple Silicon), windows-x86_64 (mingw)
# and linux-<arch> (x86_64 is the shipped one; aarch64 serves container
# testing). The FFmpeg prefix comes from scripts/build-ffmpeg-macos.sh,
# tooling/uuav/fetch-ffmpeg-windows.sh or scripts/build-ffmpeg-linux.sh.

set -euo pipefail

NATIVE_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$NATIVE_DIR"
FFMPEG="$NATIVE_DIR/.third_party/ffmpeg"

# concatenates the license texts of a from-source FFmpeg build (the summary
# plus the LGPL-3.0 text --enable-version3 selects) and mbedTLS
ship_source_licenses() {
    local dest="$1"
    cat "$NATIVE_DIR/.ffmpeg-src/LICENSE.md" "$NATIVE_DIR/.ffmpeg-src/COPYING.LGPLv3" > "$dest/LICENSE-ffmpeg.txt"
    cp "$NATIVE_DIR/.mbedtls-src/LICENSE" "$dest/LICENSE-mbedtls.txt"
}

case "$(uname -s)" in
Darwin)
    DEST="$NATIVE_DIR/dist/macos-universal"
    FFMPEG_LIB="$FFMPEG/lib"

    # Rust's x86_64-apple-darwin default is 10.12; match FFmpeg's slices
    export MACOSX_DEPLOYMENT_TARGET=11.0

    # macOS binaries ship universal; a thin FFmpeg means a stale prefix
    if ! lipo "$FFMPEG_LIB/libavutil.dylib" -verify_arch arm64 x86_64 2> /dev/null; then
        echo "error: $FFMPEG_LIB dylibs are missing or not universal (arm64 + x86_64); re-run scripts/build-ffmpeg-macos.sh" >&2
        exit 1
    fi

    cargo build --release -p uuav-server --target aarch64-apple-darwin
    cargo build --release -p uuav-server --target x86_64-apple-darwin

    rm -rf "$DEST"
    mkdir -p "$DEST"
    # the helper hosts the decode core out of process; the FFmpeg dylibs
    # resolve next to it via its @loader_path rpath
    lipo -create \
        ".target/aarch64-apple-darwin/release/uuav-helper" \
        ".target/x86_64-apple-darwin/release/uuav-helper" \
        -output "$DEST/uuav-helper"

    # deploy each FFmpeg dylib under its major-version name (dereferencing
    # the libavutil.60.dylib -> libavutil.60.26.100.dylib symlink): that is
    # the exact name the @rpath install-name references resolve to
    for lib in avcodec avdevice avfilter avformat avutil swresample swscale; do
        major=$(find "$FFMPEG_LIB" -type l -name "lib$lib.*.dylib" | grep -E "lib$lib\.[0-9]+\.dylib$")
        cp -L "$major" "$DEST/"
    done
    ship_source_licenses "$DEST"

    # ad-hoc code signing is mandatory on arm64; lipo and the copies above
    # invalidate whatever signature the build produced, so sign last
    codesign -f -s - "$DEST"/*.dylib "$DEST/uuav-helper"

    echo "Deployed to: $DEST"

    # verification gate: fails the build (set -e) if any deployed binary is thin
    bash "$NATIVE_DIR/../../tooling/uuav/doctor-libs-macos.sh" "$DEST"
    ;;
Linux)
    ARCH="$(uname -m)"
    TARGET="$ARCH-unknown-linux-gnu"
    DEST="$NATIVE_DIR/dist/linux-$ARCH"
    FFMPEG_LIB="$FFMPEG/lib"

    if [[ ! -f "$FFMPEG_LIB/libavutil.so" ]]; then
        echo "error: $FFMPEG_LIB is missing; build it with scripts/build-ffmpeg-linux.sh first" >&2
        exit 1
    fi

    cargo build --release -p uuav-server --target "$TARGET"

    rm -rf "$DEST"
    mkdir -p "$DEST"
    cp ".target/$TARGET/release/uuav-helper" "$DEST/"

    # the major-version names the sonames reference, dereferenced
    for lib in avcodec avdevice avfilter avformat avutil swresample swscale va va-drm; do
        major=$(find "$FFMPEG_LIB" -type l -name "lib$lib.so.*" | grep -E "lib$lib\.so\.[0-9]+$")
        cp -L "$major" "$DEST/"
    done
    ship_source_licenses "$DEST"
    cp "$NATIVE_DIR/.libva-src/COPYING" "$DEST/LICENSE-libva.txt"

    echo "Deployed to: $DEST"
    ;;
*)
    TARGET="x86_64-pc-windows-gnu" # linker configured in .cargo/config.toml
    DEST="$NATIVE_DIR/dist/windows-x86_64"
    FFMPEG_BIN="$FFMPEG/bin"

    # deploying runtime DLLs from a different FFmpeg build than the import
    # libs the helper just linked is exactly the mismatch this guard catches
    if [ ! -d "$FFMPEG_BIN" ]; then
        echo "error: $FFMPEG_BIN is missing; fetch the pinned BtbN LGPL shared build with tooling/uuav/fetch-ffmpeg-windows.sh" >&2
        exit 1
    fi

    cargo build --release -p uuav-server --target "$TARGET"

    rm -rf "$DEST"
    mkdir -p "$DEST"
    # the helper's FFmpeg DLLs resolve from its own directory
    cp ".target/$TARGET/release/uuav-helper.exe" "$DEST/"

    # FFmpeg runtime DLLs from the exact build the helper linked against.
    # The helper's import closure is these five (avfilter/avdevice are never
    # imported; swscale converts software-decoded frames to NV12), and the
    # BtbN shared build links its support libs (bz2/iconv/lzma/zlib/
    # winpthread) statically.
    for lib in avcodec avformat avutil swresample swscale; do
        cp "$FFMPEG_BIN/$lib"-*.dll "$DEST/"
    done
    cp "$FFMPEG/LICENSE.txt" "$DEST/LICENSE-ffmpeg.txt"

    echo "Deployed to: $DEST"
    ;;
esac
