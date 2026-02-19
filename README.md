# CSPCL - CubeSat Space Protocol Convergence Layer

A convergence layer adapter that enables Bundle Protocol 7 (BP7) bundles to be transmitted over CubeSat Space Protocol (CSP).

## Overview

CSPCL provides a bridge between BP7 (Delay/Disruption Tolerant Networking) and CSP (commonly used in CubeSat missions). It follows the architecture:

```
BP7 Bundle → CSPCL → CSP SFP → Physical Layer (CAN/ZMQHUB/SocketCAN)
```

## Features

- **CSP v1.6 Compatible** - Works with libcsp v1.6 (not v2)
- **SFP Fragmentation** - Uses CSP's built-in Simple Fragmentation Protocol
- **Connection-Based** - Uses CSP connections for reliable bundle transfer
- **Address Translation** - IPN/DTN endpoint ID to CSP address mapping
- **Cross-Platform** - Supports POSIX (Linux) and FreeRTOS
- **Minimal Footprint** - ~300 lines of C code

## Requirements

| Requirement | Description |
|-------------|-------------|
| CSP Version | v1.6 (not v2 - different packet format) |
| CSP Mode | Connection-based with SFP |
| Platform | POSIX (Linux) or FreeRTOS |
| Interfaces | ZMQHUB, SocketCAN (ground), CAN (space) |

## Quick Start with uD3TN

The primary use case for CSPCL is integration with [uD3TN](https://gitlab.com/d3tn/ud3tn). See [ud3tn-integration/README.md](ud3tn-integration/README.md) for complete instructions.

### Quick Test

see [ud3tn-integration/README.md](ud3tn-integration/README.md)

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
   python3 waf configure --enable-can-socketcan --enable-if-zmqhub
   python3 waf build install
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

## Rust Bindings

CSPCL provides safe Rust bindings through the `cspcl` and `cspcl-sys` crates.

### Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
cspcl = "0.1"
```

### Quick Start

```rust
use cspcl::Cspcl;

// Initialize with local CSP address
let mut cspcl = Cspcl::init(1)?;

// Open RX socket
cspcl.open_rx_socket()?;

// Send a bundle
let bundle = vec![/* BP7 bundle bytes */];
cspcl.send_bundle(&bundle, 2)?;  // Send to CSP address 2

// Receive a bundle
let (data, src_addr) = cspcl.recv_bundle(5000)?;  // 5 second timeout
println!("Received {} bytes from CSP addr {}", data.len(), src_addr);

// Cleanup is automatic on drop
```

### Key Features

- **Type-safe wrappers** over C FFI
- **Error handling** via Rust Result/Error types
- **Automatic cleanup** via RAII (Drop trait)
- **Address translation** utilities (IPN↔CSP)
- **Platform support** for POSIX and FreeRTOS

### Error Handling

```rust
use cspcl::{Cspcl, CspclerError};

match cspcl.send_bundle(&data, addr) {
    Ok(_) => println!("Bundle sent"),
    Err(CspclerError::BundleTooLarge) => eprintln!("Bundle too large"),
    Err(e) => eprintln!("Error: {}", e),
}
```

### Building with libcsp

For production use with actual libcsp instead of stubs:

```bash
export CSP_PATH=/path/to/libcsp
cargo build --release
```

See [rust-bindings/README.md](rust-bindings/README.md) for detailed Rust API documentation.

## License

- University of Montpellier Space Center
