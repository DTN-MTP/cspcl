# ASABR Integration with CSPCL + µD3TN

This document describes how to replace µD3TN's default contact-based routing algorithm with [A-SABR](https://github.com/theotchlx/A-SABR-Python) (Autonomous Swarm Aware Bundle Routing) in the CSPCL + µD3TN integration.

## Overview

### Current architecture

```
aap2_config --schedule  →  µD3TN internal router  →  CSP CLA (cla_csp.c)  →  libcsp
```

Routing is configured manually per test via `aap2_config --schedule`. µD3TN's built-in contact-based Router Agent makes all forwarding decisions.

### Target architecture

```
contact_plan.cp + eid_map.json
        │
        ▼
   asabr_bdm (Python)  ──AAP2 dispatch events──▶  µD3TN (-d flag)  →  CSP CLA  →  libcsp
        │                ◀─ FIB link UP/DOWN ──
        │
   A-SABR (Rust core)
```

The `asabr_bdm` process connects to µD3TN's AAP2 socket as an external Bundle Dispatch Module (BDM). µD3TN is started with the `-d` flag, which disables its internal Router Agent and delegates all bundle forwarding decisions to the BDM. The BDM also drives link state via a second AAP2 FIB control connection, replacing the contact scheduling that `aap2_config --schedule` previously provided.

**The CSPCL CLA (`cla_csp.c`) is not modified.** The change is entirely at the routing and infrastructure layer.

---

## Components

| Component | Role |
|-----------|------|
| `A-SABR-Python` | Rust routing engine with Python bindings (PyO3/maturin). Loads `.cp` contact plans and computes next-hop contacts. |
| `asabr_bdm` | Python BDM that connects to µD3TN via AAP2, receives dispatch events, queries A-SABR, and returns next-hop EIDs. Also runs a link scheduler thread that sends FIB LINK_STATUS_UP/DOWN events based on the contact plan. |
| `µD3TN` | Started with `-d` (external dispatch). Fires a `dispatch_event` to the BDM for every bundle that needs routing; uses the BDM's response to forward via the CSP CLA. |
| `cla_csp.c` | Unchanged. Forwards bundles to the CSP address returned by µD3TN's FIB lookup. |

---

## Configuration Files

Two files are required per µD3TN node:

### Contact Plan (`.cp`)

A-SABR contact plan format. Defines nodes and scheduled contacts with data rates and propagation delays.

```
node <id> <name>
contact <tx_node_id> <rx_node_id> <start_unix> <end_unix> <rate_bps> <delay_s>
```

For integration testing, contacts are set to cover a very large time range (`0` to `9999999999`) to simulate always-on links.

### EID Map (`.json`)

Maps DTN node EIDs to A-SABR node names and CSP CLA addresses. Each µD3TN node requires its own EID map because the CLA address for the local node is `null` (no self-link is created).

```json
{
    "dtn://a.dtn/": {"name": "a", "cla_addr": null},
    "dtn://b.dtn/": {"name": "b", "cla_addr": "csp:2,10"}
}
```

`cla_addr` follows the same format as the `--cla` option passed to µD3TN: `csp:<csp_addr>,<csp_port>`.

---

## Implementation Steps

### Step 1 — Modify µD3TN startup to support external dispatch

**File:** `docker/ud3tn/entrypoint.sh`

Add support for the `ASABR_ENABLED` environment variable. When set to `1`, pass `-d` to µD3TN to disable the internal Router Agent, and optionally `-x` for BDM authentication in release builds.

```bash
# Add after building UD3TN_ARGS, before exec:
if [ "${ASABR_ENABLED:-0}" = "1" ]; then
    UD3TN_ARGS+=(-d)
    if [ -n "${BDM_SECRET:-}" ]; then
        export _BDM_SECRET_VALUE="${BDM_SECRET}"
        UD3TN_ARGS+=(-x _BDM_SECRET_VALUE)
    fi
fi
```

Nothing else in the entrypoint changes. The socket path (`/var/run/ud3tn/ud3tn.aap2.socket`) and CLA configuration remain the same.

---

### Step 2 — Create configuration files for each test topology

Create the directory `tests/interop/asabr/` to hold all contact plans and EID maps.

#### 2a. `ud3tn-basic` test topology (Node A ↔ Node B)

Two µD3TN nodes:
- Node A: CSP address 1, EID `dtn://a.dtn/`
- Node B: CSP address 2, EID `dtn://b.dtn/`

**`tests/interop/asabr/ud3tn-basic.cp`**

```
node 0 a
node 1 b
contact 0 1 0 9999999999 100000 0
contact 1 0 0 9999999999 100000 0
```

