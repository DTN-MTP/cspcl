# picod3tn-cspcl

A cspcl link for [picod3tn](https://gitlab.com/d3tn/picod3tn): BP7 bundles over the CubeSat Space Protocol, interoperable with uD3TN's CSP convergence layer.

The crate works on serialized bundles. It owns the CSP link: peer addresses, configuration, sends that say whether a failure is worth retrying, and a receive stream that keeps going while the link is open. It depends on neither hardy nor `libpicod3tn`: the spool glue (`spool.transmit`, `forwarded()` / `defer()`, `FullBundle::parse`) lives in picod3tn, which would otherwise build two copies of `libpicod3tn`.

## Usage

```rust
use futures_util::StreamExt;
use picod3tn_cspcl::{Config, CspclAddr, CspclLink, FailureKind, Interface};

let link = CspclLink::new(&Config {
    local_addr: 1,
    port: 10,
    interface: Interface::Can(Some("can0".into())),
})?;

// Receive: one stream, opened at startup, for the life of the process.
let mut incoming = link.incoming();
tokio::spawn(async move {
    while let Some(received) = incoming.next().await {
        // received.data: the serialized bundle.
    }
});

// Send.
let peer: CspclAddr = "2:10".parse()?;
match link.send(bundle, peer).await {
    Ok(()) => { /* forwarded */ }
    Err(e) => match e.kind() {
        FailureKind::Retryable => { /* defer, retry later */ }
        FailureKind::Permanent => { /* never succeeds: do not retry every second */ }
        FailureKind::Cancelled => { /* shutting down */ }
    },
}
```

`examples/peer.rs` is a complete program.

### Addresses

A peer is `<addr>:<port>`, e.g. `2:10`, so a picod3tn CLA address is `cspcl:2:10`. The port is required. uD3TN writes the same peer `csp:2,10`: a comma instead of a colon, and a port that defaults to 10 when left out. The identifier `cspcl` is exported as `CLA_ID`.

- **Address:** 0 to 30 (CSP addresses are 5 bits, and 31 is broadcast).
- **Port:** 7 to 31 (0 to 6 are CSP's services, and cspcl binds up to 31).

Interop with uD3TN: its parser accepts more than this crate does.

| | picod3tn-cspcl | uD3TN |
|---|---|---|
| Peer | `cspcl:2:10` | `csp:2,10` |
| Port | 7 to 31, required | 0 to 31, defaults to 10 |
| Address | 0 to 30 | 0 to 255 (only 0 to 31 are CSP addresses) |

A uD3TN node listening on a port from 0 to 6 can't be reached from picod3tn: give uD3TN nodes a port from 7 to 31 (10, the default on both sides, works).

### Configuration

With the `cli` feature, `CspclArgs` is a set of `clap` arguments to `#[command(flatten)]` into picod3tn's:

| Flag | Environment | |
|---|---|---|
| `--cspcl-local-addr` | `PICOD3TN_CSPCL_LOCAL_ADDR` | This node's CSP address. Enables the CLA. |
| `--cspcl-port` | `PICOD3TN_CSPCL_PORT` | Port bundles are received on (default 10). |
| `--cspcl-interface` | `PICOD3TN_CSPCL_INTERFACE` | `loopback`, `can[:<iface>]` or `zmqhub[:<host>]`. Required with `--cspcl-local-addr`. |

A bare `can` uses `vcan0`, a **virtual** bus: on real hardware, name the interface (`can:can0`). The link logs the interface it uses when it opens, and warns when it is that default (not when `can:vcan0` is named). A bare `zmqhub` uses `localhost`.

## Features

- `std` (default): the standard library. Without it, the crate is `no_std` with `alloc`, and only has the pure part: addresses, configuration and failure classification.
- `tokio` (default): `CspclLink`, through the `cspcl` bindings, on a tokio runtime.
- `cli`: `CspclArgs`.

## Behaviour to know

- **One link per process.** libcsp is a process-wide singleton, and two cspcl instances in one process would split the inbound bundles between them. `CspclLink::new` succeeds once per process; share the link between the sending and receiving tasks.
- **Receiving starts when `incoming()` is first polled.** Until then no inbound connection is accepted. Bundles acknowledged while no stream is open wait for the next one, and are lost if the link is dropped: keep the link and one stream for the life of the process.
- **`shutdown()` blocks** its thread, for up to about 100 ms (plus a stream open in progress). From async code, call it through `spawn_blocking` unless the process is exiting anyway.
- **`incoming()` only ends after `shutdown()`.** If cspcl stops receiving otherwise, the stream reopens after a backoff (500 ms, doubling up to 30 s while it keeps failing quickly).
- **Bundles up to 65535 bytes** (`CSPCL_MAX_BUNDLE_SIZE`). A larger one is a `Permanent` failure, without reaching cspcl. A peer never acknowledges a larger inbound one either, so its sender fails the same way on every retry.
- **Failure classes.** `BundleTooLarge` and `InvalidParam` are permanent; `Cancelled` and `NotInitialized` mean shutdown; everything else (`Timeout`, `CspSend`, `Connection`, `PoolFull`, …) is retryable.
- **Recovery takes time.** The first send to a peer that restarted can fail or take 10 to 20 s on the stale connection before cspcl reconnects. If the peer is gone, sends fail with `Timeout`, then `CspSend` / `Connection`, until it is back. Allow at least 30 s for redelivery after a restart.
- **Sends are serialized** (in cspcl's C library). One to an unreachable peer holds every other back for up to about 20 s. With picod3tn's single TX loop, an unreachable peer delays every other peer's bundles.
- **The source port of a received bundle is ephemeral.** `Received::src_port` is the sender's temporary port, not the one it listens on: never reply to it, nor look the peer up by it.

## Tests

As for the other crates, `CSP_REPO_DIR` must point at libcsp v1.6 built with `-fPIC` (see the [Rust API docs](../../docs/api/rust.md#installation)).

```bash
cargo test -p picod3tn-cspcl --features cli
cargo build -p picod3tn-cspcl --no-default-features   # the no_std part
```

Each integration test is its own binary, because libcsp is a process-wide singleton.

### Manual test: two processes over vcan0

```bash
sudo modprobe vcan
sudo ip link add dev vcan0 type vcan && sudo ip link set up vcan0
cargo build -p picod3tn-cspcl --features cli --example peer
P=target/debug/examples/peer

# Two terminals:
$P --cspcl-local-addr 1 --cspcl-interface can:vcan0 --to 2:10
$P --cspcl-local-addr 2 --cspcl-interface can:vcan0 --to 1:10
```

Each prints the bundles it sends and receives, once a second. Stop one and restart it: the other's pending send completes or fails as retryable, and later sends go through again.
