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
cspcl = "0.4"
futures = "0.3"
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

### `Cspcl::new`

```rust
pub fn new(local: CspAddress, interface: Interface) -> Result<Self>
```

Initialize CSPCL with the local CSP node address, CSP port, and interface.

### `send_bundle`

```rust
pub fn send_bundle(&self, bundle: &[u8], dest: CspAddress) -> Result<()>
```

Send a serialized BP7 bundle to `dest`. Fragmentation via SFP is handled internally.

### `inbound`

```rust
pub fn inbound(self: Arc<Self>) -> InboundStream
```

Share the handle with a stream of inbound bundles.

### `close_rx_socket`

```rust
pub fn close_rx_socket(&self)
```

Cancel inbound receive work and close the receive socket.

---

## Public Types

```rust
pub struct CspAddress {
    pub addr: u8,
    pub port: u8,
}

pub struct Bundle {
    pub data: Vec<u8>,
    pub src_addr: u8,
    pub src_port: u8,
}

pub enum Interface {
    Zmq(String),
    Can(String),
    Loopback,
}
```

`CspAddress` also converts to and from two-byte `bytes::Bytes` values in
`[addr, port]` order.

### `InboundStream`

```rust
pub struct InboundStream { /* ... */ }
```

Implements `futures::Stream<Item = Result<Bundle>>`.

---

## `Error`

```rust
pub enum Error {
    InvalidParam,
    NoMemory,
    BundleTooLarge,
    CspSend,
    CspRecv,
    Timeout,
    Sfp,
    NotInitialized,
    Connection,
    CspInit,
    CspStackInit,
    CspZmqhubInit,
    CspCanInit,
    CspCanNotSupported,
    CspRouter,
    PoolFull,
    UnknownError(cspcl_sys::cspcl_error_t),
    ParseAddress,
}
```

Implements `std::error::Error` and `std::fmt::Display`.

---

## Example

```rust
use cspcl::{CspAddress, Cspcl, Error, Interface};
use futures::StreamExt;
use std::sync::Arc;

fn transfer_bundle(bundle: &[u8], dest: CspAddress) -> Result<(), Error> {
    let local = CspAddress { addr: 1, port: 10 };
    let cspcl = Cspcl::new(local, Interface::Loopback)?;
    cspcl.send_bundle(bundle, dest)?;
    Ok(())
}

async fn receive_loop() -> Result<(), Error> {
    let local = CspAddress { addr: 1, port: 10 };
    let cspcl = Arc::new(Cspcl::new(local, Interface::Loopback)?);
    let mut inbound = Arc::clone(&cspcl).inbound();

    while let Some(bundle) = inbound.next().await {
        let bundle = bundle?;
        println!(
            "Bundle from {}:{}: {} bytes",
            bundle.src_addr,
            bundle.src_port,
            bundle.data.len()
        );
    }

    Ok(())
}
```
