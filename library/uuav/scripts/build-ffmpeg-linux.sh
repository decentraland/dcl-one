#!/bin/bash
# Builds FFmpeg 8.1 (LGPL, shared) from source for Linux into
# .third_party/ffmpeg - the Linux analog of build-ffmpeg-macos.sh - together
# with the two libraries the helper ships next to it:
#
#   mbedTLS   the TLS backend, statically linked into libavformat (as on
#             macOS: no OpenSSL soname to depend on)
#   libva     the VAAPI client library FFmpeg's hardware decoder talks to,
#             built from source and shipped (libva.so.2, libva-drm.so.2) so
#             the package does not depend on the distribution's libva; the
#             actual driver (iHD/i965/radeonsi/...) is dlopened from the
#             distribution's DRI directories at runtime
#
# Everything shipped gets a `$ORIGIN` RUNPATH (meson sets it on libva;
# FFmpeg's make would expand the `$` in an --extra-ldflags value, so its
# libraries are stamped with patchelf after install), so the set resolves
# from the helper's own directory; the remaining shared dependencies are
# glibc, zlib, libxml2 (the DASH demuxer) and libdrm (libva's DRM backend),
# all present on every desktop distribution. --disable-autodetect keeps
# FFmpeg from picking up whatever else the build host has installed.
#
# The build runs on x86_64 (the shipped target) and on aarch64 (local
# container testing); the host architecture is the target.

set -euo pipefail

# FFmpeg release in lockstep with ffmpeg-sys-next in Cargo.toml
FFMPEG_TAG="n8.1"
# mbedTLS LTS line, linked statically into libavformat as the TLS backend
MBEDTLS_TAG="mbedtls-3.6.7"
# libva release; VAAPI client library shipped next to the helper
LIBVA_TAG="2.22.0"

NATIVE_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SRC_DIR="$NATIVE_DIR/.ffmpeg-src"
MBEDTLS_SRC_DIR="$NATIVE_DIR/.mbedtls-src"
LIBVA_SRC_DIR="$NATIVE_DIR/.libva-src"
BUILD_DIR="$NATIVE_DIR/.ffmpeg-build"
PREFIX="$NATIVE_DIR/.third_party/ffmpeg"

if [[ "$(uname -s)" != "Linux" ]]; then
    echo "error: this script builds the Linux FFmpeg; run build-ffmpeg-macos.sh on macOS" >&2
    exit 1
fi

for tool in nasm pkg-config cmake meson ninja gcc git patchelf; do
    if ! command -v "$tool" > /dev/null; then
        echo "error: $tool not found; install it with 'apt-get install nasm pkg-config cmake meson ninja-build build-essential git patchelf'" >&2
        exit 1
    fi
done
# the DASH demuxer hard-requires libxml2; libva's DRM backend needs libdrm
for pc in libxml-2.0 libdrm zlib; do
    if ! pkg-config --exists "$pc"; then
        echo "error: pkg-config cannot find $pc; install libxml2-dev, libdrm-dev and zlib1g-dev" >&2
        exit 1
    fi
done

if [[ ! -d "$SRC_DIR" ]]; then
    git clone --depth 1 --branch "$FFMPEG_TAG" https://github.com/FFmpeg/FFmpeg.git "$SRC_DIR"
fi
# the framework submodule is a hard CMake prerequisite even with tests off
if [[ ! -d "$MBEDTLS_SRC_DIR" ]]; then
    git clone --depth 1 --recurse-submodules --shallow-submodules --branch "$MBEDTLS_TAG" \
        https://github.com/Mbed-TLS/mbedtls.git "$MBEDTLS_SRC_DIR"
fi
if [[ ! -d "$LIBVA_SRC_DIR" ]]; then
    git clone --depth 1 --branch "$LIBVA_TAG" https://github.com/intel/libva.git "$LIBVA_SRC_DIR"
fi

# out-of-tree builds refuse to configure over a stale in-tree configuration
[[ -f "$SRC_DIR/config.h" ]] && make -C "$SRC_DIR" distclean

# wipe previous outputs so a stale prefix from an older run can't leak through
rm -rf "$PREFIX" "$BUILD_DIR"
mkdir -p "$BUILD_DIR" "$PREFIX"

if ! (cd "$BUILD_DIR" && "$SRC_DIR/configure" --help | grep mbedtls > /dev/null); then
    echo "error: this FFmpeg has no --enable-mbedtls" >&2
    exit 1
fi

JOBS="$(nproc)"

# ---- mbedTLS: static archives, position independent (they end up inside
# the shared libavformat), consumed from a private prefix
MBEDTLS_PREFIX="$BUILD_DIR/mbedtls/prefix"
cmake -S "$MBEDTLS_SRC_DIR" -B "$BUILD_DIR/mbedtls" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$MBEDTLS_PREFIX" \
    -DCMAKE_INSTALL_LIBDIR=lib \
    -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
    -DENABLE_PROGRAMS=OFF \
    -DENABLE_TESTING=OFF \
    -DUSE_SHARED_MBEDTLS_LIBRARY=OFF \
    -DUSE_STATIC_MBEDTLS_LIBRARY=ON \
    -DMBEDTLS_FATAL_WARNINGS=OFF
cmake --build "$BUILD_DIR/mbedtls" -j"$JOBS"
cmake --install "$BUILD_DIR/mbedtls"

