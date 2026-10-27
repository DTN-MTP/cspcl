#!/bin/bash
#
# build-picod3tn-riscv64.sh - Cross-compile picoD3TN with the cspcl CLA for
# 64-bit RISC-V Linux
#
# Applies picod3tn-integration/picod3tn-cla-cspcl.patch to picoD3TN, then
# builds picod3tn and aapctl for riscv64gc-unknown-linux-gnu against this
# checkout's picod3tn-cspcl. The binaries are fully static (glibc included),
# so they run on the PolarFire SoC Yocto/Buildroot images whatever their
# glibc version.
#
# The C dependencies (libsocketcan, libzmq, libbz2, libcsp) come from
# tools/build-riscv64.sh, which this script runs with DEPS_ONLY=1.
#
# Requires: everything tools/build-riscv64.sh needs, plus rustup, clang and
#           libclang (for bindgen)
# Optional: qemu-user-static (to run the tests on the host)
#
# Environment:
#   DEPS_DIR        dependency directory, shared with build-riscv64.sh
#                   (default: /tmp/riscv64-deps)
#   PICOD3TN_DIR    scratch picoD3TN checkout, created by this script and
#                   reset to PICOD3TN_REV on every run (default: $DEPS_DIR/picod3tn)
#   PICOD3TN_REV    picoD3TN revision the patch is made against
#   OUT_DIR         where picod3tn and aapctl are copied
#                   (default: <repo>/build-riscv64/picod3tn)
#   RUST_TOOLCHAIN  rustup toolchain (default: stable)
#   RUN_TESTS       1 to run the tests under qemu after building (default: 0)
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"

TRIPLE=riscv64-linux-gnu
RUST_TARGET=riscv64gc-unknown-linux-gnu
DEPS_DIR="${DEPS_DIR:-/tmp/riscv64-deps}"
PREFIX="$DEPS_DIR/sysroot"
PICOD3TN_DIR="${PICOD3TN_DIR:-$DEPS_DIR/picod3tn}"
# picoD3TN main, the revision picod3tn-cla-cspcl.patch is made against
PICOD3TN_REV="${PICOD3TN_REV:-38fd7a910fb27fb32c7bfc55177eff187826679d}"
OUT_DIR="${OUT_DIR:-$REPO_DIR/build-riscv64/picod3tn}"
RUN_TESTS="${RUN_TESTS:-0}"
PATCH="$REPO_DIR/picod3tn-integration/picod3tn-cla-cspcl.patch"

# Overrides picoD3TN's rust-toolchain.toml, which would also pull its
# embedded targets.
export RUSTUP_TOOLCHAIN="${RUST_TOOLCHAIN:-stable}"

# Marks a checkout this script created, the only kind it resets.
STAMP=.cspcl-riscv64-build

fetch_picod3tn() {
    if [ ! -e "$PICOD3TN_DIR" ]; then
        mkdir -p "$PICOD3TN_DIR"
        git -C "$PICOD3TN_DIR" init -q
        git -C "$PICOD3TN_DIR" remote add origin https://gitlab.com/d3tn/picod3tn.git
        touch "$PICOD3TN_DIR/.git/$STAMP"
    elif [ ! -e "$PICOD3TN_DIR/.git/$STAMP" ]; then
        echo "$PICOD3TN_DIR was not created by this script, refusing to reset it" >&2
        exit 1
    fi
    if [ "$(git -C "$PICOD3TN_DIR" rev-parse -q --verify HEAD || true)" != "$PICOD3TN_REV" ]; then
        git -C "$PICOD3TN_DIR" fetch -q --depth 1 origin "$PICOD3TN_REV"
    fi
    # Drops the previous run's patch and Cargo.lock; keeps target/ (ignored).
    git -C "$PICOD3TN_DIR" checkout -q --force "$PICOD3TN_REV"
    git -C "$PICOD3TN_DIR" clean -q -fd
}

apply_patch() {
    git -C "$PICOD3TN_DIR" apply "$PATCH"
}

setup_cross_env() {
    local target_env="${RUST_TARGET//-/_}"
    local target_var
    target_var="$(echo "$target_env" | tr '[:lower:]' '[:upper:]')"

    export "CARGO_TARGET_${target_var}_LINKER=$TRIPLE-gcc"
    # Static, non-PIE: a static-pie needs rcrt1.o, which Ubuntu 22.04's
    # riscv64 cross glibc does not ship.
    export "CARGO_TARGET_${target_var}_RUSTFLAGS=-C target-feature=+crt-static -C relocation-model=static"
    export "CC_${target_env}=$TRIPLE-gcc"
    export "AR_${target_env}=$TRIPLE-ar"
    export "BINDGEN_EXTRA_CLANG_ARGS_${target_env}=--sysroot=/usr/$TRIPLE"

    local qemu
    qemu="$(command -v qemu-riscv64-static || command -v qemu-riscv64 || true)"
    if [ -n "$qemu" ]; then
        export "CARGO_TARGET_${target_var}_RUNNER=$qemu"
    fi

    # cspcl-sys: libcsp, and the static libraries it links
    export CSP_REPO_DIR="$DEPS_DIR/libcsp"
    export CSPCL_DEPS_PREFIX="$PREFIX"
}

# Builds against this checkout's picod3tn-cspcl rather than the git rev the
# patch pins.
CARGO_PATCH="patch.\"https://github.com/DTN-MTP/cspcl.git\".picod3tn-cspcl.path=\"$REPO_DIR/rust-bindings/picod3tn-cspcl\""

build_picod3tn() {
    (
        cd "$PICOD3TN_DIR/picod3tn"
        cargo build --release --target "$RUST_TARGET" --features cspcl --bins \
            --config "$CARGO_PATCH"
    )
    local bin_dir="$PICOD3TN_DIR/picod3tn/target/$RUST_TARGET/release"
    mkdir -p "$OUT_DIR"
    cp "$bin_dir/picod3tn" "$bin_dir/aapctl" "$OUT_DIR/"
}

run_tests() {
    if [ -z "$(command -v qemu-riscv64-static || command -v qemu-riscv64 || true)" ]; then
        echo "qemu-riscv64 not found, cannot run the riscv64 tests" >&2
        exit 1
    fi
    "$CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_GNU_RUNNER" "$OUT_DIR/picod3tn" --help > /dev/null
    # --tests: rustdoc ignores the target RUSTFLAGS, so doctests would link
    # dynamically. The rust-picod3tn CI job runs them natively.
    (
        cd "$REPO_DIR/rust-bindings"
        cargo test --release --target "$RUST_TARGET" -p picod3tn-cspcl --features cli --tests
    )
    (
        cd "$PICOD3TN_DIR/picod3tn"
        cargo test --release --target "$RUST_TARGET" --features cspcl --tests \
            --config "$CARGO_PATCH"
    )
}

rustup target add "$RUST_TARGET"
DEPS_ONLY=1 "$SCRIPT_DIR/build-riscv64.sh"
fetch_picod3tn
apply_patch
setup_cross_env
build_picod3tn

echo ""
echo "✓ picoD3TN built for riscv64 in $OUT_DIR"
file "$OUT_DIR/picod3tn" "$OUT_DIR/aapctl" 2>/dev/null || true

if [ "$RUN_TESTS" = "1" ]; then
    run_tests
fi