**`tests/interop/asabr/ud3tn-basic-node-a.json`** — used by the BDM running alongside Node A

```json
{
    "dtn://a.dtn/": {"name": "a", "cla_addr": null},
    "dtn://b.dtn/": {"name": "b", "cla_addr": "csp:2,10"}
}
```

**`tests/interop/asabr/ud3tn-basic-node-b.json`** — used by the BDM running alongside Node B

```json
{
    "dtn://a.dtn/": {"name": "a", "cla_addr": "csp:1,10"},
    "dtn://b.dtn/": {"name": "b", "cla_addr": null}
}
```

#### 2b. `cross-integration` test topology (4 nodes: µD3TN A/B + Unibo C/D)

- Node A: µD3TN, CSP 1, `dtn://a.dtn/`
- Node B: µD3TN, CSP 2, `dtn://b.dtn/`
- Node C: Unibo-BP, CSP 3, `dtn://c.dtn/` / `ipn:3.0`
- Node D: Unibo-BP, CSP 4, `dtn://d.dtn/` / `ipn:4.0`

**`tests/interop/asabr/cross-integration.cp`**

```
node 0 a
node 1 b
node 2 c
node 3 d
contact 0 1 0 9999999999 100000 0
contact 1 0 0 9999999999 100000 0
contact 0 2 0 9999999999 100000 0
contact 2 0 0 9999999999 100000 0
contact 0 3 0 9999999999 100000 0
contact 3 0 0 9999999999 100000 0
contact 1 2 0 9999999999 100000 0
contact 2 1 0 9999999999 100000 0
contact 1 3 0 9999999999 100000 0
contact 3 1 0 9999999999 100000 0
```

**`tests/interop/asabr/cross-node-a.json`**

```json
{
    "dtn://a.dtn/": {"name": "a", "cla_addr": null},
    "dtn://b.dtn/": {"name": "b", "cla_addr": "csp:2,10"},
    "dtn://c.dtn/": {"name": "c", "cla_addr": "csp:3,10"},
    "dtn://d.dtn/": {"name": "d", "cla_addr": "csp:4,10"},
    "ipn:3.0":      {"name": "c", "cla_addr": "csp:3,10"},
    "ipn:4.0":      {"name": "d", "cla_addr": "csp:4,10"}
}
```

**`tests/interop/asabr/cross-node-b.json`**

```json
{
    "dtn://a.dtn/": {"name": "a", "cla_addr": "csp:1,10"},
    "dtn://b.dtn/": {"name": "b", "cla_addr": null},
    "dtn://c.dtn/": {"name": "c", "cla_addr": "csp:3,10"},
    "dtn://d.dtn/": {"name": "d", "cla_addr": "csp:4,10"},
    "ipn:3.0":      {"name": "c", "cla_addr": "csp:3,10"},
    "ipn:4.0":      {"name": "d", "cla_addr": "csp:4,10"}
}
```

> **IPN EID entries:** The cross-integration test routes bundles to `ipn:4.55`. The BDM's `_eid_node()` helper calls `pyd3tn.eid.get_node_id()` to strip the service part (`ipn:4.55` → `ipn:4.0`). The EID map must include the node-level IPN EID (`ipn:4.0`) for the lookup to succeed. Validate this with `-vv` logging on the first run.

---

### Step 3 — Build the `asabr_bdm` Docker image

The `asabr_bdm` image requires:
1. A Rust toolchain to build the `a-sabr-python` native extension via `maturin`
2. The Python packages from `asabr_bdm/pyproject.toml` (`pyd3tn`, `ud3tn-utils`, `a-sabr-python`)

**New file: `docker/asabr-bdm/Dockerfile`**

```dockerfile
ARG BASE_IMAGE=cspcl-base:latest
FROM ${BASE_IMAGE}

LABEL description="A-SABR Bundle Dispatch Module for µD3TN"

# Rust toolchain (required by maturin to compile a-sabr-python)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain stable --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"

RUN pip3 install --no-cache-dir maturin

# Build a-sabr-python Rust extension
COPY external/a-sabr-python /opt/a-sabr-python
WORKDIR /opt/a-sabr-python
RUN maturin build --release -o /tmp/wheels && \
    pip3 install --no-cache-dir /tmp/wheels/*.whl

# Install asabr_bdm and its Python dependencies
COPY external/asabr_bdm /opt/asabr-bdm
WORKDIR /opt/asabr-bdm
RUN pip3 install --no-cache-dir -e .

COPY docker/asabr-bdm/entrypoint.sh /opt/entrypoint.sh
RUN chmod +x /opt/entrypoint.sh

ENTRYPOINT ["/opt/entrypoint.sh"]
```

