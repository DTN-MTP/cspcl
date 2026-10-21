# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- CI: a `rust-picod3tn` job runs `cargo fmt`, `cargo clippy -D warnings` and the tests of the `rust-bindings` workspace against libcsp v1.6 built with `-fPIC`, builds `picod3tn-cspcl` without default features, and checks that the picoD3TN patch applies to `38fd7a9`. CI also runs on pushes to any `feat/picod3tn-*` branch.
- Docker: `docker/picod3tn/Dockerfile` builds picoD3TN `38fd7a9` with the cspcl CLA patch, against this checkout's `picod3tn-cspcl` and a libcsp v1.6 built with `-fPIC`. The image ships `picod3tn` and `aapctl`; its entrypoint maps `CSP_ADDR`, `CSP_PORT`, `TRANSPORT` and `ZMQ_BROKER_HOST` to the `PICOD3TN_CSPCL_*` variables. CI builds and pushes it as `cspcl-picod3tn` and lints it with hadolint.
- Interop tests: picoD3TN nodes E (CSP 5) and F (CSP 6) in both compose stacks. `test-picod3tn-basic.sh` exchanges bundles between them in both directions, and `test-picod3tn-cross.sh` between picoD3TN and uD3TN, and picoD3TN and Unibo-BP, in both directions. `run-tests.sh --test picod3tn` runs both, and `all` includes them.
- picoD3TN integration: `picod3tn-integration/picod3tn-cla-cspcl.patch` adds the cspcl CLA to picoD3TN `38fd7a9`, behind an optional `cspcl` feature (`--cspcl-*` arguments, `cspcl:<addr>:<port>` FIB entries, a libcsp v1.6 Nix package, a NixOS module option and a VM test over vcan0). `picod3tn-integration/README.md` explains how to apply, build, revert and regenerate it.
- Rust: new `picod3tn-cspcl` crate (0.1.0), the cspcl link for picod3tn. It works on serialized bundles: `CspclAddr` (`<addr>:<port>`, e.g. `cspcl:2:10` in picod3tn), `Config` and, with the `cli` feature, `CspclArgs` (`--cspcl-local-addr`, `--cspcl-port`, `--cspcl-interface` for loopback, CAN and ZMQ hub, with `PICOD3TN_CSPCL_*` environment variables; a bare `can` uses the virtual bus `vcan0` and the link warns about it, while a named `can:vcan0` does not warn), `CLA_ID` (`"cspcl"`, the identifier in picod3tn's CLA addresses and spool), `CspclLink::send`, whose `SendError` says whether a failure is retryable, permanent (a bundle over 65535 bytes, without reaching cspcl) or a shutdown, and `CspclLink::incoming`, which reopens cspcl's inbound stream with a backoff until `shutdown`. One link opens per process. Without default features, the address, config and classification part builds `no_std`. The picod3tn spool glue is left to picod3tn.
- Rust: `Cspcl::send_bundle_async` (`cspcl` 0.5.0), which sends without blocking the tokio runtime. `hardy-cspcl` now uses it.
- Rust: `cspcl-sys` 0.4.0 exposes raw-pointer `send_bundle_raw`, `accept_conn_raw` and `close_rx_socket_raw`, and `recv_bundle_from_conn_into`, which receives into a reusable buffer, `types::rdp_conn_timeout_ms`, and the raw `csp_rdp_get_opt` binding.

### Changed
- Rust (breaking, `cspcl` 0.5.0): with several `InboundStream`s open, each bundle now goes only to the most recently opened one. Inbound bundles are received by one receiver per `Cspcl` (an accept thread and one thread per connection, on plain threads) shared by all its streams, instead of tasks per stream, so receiving no longer needs a tokio runtime nor occupies tokio's blocking pool. The full behaviour is documented on `Cspcl::inbound`.
- Rust: `hardy-cspcl` reopens its inbound stream when it ends without the CLA being unregistered, instead of no longer dispatching bundles. The stream ends that way only if cspcl's accept thread could not start or failed; reopening backs off from 500 ms to 30 s while it keeps failing quickly, and peers whose connection was closed meanwhile reconnect on their next send. Streams are opened on a thread of their own, as opening may wait for a close of the receive socket; unregistering closes it inline, blocking for an open in progress and at most one accept poll.
- Rust (breaking, `cspcl` 0.5.0): new `Error::Spawn` variant, yielded by an `InboundStream` when a receive thread cannot be started: the stream then ends if it was the accept thread (as do streams opened while it was being started), and stays open if it was the thread of one connection (which is closed).
- Rust (breaking, `cspcl` 0.5.0): new `Error::Cancelled` variant. A send whose blocking task was cancelled (runtime shutdown, or a dropped future) now returns it instead of `Error::CspSend`, so it is no longer mistaken for a retryable transport failure.
- Rust (breaking, `cspcl-sys` 0.4.0): `AcceptedConn` now owns its connection. It is no longer `Copy`, closes the connection on drop, and `recv_bundle_from_conn` takes `&AcceptedConn`. Use `as_ptr()` / `into_raw()` to get the raw `csp_conn_t*`. This makes it impossible to read a closed connection from safe code.

### Removed
- Rust (breaking, `cspcl-sys` 0.4.0): `types::conn_pool_add_accepted`. It put inbound connections into the outbound pool under the peer's ephemeral port, where sends never reused them. The raw `unsafe fn conn_pool_add` remains.

### Fixed
- Docker: every node of the CAN stack now uses CSP port 10. The Unibo-BP daemon sends to its own port and the interop routes use port 10, so with ports 9 to 12 only uD3TN node B was reachable, and the Unibo-BP and cross-integration tests failed whenever the runner could load vcan.
- Rust: a node could not accept or acknowledge inbound bundles while one of its own sends was waiting for an ack. Simultaneous sends between two nodes, and sends to self, timed out. Sends and accepts no longer share a lock.
- Rust: accepted inbound connections are no longer added to the outbound connection pool. The pool never reused them and could close one while it was still being read. The inbound listener now owns and closes them.
- Rust: dead inbound connections are released. Before, they were held forever and could exhaust libcsp's connection table. Connections reset by their peer are closed when libcsp wakes the read (assuming both nodes use the same RDP connection timeout), idle ones after 10 min, and at most 8 stay open per `Cspcl` (up to 10 while evicted ones finish their read), least recently active closed first; a connection reopened from the same address and port replaces the old one. Connections are tracked by address and port, so agents sharing a CSP address do not close each other's connections.
- Rust: a `send_bundle_async` future dropped before its send started, including while queued behind a slow send, no longer sends the bundle. A send already in progress still runs to completion; this is documented.
- Rust: dropping an `InboundStream` no longer stops every stream created later on the same `Cspcl`, and a stream opened after `close_rx_socket` receives again. `close_rx_socket` now ends the open streams (they yield `None`), as if they were dropped. It may block for one accept poll (`CSPCL_RX_ACCEPT_POLL_MS`, 100 ms by default, up from 10 ms).
- Rust: bundles acknowledged to their sender are no longer dropped with their `InboundStream`: those left unread, and those whose read was in progress when receiving stopped, go to the next stream opened on the same `Cspcl` (they are lost if the `Cspcl` is dropped first, e.g. after a `hardy-cspcl` CLA is unregistered).
- Rust: inbound listeners keep the `Cspcl` instance alive until they have closed their connection, so the instance is never cleaned up under an open connection.

### Documentation
- picoD3TN: new `docs/integration/picod3tn.md` (architecture, prerequisites, applying the patch, CLA configuration, FIB routes, address formats across picoD3TN, uD3TN and Unibo-BP, a two-node walkthrough, limitations), and a picoD3TN row in the README's documentation table. The Docker and interop READMEs cover the picoD3TN image, nodes and tests, `send-message.sh` gets `picod3tn` and `picod3tn-cross` scenarios, and `docker-compose.ghcr.yml` the two picoD3TN nodes. `docs/architecture.md`'s file layout now lists every crate and integration.
- Rust API: document the real libcsp build variables (`CSP_REPO_DIR`, `CSP_INCLUDE_DIR`, `CSP_BUILD_DIR`), replacing the non-existent `CSP_PATH`, and the `-fPIC` requirement.

## [1.0.0] - 2025-02-28

### Added
- Initial release of CSPCL — CubeSat Space Protocol Convergence Layer.
- BP7 bundle transmission over CSP using SFP (Simple Fragmentation Protocol).
- Support for ZMQHUB and SocketCAN physical interfaces.
- Connection pool for managing CSP connections.
- POSIX and FreeRTOS build targets.
- CMake build system with optional system libcsp (v1.6) or built-in stubs.
- Rust FFI bindings (`cspcl-sys`) and safe Rust wrapper (`cspcl`).
- uD3TN integration patch.
- UniBo (µPCN) integration.
- Full documentation site at [dtn-mtp.github.io/cspcl](https://dtn-mtp.github.io/cspcl).

[Unreleased]: https://github.com/DTN-MTP/cspcl/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/DTN-MTP/cspcl/releases/tag/v1.0.0
