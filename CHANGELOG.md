# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Rust: `Cspcl::send_bundle_async` (`cspcl` 0.5.0), which sends without blocking the tokio runtime. `hardy-cspcl` now uses it.
- Rust: `cspcl-sys` 0.4.0 exposes raw-pointer `send_bundle_raw`, `accept_conn_raw` and `close_rx_socket_raw`.

### Changed
- Rust (breaking, `cspcl-sys` 0.4.0): `AcceptedConn` now owns its connection. It is no longer `Copy`, closes the connection on drop, and `recv_bundle_from_conn` takes `&AcceptedConn`. Use `as_ptr()` / `into_raw()` to get the raw `csp_conn_t*`. This makes it impossible to read a closed connection from safe code.

### Removed
- Rust (breaking, `cspcl-sys` 0.4.0): `types::conn_pool_add_accepted`. It put inbound connections into the outbound pool under the peer's ephemeral port, where sends never reused them. The raw `unsafe fn conn_pool_add` remains.

### Fixed
- Rust: a node could not accept or acknowledge inbound bundles while one of its own sends was waiting for an ack. Simultaneous sends between two nodes, and sends to self, timed out. `Cspcl` now takes an exclusive lock only for teardown.
- Rust: accepted inbound connections are no longer added to the outbound connection pool. The pool never reused them and could close one while it was still being read. The inbound listener now owns and closes them.
- Rust: inbound connections are released when their peer reconnects or after 60 s without a bundle. Before, dead connections were held forever and could exhaust libcsp's connection table.

### Documentation
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
