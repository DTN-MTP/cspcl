---
layout: default
title: Getting Started
nav_order: 2
permalink: /getting-started/
---

# Getting Started

## Prerequisites

- CMake 3.10+
- C11-compatible compiler (gcc / clang)
- libcsp v1.6
- *(Optional)* Python 3 + `pyzmq` for ZMQHUB-based testing
- *(Optional)* Rust toolchain for Rust bindings

## Building

```bash
git clone https://github.com/dtn-mtp/cspcl.git
cd cspcl
mkdir build && cd build
cmake ..
make
ctest --verbose
```

### CMake Options

| Option | Default | Description |
| --- | --- | --- |
| `CSPCL_BUILD_TESTS` | `ON` | Build unit tests |
| `CSPCL_BUILD_EXAMPLES` | `ON` | Build example applications |
| `CSPCL_USE_FREERTOS` | `OFF` | Target FreeRTOS instead of POSIX |
| `CSPCL_DEBUG` | `OFF` | Enable verbose debug output |

### Using a custom libcsp location

```bash
cmake -DCSP_PATH=/path/to/libcsp ..
```

---

## C — First Bundle Transfer

### 1. Initialize

```c
#include "cspcl.h"

cspcl_t cspcl;

// Initialize with local CSP node address
cspcl_error_t err = cspcl_init(&cspcl, 1);

// Open receive socket (bind to BP port once)
err = cspcl_open_rx_socket(&cspcl);
```

### 2. Send a bundle

```c
uint8_t bundle[] = { /* serialized BP7 bundle */ };

err = cspcl_send_bundle(&cspcl, bundle, sizeof(bundle), 2 /* dest CSP addr */);
if (err != CSPCL_OK) {
    fprintf(stderr, "send error: %s\n", cspcl_strerror(err));
}
```

Large bundles are automatically fragmented using CSP's SFP — no extra work required.

### 3. Receive a bundle

```c
uint8_t buf[CSPCL_MAX_BUNDLE_SIZE];
size_t  buf_len = sizeof(buf);
uint8_t src_addr;

err = cspcl_recv_bundle(&cspcl, buf, &buf_len, &src_addr, 5000 /* ms */);
if (err == CSPCL_OK) {
    printf("Received %zu bytes from CSP addr %u\n", buf_len, src_addr);
}
```

### 4. Cleanup

```c
cspcl_cleanup(&cspcl);
```

---

## Rust — First Bundle Transfer

Add the dependency:

```toml
[dependencies]
cspcl = "0.4"
futures = "0.3"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
use cspcl::{CspAddress, Cspcl, Interface};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let local = CspAddress { addr: 1, port: 10 };
    let mut node = Cspcl::new(local, Interface::Loopback)?;

    // Send
    let bundle: Vec<u8> = vec![/* BP7 bundle bytes */];
    let remote = CspAddress { addr: 2, port: 10 };
    node.send_bundle(&bundle, remote)?;

    // Receive from the inbound bundle stream.
    let mut inbound = node.inbound();
    while let Some(bundle) = inbound.next().await {
        let bundle = bundle?;
        println!(
            "Received {} bytes from CSP {}:{}",
            bundle.data.len(),
            bundle.src_addr,
            bundle.src_port
        );
    }

    Ok(())
}
```

---

## Next Steps

- [C API Reference]({% link api/c.md %}) — complete function documentation
- [uD3TN Integration]({% link integration/ud3tn.md %}) — use CSPCL with uD3TN
- [Unibo Integration]({% link integration/unibo.md %}) — use CSPCL with Unibo-BP
- [Architecture]({% link architecture.md %}) — understand how SFP fragmentation works
