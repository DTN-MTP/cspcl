# uD3TN Integration with CSPCL

This guide explains how to integrate CSPCL (CubeSat Space Protocol Convergence Layer) with uD3TN to enable Bundle Protocol communication over CSP using the ZMQHUB interface.

## Architecture

```
+-----------------------------------------------------------------------------+
|                              Application Layer                               |
|                         (aap2-send / aap2-receive)                          |
+-----------------------------------------------------------------------------+
                                      |
                                      v
+-----------------------------------------------------------------------------+
|                                  uD3TN                                       |
|                            Bundle Processor                                  |
|                          (BP7 bundles handling)                              |
+-----------------------------------------------------------------------------+
                                      |
                                      v
+-----------------------------------------------------------------------------+
|                               CLA_CSP                                        |
|                    (Convergence Layer Adapter for CSP)                       |
|                         components/cla/posix/cla_csp.c                       |
+-----------------------------------------------------------------------------+
                                      |
                                      v
+-----------------------------------------------------------------------------+
|                                CSPCL                                         |
|                  (CSP Convergence Layer - fragmentation/reassembly)          |
|                           external/cspcl/cspcl.c                             |
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
|                      tcp://localhost:6000 (XSUB)                             |
|                      tcp://localhost:7000 (XPUB)                             |
+-----------------------------------------------------------------------------+
```

## Prerequisites

- **uD3TN source code**: Clone from https://gitlab.com/d3tn/ud3tn.git
- **libcsp 1.6**: Built with ZMQHUB support (Clone from https://github.com/libcsp/libcsp/tree/v1.6#)
- **Python 3** with pyzmq: `pip3 install pyzmq`
- **Build tools**: gcc, make, cmake

## Installation

### Option 1: Apply Patch (Recommended)

```bash
# Clone uD3TN
git clone https://gitlab.com/d3tn/ud3tn.git ud3tn-src
cd ud3tn-src

# You will need to replace the path in the patch /home/mathias/libcsp-src with your actual path
sed -i 's|/home/mathias/libcsp-src|/path/to/your/sources/libcsp-src|g' ud3tn-cla-csp.patch

# Apply the CSPCL integration patch
git apply /path/to/cspcl/ud3tn-integration/ud3tn-cla-csp.patch

# Build
make
```

### Option 2: Manual File Copy (Development)

```bash
# You will need to replace the path in the patch /home/mathias/libcsp-src with your actual path
sed -i 's|/home/mathias/libcsp-src|/path/to/your/sources/libcsp-src|g' dev.patch

# Apply the CSPCL integration patch
git apply /path/to/cspcl/ud3tn-integration/dev.patch

# Set environment variables
export CSPCL_REPO=/path/to/cspcl
export UD3TN_REPO=/path/to/ud3tn-src

# Copy CSPCL library files
cp ${CSPCL_REPO}/src/cspcl.c ${UD3TN_REPO}/external/cspcl/
cp ${CSPCL_REPO}/src/cspcl.h ${UD3TN_REPO}/external/cspcl/

# Copy CLA_CSP integration files
cp ${CSPCL_REPO}/ud3tn-integration/src/cla_csp.c ${UD3TN_REPO}/components/cla/posix/
cp ${CSPCL_REPO}/ud3tn-integration/src/cla_csp.h ${UD3TN_REPO}/include/cla/

# Build uD3TN
cd ${UD3TN_REPO}
make
```

## Building libcsp with ZMQHUB Support

If libcsp is not already configured with ZMQHUB:

```bash
cd /path/to/libcsp

# Using waf (libcsp 1.x) - version 1.6 recommended, others untested, requires python 3.11
python3 waf configure --enable-zmqhub
python3 waf build

# Or using CMake (libcsp 2.x) - not working yet
mkdir build && cd build
cmake .. -DCSP_USE_ZMQHUB=ON
make
```

## Testing Bundle Transfer

### Refer to the script at `tools/simple_bundle_transfer.sh`

```bash
./tools/simple_bundle_transfer.sh -h
```

## CLA Configuration

The CSP CLA is configured via command line:

```
--cla "csp:<local_addr>,<port>"
```

| Parameter | Description | Example |
|-----------|-------------|---------|
| `local_addr` | CSP address of this node (0-255) | `1` |
| `port` | CSP port for Bundle Protocol | `10` |

Example configurations:
- Node A (space): `--cla "csp:1,10"`
- Node B (ground): `--cla "csp:2,10"`

## Troubleshooting

### Error: "Port 10 is already in use"

This means CSP tried to bind to port 10 multiple times.

**Solution**: Kill existing uD3TN processes and restart:
```bash
pkill -f ud3tn
rm -f /tmp/*.aap2.socket
```

### No packets visible in ZMQ broker

**Check**:
1. Broker is running on ports 6000/7000
2. Both uD3TN nodes started successfully
3. Route was configured correctly

```bash
# Verify broker ports
ss -tlnp | grep -E "6000|7000"

# Verify uD3TN processes
ps aux | grep ud3tn
```

### Message not received

**Check**:
1. Route configured: `aap2-config --socket ... --schedule ...`
2. Receiver running: `aap2-receive --socket ... --agentid ...`
3. Destination EID matches: `dtn://b.dtn/bundlesink`

### Enable Debug Logging

```bash
export UD3TN_LOG_LEVEL=debug
./build/posix/ud3tn ...
```

## File Structure

```
cspcl/
|-- src/
|   |-- cspcl.c              # CSPCL library implementation
|   |-- cspcl.h              # CSPCL public API
|   +-- cspcl_config.h       # Configuration constants
|-- tools/
|   +-- zmqhub_broker.py     # ZMQ hub broker for testing
|-- ud3tn-integration/
|   |-- README.md            # This file
|   |-- src/
|   |   |-- cla_csp.c        # uD3TN CLA implementation
|   |   +-- cla_csp.h        # CLA header
|   +-- ud3tn-cla-csp.patch  # Patch for uD3TN integration
+-- stubs/
    +-- csp/                 # CSP stubs for standalone build
```

## Quick Reference

| Command | Description |
|---------|-------------|
| `python3 tools/zmqhub_broker.py -v` | Start ZMQ broker with verbose output |
| `aap2-config --socket <sock> --schedule 1 3600 100000 <dest_eid> csp:<addr>` | Configure route |
| `aap2-receive --socket <sock> --agentid <agent>` | Start bundle receiver |
| `aap2-send --socket <sock> <dest_eid>/<agent> '<message>'` | Send bundle |
| `pkill -f ud3tn && pkill -f zmqhub` | Cleanup all processes |

## References

- [uD3TN Documentation](https://gitlab.com/d3tn/ud3tn)
- [libcsp Documentation](https://github.com/libcsp/libcsp)
- [Bundle Protocol 7 (RFC 9171)](https://www.rfc-editor.org/rfc/rfc9171)
- [CubeSat Space Protocol](https://github.com/libcsp/libcsp/wiki)

