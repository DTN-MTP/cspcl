---
layout: default
title: Rust API Reference
nav_order: 2
parent: API Reference
permalink: /api/rust/
---

# Rust API Reference

The `cspcl` crate provides minimal safe Rust bindings over the C library.

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

## `Cspcl` — Bootstrap Handle

```rust
pub struct Cspcl { /* ... */ }
```

Owns the underlying `cspcl_t` instance. It can be split into dedicated outbound
and inbound handles without exposing the raw C struct.

### `CspclConfig`

```rust
pub struct CspclConfig { /* ... */ }
```

```rust
pub fn new(local_addr: u8) -> Self
pub fn with_port(self, local_port: u8) -> Self
pub fn with_interface(self, interface: Interface) -> Self
```

### `Cspcl::from_config`

```rust
pub fn from_config(config: CspclConfig) -> Result<Cspcl, Error>
```

Initialize CSPCL with explicit port/interface settings.

### `Cspcl::split`

```rust
pub fn split(&self) -> (Sender, Receiver)
```

Create dedicated sender and receiver handles that share the same native instance.

### `Sender`

```rust
pub struct Sender { /* ... */ }
```

```rust
pub fn send_bundle(&self, bundle: &[u8], dest_addr: u8, dest_port: u8) -> Result<(), Error>
```

Send a serialized BP7 bundle to `dest_addr:dest_port`. Fragmentation via SFP is handled internally.

### `Receiver`

```rust
pub struct Receiver { /* ... */ }
```

```rust
pub fn recv_bundle(&self, timeout_ms: u32) -> Result<ReceivedBundle, Error>
```

Receive a complete bundle. `ReceivedBundle` contains the payload and source metadata.

---

## Address Translation

```rust
// IPN / DTN endpoint to CSP address
pub fn endpoint_to_addr(endpoint_id: &str) -> u8

// CSP address to IPN endpoint string ("ipn:X.0")
pub fn addr_to_endpoint(addr: u8) -> Result<String, CspclerError>
```

---

## `Error`

```rust
pub struct Error(/* raw cspcl_error_t */);
```

Implements `std::error::Error` and `std::fmt::Display`.

---

## Example

```rust
use cspcl::{Cspcl, CspclConfig, Error, Interface, InterfaceName};

fn transfer_bundle(bundle: &[u8], dest: u8) -> Result<(), Error> {
    let cspcl = Cspcl::from_config(
        CspclConfig::new(1)
            .with_interface(Interface::Loopback(InterfaceName::new("loopback"))),
    )?;
    let (sender, receiver) = cspcl.split();

    sender.send_bundle(bundle, dest, 10)?;
    let received = receiver.recv_bundle(10_000)?;

    println!(
        "Bundle from {}:{}: {} bytes",
        received.src_addr,
        received.src_port,
        received.data.len()
    );
    Ok(())
}
```

## Testing And Coverage

The Rust tests target the safe crate against a built `libcsp` v1.6 checkout.
Point `CSP_REPO_DIR` at that checkout before running the commands below.

```bash
cd rust-bindings
export CSP_REPO_DIR=/path/to/libcsp
cargo test -p cspcl
```

Coverage is reported with `cargo-llvm-cov`:

```bash
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov
cd rust-bindings
export CSP_REPO_DIR=/path/to/libcsp
cargo llvm-cov -p cspcl --summary-only
```

Optional outputs:

```bash
cargo llvm-cov -p cspcl --html
cargo llvm-cov -p cspcl --lcov --output-path lcov.info
```
