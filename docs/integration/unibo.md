---
layout: default
title: Unibo Integration
nav_order: 2
parent: BP Integrations
permalink: /integration/unibo/
---

# Unibo-BP Integration with CSPCL

This guide explains how to integrate CSPCL (CubeSat Space Protocol Convergence Layer) with Unibo-BP to enable Bundle Protocol communication over CSP using the ZMQHUB interface.

## ASABR Adapter

The Unibo integration supports ASABR-backed route selection through the CSPCL adapter process provider.

## Architecture

```bash
+-----------------------------------------------------------------------------+
|                              Application Layer                               |
|                      (aap2-send / aap2-receive / custom apps)               |
+-----------------------------------------------------------------------------+
                                      |
                                      v
+-----------------------------------------------------------------------------+
|                             Unibo-BP Core                                    |
|                        Bundle Processor (BP7)                                |
|                   (Bundle routing and forwarding)                            |
+-----------------------------------------------------------------------------+
                                      |
                                      v
+-----------------------------------------------------------------------------+
|                        CSPCL Daemon (unibo-bp-cspcl)                         |
|                    (CSP Convergence Layer Adapter)                           |
|                     (fragmentation/reassembly, socket I/O)                   |
+-----------------------------------------------------------------------------+
                                      |
                                      v
+-----------------------------------------------------------------------------+
|                                CSPCL Library                                 |
|                   (CSP message handling & assembly)                          |
|                            cspcl.c / cspcl.h                                 |
+-----------------------------------------------------------------------------+
                                      |
                                      v
+-----------------------------------------------------------------------------+
|                                libcsp                                        |
|                      (CubeSat Space Protocol stack)                          |
|                            ZMQHUB Interface                                  |
+-----------------------------------------------------------------------------+
                                      |
                                      v
+-----------------------------------------------------------------------------+
|                            ZMQ Hub Broker                                    |
|                    (Routes CSP packets between nodes)                        |
|                      tcp://localhost:2000+ (node ports)                      |
+-----------------------------------------------------------------------------+
```

## Prerequisites