**New file: `docker/asabr-bdm/entrypoint.sh`**

```bash
#!/bin/bash
set -e

SOCKET="${UD3TN_SOCKET:-/var/run/ud3tn/ud3tn.aap2.socket}"
CP_FILE="${ASABR_CP_FILE:-/config/contact_plan.cp}"
EID_MAP="${ASABR_EID_MAP:-/config/eid_map.json}"
ROUTER_TYPE="${ASABR_ROUTER_TYPE:-VolCgrHybridParenting}"

echo "Waiting for µD3TN socket at ${SOCKET}..."
until [ -S "${SOCKET}" ]; do sleep 1; done
echo "Socket ready. Starting ASABR BDM..."

ARGS=(--socket "${SOCKET}" "${CP_FILE}" "${EID_MAP}" --router-type "${ROUTER_TYPE}")
if [ -n "${BDM_SECRET:-}" ]; then
    ARGS+=(--secret "${BDM_SECRET}")
fi

exec python3 /opt/asabr-bdm/main.py "${ARGS[@]}"
```

> **Build context:** `A-SABR-Python` and `asabr_bdm` live outside `cspcl/`. Add them as git submodules at `cspcl/external/a-sabr-python` and `cspcl/external/asabr_bdm`, or symlink them, so the Docker build context can reach them.

---

### Step 4 — Add BDM services to Docker Compose

**File:** `docker/docker-compose.zmqhub.yml`

Add `ASABR_ENABLED=1` to both µD3TN node services:

```yaml
  ud3tn-node-a:
    ...
    environment:
      - CSP_ADDR=1
      - CSP_PORT=10
      - TRANSPORT=zmqhub
      - UD3TN_EID=dtn://a.dtn/
      - ZMQ_BROKER_HOST=zmq-broker
      - ASABR_ENABLED=1                  # new
      - BDM_SECRET=${BDM_SECRET:-}       # new (optional)
```

Add two new BDM services:

```yaml
  asabr-bdm-node-a:
    image: cspcl-asabr-bdm:latest
    container_name: cspcl-asabr-bdm-node-a
    build:
      context: ..
      dockerfile: docker/asabr-bdm/Dockerfile
    depends_on:
      ud3tn-node-a:
        condition: service_healthy
    environment:
      - UD3TN_SOCKET=/var/run/ud3tn/ud3tn.aap2.socket
      - ASABR_CP_FILE=/config/ud3tn-basic.cp
      - ASABR_EID_MAP=/config/ud3tn-basic-node-a.json
      - ASABR_ROUTER_TYPE=VolCgrHybridParenting
      - BDM_SECRET=${BDM_SECRET:-}
    volumes:
      - ud3tn-a-data:/var/run/ud3tn           # shared AAP2 socket
      - ../tests/interop/asabr:/config:ro     # contact plans + EID maps
    networks:
      - csp-zmq-net

  asabr-bdm-node-b:
    image: cspcl-asabr-bdm:latest
    container_name: cspcl-asabr-bdm-node-b
    build:
      context: ..
      dockerfile: docker/asabr-bdm/Dockerfile
    depends_on:
      ud3tn-node-b:
        condition: service_healthy
    environment:
      - UD3TN_SOCKET=/var/run/ud3tn/ud3tn.aap2.socket
      - ASABR_CP_FILE=/config/ud3tn-basic.cp
      - ASABR_EID_MAP=/config/ud3tn-basic-node-b.json
      - ASABR_ROUTER_TYPE=VolCgrHybridParenting
      - BDM_SECRET=${BDM_SECRET:-}
    volumes:
      - ud3tn-b-data:/var/run/ud3tn
      - ../tests/interop/asabr:/config:ro
    networks:
      - csp-zmq-net
```

The BDM containers share the named volumes (`ud3tn-a-data`, `ud3tn-b-data`) with the corresponding µD3TN containers. This is how the BDM reaches the AAP2 Unix socket without any network configuration.

---

### Step 5 — Update test scripts

#### `tests/interop/test-ud3tn-basic.sh`

Remove the manual route configuration block (previously step 2):

```bash
# DELETE this entire block:
docker exec "$NODE_A_CONTAINER" \
    /opt/ud3tn-src/build/posix/aap2/aap2_config \
    --socket /var/run/ud3tn/ud3tn.aap2.socket \
    --schedule 1 3600 100000 \
    --reaches dtn://b.dtn/bundlesink \
    dtn://b.dtn/ "csp:2,10"
```

Replace with a BDM readiness probe:

