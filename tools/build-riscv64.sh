#!/bin/bash
#
# build-riscv64.sh - Cross-compile CSPCL for 64-bit RISC-V Linux
#
# Builds static libsocketcan, a minimal static libzmq, libbz2 and libcsp v1.6
# for riscv64, then CSPCL itself. Everything is linked statically so the test
# binaries can be copied straight onto a PolarFire SoC board.
#
# The dependencies are built with -fPIC so tools/build-picod3tn-riscv64.sh can
# link them into Rust binaries too.
#
# Requires: gcc-riscv64-linux-gnu g++-riscv64-linux-gnu cmake python3 git
#           autoconf automake libtool pkg-config
# Optional: qemu-user-static (to run the unit tests on the host)
#
# Environment:
#   DEPS_DIR    where dependencies are built/installed (default: /tmp/riscv64-deps)
#   BUILD_DIR   CSPCL build directory (default: <repo>/build-riscv64)
#   BUILD_TYPE  CMake build type (default: Release)
#   RUN_TESTS   1 to run ctest under qemu after building (default: 0)
#   PYTHON      interpreter for libcsp's waf, must be <= 3.11 (default: python3)
#   DEPS_ONLY   1 to build the dependencies only, not CSPCL (default: 0)
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"

TRIPLE=riscv64-linux-gnu
TOOLCHAIN_FILE="$REPO_DIR/cmake/$TRIPLE.cmake"
DEPS_DIR="${DEPS_DIR:-/tmp/riscv64-deps}"
PREFIX="$DEPS_DIR/sysroot"
BUILD_DIR="${BUILD_DIR:-$REPO_DIR/build-riscv64}"
BUILD_TYPE="${BUILD_TYPE:-Release}"
RUN_TESTS="${RUN_TESTS:-0}"
PYTHON="${PYTHON:-python3}"
DEPS_ONLY="${DEPS_ONLY:-0}"
JOBS="$(nproc)"

LIBSOCKETCAN_VERSION=v0.0.12
LIBZMQ_VERSION=v4.3.5
LIBCSP_VERSION=v1.6
BZIP2_VERSION=bzip2-1.0.8

mkdir -p "$DEPS_DIR" "$PREFIX"

fetch() {
    local url="$1" tag="$2" dir="$3"
    if [ ! -d "$dir" ]; then
        git clone --branch "$tag" --depth 1 "$url" "$dir"
    fi
}

build_libsocketcan() {
    if [ -f "$PREFIX/lib/libsocketcan.a" ]; then
        echo "✓ libsocketcan already built"
        return
    fi
    local src="$DEPS_DIR/libsocketcan"
    fetch https://github.com/linux-can/libsocketcan.git "$LIBSOCKETCAN_VERSION" "$src"
    (
        cd "$src"
        ./autogen.sh
        CFLAGS="-O2 -fPIC" \
            ./configure --host="$TRIPLE" --prefix="$PREFIX" --disable-shared --enable-static
        make -j"$JOBS"
        make install
    )
}

build_libzmq() {
    if [ -f "$PREFIX/lib/libzmq.a" ]; then
        echo "✓ libzmq already built"
        return
    fi
    local src="$DEPS_DIR/libzmq"
    fetch https://github.com/zeromq/libzmq.git "$LIBZMQ_VERSION" "$src"
    # Minimal build: no CURVE/libsodium, no GSSAPI/PGM/NORM, so the static
    # library only depends on libstdc++, pthread and rt.
    cmake -S "$src" -B "$src/build" \
        -DCMAKE_TOOLCHAIN_FILE="$TOOLCHAIN_FILE" \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_INSTALL_PREFIX="$PREFIX" \
        -DCMAKE_INSTALL_LIBDIR=lib \
        -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
        -DBUILD_SHARED=OFF \
        -DBUILD_STATIC=ON \
        -DBUILD_TESTS=OFF \
        -DWITH_PERF_TOOL=OFF \
        -DWITH_DOCS=OFF \
        -DWITH_LIBSODIUM=OFF \
        -DENABLE_CURVE=OFF \
        -DENABLE_DRAFTS=OFF \
        -DENABLE_CPACK=OFF
    cmake --build "$src/build" -j"$JOBS"
    cmake --install "$src/build"
}

build_libbz2() {
    if [ -f "$PREFIX/lib/libbz2.a" ]; then
        echo "✓ libbz2 already built"
        return
    fi
    local src="$DEPS_DIR/bzip2"
    fetch https://sourceware.org/git/bzip2.git "$BZIP2_VERSION" "$src"
    make -C "$src" -j"$JOBS" libbz2.a \
        CC="$TRIPLE-gcc" AR="$TRIPLE-ar" RANLIB="$TRIPLE-ranlib" \
        CFLAGS="-O2 -fPIC -D_FILE_OFFSET_BITS=64"
    mkdir -p "$PREFIX/lib" "$PREFIX/include"
    cp "$src/libbz2.a" "$PREFIX/lib/"
    cp "$src/bzlib.h" "$PREFIX/include/"
}

build_libcsp() {
    local src="$DEPS_DIR/libcsp"
    if [ -f "$src/build/libcsp.a" ]; then
        echo "✓ libcsp already built"
        return
    fi
    fetch https://github.com/libcsp/libcsp.git "$LIBCSP_VERSION" "$src"
    (
        cd "$src"
        # waf locates libzmq/libsocketcan through pkg-config: only look in
        # the riscv64 prefix, never at the host's packages.
        # v1.6's wscript only applies its default flags when CFLAGS is
        # empty, so they are repeated here with -fPIC (Rust links as PIE).
        PKG_CONFIG_LIBDIR="$PREFIX/lib/pkgconfig" \
            CFLAGS="-std=gnu99 -g -Os -fPIC -Wall -Wextra -Wshadow -Wcast-align -Wwrite-strings -Wno-unused-parameter" \
            "$PYTHON" waf configure \
            --toolchain="$TRIPLE-" \
            --includes="$PREFIX/include" \
            --enable-can-socketcan \
            --enable-if-zmqhub \
            --enable-rdp \
            --with-os=posix
        "$PYTHON" waf build
    )
}

build_cspcl() {
    # libzmq is C++: the C linker needs libstdc++ for a static link.
    cmake -S "$REPO_DIR" -B "$BUILD_DIR" \
        -DCMAKE_TOOLCHAIN_FILE="$TOOLCHAIN_FILE" \
        -DCMAKE_BUILD_TYPE="$BUILD_TYPE" \
        -DCMAKE_C_STANDARD_LIBRARIES="-lstdc++ -lm" \
        -DCSPCL_USE_SYSTEM_CSP=ON \
        -DCSP_REPO_DIR="$DEPS_DIR/libcsp" \
        -DZMQ_LIBRARY="$PREFIX/lib/libzmq.a" \
        -DSOCKETCAN_LIBRARY="$PREFIX/lib/libsocketcan.a" \
        -DCSPCL_BUILD_TESTS=ON
    cmake --build "$BUILD_DIR" -j"$JOBS"
}

build_libsocketcan
build_libzmq
build_libbz2
build_libcsp

if [ "$DEPS_ONLY" = "1" ]; then
    echo ""
    echo "✓ riscv64 dependencies built in $PREFIX and $DEPS_DIR/libcsp"
    exit 0
fi

build_cspcl

echo ""
echo "✓ CSPCL built for riscv64 in $BUILD_DIR"
file "$BUILD_DIR/tests/test_cspcl" 2>/dev/null || true

if [ "$RUN_TESTS" = "1" ]; then
    ctest --test-dir "$BUILD_DIR" --output-on-failure
fi