# mbedtls.pc lists mbedx509/mbedcrypto under Requires.private, which
# pkg-config only expands with --static - and --static would also drag
# libxml2's private ICU/lzma dependencies into libavformat. A private .pc
# naming all three archives outright keeps the rest of the build dynamic.
mkdir -p "$BUILD_DIR/pkgconfig"
cat > "$BUILD_DIR/pkgconfig/mbedtls.pc" <<EOF
prefix=$MBEDTLS_PREFIX
libdir=\${prefix}/lib
includedir=\${prefix}/include

Name: mbedtls
Description: Mbed TLS, static archives for libavformat
Version: ${MBEDTLS_TAG#mbedtls-}
Libs: -L\${libdir} -lmbedtls -lmbedx509 -lmbedcrypto
Cflags: -I\${includedir}
EOF

# ---- libva: shared, no window-system backends (the helper is headless),
# installed straight into the shipped prefix. The driver directories are the
# distribution's: that is where the hardware-specific VA driver lives.
meson setup "$BUILD_DIR/libva" "$LIBVA_SRC_DIR" \
    --prefix="$PREFIX" \
    --libdir=lib \
    --buildtype=release \
    --default-library=shared \
    -Ddriverdir=/usr/lib/x86_64-linux-gnu/dri:/usr/lib/aarch64-linux-gnu/dri:/usr/lib64/dri:/usr/lib/dri \
    -Dwith_x11=no \
    -Dwith_glx=no \
    -Dwith_wayland=no \
    -Denable_docs=false \
    -Dc_link_args='-Wl,-rpath,$ORIGIN'
ninja -C "$BUILD_DIR/libva"
ninja -C "$BUILD_DIR/libva" install

# ---- FFmpeg
# LGPL is the default (no --enable-gpl); --enable-version3 lifts it to
# LGPL-3.0, which configure demands for the Apache-2.0 mbedTLS.
# --disable-autodetect: nothing from the build host's library set may leak
# into the shipped dylibs; every external library is enabled by name.
# Do NOT disable avdevice/avfilter: ffmpeg-sys-next's default features link
# all seven libraries.
cd "$BUILD_DIR"
PKG_CONFIG_PATH="$BUILD_DIR/pkgconfig:$PREFIX/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}" \
"$SRC_DIR/configure" \
    --prefix="$PREFIX" \
    --enable-shared \
    --disable-static \
    --disable-programs \
    --disable-doc \
    --disable-autodetect \
    --enable-pthreads \
    --enable-zlib \
    --enable-libxml2 \
    --enable-mbedtls \
    --enable-version3 \
    --enable-libdrm \
    --enable-vaapi
make -j"$JOBS"
make install

# the FFmpeg libraries resolve each other and libva from their own
# directory, exactly like the helper does
for lib in "$PREFIX"/lib/libav*.so.*.*.* "$PREFIX"/lib/libsw*.so.*.*.*; do
    [[ -L "$lib" ]] && continue
    patchelf --set-rpath '$ORIGIN' "$lib"
done

echo
echo "Installed into: $PREFIX"

# ---- regression gates
# 1. every shipped library resolves its siblings from $ORIGIN and nothing
#    else from a build-host path
# 2. the only non-shipped dependencies are glibc, zlib, libxml2 and libdrm
# 3. TLS comes from the statically linked mbedTLS, never OpenSSL/GnuTLS
allowed_needed='^(libc|libm|libpthread|libdl|librt|libgcc_s|ld-linux-[a-z0-9-]+|libz|libxml2|libdrm|libav[a-z]+|libsw[a-z]+|libva|libva-drm)\.so(\.[0-9]+)*$'
status=0
for lib in "$PREFIX"/lib/lib*.so.*.*; do
    [[ -L "$lib" ]] && continue
    name="$(basename "$lib")"
    runpath="$(readelf -d "$lib" | awk '/RUNPATH|RPATH/ {gsub(/[\[\]]/, "", $NF); print $NF}')"
    if [[ "$runpath" != '$ORIGIN' ]]; then
        echo "error: $name RUNPATH is '$runpath', expected '\$ORIGIN'" >&2
        status=1
    fi
    while read -r needed; do
        if ! [[ "$needed" =~ $allowed_needed ]]; then
            echo "error: $name links $needed, which is not shipped and not a base-system library" >&2
            status=1
        fi
    done < <(readelf -d "$lib" | awk '/NEEDED/ {gsub(/[\[\]]/, "", $NF); print $NF}')
    printf '%-28s RUNPATH=%s\n' "$name" "$runpath"
done
if [[ "$status" -ne 0 ]]; then
    echo "error: FFmpeg build leaked build-host dependencies; see above" >&2
    exit 1
fi
echo "All shared libraries resolve from \$ORIGIN with base-system dependencies only."

avformat="$(ls "$PREFIX"/lib/libavformat.so.*.*.*)"
if readelf -d "$avformat" | grep -E 'libssl|libgnutls' > /dev/null; then
    echo "error: libavformat links an external TLS library" >&2
    exit 1
fi
if ! strings -a "$avformat" | grep 'mbedtls_ssl_config_defaults' > /dev/null; then
    echo "error: libavformat did not link mbedTLS" >&2
    exit 1
fi
echo "TLS backend: mbedTLS $MBEDTLS_TAG (static), no external TLS library."

avcodec="$(ls "$PREFIX"/lib/libavcodec.so.*.*.*)"
if ! readelf -d "$avcodec" | grep 'libva.so' > /dev/null; then
    echo "error: libavcodec does not link libva; VAAPI decoding is off" >&2
    exit 1
fi
echo "VAAPI: libavcodec links the shipped libva $LIBVA_TAG."
