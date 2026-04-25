# CSPCL — CubeSat Space Protocol Convergence Layer

[![CI](https://github.com/DTN-MTP/cspcl/actions/workflows/ci.yml/badge.svg)](https://github.com/DTN-MTP/cspcl/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

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
| [Unibo Integration](https://dtn-mtp.github.io/cspcl/integration/unibo/) | Use CSPCL inside Unibo |
| [Architecture](https://dtn-mtp.github.io/cspcl/architecture/) | Design decisions, SFP fragmentation, protocol stack |

## Quick Start

### Build from Source

```bash
git clone https://github.com/dtn-mtp/cspcl.git
cd cspcl && mkdir build && cd build
cmake .. && make
ctest --verbose
```

### Docker Deployment

**Recommended for testing and integration development.**

Docker deployment provides complete DTN environments with both uD3TN and Unibo-BP integrations, automated interoperability tests, and support for ZMQHUB (virtual) and CAN (hardware-like) transports.

```bash
# Quick start - Run all interoperability tests
cd cspcl
docker build -t cspcl-base:latest -f docker/base/Dockerfile .
cd tests/interop
./run-tests.sh
./run-tests.sh --transport can

# Manual stack/message workflow helpers
./stack-up.sh --transport zmqhub
./send-message.sh --scenario all

# Or use Docker Compose directly
cd docker
docker-compose -f docker-compose.zmqhub.yml up
```

**What's included:**
- 🐳 Multi-stage Docker builds (base, uD3TN, Unibo-BP)
- 🧪 Automated test suite (uD3TN basic, Unibo-BP basic, cross-integration)
- 🔌 Two transport modes (ZMQHUB virtual, CAN with vcan)
- 📊 Test runner with reporting and interactive mode
- 📚 Comprehensive documentation

**System requirements:**
- Docker 20.10+
- Docker Compose v2+
- 2GB RAM, 2GB disk space
- Linux (recommended) or macOS/Windows with WSL2

**See:** [docker/README.md](docker/README.md) for complete Docker deployment guide.

## Development

### Code Formatting

CSPCL uses automated code formatting to maintain consistency:

```bash
# Format all code (C, Python, shell scripts)
./tools/format-code.sh

# Check formatting without modifying files
clang-format --dry-run --Werror src/*.c src/*.h
black tools/ --check
shellcheck tools/*.sh tests/interop/*.sh
```

**Style guides:**
- **C code:** Based on LLVM style (see `.clang-format`)
- **Python:** Black formatter, 100 character line limit
- **Shell:** ShellCheck compliant

### Running Tests Locally

```bash
# Unit tests
mkdir build && cd build
cmake .. -DCSPCL_BUILD_TESTS=ON -DCSP_REPO_DIR=/path/to/libcsp
make && ctest --output-on-failure

# Integration tests  
cd tests/interop
./run-tests.sh --transport zmqhub
```

### Continuous Integration

The project uses GitHub Actions for CI/CD. Every push and pull request runs:

✅ Code quality checks (formatting, linting, style)  
✅ Build verification (Debug + Release)  
✅ Unit tests  
✅ Docker image builds  
✅ Integration tests  
✅ Documentation validation  
✅ Security scanning  

**See:** [.github/CI.md](.github/CI.md) for detailed CI documentation.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

This project is licensed under the [MIT License](LICENSE).  
Copyright © 2025 DTN-MTP contributors.
