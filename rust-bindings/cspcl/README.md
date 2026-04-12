# CSPCL - Rust Bindings

Minimal Rust bindings for the CubeSat Space Protocol Convergence Layer (CSPCL), enabling Bundle Protocol 7 (BP7) bundles to be transmitted over CubeSat Space Protocol (CSP).

## Features

- **Operational sys crate** - The workspace builds the local C implementation instead of assuming prelinked symbols
- **Minimal safe API** - Bootstrap with `Cspcl`, then split into `Sender` and `Receiver`
- **Automatic cleanup** - Resource management via RAII
- **Cross-platform** - POSIX (Linux) and FreeRTOS support at the C layer

## Quick Start

Add to `Cargo.toml`:
```toml
[dependencies]
cspcl = "0.1"
```

Basic usage:
```rust
use cspcl::{Cspcl, CspclConfig, Interface, InterfaceName};

let cspcl = Cspcl::from_config(
    CspclConfig::new(1)
        .with_port(10)
        .with_interface(Interface::Loopback(InterfaceName::new("loopback"))),
)?;

let (sender, receiver) = cspcl.split();

sender.send_bundle(&[1, 2, 3], 2, 10)?;

let bundle = receiver.recv_bundle(5_000)?;
println!(
    "Received {} bytes from {}:{}",
    bundle.data.len(),
    bundle.src_addr,
    bundle.src_port
);
```

## Documentation

See [main repository README](../../README.md) for complete documentation and examples.

## Testing

The Rust tests exercise the safe crate against a built `libcsp` v1.6 checkout.
Point `CSP_REPO_DIR` at that checkout, or update the local override in
`rust-bindings/.cargo/config.toml` to match your machine.

Example setup:

```bash
git clone https://github.com/libcsp/libcsp.git
cd libcsp
git checkout v1.6
python3 waf configure build

cd /path/to/cspcl/rust-bindings
export CSP_REPO_DIR=/path/to/libcsp
cargo test -p cspcl
```

If `rust-bindings/.cargo/config.toml` already points at your local libcsp checkout,
the explicit export is not required.

## Coverage

Install the coverage tool once:

```bash
cargo install cargo-llvm-cov
rustup component add llvm-tools-preview
```

Then generate a coverage summary for the safe Rust crate:

```bash
cd rust-bindings
export CSP_REPO_DIR=/path/to/libcsp
cargo llvm-cov -p cspcl --summary-only
```

Optional report variants:

```bash
cargo llvm-cov -p cspcl --html
cargo llvm-cov -p cspcl --lcov --output-path lcov.info
```

## License

MIT
