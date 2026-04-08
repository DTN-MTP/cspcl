# Unibo-BP integration image with CSPCL daemon
ARG BASE_IMAGE=cspcl-base:latest
FROM ${BASE_IMAGE}

LABEL maintainer="DTN-MTP contributors"
LABEL description="Unibo-BP with CSPCL daemon for CSP transport"

# Install additional dependencies for Unibo-BP
RUN apt-get update && apt-get install -y \
    default-jdk \
    ant \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Note: This Dockerfile assumes Unibo-BP binaries are provided
# You can either:
# 1. Build from source if you have access to the repository
# 2. Copy pre-built binaries
# 3. Mount them at runtime

# For this example, we'll set up the environment expecting binaries in /opt/unibo-bp
RUN mkdir -p /opt/unibo-bp/bin /opt/unibo-bp/lib

# Build CSPCL daemon for Unibo-BP integration
COPY unibo-integration/src/cspcl_daemon.c /opt/build/
COPY src/cspcl.c /opt/build/
COPY src/cspcl.h /opt/build/
COPY src/cspcl_config.h /opt/build/

# Compile cspcl_daemon
# Note: This assumes Unibo-BP headers and libraries will be available
# Adjust paths based on your Unibo-BP installation
WORKDIR /opt/build
RUN gcc -O2 -Wall -Wextra \
    cspcl_daemon.c cspcl.c \
    -o /opt/unibo-bp/bin/unibo-bp-cspcl \
    -I. \
    -I/opt/libcsp/include \
    -I/opt/libcsp/build/include \
    /opt/libcsp/build/libcsp.a \
    -lzmq -lpthread -lm -lsocketcan || \
    echo "Warning: Build may fail without Unibo-BP libraries. See README for setup."

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
