---
layout: default
title: picoD3TN Integration
nav_order: 3
parent: BP Integrations
permalink: /integration/picod3tn/
---

# picoD3TN Integration

CSPCL adds a convergence layer to [picoD3TN](https://gitlab.com/d3tn/picod3tn), D3TN's
lightweight async Rust BPv7 node. With it, a picoD3TN node exchanges BP7 bundles over CSP
on **CAN** (SocketCAN), **ZMQHUB** or **loopback**: with other picoD3TN nodes, and with
uD3TN and Unibo-BP nodes running their CSPCL integrations.

The integration is a patch, `picod3tn-integration/picod3tn-cla-cspcl.patch`, against
picoD3TN `38fd7a9`, built on the [`picod3tn-cspcl`](../../api/rust/) crate. The CLA sits
behind an optional cargo feature, `cspcl`: without it, picoD3TN builds as before and
needs no libcsp. The [patch README](https://github.com/DTN-MTP/cspcl/blob/main/picod3tn-integration/README.md)
lists every file it changes, and covers reverting and regenerating it and the Nix module.

## Architecture

```
aapctl (AAP2)
        |
        v
    picoD3TN (spool + forwarder)
        |   FIB: dtn://b.dtn/ -> cspcl:2:10
        v
    cspcl TX / RX tasks  <--- picod3tn/src/tasks/cla/cspcl.rs (patch)
        |
        v
    picod3tn-cspcl (CspclLink)  <--- rust-bindings/picod3tn-cspcl
        |
        v
    cspcl / cspcl-sys  <--- Rust bindings over the C library (src/cspcl.c)
        |
        v
    libcsp v1.6
        |
        +-- ZMQHUB interface ---> ZMQ Hub Broker (tcp://:6000 / :7000)
        |
        +-- SocketCAN interface ---> vcan0 / can0
        |
        +-- Loopback
```

- **TX task:** takes the bundles the forwarder assigns to the `cspcl` CLA from the
  spool, and sends each to the CSP address of its FIB entry. On success the bundle is
  forwarded. A failure worth retrying (peer unreachable, timeout) defers it by 1 s.
- **RX task:** parses each received bundle and puts it into the spool. A bundle that
  doesn't parse, or is too large for the spool, is logged and dropped. It was already
  acknowledged at the CSP level.
- **One link per process.** libcsp is a process-wide singleton, so both tasks share a
  single `CspclLink`, opened at startup and shut down on Ctrl-C.

---

## Prerequisites

| Requirement   | Notes                                                                |
| ------------- | -------------------------------------------------------------------- |
| picoD3TN      | `git clone https://gitlab.com/d3tn/picod3tn.git`, at commit `38fd7a9` |
| Rust          | A stable toolchain; edition 2024 (picoD3TN's flake pins 1.96)         |
| libcsp v1.6   | Static library built with `-fPIC` -- see below                        |
| Python ≤ 3.11 | For libcsp v1.6's bundled waf (it imports `imp`, removed in 3.12)     |
| libclang      | For bindgen in `cspcl-sys`                                            |
| System libs   | `libzmq`, `libsocketcan`, `libbz2` (with their development headers)   |

For the transports, the requirements are those of the
[uD3TN integration](../ud3tn/#prerequisites): a ZMQ hub broker for ZMQHUB, and the
`vcan` module or a real adapter for CAN.

---

## Building libcsp

Rust links binaries as PIE, so `libcsp.a` must be built with `-fPIC`. libcsp v1.6 only
applies its default flags when `CFLAGS` is empty, so pass them all:

```bash
git clone --branch v1.6 https://github.com/libcsp/libcsp.git
cd libcsp
CFLAGS="-std=gnu99 -g -Os -fPIC -Wall -Wextra -Wshadow -Wcast-align -Wwrite-strings -Wno-unused-parameter" \
  python3.11 waf configure --enable-can-socketcan --enable-if-zmqhub --enable-rdp --with-os=posix
python3.11 waf build
export CSP_REPO_DIR=$PWD
```

`-std=gnu99` is needed with GCC 15 and later, which default to C23. A `libcsp.a` built
without `-fPIC` fails at link time with
`relocation R_X86_64_32 against '.rodata.str1.1' can not be used when making a PIE object`.

---

## Applying the Patch

```bash
cd /path/to/picod3tn
git switch --create cla-cspcl 38fd7a9
git apply --check /path/to/cspcl/picod3tn-integration/picod3tn-cla-cspcl.patch
git apply /path/to/cspcl/picod3tn-integration/picod3tn-cla-cspcl.patch

cd picod3tn
cargo build --features cspcl
```

The patch takes the `picod3tn-cspcl` crate from GitHub, at a pinned rev. To build
against a local cspcl checkout instead, override it for one command:

```bash
cargo build --features cspcl \
  --config 'patch."https://github.com/DTN-MTP/cspcl.git".picod3tn-cspcl.path="/path/to/cspcl/rust-bindings/picod3tn-cspcl"'
```

The override rewrites `Cargo.lock`, so the patch no longer reverses until
`cargo metadata --format-version 1 > /dev/null` puts the lock back. The
[patch README](https://github.com/DTN-MTP/cspcl/blob/main/picod3tn-integration/README.md#build)
has the details.

---

## CLA Configuration

`--cspcl-local-addr` enables the CLA. It needs `--cspcl-interface` too:

| Flag                 | Environment                 | Description                                                     |
| -------------------- | --------------------------- | --------------------------------------------------------------- |
| `--cspcl-local-addr` | `PICOD3TN_CSPCL_LOCAL_ADDR` | This node's CSP address, 0 to 30. Enables the CLA.              |
| `--cspcl-port`       | `PICOD3TN_CSPCL_PORT`       | Port bundles are received on, 7 to 31 (default 10)              |
| `--cspcl-interface`  | `PICOD3TN_CSPCL_INTERFACE`  | `loopback`, `can[:<iface>]` or `zmqhub[:<host>]`                |

| `--cspcl-interface` value | Transport | Parameter used                          |
| ------------------------- | --------- | --------------------------------------- |
| `zmqhub`                  | ZMQHUB    | broker at `localhost`                   |
| `zmqhub:<host>`           | ZMQHUB    | broker at `<host>` (hostname or IP)     |
| `can`                     | SocketCAN | device `vcan0`, a **virtual** bus        |
| `can:<iface>`             | SocketCAN | device `<iface>` (e.g. `can0`)           |
| `loopback`                | Loopback  | built-in CSP loopback                   |

On real hardware, name the interface (`can:can0`). The link logs the interface it opens,
and warns when a bare `can` falls back to `vcan0`.

---

## Routes (FIB)

The CSP CLA is FIB-only: a peer is reached through a FIB entry set over AAP2 with
`aapctl configure`, whose CLA address is `cspcl:<addr>:<port>`:

```bash
aapctl configure -c tcp:127.0.0.1:1110 dtn://a.dtn/admin dtn://b.dtn/ cspcl:2:10 LINK_STATUS_UP
```

A cspcl peer cannot be the default CLA: `--default-peer-cla-address cspcl:2:10` is
refused with an error.

---

## Address Format

picoD3TN takes a colon and a required port. The three integrations write the same peer
(CSP address 2, port 10) differently:

| | picoD3TN | uD3TN | Unibo-BP |
|---|---|---|---|
| Peer | `cspcl:2:10` (FIB entry) | `csp:2,10` (contact) | `ipn:2.0` (CSP address = ipn node number) |
| Port | 7 to 31, required | 0 to 31, defaults to 10 | Its own `CSP_PORT`, used for every peer |
| Address | 0 to 30 | 0 to 255 (only 0 to 31 are CSP addresses) | The ipn node number |

What this means for mixed networks:

- **Use one port, 10, on every node.** The Unibo-BP daemon sends to its own port, so its
  peers must listen on that same port.
- **A uD3TN node on a port from 0 to 6 can't be reached from picoD3TN.** Those ports are
  CSP's services, and picoD3TN refuses them.
- **Unibo-BP reaches a picoD3TN node as `ipn:<csp-addr>.0`.** For example, a picoD3TN node
  at CSP address 6 with node ID `dtn://f.dtn/` needs a contact with `ipn:6.0` on the
  Unibo-BP side, and a static route for `dtn://f.dtn/` via `ipn:6.0`.

---

## Running a Two-Node Test

Each node needs its own AAP2 and MTCPCL ports when both run on one host.

### CAN (SocketCAN)

```bash
sudo modprobe vcan
sudo ip link add dev vcan0 type vcan && sudo ip link set up vcan0

B=target/debug
# Node A, CSP address 1
RUST_LOG=debug $B/picod3tn --node-id dtn://a.dtn/ --aap2-listen tcp:127.0.0.1:1110 \
  --mtcpcl-listen-address 127.0.0.1:1111 --cspcl-local-addr 1 --cspcl-interface can:vcan0

# Node B, CSP address 2
RUST_LOG=debug $B/picod3tn --node-id dtn://b.dtn/ --aap2-listen tcp:127.0.0.1:2220 \
  --mtcpcl-listen-address 127.0.0.1:2222 --cspcl-local-addr 2 --cspcl-interface can:vcan0
```

### ZMQHUB

Start the broker, then the same two nodes with `--cspcl-interface zmqhub`:

```bash
python3 tools/zmqhub_broker.py -v
```

### Common steps (both transports)

```bash
# Routes: each node reaches the other via cspcl
$B/aapctl configure -c tcp:127.0.0.1:1110 dtn://a.dtn/admin dtn://b.dtn/ cspcl:2:10 LINK_STATUS_UP
$B/aapctl configure -c tcp:127.0.0.1:2220 dtn://b.dtn/admin dtn://a.dtn/ cspcl:1:10 LINK_STATUS_UP

# Receive on B, send from A
$B/aapctl listen -c tcp:127.0.0.1:2220 dtn://b.dtn/inbox &
$B/aapctl send -c tcp:127.0.0.1:1110 dtn://a.dtn/app dtn://b.dtn/inbox hello-from-a
```

The listener prints the received bundle, with its payload as a list of bytes.

### Docker

The interop stack runs two picoD3TN nodes (CSP addresses 5 and 6) next to the uD3TN and
Unibo-BP nodes, on both transports:

```bash
cd tests/interop
./run-tests.sh --test picod3tn
./run-tests.sh --transport can --test picod3tn
```

See [`tests/interop/README.md`](https://github.com/DTN-MTP/cspcl/blob/main/tests/interop/README.md)
for the scenarios.

---

## Behaviour and Limitations

- **FIB-only.** The CSP CLA can't be the default CLA.
- **No BPSec keys on RX.** Received bundles are parsed without keys, so bundles that need
  BPSec verification or decryption on reception are not supported yet.
- **Linux only.** The CLA runs on std and tokio. A `no_std` port (e.g. `picod3tn-rp235x`)
  needs libcsp and cspcl cross-built for the target, and is not done yet.
- **Bundles up to 65535 bytes.** A larger bundle can never be sent: it is logged as an
  error and dropped.
- **Sends are serialized** in cspcl's C library. An unreachable peer delays the bundles to
  every other peer by up to about 20 s per attempt.
- **Recovery takes time.** After a peer restarts, allow up to about 30 s before
  redelivery succeeds. The bundles sent while it was down are kept and delivered then:
  `tests/interop/test-picod3tn-redelivery.sh` checks this.
- **A missing or invalid next-hop address** is logged and the bundle deferred. The TX
  task keeps running.

The [`picod3tn-cspcl` README](https://github.com/DTN-MTP/cspcl/blob/main/rust-bindings/picod3tn-cspcl/README.md#behaviour-to-know)
describes the link's behaviour in more detail.

---

## Troubleshooting

**Link fails at build time with `relocation R_X86_64_32 ... PIE object`** -- `libcsp.a`
was built without `-fPIC`. Rebuild it as shown in [Building libcsp](#building-libcsp).

**waf fails with `ModuleNotFoundError: No module named 'imp'`** -- run waf with Python
3.11 or older.

**`aapctl configure` rejects the CLA address** -- the separator is a colon and the port
is required: `cspcl:2:10`, not `cspcl:2,10` or `cspcl:2`.

**Bundle not delivered to a Unibo-BP node** -- check that both nodes use the same CSP
port, and that the Unibo-BP node has a contact with `ipn:<csp-addr>.0`.

**Enable debug logging**

```bash
export RUST_LOG=debug
```

Look for these log lines in picoD3TN's output:

- `cspcl link open as CSP X:Y on <interface>`
- `CSPCL RX task started at CSP X:Y`
- `CSPCL TX task started`
- `transmitting bundle to <node-id> via CLA cspcl:X:Y`
- `Received a N byte bundle from X:P`
