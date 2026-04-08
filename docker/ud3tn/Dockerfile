# uD3TN integration image with CSPCL CLA
ARG BASE_IMAGE=cspcl-base:latest
FROM ${BASE_IMAGE}

LABEL maintainer="DTN-MTP contributors"
LABEL description="uD3TN with CSPCL convergence layer adapter"

# Install additional dependencies for uD3TN
RUN apt-get update && apt-get install -y \
    libjansson-dev \
    libsqlite3-dev \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*

# Clone uD3TN (pinned tag for reproducible builds)
WORKDIR /opt
RUN git clone --depth 1 --branch v0.14.5 --recurse-submodules --shallow-submodules \
    https://gitlab.com/d3tn/ud3tn.git ud3tn-src

# Copy CSPCL integration files
COPY ud3tn-integration/src/cla_csp.c /opt/ud3tn-src/components/cla/posix/
COPY ud3tn-integration/src/cla_csp.h /opt/ud3tn-src/include/cla/
COPY ud3tn-integration/ud3tn-cla-csp.patch /tmp/

# Copy CSPCL library to uD3TN external directory
RUN mkdir -p /opt/ud3tn-src/external/cspcl && \
    cp /opt/cspcl/src/cspcl.c /opt/ud3tn-src/external/cspcl/ && \
    cp /opt/cspcl/src/cspcl.h /opt/ud3tn-src/external/cspcl/ && \
    cp /opt/cspcl/src/cspcl_config.h /opt/ud3tn-src/external/cspcl/

# Apply patch (adjust libcsp path in patch for container environment)
WORKDIR /opt/ud3tn-src
RUN sed -i 's|/home/mathias/libcsp-src|/opt/libcsp|g' /tmp/ud3tn-cla-csp.patch && \
    git apply /tmp/ud3tn-cla-csp.patch

# Build uD3TN
RUN make posix

# Create runtime directory
RUN mkdir -p /var/run/ud3tn

# Copy entrypoint script
COPY docker/ud3tn/entrypoint.sh /opt/entrypoint.sh
RUN chmod +x /opt/entrypoint.sh

# Set environment variables with defaults
ENV CSP_ADDR=1
ENV CSP_PORT=10
ENV TRANSPORT=zmqhub
ENV UD3TN_EID=dtn://a.dtn/

WORKDIR /var/run/ud3tn

ENTRYPOINT ["/opt/entrypoint.sh"]
