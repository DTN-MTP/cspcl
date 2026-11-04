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
cspcl = "0.5"
futures = "0.3"
```

`cspcl-sys` compiles the C library and links it against libcsp v1.6. Point the
build at libcsp with either:

- `CSP_REPO_DIR`: a libcsp checkout with `include/` and a `build/` directory
  containing `libcsp.a` or `libcsp.so` (`CSP_BUILD_DIR` overrides `build/`), or
- `CSP_INCLUDE_DIR`: the libcsp headers.

```bash
export CSP_REPO_DIR=/path/to/libcsp
cargo build --release
```

Rust links position-independent executables, so a static `libcsp.a` must be
built with `-fPIC`. libcsp v1.6 only applies its default flags when `CFLAGS`
is empty, so pass them all:

```bash
cd /path/to/libcsp   # checked out at v1.6
CFLAGS="-std=gnu99 -g -Os -fPIC -Wall -Wextra -Wshadow -Wcast-align -Wwrite-strings -Wno-unused-parameter" \
  python3.11 waf configure --enable-can-socketcan --enable-if-zmqhub --enable-rdp --with-os=posix
python3.11 waf build
```

The build also links `libzmq`, `libsocketcan` and `libbz2`, and bindgen needs
`libclang`. Building `hardy-cspcl-server` additionally requires `protoc` and
the protobuf well-known types (`protobuf-devel` / `libprotobuf-dev`).

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

This call blocks until the peer acknowledges the bundle (see
[Delivery Acknowledgement](../../delivery-ack/)), which takes up to twice
`CSPCL_ACK_TIMEOUT_MS` when the peer is unreachable. Use `send_bundle_async`
from async code.

### `send_bundle_async`

```rust
pub async fn send_bundle_async(
    self: &Arc<Self>,
    bundle: impl Into<Vec<u8>>,
    dest: CspAddress,
) -> Result<()>
```

Same as `send_bundle`, but runs on tokio's blocking thread pool so it does not
stall the async runtime. Requires a tokio runtime.

Dropping the future (for example on a timeout) skips the send if it has not
started yet. A send that has started cannot be interrupted, so the bundle may
still be delivered: treat a cancelled send as having an unknown outcome.

Sending never blocks receiving: inbound connections are accepted and
acknowledged while a send is waiting for its ack. Sends themselves are
serialized, because the C connection pool stays locked for the whole
send/ack cycle.

### `inbound`

```rust
pub fn inbound(self: Arc<Self>) -> InboundStream
```

Share the handle with a stream of inbound bundles. Does not need a tokio
runtime: receiving runs on its own threads while at least one stream is open.
Bundles go to the most recently opened stream.

The full behaviour (what happens to bundles when streams close, connection
limits, reset and idle detection, errors) is specified once, in the rustdoc
of `Cspcl::inbound` (`cargo doc -p cspcl --open`).

### `close_rx_socket`

```rust
pub fn close_rx_socket(&self)
```

End all open inbound streams, as if they were all dropped (they yield `None`
once read), and stop receiving; see the rustdoc of `Cspcl::inbound` for what
happens to reads and connections then. Bundles acknowledged meanwhile go to
the next stream opened with `inbound`, which receives again; they are lost if
the `Cspcl` is dropped first.

The CSP port stays bound for the life of the process (libcsp 1.6 has no safe
unbind): peers can still connect and send, and their sends time out instead
of being refused. The next stream opened then receives those bundles, which
their senders may send again.

`close_rx_socket` blocks while an accept poll is in progress, and `inbound`
waits for a close in progress and, while no accept thread runs, starts one; the
rustdoc of `Cspcl::close_rx_socket` states for how long. From async code,
call them with `tokio::task::spawn_blocking` where such a pause matters.

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
    Cancelled,
    Spawn,
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
        // An error does not end the stream: dropping it would stop receiving.
        let bundle = match bundle {
            Ok(bundle) => bundle,
            Err(e) => {
                eprintln!("Receive error: {e}");
                continue;
            }
        };
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

---

## Other crates

- `hardy-cspcl`: a cspcl convergence layer for the [hardy](https://github.com/ricktaylor/hardy) BPA, served by `hardy-cspcl-server`.
- [`picod3tn-cspcl`](https://github.com/DTN-MTP/cspcl/tree/main/rust-bindings/picod3tn-cspcl): the cspcl link for [picod3tn](https://gitlab.com/d3tn/picod3tn), on serialized bundles. Its README covers its configuration and the behaviour that matters to a BPA: one link per process, bundles up to 65535 bytes, serialized sends, recovery times after a peer restart, and ephemeral source ports.
