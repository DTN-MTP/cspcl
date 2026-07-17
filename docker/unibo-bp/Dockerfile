# Unibo-BP integration image with CSPCL daemon
ARG BASE_IMAGE=cspcl-base:latest
FROM ${BASE_IMAGE}

LABEL maintainer="DTN-MTP contributors"
LABEL description="Unibo-BP with CSPCL daemon for CSP transport"

# Install additional dependencies for Unibo-BP
RUN apt-get update && apt-get install -y --no-install-recommends \
    default-jdk \
    ant \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Build Unibo-BP from source for reproducible CI/local images.
# Pinned to a known-good commit: upstream main later broke the build
# (chrono::duration operator<< in unibo-bp-admin needs a newer compiler).
# UNIBO_BP_REF accepts a commit SHA, branch, or tag.
ARG UNIBO_BP_REF=922d233307589785706d03422940afd720db65b8
RUN git init /opt/unibo-bp-src && \
    cd /opt/unibo-bp-src && \
    git remote add origin https://gitlab.com/unibo-dtn/unibo-bp.git && \
    git fetch --depth 1 origin ${UNIBO_BP_REF} && \
    git checkout --detach FETCH_HEAD && \
    git submodule update --init --recursive --depth 1

WORKDIR /opt/unibo-bp-src
RUN cmake -S . -B build -DCMAKE_BUILD_TYPE=Release && \
    cmake --build build --parallel "$(nproc)"

RUN mkdir -p /opt/unibo-bp/bin /opt/unibo-bp/lib && \
    cp -a /opt/unibo-bp-src/build/Unibo-BP/bin/. /opt/unibo-bp/bin/ && \
    cp -a /opt/unibo-bp-src/build/Unibo-BP/lib/. /opt/unibo-bp/lib/

# Build CSPCL daemon for Unibo-BP integration
COPY unibo-integration/src/cspcl_daemon.c /opt/build/
COPY src/cspcl.c /opt/build/
COPY src/cspcl.h /opt/build/
COPY src/cspcl_config.h /opt/build/

# Compile cspcl_daemon
WORKDIR /opt/build
RUN gcc -O2 -Wall -Wextra \
    cspcl_daemon.c cspcl.c \
    -o /opt/unibo-bp/bin/unibo-bp-cspcl \
    -I. \
    -I/opt/unibo-bp-src/include \
    -I/opt/unibo-bp-src/export/include \
    -I/opt/unibo-bp-src/build/Unibo-BP/include \
    -I/opt/libcsp/include \
    -I/opt/libcsp/build/include \
    -L/opt/unibo-bp/lib \
    -Wl,-rpath,/opt/unibo-bp/lib \
    -lunibo-bp-api \
    /opt/libcsp/build/libcsp.a \
    -lzmq -lpthread -lm -lsocketcan

# Copy entrypoint script
COPY docker/unibo-bp/entrypoint.sh /opt/entrypoint.sh
RUN chmod +x /opt/entrypoint.sh

# Create runtime directories
RUN mkdir -p /tmp/unibo-node

# Set environment variables with defaults
ENV CSP_ADDR=1
ENV CSP_PORT=10
ENV TRANSPORT=zmqhub
ENV UNIBO_SOCKET=2001
ENV UNIBO_NODE_DIR=/tmp/unibo-node
ENV UNIBO_ADMIN_EID=ipn:1.0
ENV UNIBO_ADMIN_DTN_EID=dtn://a.dtn/

# Environment variables for Unibo-BP paths (override in docker-compose if needed)
ENV UNIBO_BP_BIN=/opt/unibo-bp/bin
ENV UNIBO_BP_LIB=/opt/unibo-bp/lib
ENV LD_LIBRARY_PATH=/opt/unibo-bp/lib:/opt/libcsp/build:$LD_LIBRARY_PATH

WORKDIR /tmp/unibo-node

ENTRYPOINT ["/opt/entrypoint.sh"]
