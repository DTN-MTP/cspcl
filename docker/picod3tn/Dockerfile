# picoD3TN integration image with the cspcl CLA
ARG BASE_IMAGE=cspcl-base:latest

# --- Build stage: Rust toolchain, libcsp with -fPIC and picoD3TN ---
# The tag comes from BASE_IMAGE
# hadolint ignore=DL3006
FROM ${BASE_IMAGE} AS builder

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

# clang/libclang: bindgen in cspcl-sys
# hadolint ignore=DL3008
RUN apt-get update && apt-get install -y --no-install-recommends \
    clang \
    libclang-dev \
    libbz2-dev \
    curl \
    && rm -rf /var/lib/apt/lists/*

# The toolchain picoD3TN's flake pins. RUSTUP_TOOLCHAIN overrides the
# picoD3TN rust-toolchain.toml, which would also pull embedded targets.
ARG RUST_VERSION=1.96.1
ENV RUSTUP_HOME=/opt/rustup \
    CARGO_HOME=/opt/cargo \
    RUSTUP_TOOLCHAIN=${RUST_VERSION} \
    PATH=/opt/cargo/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain "${RUST_VERSION}"

# Rust links binaries as PIE, so libcsp.a must be built with -fPIC. The base
# image's libcsp is not, and v1.6's wscript only applies its default flags
# when CFLAGS is empty.
RUN git clone --branch v1.6 --depth 1 https://github.com/libcsp/libcsp.git /opt/libcsp-pic
WORKDIR /opt/libcsp-pic
RUN CFLAGS="-std=gnu99 -g -Os -fPIC -Wall -Wextra -Wshadow -Wcast-align -Wwrite-strings -Wno-unused-parameter" \
    python3 waf configure \
        --enable-can-socketcan \
        --enable-if-zmqhub \
        --enable-rdp \
        --with-os=posix \
    && python3 waf build
ENV CSP_REPO_DIR=/opt/libcsp-pic

# picoD3TN main, the revision picod3tn-cla-cspcl.patch is made against
ARG PICOD3TN_REV=38fd7a910fb27fb32c7bfc55177eff187826679d
WORKDIR /opt/picod3tn-src
RUN git init -q \
    && git remote add origin https://gitlab.com/d3tn/picod3tn.git \
    && git fetch -q --depth 1 origin "${PICOD3TN_REV}" \
    && git checkout -q FETCH_HEAD

COPY picod3tn-integration/picod3tn-cla-cspcl.patch /tmp/
RUN git apply /tmp/picod3tn-cla-cspcl.patch

# Build against this checkout's picod3tn-cspcl rather than the git rev the
# patch pins. cspcl-sys finds the C sources in /opt/cspcl/src (base image).
COPY rust-bindings/ /opt/cspcl/rust-bindings/
WORKDIR /opt/picod3tn-src/picod3tn
RUN cargo build --release --features cspcl --bins \
        --config 'patch."https://github.com/DTN-MTP/cspcl.git".picod3tn-cspcl.path="/opt/cspcl/rust-bindings/picod3tn-cspcl"'

# --- Runtime stage ---
# hadolint ignore=DL3006
FROM ${BASE_IMAGE}

LABEL maintainer="DTN-MTP contributors"
LABEL description="picoD3TN with the cspcl convergence layer"

COPY --from=builder \
    /opt/picod3tn-src/picod3tn/target/release/picod3tn \
    /opt/picod3tn-src/picod3tn/target/release/aapctl \
    /opt/picod3tn/bin/

# Create runtime directory
RUN mkdir -p /var/run/picod3tn

# Copy entrypoint script
COPY docker/picod3tn/entrypoint.sh /opt/entrypoint.sh
RUN chmod +x /opt/entrypoint.sh

# Set environment variables with defaults. AAPCTL_CONNECT lets interop
# scripts run aapctl in the container without --connect.
ENV CSP_ADDR=5
ENV CSP_PORT=10
ENV TRANSPORT=zmqhub
ENV PICOD3TN_NODE_ID=dtn://e.dtn/
ENV AAPCTL_CONNECT=unix:/var/run/picod3tn/aap2.sock
ENV PATH=/opt/picod3tn/bin:$PATH

WORKDIR /var/run/picod3tn

ENTRYPOINT ["/opt/entrypoint.sh"]
