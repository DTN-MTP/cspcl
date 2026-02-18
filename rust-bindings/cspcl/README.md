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
cspcl = "0.1"
```

Basic usage:
```rust
use cspcl::Cspcl;

// Initialize with local CSP address
let mut cspcl = Cspcl::init(1)?;
cspcl.open_rx_socket()?;

// Send bundle to CSP address 2
let bundle = vec![/* BP7 bundle data */];
cspcl.send_bundle(&bundle, 2)?;

// Receive bundle (5 second timeout)
let (data, src_addr) = cspcl.recv_bundle(5000)?;
println!("Received {} bytes from CSP addr {}", data.len(), src_addr);
```

## Documentation

See [main repository README](../../README.md) for complete documentation and examples.

## License

MIT
