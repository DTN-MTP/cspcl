---
layout: default
title: Rust API Reference
nav_order: 2
parent: API Reference
permalink: /api/rust/
---

# Rust API Reference

The `cspcl` crate provides safe, idiomatic Rust bindings over the C library.

## Installation

```toml
[dependencies]
cspcl = "0.1"
```

For production use with real libcsp set the `CSP_PATH` environment variable before building:

```bash
export CSP_PATH=/path/to/libcsp
cargo build --release
```

---

## `Cspcl` — Main Handle

```rust
pub struct Cspcl { /* ... */ }
```

Owns the underlying `cspcl_t` instance. Resources are released automatically when
the value is dropped.

### `Cspcl::init`

```rust
pub fn init(local_addr: u8) -> Result<Self, CspclerError>
```

Initialize CSPCL with the given local CSP node address.

### `open_rx_socket`

```rust
pub fn open_rx_socket(&mut self) -> Result<(), CspclerError>
```

Bind the receive socket to the Bundle Protocol port. Call once before `recv_bundle`.

### `send_bundle`

```rust
pub fn send_bundle(&self, bundle: &[u8], dest_addr: u8) -> Result<(), CspclerError>
```

Send a serialized BP7 bundle to `dest_addr`. Fragmentation via SFP is handled internally.

### `recv_bundle`

```rust
pub fn recv_bundle(&self, timeout_ms: u32) -> Result<(Vec<u8>, u8), CspclerError>
```

Receive a complete bundle. Returns `(bundle_bytes, src_addr)`. Blocks until data
arrives or `timeout_ms` elapses.

---

## Address Translation

```rust
// IPN / DTN endpoint to CSP address
pub fn endpoint_to_addr(endpoint_id: &str) -> u8

// CSP address to IPN endpoint string ("ipn:X.0")
pub fn addr_to_endpoint(addr: u8) -> Result<String, CspclerError>
```

---

## `CspclerError`

```rust
pub enum CspclerError {
    InvalidParam,
    NoMemory,
    BundleTooLarge,
    CspSend,
    CspRecv,
    Timeout,
    Sfp,
    NotInitialized,
    Connection,
    Unknown(i32),
}
```

Implements `std::error::Error` and `std::fmt::Display`.

---

## Example

```rust
use cspcl::{Cspcl, CspclerError};

fn transfer_bundle(bundle: &[u8], dest: u8) -> Result<(), CspclerError> {
    let cspcl = Cspcl::init(1)?;
    cspcl.send_bundle(bundle, dest)?;
    Ok(())
}

fn receive_loop() -> Result<(), CspclerError> {
    let mut cspcl = Cspcl::init(1)?;
    cspcl.open_rx_socket()?;

    loop {
        match cspcl.recv_bundle(10_000) {
            Ok((data, src)) => println!("Bundle from {}: {} bytes", src, data.len()),
            Err(CspclerError::Timeout) => continue,
            Err(e) => return Err(e),
        }
    }
}
```
