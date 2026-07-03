# CSPCL - Rust Bindings

Safe Rust bindings for the CubeSat Space Protocol Convergence Layer (CSPCL), enabling Bundle Protocol 7 (BP7) bundles to be transmitted over CubeSat Space Protocol (CSP).

## Features

- **Type-safe API** - Safe Rust wrappers around C FFI
- **Bundle Send/Receive** - Simple interface for BP7 bundle handling
- **Automatic Cleanup** - Resource management via RAII (Drop trait)
- **Cross-platform** - POSIX (Linux) and FreeRTOS support

## Quick Start

Add to `Cargo.toml`:
```toml
[dependencies]
cspcl = "0.4"
futures = "0.3"
```

Basic usage:
```rust
use cspcl::{CspAddress, Cspcl, Interface};
use futures::StreamExt;
use std::sync::Arc;

// Initialize with the local CSP address and port.
let local = CspAddress { addr: 1, port: 10 };
let cspcl = Arc::new(Cspcl::new(local, Interface::Loopback)?);

// Send a bundle to a remote CSP address and port.
let bundle = vec![/* BP7 bundle data */];
let remote = CspAddress { addr: 2, port: 10 };
cspcl.send_bundle(&bundle, remote)?;

// Share the instance with an inbound bundle stream.
let mut inbound = Arc::clone(&cspcl).inbound();
while let Some(bundle) = inbound.next().await {
    let bundle = bundle?;
    println!(
        "Received {} bytes from CSP {}:{}",
        bundle.data.len(),
        bundle.src_addr,
        bundle.src_port
    );
}
```

## Documentation

See the crate-level Rust documentation for the current public API.

## License

MIT
