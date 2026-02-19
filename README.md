# CSPCL — CubeSat Space Protocol Convergence Layer

A lightweight convergence layer that bridges **Bundle Protocol 7 (BP7)** and **CubeSat Space Protocol (CSP)**, enabling Delay/Disruption Tolerant Networking (DTN) over space-grade radio links.

```
BP7 Bundle → CSPCL → CSP SFP → Physical Layer (CAN / ZMQHUB / SocketCAN)
```

## Documentation

Full documentation is available at **[dtn-mtp.github.io/cspcl](https://dtn-mtp.github.io/cspcl)**.

| Page | Description |
|------|-------------|
| [Getting Started](https://dtn-mtp.github.io/cspcl/getting-started/) | Build, initialize, and send your first bundle |
| [C API Reference](https://dtn-mtp.github.io/cspcl/api/c/) | Complete C function and type reference |
| [Rust API Reference](https://dtn-mtp.github.io/cspcl/api/rust/) | Rust crate documentation |
| [uD3TN Integration](https://dtn-mtp.github.io/cspcl/integration/ud3tn/) | Use CSPCL inside uD3TN |
| [Architecture](https://dtn-mtp.github.io/cspcl/architecture/) | Design decisions, SFP fragmentation, protocol stack |

## Quick Start

```bash
git clone https://github.com/dtn-mtp/cspcl.git
cd cspcl && mkdir build && cd build
cmake .. && make
ctest --verbose
```

## License

University of Montpellier Space Center
