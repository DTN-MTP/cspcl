# CSPCL - CubeSat Space Protocol Convergence Layer

A convergence layer adapter that enables Bundle Protocol 7 (BP7) bundles to be transmitted over CubeSat Space Protocol (CSP).

## Overview

CSPCL provides a bridge between BP7 (Delay/Disruption Tolerant Networking) and CSP (commonly used in CubeSat missions). It follows the architecture:

```
BP7 Bundle → CSPCL → CSP UDP → Physical Layer (CAN/ZMQHUB/SocketCAN)
```

## Features

- **CSP v1.6 Compatible** - Works with libcsp v1.6 (not v2)
- **UDP Mode** - Uses connectionless CSP for minimal overhead
- **Fragmentation** - Automatic bundle fragmentation/reassembly
- **Address Translation** - IPN/DTN endpoint ID to CSP address mapping
- **Cross-Platform** - Supports POSIX (Linux) and FreeRTOS
- **Minimal Footprint** - ~800 lines of C code

## Requirements

| Requirement | Description |
|-------------|-------------|
| CSP Version | v1.6 (not v2 - different packet format) |
| CSP Mode | UDP (connectionless, no RDP) |
| Platform | POSIX (Linux) or FreeRTOS |
| Interfaces | ZMQHUB, SocketCAN (ground), CAN (space) |

## Quick Start with uD3TN

