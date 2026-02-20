# Unibo-BP Integration with CSPCL

This guide explains how to integrate CSPCL (CubeSat Space Protocol Convergence Layer) with Unibo-BP to enable Bundle Protocol communication over CSP using the ZMQHUB interface.

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
cd /path/to/cspcl/unibo-integration
mkdir -p build

gcc -O2 -Wall -Wextra -DCSPCLA_STANDALONE_MAIN \
  src/cspcl_daemon.c ../src/cspcl.c \
  -o build/unibo-bp-cspcl \
  -I../src \
  -I/path/to/unibo-bp/include \
  -I/path/to/libcsp/include \
  -I/path/to/libcsp/build/include \
  -L/path/to/unibo-bp/build/Unibo-BP/lib \
  -Wl,-rpath,/path/to/unibo-bp/build/Unibo-BP/lib \
  -lunibo-bp-api \
  /path/to/libcsp/build/libcsp.a \
  -lzmq -lpthread -lm

# Verify build
ls -la build/unibo-bp-cspcl
file build/unibo-bp-cspcl
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

# Optional: Enable CSPCL tracing (recommended for debugging)
export CSPCLA_TRACE_PAYLOAD=1
export CSPCLA_TRACE_BYTES=64
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

| Parameter | Description | Example |
| ----------- | ------------- | --------- |
| `node_id` | CSP node address (0-255) | `1` |
| `port` | CSP port for BP messages | `10` |
| `broker_type` | Broker interface type | `zmqhub` |
| `broker_port` | Broker connection port (ZMQ) | `2001` (for node1), `2002` (for node2) |
| `node_workdir` | Unibo-BP working directory | `/tmp/unibo-node1` |

**Example usage**:

```bash
# Node 1 (CSP addr 1)
./build/unibo-bp-cspcl 1 10 zmqhub 2001 /tmp/unibo-node1

# Node 2 (CSP addr 2)  
./build/unibo-bp-cspcl 2 10 zmqhub 2002 /tmp/unibo-node2
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
nohup stdbuf -oL -eL ./build/unibo-bp-cspcl 1 10 zmqhub 2001 /tmp/unibo-node1 > /tmp/cspcl-node1.log 2>&1 &
nohup stdbuf -oL -eL ./build/unibo-bp-cspcl 2 10 zmqhub 2002 /tmp/unibo-node2 > /tmp/cspcl-node2.log 2>&1 &

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

## File Structure

```bash
cspcl/
├── src/
│   ├── cspcl.c              # CSPCL library implementation
│   ├── cspcl.h              # CSPCL public API
│   └── cspcl_config.h       # Configuration constants
├── tools/
│   └── zmqhub_broker.py     # ZMQ hub broker for testing
├── unibo-integration/
│   ├── README.md            # This file
│   ├── COMMANDS.md          # Step-by-step runbook
│   ├── src/
│   │   ├── cspcl_daemon.c   # Unibo-BP CSPCL daemon implementation
│   │   └── cspcl_daemon.h   # Daemon header
│   └── build/
│       └── unibo-bp-cspcl   # Compiled daemon executable
└── stubs/
    └── csp/                 # CSP stubs for standalone build
```

## References

- [Unibo-BP on GitHub](https://gitlab.com/unibo-dtn/unibo-bp)
- [CSPCL on GitHub](hhttps://github.com/DTN-MTP/cspcl)
- [libcsp v1.6 on GitHub](https://github.com/libcsp/libcsp/tree/v1.6)
- [Bundle Protocol 7 (RFC 9171)](https://www.rfc-editor.org/rfc/rfc9171)
- [CubeSat Space Protocol Documentation](https://github.com/libcsp/libcsp/wiki)

## Quick Reference

| Task | Command |
| ------ | --------- |
| Start ZMQ broker | `python3 $CSPCL_DIR/tools/zmqhub_broker.py -v` |
| Start Unibo-BP node | `$UNIBO_BP_BIN/unibo-bp start --daemon --dtn-admin dtn://X.dtn/ --ipn-admin ipn:X.0` |
| Configure DTN routes | `$UNIBO_BP_BIN/unibo-bp-admin routing static add --destination ipn:Y.Z --gateway ipn:Y.0` |
| Start CSPCL daemon | `./build/unibo-bp-cspcl 1 10 zmqhub 2001 /tmp/unibo-node1` |
| Send test bundle | `$UNIBO_BP_BIN/unibo-bp-send --source ipn:1.55 --destination ipn:2.55 --payload-string 'Hello'` |
| Receive bundle | `$UNIBO_BP_BIN/unibo-bp-sink ipn:2.55` |
| Check processes | `pgrep -fa 'unibo-bp\|cspcl\|zmqhub'` |
| Cleanup all | `pkill -9 -f 'unibo-bp-cspcl\|unibo-bp-sink\|zmqhub_broker.py'` |
