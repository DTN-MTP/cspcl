FROM ubuntu:22.04

LABEL description="A-SABR Bundle Dispatch Module for µD3TN"

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    python3 \
    python3-pip \
    python3-dev \
    git \
    wget \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/opt/rust/rustup \
    CARGO_HOME=/opt/rust/cargo \
    PATH="/opt/rust/cargo/bin:${PATH}"

RUN wget -qO- https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain stable --profile minimal \
    && cargo --version

# cbor2: pyd3tn/bundle7.py; jsonschema: ud3tn_utils/config.py; protobuf: ud3tn_utils/aap2
RUN pip3 install --no-cache-dir maturin patchelf cbor2 jsonschema protobuf

# --- pyd3tn and python-ud3tn-utils ---
# These packages have broken metadata at v0.14.5 — pip silently no-ops on them.
# The modules are pure Python so we add their source dirs to PYTHONPATH directly.
RUN git clone --depth 1 --branch v0.14.5 \
    https://gitlab.com/d3tn/ud3tn.git /opt/ud3tn-src
ENV PYTHONPATH="/opt/ud3tn-src/pyd3tn:/opt/ud3tn-src/python-ud3tn-utils"

# --- a-sabr-python ---
# Use the maintained fork on feat/asabr-bdm which includes get_all_nodes and
# other fixes required by asabr-bdm/main.py. The submodule in external/ is an
# older version missing these methods.
RUN git clone --depth 1 --branch feat/asabr-bdm \
    https://github.com/theotchlx/A-SABR-Python.git /opt/a-sabr-python
WORKDIR /opt/a-sabr-python
RUN maturin build --release --interpreter python3 -o /tmp/a-sabr-wheels
RUN pip3 install --no-cache-dir --ignore-requires-python /tmp/a-sabr-wheels/*.whl

# --- asabr-bdm ---
# Use the maintained fork. pyproject.toml uses [tool.uv.sources] path overrides
# that pip ignores; all dependencies are already installed above.
RUN git clone --depth 1 \
    https://github.com/theotchlx/asabr_bdm.git /opt/asabr-bdm
WORKDIR /opt/asabr-bdm
RUN pip3 install --no-cache-dir --no-deps --ignore-requires-python .

# Smoke-test at build time: catches missing deps before they surface at runtime.
RUN python3 -c "from a_sabr_python import AsabrRouter, AsabrBundle; from ud3tn_utils.aap2 import AAP2UnixClient, AuthType; from pyd3tn.eid import get_node_id; print('imports OK')"

COPY docker/asabr-bdm/entrypoint.sh /opt/entrypoint.sh
RUN chmod +x /opt/entrypoint.sh

ENV PYTHONUNBUFFERED=1

ENTRYPOINT ["/opt/entrypoint.sh"]