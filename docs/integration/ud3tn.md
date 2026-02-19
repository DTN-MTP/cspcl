---
layout: default
title: uD3TN Integration
nav_order: 4
permalink: /integration/ud3tn/
---

# uD3TN Integration

CSPCL ships with a ready-to-use Convergence Layer Adapter (`cla_csp`) that plugs into
[uD3TN](https://gitlab.com/d3tn/ud3tn), allowing uD3TN to exchange BP7 bundles over
CSP on two transports: **ZMQHUB** (virtual/Ethernet-tunnelled) and **CAN** (SocketCAN).

## Architecture

```
aap2-send / aap2-receive
        |
        v
    uD3TN (Bundle Processor)
        |
        v
    cla_csp  <--- components/cla/posix/cla_csp.c
        |
        v
    CSPCL  <----- external/cspcl/cspcl.c
        |
        v
    libcsp
        |
        +-- ZMQHUB interface ---> ZMQ Hub Broker (tcp://:6000 / :7000)
        |
        +-- SocketCAN interface ---> vcan0 / can0
```

---

## Prerequisites

### Common

| Requirement | Notes |
| --- | --- |
| uD3TN source | `git clone https://gitlab.com/d3tn/ud3tn.git` |
| libcsp v1.6 | Built with the appropriate interface(s) enabled — see below |
| Build tools | gcc, make, cmake |

### ZMQHUB transport

| Requirement | Notes |
| --- | --- |
| libcsp v1.6 | Built with `--enable-if-zmqhub` |
| Python 3 + pyzmq | `pip3 install pyzmq` — needed to run the broker |

### CAN transport

| Requirement | Notes |
| --- | --- |
| libcsp v1.6 | Built with `--enable-can-socketcan` |
| SocketCAN kernel modules | `vcan` for virtual testing, or a real CAN adapter (e.g. Peak, Kvaser) |
| `can-utils` *(optional)* | `candump` / `cansend` for low-level debugging |

---

## Building libcsp

```bash
cd /path/to/libcsp

# ZMQHUB only
python3 waf configure --enable-if-zmqhub
python3 waf build

# CAN (SocketCAN) only
python3 waf configure --enable-can-socketcan
python3 waf build

# Both (recommended for development)
python3 waf configure --enable-can-socketcan --enable-if-zmqhub
python3 waf build
```

> libcsp v1.6 requires Python 3.11+ for the waf build system.

---

## Applying the Patch

```bash
cd /path/to/ud3tn

# Replace the hardcoded libcsp path inside the patch
sed -i 's|/home/mathias/libcsp-src|/path/to/libcsp-src|g' \
    /path/to/cspcl/ud3tn-integration/ud3tn-cla-csp.patch

git apply /path/to/cspcl/ud3tn-integration/ud3tn-cla-csp.patch
make
```

## Manual File Copy (Alternative)

```bash
export CSPCL=/path/to/cspcl
export UD3TN=/path/to/ud3tn

cp $CSPCL/src/cspcl.c   $UD3TN/external/cspcl/
cp $CSPCL/src/cspcl.h   $UD3TN/external/cspcl/

cp $CSPCL/ud3tn-integration/src/cla_csp.c  $UD3TN/components/cla/posix/
cp $CSPCL/ud3tn-integration/src/cla_csp.h  $UD3TN/include/cla/

cd $UD3TN && make
```

---

## CLA Configuration

The transport is selected via the `--cla` argument passed to uD3TN:

```
--cla "csp:<local_addr>,<port>"          # ZMQHUB (default)
--cla "csp:<local_addr>,<port>,can"      # SocketCAN
```

| Parameter | Description | Default |
| --- | --- | --- |
| `local_addr` | CSP node address of this uD3TN instance (0-255) | — |
| `port` | CSP port for Bundle Protocol traffic | `10` |
| `can` *(optional)* | Use SocketCAN instead of ZMQHUB | ZMQHUB |

---

## Running a Two-Node Test

Use the helper script for a guided walkthrough including diagnostics:

```bash
# ZMQHUB transport
./tools/simple_bundle_transfer.sh --transport zmqhub

# CAN transport
./tools/simple_bundle_transfer.sh --transport can
```

Or follow the steps below manually.

### ZMQHUB

**1. Start the broker**

```bash
python3 tools/zmqhub_broker.py -v
```

**2. Start Node A (CSP addr 1)**

```bash
ud3tn --node-id dtn://a.dtn/ --aap-port 4242 \
      --aap2-socket 1.aap2.socket --cla "csp:1,10"
```

**3. Start Node B (CSP addr 2)**

```bash
ud3tn --node-id dtn://b.dtn/ --aap-port 4243 \
      --aap2-socket 2.aap2.socket --cla "csp:2,10"
```

---

### CAN (SocketCAN)

**1. Set up the virtual CAN interface** *(skip for physical CAN)*

```bash
sudo modprobe vcan
sudo ip link add dev vcan0 type vcan
sudo ip link set up vcan0
```

**2. Start Node A (CSP addr 1)**

```bash
ud3tn --node-id dtn://a.dtn/ --aap-port 4242 \
      --aap2-socket 1.aap2.socket --cla "csp:1,10,can"
```

**3. Start Node B (CSP addr 2)**

```bash
ud3tn --node-id dtn://b.dtn/ --aap-port 4243 \
      --aap2-socket 2.aap2.socket --cla "csp:2,10,can"
```

> Both nodes share the same CAN bus (`vcan0`), so no broker process is needed.

---

### Common steps (both transports)

**4. Configure route A to B**

```bash
aap2-config --socket 1.aap2.socket \
    --schedule 1 3600 100000 dtn://b.dtn/ csp:2
```

**5. Start receiver on B**

```bash
aap2-receive --socket 2.aap2.socket --agentid bundlesink
```

**6. Send a bundle from A**

```bash
aap2-send --socket 1.aap2.socket dtn://b.dtn/bundlesink 'hello,world!'
```

---

## Troubleshooting

**`Port 10 is already in use`** — a previous uD3TN process left the CSP port bound.

```bash
pkill -f ud3tn && rm -f /tmp/*.aap2.socket
```

**No packets at the ZMQ broker** — verify both nodes connect to the same host/ports
and that libcsp was built with `--enable-if-zmqhub`.

**CAN frames not appearing** — verify `vcan0` is up (`ip link show vcan0`) and that
libcsp was built with `--enable-can-socketcan`. Use `candump vcan0` to inspect raw frames.

**Bundle not delivered** — confirm the route was scheduled with `aap2-config` and that
the agent ID in the receiver matches the service portion of the destination EID.

**Enable debug logging**

```bash
export UD3TN_LOG_LEVEL=debug
```

Look for these log lines in uD3TN output:

- `CSP: Initialized with local address X, port Y`
- `CSP: Starting scheduled contact to csp:X`
- `CSP: Sending N bytes to csp:X`
- `CSP: RX task started`
