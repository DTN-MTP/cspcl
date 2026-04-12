---
layout: default
title: Rust API Reference
nav_order: 2
parent: API Reference
permalink: /api/rust/
---

# Rust API Reference

The `cspcl` crate provides minimal safe Rust bindings over the C library.
The sync API remains the source of truth. Optional Tokio wrappers live in
`cspcl::async_api` behind the `async-tokio` Cargo feature.

## Installation

```toml
[dependencies]
cspcl = "0.1"
```

Enable the optional async wrappers with:

```toml
[dependencies]
cspcl = { version = "0.1", features = ["async-tokio"] }
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

### `Cspcl::shutdown`

```rust
pub fn shutdown(&self) -> Result<(), Error>
```

Explicitly tear down the native runtime. After shutdown, send and receive calls
return `CSPCL_ERR_NOT_INITIALIZED`.

### `Cspcl::connection_stats`

```rust
pub fn connection_stats(&self) -> ConnectionStats
```

Read the native outbound connection-pool counters.

### `Sender`

```rust
pub struct Sender { /* ... */ }
```

```rust
pub fn send_bundle(&self, bundle: &[u8], dest_addr: u8, dest_port: u8) -> Result<(), Error>
pub fn connection_stats(&self) -> ConnectionStats
```

Send a serialized BP7 bundle to `dest_addr:dest_port`. Fragmentation via SFP is handled internally.

### `Receiver`

```rust
pub struct Receiver { /* ... */ }
```

```rust
pub fn recv_bundle(&self, timeout_ms: u32) -> Result<ReceivedBundle, Error>
pub fn recv_bundle_into(&self, buffer: &mut [u8], timeout_ms: u32) -> Result<ReceivedBundleView, Error>
```

Receive a complete bundle. `ReceivedBundle` contains the payload and source metadata.
`ReceivedBundleView` reports the received length and source metadata for a caller-provided buffer.

---

## `async_api` — Optional Tokio Wrappers

Available behind the `async-tokio` Cargo feature.

### `AsyncCspcl`

```rust
pub struct AsyncCspcl { /* ... */ }
```

```rust
pub fn from_sync(cspcl: Cspcl) -> Self
pub fn sender(&self) -> AsyncSender
pub fn receiver(&self) -> AsyncReceiver
pub fn split(&self) -> (AsyncSender, AsyncReceiver)
pub fn is_initialized(&self) -> bool
pub fn local_addr(&self) -> u8
pub fn connection_stats(&self) -> ConnectionStats
pub async fn shutdown(&self) -> Result<(), Error>
```

Wraps the existing sync runtime and delegates blocking operations through Tokio's
`spawn_blocking`. It does not introduce a second transport implementation.

### `AsyncSender`

```rust
pub struct AsyncSender { /* ... */ }
```

```rust
pub fn from_sync(sender: Sender) -> Self
pub fn connection_stats(&self) -> ConnectionStats
pub async fn send_bundle(&self, bundle: &[u8], dest_addr: u8, dest_port: u8) -> Result<(), Error>
```

### `AsyncReceiver`

```rust
pub struct AsyncReceiver { /* ... */ }
```

```rust
pub fn from_sync(receiver: Receiver) -> Self
pub async fn recv_bundle(&self, timeout_ms: u32) -> Result<ReceivedBundle, Error>
pub async fn recv_bundle_into(&self, buffer: &mut [u8], timeout_ms: u32) -> Result<ReceivedBundleView, Error>
```

These wrappers preserve the sync API's validation and error mapping, including
timeout and post-shutdown behavior.

### `RemotePeer`

```rust
pub struct RemotePeer {
    pub addr: u8,
    pub port: u8,
}
```

### Async Example

```rust
use cspcl::async_api::AsyncCspcl;
use cspcl::{Cspcl, CspclConfig, Error, Interface, InterfaceName};

async fn transfer_bundle(bundle: &[u8], dest: u8) -> Result<(), Error> {
    let cspcl = Cspcl::from_config(
        CspclConfig::new(1)
            .with_interface(Interface::Loopback(InterfaceName::new("loopback"))),
    )?;
    let async_cspcl = AsyncCspcl::from_sync(cspcl);
    let (sender, receiver) = async_cspcl.split();

    sender.send_bundle(bundle, dest, 10).await?;

    let mut buffer = [0_u8; 1024];
    let received = receiver.recv_bundle_into(&mut buffer, 10_000).await?;

    println!(
        "Bundle from {}:{}: {} bytes",
        received.src_addr,
        received.src_port,
        received.len
    );
    println!("pool hits={}", async_cspcl.connection_stats().hits);
    async_cspcl.shutdown().await?;
    Ok(())
}
```

```rust
pub fn new(addr: u8, port: u8) -> Self
pub fn from_endpoint(endpoint: &str, port: u8) -> Option<Self>
pub fn endpoint(&self) -> Result<String, Error>
```

Minimal transport-native identity helper for Hardy-side peer tracking.

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
    println!("pool hits={}", cspcl.connection_stats().hits);
    cspcl.shutdown()?;
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

Run the optional Tokio async tests with:

```bash
cd rust-bindings
export CSP_REPO_DIR=/path/to/libcsp
cargo test -p cspcl --features async-tokio
```

Coverage is reported with `cargo-llvm-cov`:

```bash
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov
cd rust-bindings
export CSP_REPO_DIR=/path/to/libcsp
cargo llvm-cov -p cspcl --summary-only
```

To include the async wrappers:

```bash
cargo llvm-cov -p cspcl --features async-tokio --summary-only
```

Optional outputs:

```bash
cargo llvm-cov -p cspcl --html
cargo llvm-cov -p cspcl --lcov --output-path lcov.info
```
