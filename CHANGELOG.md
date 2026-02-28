# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