```bash
echo "[2/5] Waiting for ASABR BDM to connect..."
BDM_A_CONTAINER="${BDM_A_CONTAINER:-cspcl-asabr-bdm-node-a}"
for ((i=1; i<=TIMEOUT; i++)); do
    if docker logs "$BDM_A_CONTAINER" 2>&1 \
        | grep -q "Listening for dispatch events"; then
        echo "✓ ASABR BDM ready on Node A"
        break
    fi
    if (( i == TIMEOUT )); then
        echo "✗ FAILED: ASABR BDM did not connect after ${TIMEOUT}s"
        docker logs "$BDM_A_CONTAINER"
        exit 1
    fi
    sleep 1
done
```

Steps 3–5 (start receiver, send bundle, verify reception) are unchanged.

#### `tests/interop/test-cross-integration.sh`

Remove `aap2_config --schedule` calls **only for the µD3TN-side routes**. The Unibo-BP nodes do not use AAP2 and cannot connect a BDM, so their native route configuration must remain:

```bash
# DELETE: µD3TN Node A → Unibo Node 2 route (BDM handles this now)
# KEEP:   Unibo Node 1 → µD3TN Node B route (Unibo still needs manual config)
```

Add BDM readiness probes for both µD3TN nodes before sending any bundles.

#### `tests/interop/run-tests.sh`

Add the BDM image build before composing:

```bash
docker build -t cspcl-asabr-bdm:latest -f docker/asabr-bdm/Dockerfile .
```

---

### Step 6 — Wire up the build

Add `A-SABR-Python` and `asabr_bdm` as git submodules:

```bash
cd cspcl
git submodule add https://github.com/theotchlx/A-SABR-Python external/a-sabr-python
git submodule add <asabr_bdm_repo_url> external/asabr_bdm
```

Or, if they remain as sibling directories under `DTN/`, adjust the Docker build context in `run-tests.sh` to pass the parent `DTN/` directory and update all Dockerfile `COPY` paths accordingly.

---

### Step 7 - Update documentation

Create a new file to describe the asabr with ud3tn implementation. It should describe the main components and changes it brings

Notably add a part to explain the following

BDM_SECRET is a shared credential that prevents an unauthorized process from acting as the routing authority for µD3TN.

When µD3TN runs with -d (external dispatch), the first AAP2 client that connects with AUTH_TYPE_BUNDLE_DISPATCH takes full control of all bundle forwarding decisions. That is a privileged position — a rogue process on the same host could connect to the socket first and silently blackhole or reroute all traffic.

The secret closes that gap:

µD3TN is given the secret via -x <ENV_VAR_NAME>. It reads the value from that environment variable at startup and requires any BDM client to present it during the AAP2 configure() handshake.
The asabr_bdm is given the same value via --secret and sends it in configure().
If they don't match, µD3TN rejects the connection.
In practice for the cspcl integration:

The socket lives inside a named Docker volume (/var/run/ud3tn/ud3tn.aap2.socket), so only containers that explicitly mount that volume can reach it. In that environment the secret is mostly defense-in-depth rather than the primary access control.
In µD3TN debug builds the secret check may not be enforced at all, so you can leave BDM_SECRET unset and everything still works.
In µD3TN release builds the check is mandatory and connecting without a matching secret causes the BDM registration to be rejected.
So for local testing with debug builds you can ignore it entirely. It becomes important when deploying a release build or in any environment where the AAP2 socket is reachable by more than just the intended BDM container.

Be cautious of the following also

> **IPN EID entries:** The cross-integration test routes bundles to `ipn:4.55`. The BDM's `_eid_node()` helper calls `pyd3tn.eid.get_node_id()` to strip the service part (`ipn:4.55` → `ipn:4.0`). The EID map must include the node-level IPN EID (`ipn:4.0`) for the lookup to succeed. Validate this with `-vv` logging on the first run.

## Data Flow at Runtime

The following describes what happens when a bundle is sent from Node A to Node B after the BDM is running:

1. **Link scheduler** (BDM background thread) reads all contacts from `ud3tn-basic.cp` where `tx_node == local_id`. At `t=0` it fires `LINK_STATUS_UP` for `dtn://b.dtn/` via `csp:2,10` to µD3TN's FIB control connection. µD3TN now knows Node B is reachable via the CSP CLA.

2. **Bundle sent** by `aap2_send` to `dtn://b.dtn/bundlesink`. µD3TN's bundle processor receives it.

3. **Dispatch event** fired by µD3TN to the BDM (reason: `DISPATCH_REASON_NO_FIB_ENTRY`). Event carries `src_eid`, `dst_eid`, `payload_length`, `lifetime_ms`.

