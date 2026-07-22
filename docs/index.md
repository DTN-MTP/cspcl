---
layout: default
title: Home
nav_order: 1
description: "CubeSat Space Protocol Convergence Layer for Bundle Protocol 7"
permalink: /
---

# CSPCL — CubeSat Space Protocol Convergence Layer

CSPCL is a lightweight convergence layer that bridges **Bundle Protocol 7 (BP7)** and
**CubeSat Space Protocol (CSP)**, enabling Delay/Disruption Tolerant Networking (DTN)
over space-grade radio links.

```bash
BP7 Bundle → CSPCL → CSP SFP → Physical Layer (CAN / ZMQHUB / SocketCAN)
```

---

## Why CSPCL?

CubeSat missions often rely on CSP as their on-board network layer. BP7 / DTN is
increasingly adopted for inter-satellite and ground-to-space data exchange. CSPCL
provides the glue between the two — handling fragmentation of large bundles across
CSP's small MTU without requiring changes to either stack.

---

## Key Features

| Feature | Detail |
| --- | --- |
| **SFP Fragmentation** | Transparently fragments and reassembles bundles using CSP's built-in Simple Fragmentation Protocol |
| **Connection-Based** | One CSP connection per bundle transfer — clean lifecycle |
| **Address Translation** | IPN (`ipn:X.Y`) and DTN (`dtn://nodeX/`) endpoint IDs map directly to CSP node addresses |
| **Minimal Footprint** | ~300 lines of C, no dynamic allocation beyond SFP reassembly buffer |
| **CSP v1.6** | Compatible with libcsp v1.6 (the version widely deployed on CubeSats) |
| **Rust Bindings** | Safe, idiomatic Rust wrappers included |

---

## Documentation

- [Getting Started]({% link getting-started.md %}) — build, initialize, send your first bundle
- [C API Reference]({% link api/c.md %}) — full function and type reference
- [Rust API Reference]({% link api/rust.md %}) — Rust crate documentation
- [uD3TN Integration]({% link integration/ud3tn.md %}) — use CSPCL inside uD3TN
- [Unibo Integration]({% link integration/unibo.md %}) — use CSPCL with Unibo-BP
- [Architecture]({% link architecture.md %}) — design decisions and protocol stack
- [Delivery Acknowledgement]({% link delivery-ack.md %}) — how CSPCL confirms bundles actually arrive, without modifying libcsp

---

## License

University of Montpellier Space Center
