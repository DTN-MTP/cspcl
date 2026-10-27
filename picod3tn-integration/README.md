# picoD3TN Integration with CSPCL

`picod3tn-cla-cspcl.patch` adds a cspcl convergence layer to [picoD3TN](https://gitlab.com/d3tn/picod3tn), built on the [`picod3tn-cspcl`](../rust-bindings/picod3tn-cspcl) crate. picoD3TN nodes then exchange bundles over CSP (CAN, ZMQ hub or loopback), with each other and with uD3TN nodes running the [CSP CLA](../ud3tn-integration).

The CLA is behind an optional cargo feature, `cspcl`. Without it, picoD3TN builds as before and needs no libcsp.

The patch applies to picoD3TN commit `38fd7a9` (`main`, "Merge branch 'refactor/picod3tn' into 'main'").

## What the patch changes

| File | Change |
|---|---|
| `picod3tn/Cargo.toml` | Optional `picod3tn-cspcl` dependency (git, pinned rev) and the `cspcl` feature; tokio's `time` feature for the tests |
| `picod3tn/Cargo.lock` | The cspcl crates; `bytes`, `clap` and `tokio` bumped to versions they accept |
| `picod3tn/src/lib.rs` | `ClaAddr::Cspcl` (`cspcl:<addr>:<port>`); `ClaAddr::addr()` replaced by `link_addr()`; `DefaultCla` rejects `cspcl:` |
| `picod3tn/src/tasks.rs` | The forwarder passes `link_addr()` to the CLA |
| `picod3tn/src/tasks/cla.rs` | The default CLA TX task logs and defers a cspcl address |
| `picod3tn/src/tasks/cla/cspcl.rs` | New: the cspcl RX and TX tasks, and their tests over a loopback link |
| `picod3tn/src/bin/picod3tn.rs` | `--cspcl-*` arguments; opens the link and starts the tasks; shuts the link down on Ctrl-C |
| `nix/packages/libcsp.nix` | New: libcsp v1.6, built with `-fPIC` |
| `nix/packages/libsocketcan.nix` | New: libsocketcan 0.0.12, which libcsp and `cspcl-sys` link (not in nixpkgs) |
| `nix/packages/picod3tn.nix` | libcsp, bindgen and the link libraries; `withCspcl` argument |
| `flake.nix` | `picod3tn-cspcl` package in the overlay |
| `nix/modules/picod3tn.nix` | `services.picod3tn.<name>.cspcl` option |
| `nix/checks/tests/picod3tn-cspcl.nix` | New VM test: two instances over vcan0 |

## Prerequisites

- **picoD3TN** checked out at `38fd7a9`: `git clone https://gitlab.com/d3tn/picod3tn.git`
- **libcsp v1.6** built with `-fPIC`, with `CSP_REPO_DIR` pointing at it (see [Building libcsp](#building-libcsp)). Only needed for the `cspcl` feature.
- `libzmq`, `libsocketcan`, `libbz2` and `libclang` (for bindgen), as for the other Rust crates of this repository.

## Apply

From the picoD3TN checkout, on a branch of your own so that `main` stays clean:

```bash
cd /path/to/picod3tn
git switch --create cla-cspcl 38fd7a9

# Dry run: lists the files and fails without touching anything if it does not apply.
git apply --check --verbose /path/to/cspcl/picod3tn-integration/picod3tn-cla-cspcl.patch

git apply /path/to/cspcl/picod3tn-integration/picod3tn-cla-cspcl.patch
```

The patch creates four files (`picod3tn/src/tasks/cla/cspcl.rs`, `nix/packages/libcsp.nix`, `nix/packages/libsocketcan.nix`, `nix/checks/tests/picod3tn-cspcl.nix`) and modifies the others. Nothing is staged or committed.

## Build

```bash
cd picod3tn

# Without the CLA: no libcsp needed.
cargo build

# With the CLA.
export CSP_REPO_DIR=/path/to/libcsp
cargo build --features cspcl
cargo test --features cspcl
```

The `picod3tn-cspcl` dependency is fetched from GitHub at the rev pinned in `Cargo.toml`. To build against your local cspcl checkout instead (e.g. while changing the crate), override it for one command without editing any file:

```bash
cargo build --features cspcl \
  --config 'patch."https://github.com/DTN-MTP/cspcl.git".picod3tn-cspcl.path="/path/to/cspcl/rust-bindings/picod3tn-cspcl"'
```

The override rewrites `picod3tn/Cargo.lock` (it drops the git `source` of the three cspcl crates), and the patch then no longer reverses. Any cargo command without the override puts the lock back as the patch left it:

```bash
cargo metadata --format-version 1 > /dev/null
```

## Usage

The CLA is enabled by `--cspcl-local-addr`, which needs `--cspcl-interface` too:

| Flag | Environment | |
|---|---|---|
| `--cspcl-local-addr` | `PICOD3TN_CSPCL_LOCAL_ADDR` | This node's CSP address (0 to 30). Enables the CLA. |
| `--cspcl-port` | `PICOD3TN_CSPCL_PORT` | Port bundles are received on (7 to 31, default 10). |
| `--cspcl-interface` | `PICOD3TN_CSPCL_INTERFACE` | `loopback`, `can[:<iface>]` (default `vcan0`) or `zmqhub[:<host>]` (default `localhost`). |

Peers are added to the FIB with `aapctl configure`, as `cspcl:<addr>:<port>` (e.g. `cspcl:2:10`). uD3TN writes the same peer `csp:2,10`: picoD3TN takes a colon, and the port is required.

A cspcl peer cannot be the default CLA: `--default-peer-cla-address cspcl:2:10` is refused.

### Example: two nodes over vcan0

```bash
sudo modprobe vcan
sudo ip link add dev vcan0 type vcan && sudo ip link set up vcan0

B=target/debug
# Node A, CSP address 1
RUST_LOG=debug $B/picod3tn --node-id dtn://a.dtn/ --aap2-listen tcp:127.0.0.1:1110 \
  --mtcpcl-listen-address 127.0.0.1:1111 --cspcl-local-addr 1 --cspcl-interface can:vcan0 

B=target/debug
# Node B, CSP address 2
RUST_LOG=debug $B/picod3tn --node-id dtn://b.dtn/ --aap2-listen tcp:127.0.0.1:2220 \
  --mtcpcl-listen-address 127.0.0.1:2222 --cspcl-local-addr 2 --cspcl-interface can:vcan0 

# Routes: each node reaches the other via cspcl.
$B/aapctl configure -c tcp:127.0.0.1:1110 dtn://a.dtn/admin dtn://b.dtn/ cspcl:2:10 LINK_STATUS_UP
$B/aapctl configure -c tcp:127.0.0.1:2220 dtn://b.dtn/admin dtn://a.dtn/ cspcl:1:10 LINK_STATUS_UP

# Receive on B, send from A.
$B/aapctl listen -c tcp:127.0.0.1:2220 dtn://b.dtn/inbox &
$B/aapctl send -c tcp:127.0.0.1:1110 dtn://a.dtn/app dtn://b.dtn/inbox hello-from-a
```

The listener prints the received bundle. Send from B to A the same way.

### Behaviour to know

See the [`picod3tn-cspcl` README](../rust-bindings/picod3tn-cspcl/README.md#behaviour-to-know). For picoD3TN:

- A failed send that is worth retrying (peer unreachable, timeout) is deferred by 1 s. Sends are serialized, so an unreachable peer delays the bundles to every other peer by up to about 20 s per attempt.
- A bundle that can never be sent (over 65535 bytes) is dropped and logged as an error.
- An invalid or missing next-hop address is logged and the bundle deferred; the TX task keeps running.

## Nix

With the patch applied, a `services.picod3tn.<name>.cspcl` option runs that instance with the `picod3tn-cspcl` package:

```nix
services.picod3tn.alice = {
  enable = true;
  cspcl = {
    localAddr = 1;
    interface = "can:can0";
    # port = 10;
  };
};
```

The VM test runs both instances in one VM, because vcan0 only exists within one kernel:

```bash
nix build --print-build-logs .#checks.x86_64-linux.picod3tnCspcl
```

## Building libcsp

As for the other Rust crates (see the [Rust API docs](../docs/api/rust.md#installation)): libcsp v1.6, as a static library built with `-fPIC`. Its bundled waf needs Python 3.11 or older (it imports `imp`, which Python 3.12 removed). libcsp v1.6 only applies its default flags when `CFLAGS` is empty, so pass them all:

```bash
git clone --branch v1.6 https://github.com/libcsp/libcsp.git
cd libcsp
CFLAGS="-std=gnu99 -g -Os -fPIC -Wall -Wextra -Wshadow -Wcast-align -Wwrite-strings -Wno-unused-parameter" \
  python3.11 waf configure --enable-can-socketcan --enable-if-zmqhub --enable-rdp --with-os=posix
python3.11 waf build
export CSP_REPO_DIR=$PWD
```

## Revert

To remove the patch while keeping any other change in the checkout, reverse it:

```bash
cd /path/to/picod3tn
git apply --reverse --check /path/to/cspcl/picod3tn-integration/picod3tn-cla-cspcl.patch
git apply --reverse /path/to/cspcl/picod3tn-integration/picod3tn-cla-cspcl.patch
```

This also deletes the four files the patch created. It only works while the patched files are as the patch left them:

- If the check fails on `picod3tn/Cargo.lock` after a build with the local cspcl override, run `cargo metadata --format-version 1 > /dev/null` in `picod3tn/` first (see [Build](#build)).
- If you edited the patched files since, either reverse it with `--3way`, or discard everything instead.

To discard everything (the patch and any other uncommitted change):

```bash
git restore .
git clean --dry-run          # lists the untracked files that would go
git clean --force            # removes them
git switch main && git branch --delete --force cla-cspcl
```

`cargo build` leaves `target/`, which git ignores; delete it if you want the space back.

## Updating the patch

Make the changes on a picoD3TN branch from `38fd7a9`, then export them from the picoD3TN checkout. `git diff` leaves out untracked files, so the new files are appended one by one:

```bash
cd /path/to/picod3tn
P=/path/to/cspcl/picod3tn-integration/picod3tn-cla-cspcl.patch
{
  git diff 38fd7a9
  for f in $(git ls-files --others --exclude-standard); do git diff --no-index -- /dev/null "$f"; done
} > "$P"
```

`git diff --no-index` exits with status 1 when the files differ, which is expected here.

Check that it applies to a clean checkout before committing it:

```bash
git clone --quiet /path/to/picod3tn /tmp/picod3tn-check
git -C /tmp/picod3tn-check switch --detach 38fd7a9
git -C /tmp/picod3tn-check apply --check "$P"
```

## File Structure

```
cspcl/
|-- rust-bindings/
|   +-- picod3tn-cspcl/              # The cspcl link crate the patch depends on
+-- picod3tn-integration/
    |-- README.md                    # This file
    +-- picod3tn-cla-cspcl.patch     # Patch for picoD3TN 38fd7a9
```