The primary use case for CSPCL is integration with [uD3TN](https://gitlab.com/d3tn/ud3tn). See [ud3tn-integration/README.md](ud3tn-integration/README.md) for complete instructions.

### Quick Test (5 terminals)

```bash
# Terminal 1: Start ZMQ broker
python3 tools/zmqhub_broker.py -v

# Terminal 2: Start Node A
./ud3tn --node-id dtn://a.dtn/ --aap-port 4242 --aap2-socket /tmp/1.aap2.socket --cla "csp:1,10"

# Terminal 3: Start Node B
./ud3tn --node-id dtn://b.dtn/ --aap-port 4243 --aap2-socket /tmp/2.aap2.socket --cla "csp:2,10"

# Terminal 4: Configure route + Receive
aap2-config --socket /tmp/1.aap2.socket --schedule 1 3600 100000 dtn://b.dtn/ csp:2
aap2-receive --socket /tmp/2.aap2.socket --agentid bundlesink

# Terminal 5: Send message
aap2-send --socket /tmp/1.aap2.socket dtn://b.dtn/bundlesink 'Hello via CSPCL!'
```

## Building

### Prerequisites

- CMake 3.10+
- C11 compatible compiler
- libcsp v1.6 (optional for stub-based testing)
- Python 3 with pyzmq (`pip3 install pyzmq`) for ZMQHUB testing

### Build Commands

```bash
mkdir build && cd build
cmake ..
make

# Run tests
ctest --verbose
```

### Build Options

| Option | Default | Description |
|--------|---------|-------------|
| `CSPCL_BUILD_TESTS` | ON | Build unit tests |
| `CSPCL_BUILD_EXAMPLES` | ON | Build example applications |
| `CSPCL_USE_FREERTOS` | OFF | Build for FreeRTOS (default: POSIX) |
| `CSPCL_DEBUG` | OFF | Enable debug output |

Example with options:
```bash
cmake -DCSPCL_DEBUG=ON -DCSPCL_BUILD_EXAMPLES=ON ..
```

## Usage

### Initialization

```c
#include "cspcl.h"

cspcl_t cspcl;
cspcl_error_t err;

// Initialize with local CSP address
err = cspcl_init(&cspcl, 1);  // CSP address 1
if (err != CSPCL_OK) {
    printf("Error: %s\n", cspcl_strerror(err));
}

// Open RX socket (bind to port once)
err = cspcl_open_rx_socket(&cspcl);
```

### Sending a Bundle

```c
uint8_t bundle[] = { /* serialized BP7 bundle */ };
size_t bundle_len = sizeof(bundle);
uint8_t dest_addr = 2;  // Destination CSP address

err = cspcl_send_bundle(&cspcl, bundle, bundle_len, dest_addr);
if (err != CSPCL_OK) {
    printf("Send failed: %s\n", cspcl_strerror(err));
}
```

### Receiving a Bundle

```c
uint8_t bundle[CSPCL_MAX_BUNDLE_SIZE];
size_t bundle_len = sizeof(bundle);
uint8_t src_addr;
uint32_t timeout_ms = 5000;

err = cspcl_recv_bundle(&cspcl, bundle, &bundle_len, &src_addr, timeout_ms);
if (err == CSPCL_OK) {
    printf("Received %zu bytes from CSP addr %d\n", bundle_len, src_addr);
}
```

### Address Translation

```c
// BP endpoint ID to CSP address
uint8_t addr = cspcl_endpoint_to_addr("ipn:5.0");  // Returns 5

// CSP address to BP endpoint ID
char endpoint[32];
cspcl_addr_to_endpoint(5, endpoint, sizeof(endpoint));  // "ipn:5.0"
```

### Cleanup

```c
cspcl_cleanup(&cspcl);
```

## CSPCL Header Format

Each CSP packet carrying bundle data includes a 10-byte CSPCL header:

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0 | 1 | version | Protocol version (1) |
| 1 | 1 | flags | Fragment flags |
| 2 | 2 | fragment_id | Bundle transfer identifier |
| 4 | 2 | fragment_offset | Offset in original bundle |
| 6 | 4 | bundle_size | Total bundle size |

### Flag Bits

| Bit | Name | Description |
|-----|------|-------------|
| 0x01 | FIRST | First fragment |
| 0x02 | LAST | Last fragment |
| 0x04 | MORE | More fragments follow |

## Configuration

Edit `cspcl_config.h` to customize:

```c
// CSP port for Bundle Protocol
#define CSPCL_PORT_BP               10

// Maximum CSP MTU
#define CSPCL_CSP_MTU               256

// Maximum bundle size
#define CSPCL_MAX_BUNDLE_SIZE       65535

// Reassembly contexts
#define CSPCL_MAX_REASSEMBLY_CTX    8

// Reassembly timeout (ms)
#define CSPCL_REASSEMBLY_TIMEOUT_MS 30000
```

## Project Structure

```
cspcl/
|-- CMakeLists.txt          # Build configuration
|-- README.md               # This file
|-- implementation.md       # Design documentation
|-- src/
|   |-- cspcl.h             # Public API header
|   |-- cspcl.c             # Implementation
|   +-- cspcl_config.h      # Configuration options
|-- stubs/
|   |-- csp/
|   |   +-- csp.h           # CSP API stub
|   +-- csp_stub.c          # CSP stub implementation
|-- tests/
|   |-- CMakeLists.txt
|   +-- test_cspcl.c        # Unit tests
|-- tools/
|   +-- zmqhub_broker.py    # ZMQ hub broker for testing
+-- ud3tn-integration/      # uD3TN CLA integration
    |-- README.md           # Integration documentation
    +-- src/
        |-- cla_csp.c       # uD3TN CLA implementation
        +-- cla_csp.h       # CLA header
```

## Error Handling

All functions return `cspcl_error_t`:

| Error Code | Description |
|------------|-------------|
| `CSPCL_OK` | Success |
| `CSPCL_ERR_INVALID_PARAM` | Invalid parameter |
| `CSPCL_ERR_NO_MEMORY` | Memory allocation failed |
| `CSPCL_ERR_BUNDLE_TOO_LARGE` | Bundle exceeds max size |
| `CSPCL_ERR_CSP_SEND` | CSP send failed |
| `CSPCL_ERR_CSP_RECV` | CSP receive failed |
| `CSPCL_ERR_TIMEOUT` | Operation timed out |
| `CSPCL_ERR_REASSEMBLY` | Reassembly failed |
| `CSPCL_ERR_VERSION_MISMATCH` | Protocol version mismatch |
| `CSPCL_ERR_NOT_INITIALIZED` | CSPCL not initialized |

Use `cspcl_strerror(err)` to get human-readable error messages.

## Integration with Real CSP

To use with actual libcsp instead of stubs:

1. Install libcsp v1.6 with ZMQHUB support:
   ```bash
   cd /path/to/libcsp
   python3 waf configure --enable-zmqhub
   python3 waf build
   ```

2. Set `CSP_PATH` environment variable or update CMake paths

3. Rebuild:
   ```bash
   export CSP_PATH=/path/to/libcsp
   cmake -DCSP_PATH=$CSP_PATH ..
   make
   ```

## References

- [uD3TN Documentation](https://gitlab.com/d3tn/ud3tn)
- [libcsp Documentation](https://github.com/libcsp/libcsp)
- [Bundle Protocol 7 (RFC 9171)](https://www.rfc-editor.org/rfc/rfc9171)
- [CubeSat Space Protocol](https://github.com/libcsp/libcsp/wiki)

## License

None for now
