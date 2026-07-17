# Base image for CSPCL with libcsp v1.6
# This image provides the foundation for both uD3TN and Unibo-BP integrations
FROM ubuntu:22.04

LABEL maintainer="DTN-MTP contributors"
LABEL description="Base image with libcsp v1.6 and CSPCL library"

# Avoid interactive prompts during build
ENV DEBIAN_FRONTEND=noninteractive

# Install build dependencies
# hadolint ignore=DL3008
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    gcc \
    g++ \
    make \
    cmake \
    git \
    python3 \
    python3-pip \
    python3-dev \
    libzmq3-dev \
    netcat-openbsd \
    iproute2 \
    kmod \
    libsocketcan-dev \
    pkg-config \
    wget \
    can-utils \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Install Python dependencies for ZMQ broker and libcsp build
RUN pip3 install --no-cache-dir pyzmq==26.0.3

# Set working directory
WORKDIR /opt

# Clone and build libcsp v1.6
RUN git clone --branch v1.6 --depth 1 https://github.com/libcsp/libcsp.git libcsp

# Patch: RDP peer timeout. Upstream v1.6 never times out established
# connections, so a dead peer causes infinite retransmission and sends
# into the zombie connection succeed silently.
COPY docker/base/libcsp-rdp-peer-timeout.patch /tmp/
RUN git -C /opt/libcsp apply /tmp/libcsp-rdp-peer-timeout.patch

WORKDIR /opt/libcsp
RUN python3 waf configure \
        --enable-can-socketcan \
        --enable-if-zmqhub \
        --enable-rdp \
        --with-os=posix \
    && python3 waf build

# Copy CSPCL source code
COPY src/ /opt/cspcl/src/
COPY stubs/ /opt/cspcl/stubs/
COPY CMakeLists.txt /opt/cspcl/
COPY tests/CMakeLists.txt /opt/cspcl/tests/

# Build CSPCL library
WORKDIR /opt/cspcl
RUN cmake -S . -B build \
        -DCSPCL_USE_SYSTEM_CSP=ON \
        -DCSP_REPO_DIR=/opt/libcsp \
        -DCSPCL_BUILD_TESTS=OFF \
    && cmake --build build

# Copy ZMQ broker utility
COPY tools/zmqhub_broker.py /opt/tools/

# Set library paths
ENV LD_LIBRARY_PATH=/opt/libcsp/build:/opt/cspcl/build:$LD_LIBRARY_PATH
ENV CSP_REPO_DIR=/opt/libcsp
ENV CSPCL_DIR=/opt/cspcl
ENV PYTHONUNBUFFERED=1

# Default working directory
WORKDIR /opt

CMD ["/bin/bash"]