- **Unibo-BP source code**: Clone from [Unibo-BP repository](https://gitlab.com/unibo-dtn/unibo-bp.git)
- **Unibo-BP build requirements**: CMake 3.16+ (tested with 3.31.x), C compiler, standard libraries
- **libcsp 1.6**: Built with ZMQHUB support [libcsp v1.6 repository](https://github.com/libcsp/libcsp/tree/v1.6)
- **CSPCL source code**: See [CSPCL repository](https://github.com/DTN-MTP/cspcl)
- **Python 3** with pyzmq: `pip3 install pyzmq`
- **Build tools**: gcc, make, CMake 3.16+

## Installation

### Step 1: Build Unibo-BP

Follow the [Unibo-BP README](https://gitlab.com/unibo-dtn/unibo-bp/-/blob/main/README.md) for build instructions using CMake.

### Step 2: Build libcsp with ZMQHUB Support

```bash
cd /path/to/libcsp

# Using waf (libcsp 1.x, version 1.6 recommended)
python3 waf configure --enable-can-socketcan --enable-if-zmqhub
python3 waf build

# Verify build output
ls -la build/
# Expected: include/, src/
```

### Step 3: Build CSPCLA Daemon

The CSPCLA daemon is a standalone executable that bridges Unibo-BP with libcsp over ZMQHUB.

```bash
cd "$INTEG_DIR"
mkdir -p build

gcc -O2 -Wall -Wextra \
  src/cspcl_daemon.c $CSPCL_C_SRC/cspcl.c \
  $CSPCL_C_SRC/cspcl_route_bridge.c $CSPCL_C_SRC/cspcl_asabr_process_provider.c \
  -o build/unibo-bp-cspcl \
  -I$CSPCL_C_SRC \
  -I$DTN_ROOT/unibo-dtn/unibo-bp/include \
  -I$DTN_ROOT/libcsp/include \
  -I$DTN_ROOT/libcsp/build/include \
  -L$UNIBO_BP_LIB \
  -Wl,-rpath,$UNIBO_BP_LIB \
  -lunibo-bp-api \
  $LIBCSP_BUILD/libcsp.a \
  -lzmq -lpthread -lm -lsocketcan

# Verify build
ls -la build/unibo-bp-cspcl
file build/unibo-bp-cspcl
```

### Step 3b: Build CSPCLA Daemon for CAN (SocketCAN)

If you use CAN instead of ZMQHUB, compile the daemon with SocketCAN support:

```bash
cd "$INTEG_DIR"
mkdir -p build

gcc -O2 -Wall -Wextra \
  src/cspcl_daemon.c $CSPCL_C_SRC/cspcl.c \
  $CSPCL_C_SRC/cspcl_route_bridge.c $CSPCL_C_SRC/cspcl_asabr_process_provider.c \
  -o build/unibo-bp-cspcl \
  -I$CSPCL_C_SRC \
  -I$DTN_ROOT/unibo-dtn/unibo-bp/include \
  -I$DTN_ROOT/libcsp/include \
  -I$DTN_ROOT/libcsp/build/include \
  -L$UNIBO_BP_LIB \
  -Wl,-rpath,$UNIBO_BP_LIB \
  -lunibo-bp-api \
  $LIBCSP_BUILD/libcsp.a \
  -lzmq -lpthread -lm \
  -lsocketcan
```

## Environment Setup

Create environment variables for easy access to key paths:

```bash
# Add to ~/.bashrc (or your shell config)
cat >> ~/.bashrc <<'EOF'
export DTN_ROOT=/path/to/dtn/workspace
export CSPCL_DIR=$DTN_ROOT/cspcl
export INTEG_DIR=$CSPCL_DIR/unibo-integration
export UNIBO_BP_BIN=$DTN_ROOT/unibo-dtn/unibo-bp/build/Unibo-BP/bin
export UNIBO_BP_LIB=$DTN_ROOT/unibo-dtn/unibo-bp/build/Unibo-BP/lib
export LIBCSP_BUILD=$DTN_ROOT/libcsp/build
EOF

source ~/.bashrc
export CSPCL_C_SRC=$CSPCL_DIR/rust-bindings/cspcl-sys/c_src

# Optional: Enable CSPCL tracing (recommended for debugging)
export CSPCLA_TRACE_PAYLOAD=1
export CSPCLA_TRACE_BYTES=64

# Required to enable ASABR route provider in unibo-bp-cspcl
export CSPCL_ASABR_ADAPTER_BIN=$CSPCL_DIR/rust-bindings/target/release/cspcl-asabr-adapter

# Use an epoch-time-compatible contact plan for live tests
cat > /tmp/asabr-dtn-dynamic3.cp <<'EOF'
node 0 n0
node 1 n1
node 2 n2
contact 1 2 0 4000000000 evl 1000000 1
EOF
export CSPCL_ASABR_CONTACT_PLAN_PATH=/tmp/asabr-dtn-dynamic3.cp

# Build adapter binary if not already built
cd "$CSPCL_DIR/rust-bindings"
cargo build --release -p cspcl-asabr-adapter
```

## Testing Bundle Transfer

For a complete, step-by-step runbook of a minimal end-to-end transfer, **refer to [COMMANDS.md](COMMANDS.md)**.

The runbook includes:

1. **Environment validation** - Verify all paths and tools
2. **Process cleanup** - Kill stale services
3. **Build the CSPCL daemon** - Single GCC command
4. **Start components** - ZMQ broker, Unibo-BP nodes, CSPCL daemons
5. **Configure nodes** - Routes, contacts, ranges (DTN topology)
6. **Send a bundle** - End-to-end transfer verification

## CSPCL Daemon Configuration

The `unibo-bp-cspcl` daemon bridges a Unibo-BP node to CSP via ZMQHUB.

**Syntax**:

```bash
unibo-bp-cspcl <node_id> <port> <broker_type> <broker_port> <node_workdir>
```

| Parameter      | Description                  | Example                                |
| -------------- | ---------------------------- | -------------------------------------- |
| `node_id`      | CSP node address (0-255)     | `1`                                    |
| `port`         | CSP port for BP messages     | `10`                                   |
| `broker_type`  | Broker interface type        | `zmqhub`                               |
| `broker_port`  | Broker connection port (ZMQ) | `2001` (for node1), `2002` (for node2) |
| `node_workdir` | Unibo-BP working directory   | `/tmp/unibo-node1`                     |

The `broker_type` is interface-dependent:

- Use `zmqhub` for ZMQHUB transport.
- Use `can` for CAN transport.

**Example usage**:

```bash
# Node 1 (CSP addr 1)
stdbuf -oL -eL env \
  CSPCL_ASABR_ADAPTER_BIN="$CSPCL_ASABR_ADAPTER_BIN" \
  CSPCL_ASABR_CONTACT_PLAN_PATH="$CSPCL_ASABR_CONTACT_PLAN_PATH" \
  ./build/unibo-bp-cspcl 1 10 zmqhub 2001 /tmp/unibo-node1

# Node 2 (CSP addr 2)
stdbuf -oL -eL env \
  CSPCL_ASABR_ADAPTER_BIN="$CSPCL_ASABR_ADAPTER_BIN" \
  CSPCL_ASABR_CONTACT_PLAN_PATH="$CSPCL_ASABR_CONTACT_PLAN_PATH" \
  ./build/unibo-bp-cspcl 2 10 zmqhub 2002 /tmp/unibo-node2
```

For CAN, use the same command shape and switch `broker_type`:

```bash
# Node 1 (CAN)
stdbuf -oL -eL env \
  CSPCL_ASABR_ADAPTER_BIN="$CSPCL_ASABR_ADAPTER_BIN" \
  CSPCL_ASABR_CONTACT_PLAN_PATH="$CSPCL_ASABR_CONTACT_PLAN_PATH" \
  ./build/unibo-bp-cspcl 1 10 can 2001 /tmp/unibo-node1

# Node 2 (CAN)
stdbuf -oL -eL env \
  CSPCL_ASABR_ADAPTER_BIN="$CSPCL_ASABR_ADAPTER_BIN" \
  CSPCL_ASABR_CONTACT_PLAN_PATH="$CSPCL_ASABR_CONTACT_PLAN_PATH" \
  ./build/unibo-bp-cspcl 2 10 can 2002 /tmp/unibo-node2
```

## Troubleshooting

### Issue: CSPCL daemon fails to start with "libunibo-bp-api.so not found"

**Cause**: Library path not correct or not in `LD_LIBRARY_PATH`.

**Solution**: Ensure the shared library path is set when building:

```bash
gcc ... -L$UNIBO_BP_LIB -Wl,-rpath,$UNIBO_BP_LIB ...
```

Verify:

```bash
ldd ./build/unibo-bp-cspcl | grep unibo-bp-api
```

### Issue: Bundles buffer but do not send

**Cause**: CSPCL daemon may have crashed or connection lost.

**Solution**: Restart only the CSPCL daemons (keep broker and Unibo-BP cores alive):

```bash
pkill -9 -f 'unibo-bp-cspcl'

# Restart daemons
nohup stdbuf -oL -eL env \
  CSPCL_ASABR_ADAPTER_BIN="$CSPCL_ASABR_ADAPTER_BIN" \
  CSPCL_ASABR_CONTACT_PLAN_PATH="$CSPCL_ASABR_CONTACT_PLAN_PATH" \
  ./build/unibo-bp-cspcl 1 10 zmqhub 2001 /tmp/unibo-node1 > /tmp/cspcl-node1.log 2>&1 &
nohup stdbuf -oL -eL env \
  CSPCL_ASABR_ADAPTER_BIN="$CSPCL_ASABR_ADAPTER_BIN" \
  CSPCL_ASABR_CONTACT_PLAN_PATH="$CSPCL_ASABR_CONTACT_PLAN_PATH" \
  ./build/unibo-bp-cspcl 2 10 zmqhub 2002 /tmp/unibo-node2 > /tmp/cspcl-node2.log 2>&1 &

sleep 1
pgrep -fa 'unibo-bp-cspcl'
```

### Issue: ZMQ broker fails to start on ports 6000/7000

**Cause**: Ports already bound or permission denied.

**Solution**: Check what's using the ports:

```bash
ss -tlnp | grep -E "6000|7000"

# Kill conflicting process if safe:
pkill -f zmqhub_broker.py
```

### Issue: No route to destination or bundle discarded

**Cause**: DTN topology not configured or routes missing.

**Solution**: Verify configuration using COMMANDS.md:

1. Both nodes registered in "home" region
2. Ranges configured for both directions (owlt=0 for local testing)
3. Contacts configured with adequate transmit rate
4. Static routes added for destination endpoints

```bash
cd /tmp/unibo-node1
$UNIBO_BP_BIN/unibo-bp-admin region home
$UNIBO_BP_BIN/unibo-bp-admin contact list
$UNIBO_BP_BIN/unibo-bp-admin routing static list
```

### Enable Debug Logging

Check CSPCL daemon logs for packet assembly/disassembly issues:

```bash
tail -f /tmp/cspcl-node1.log
tail -f /tmp/cspcl-node2.log

# Check Unibo-BP logs (if running in foreground)
cd /tmp/unibo-node1
tail -f .uD3TN/unibo-bp.log
```

## Quick Reference

| Task                 | Command                                                                                         |
| -------------------- | ----------------------------------------------------------------------------------------------- |
| Start ZMQ broker     | `python3 $CSPCL_DIR/tools/zmqhub_broker.py -v`                                                  |
| Start Unibo-BP node  | `$UNIBO_BP_BIN/unibo-bp start --daemon --dtn-admin dtn://X.dtn/ --ipn-admin ipn:X.0`            |
| Configure DTN routes | `$UNIBO_BP_BIN/unibo-bp-admin routing static add --destination ipn:Y.Z --gateway ipn:Y.0`       |
| Start CSPCL daemon   | `env CSPCL_ASABR_ADAPTER_BIN="$CSPCL_ASABR_ADAPTER_BIN" CSPCL_ASABR_CONTACT_PLAN_PATH="$CSPCL_ASABR_CONTACT_PLAN_PATH" ./build/unibo-bp-cspcl 1 10 zmqhub 2001 /tmp/unibo-node1` |
| Send test bundle     | `$UNIBO_BP_BIN/unibo-bp-send --source ipn:1.55 --destination ipn:2.55 --payload-string 'Hello'` |
| Receive bundle       | `$UNIBO_BP_BIN/unibo-bp-sink ipn:2.55`                                                          |
| Check processes      | `pgrep -fa 'unibo-bp\|cspcl\|zmqhub'`                                                           |
| Cleanup all          | `pkill -9 -f 'unibo-bp-cspcl\|unibo-bp-sink\|zmqhub_broker.py'`                                 |