4. **A-SABR routing** called by the BDM: `router.route(local_id=0, bundle, current_time, excluded=[])`. A-SABR walks the contact graph and returns `[(contact(0→1), {1})]`, meaning the destination is reachable via the direct contact to Node B.

5. **Dispatch result** sent back to µD3TN: `next_hops=[{node_id: "dtn://b.dtn/"}]`.

6. **FIB lookup** by µD3TN: `dtn://b.dtn/` → `csp:2,10` (set in step 1). µD3TN calls `csp_cla_begin_packet()` with `cla_addr="csp:2,10"`.

7. **CSP CLA** calls `cspcl_send_bundle()` which opens a CSP RDP connection to address 2, port 10, and sends the bundle via SFP fragmentation.

8. **Node B** receives the bundle via its CSP RDP socket, delivers it to `dtn://b.dtn/bundlesink`, and `aap2_receive` logs it.

---

## Open Questions

| # | Question | Impact | Resolution |
|---|----------|--------|------------|
| 1 | **Build context for external sources** | `A-SABR-Python` and `asabr_bdm` live outside `cspcl/`. The Docker build needs them reachable. | Use git submodules at `cspcl/external/` (recommended) or widen the build context to the parent `DTN/` directory. |
| 2 | **BDM secret in debug vs release builds** | µD3TN debug builds may not enforce `-x`. Release builds require it or the BDM connection is rejected. | Test with a debug build first; add `BDM_SECRET` env var and `-x` flag for release builds. |
| 3 | **IPN EID normalization** | Cross-integration routes to `ipn:4.55`. The BDM must normalize this to `ipn:4.0` via `pyd3tn.eid.get_node_id()` for the EID map lookup to match. | Run BDM with `-vv` and inspect the first dispatch event log line to confirm the normalized EID. |
| 4 | **FIB link UP timing** | The test sends a bundle shortly after the BDM starts. The link scheduler sends LINK_STATUS_UP immediately for `t=0` contacts, but the FIB update must complete before the bundle arrives. | The BDM readiness probe (grep for "Listening for dispatch events") is a sufficient gate; the link UP is sent before the main loop starts listening. Add a short `sleep 1` after the probe if race conditions appear. |
| 5 | **Unibo-BP routing** | Unibo nodes are not µD3TN and cannot connect a BDM. The cross-integration test's Unibo→µD3TN direction still requires Unibo's native route configuration. | Keep `aap2_config --schedule` (or Unibo equivalent) only for the routes that originate from Unibo nodes. |
| 6 | **Multiple BDMs per topology** | Each µD3TN node needs its own BDM instance with its own EID map (because `cla_addr: null` marks the local node). The contact plan `.cp` file can be shared. | Two BDM containers per compose file, each mounting the same `/config` volume but with different `ASABR_EID_MAP` env vars. |

---

## Summary of Files to Create / Modify

### New files

| File | Purpose |
|------|---------|
| `docker/asabr-bdm/Dockerfile` | BDM container image (Rust + maturin + a-sabr-python + asabr_bdm) |
| `docker/asabr-bdm/entrypoint.sh` | Waits for socket, launches `asabr_bdm/main.py` |
| `tests/interop/asabr/ud3tn-basic.cp` | Contact plan for 2-node µD3TN test |
| `tests/interop/asabr/ud3tn-basic-node-a.json` | EID map for Node A |
| `tests/interop/asabr/ud3tn-basic-node-b.json` | EID map for Node B |
| `tests/interop/asabr/cross-integration.cp` | Contact plan for 4-node cross test |
| `tests/interop/asabr/cross-node-a.json` | EID map for µD3TN Node A (cross test) |
| `tests/interop/asabr/cross-node-b.json` | EID map for µD3TN Node B (cross test) |
| `external/a-sabr-python` | Git submodule (A-SABR Python bindings) |
| `external/asabr_bdm` | Git submodule (BDM Python package) |

### Modified files

| File | Change |
|------|--------|
| `docker/ud3tn/entrypoint.sh` | Add `ASABR_ENABLED` guard; pass `-d` (and optionally `-x`) to µD3TN |
| `docker/docker-compose.zmqhub.yml` | Add `ASABR_ENABLED=1` to µD3TN services; add two `asabr-bdm-node-*` services |
| `tests/interop/test-ud3tn-basic.sh` | Remove `aap2_config --schedule`; add BDM readiness probe |
| `tests/interop/test-cross-integration.sh` | Remove µD3TN-side `aap2_config --schedule`; add BDM readiness probes |
| `tests/interop/run-tests.sh` | Add `docker build` for the BDM image |
