---
layout: default
title: libcsp Patch
nav_order: 6
permalink: /libcsp-patch/
---

# The libcsp Reliability Patch

CSPCL ships a patch for libcsp v1.6, `docker/base/libcsp-rdp-peer-timeout.patch`,
that fixes two transport-level gaps which break Bundle Protocol semantics. Without
it, a bundle sent to a dead peer is **reported as delivered and silently lost**,
and the zombie connection retransmits on the link forever.

## Why the patch exists

Upstream libcsp v1.6 RDP has two behaviors that are fine for request/response
CSP applications but wrong for a convergence layer:

1. **Established connections never time out.** The connection-timeout check in
   `csp_rdp_check_timeouts()` only covers not-yet-accepted connections. If the
   peer disappears (reboot, power loss, link cut), the connection stays
   `RDP_OPEN` and retransmits unacknowledged packets indefinitely — visible as
   endless traffic on `candump`.
2. **`csp_send()` / `csp_sfp_send()` succeed without delivery.** RDP returns
   `CSP_ERR_NONE` as soon as a packet is queued in the transmit window
   (default: 4 packets). It never waits for the peer's acknowledgment, so a
   small bundle sent to a dead peer "succeeds" instantly. The BP marks the
   bundle transmitted and deletes it — the bundle is lost.

## What the patch changes

All changes are in `src/transport/csp_rdp.c` plus one declaration in
`include/csp/csp.h`:

| Change | Effect |
| --- | --- |
| Timestamp refresh on RX | Every packet received from the peer refreshes `conn->timestamp`, making it a liveness indicator. |
| Peer timeout | `csp_rdp_check_timeouts()` closes an `RDP_OPEN` connection that has unacknowledged data (`tx_queue` non-empty) and no packet from the peer for `conn_timeout` (10 s by default). Idle connections without pending data are **not** closed, so pooled connections survive. |
| `csp_rdp_wait_acked()` | New public API to block until the peer has acknowledged all transmitted data. |

### `csp_rdp_wait_acked()`

```c
int csp_rdp_wait_acked(csp_conn_t *conn, uint32_t timeout_ms);
```

Blocks until every transmitted RDP sequence number has been acknowledged
(`snd_una == snd_nxt`), the connection closes, or the timeout expires. Wakes on
the `tx_wait` semaphore the router task posts when acknowledgments arrive, so
it does not busy-wait.

| Return | Meaning |
| --- | --- |
| `CSP_ERR_NONE` | All sent data acknowledged by the peer (also returned immediately for non-RDP connections). |
| `CSP_ERR_TIMEDOUT` | Timeout expired with data still unacknowledged. |
| `CSP_ERR_RESET` | The connection closed while waiting (e.g. the peer timeout above fired). |

### How CSPCL uses it

`cspcl_send_bundle()` calls `csp_rdp_wait_acked(conn, CSPCL_ACK_TIMEOUT_MS)`
after every successful `csp_sfp_send()` and only reports success once the peer
has acknowledged the data. On failure it invalidates the pooled connection,
reconnects, and retries once; if that also fails it returns:

| cspcl error | Cause | ud3tn CLA behavior |
| --- | --- | --- |
| `CSPCL_ERR_TIMEOUT` | No acks within `CSPCL_ACK_TIMEOUT_MS` | Logs `TX TIMEOUT - No ACKs received`, reports `UD3TN_FAIL` to the BP, tears down the link |
| `CSPCL_ERR_CSP_SEND` | Connection reset / CSP-layer error | Logs the error, reports `UD3TN_FAIL`, tears down the link |

This means the bundle processor is informed about the **first** bundle that
fails after a peer dies, instead of silently losing it.

## Applying the patch in each usage mode

### Docker (ud3tn / Unibo integration) — automatic

The base image applies the patch while building libcsp
(`docker/base/Dockerfile`). Nothing to do — but remember that **any change to
the patch or to `src/` requires rebuilding `cspcl-base`** before the node
images pick it up:

```bash
docker build -t cspcl-base:latest -f docker/base/Dockerfile .
cd docker && docker compose -f docker-compose.can.yml build
```

### Native build against your own libcsp checkout

```bash
git clone --branch v1.6 --depth 1 https://github.com/libcsp/libcsp.git
git -C libcsp apply /path/to/cspcl/docker/base/libcsp-rdp-peer-timeout.patch
cd libcsp && python3 waf configure --enable-rdp --with-os=posix && python3 waf build
# waf needs Python <= 3.11 (it imports the removed `imp` module)

cd /path/to/cspcl
cmake -S . -B build -DCSP_REPO_DIR=/path/to/libcsp
cmake --build build
```

Linking cspcl against an **unpatched** libcsp fails with an undefined reference
to `csp_rdp_wait_acked` — that is intentional, so a build can never silently
lose the delivery guarantee.

### Rust bindings

`cspcl-sys` locates libcsp through the `CSP_INCLUDE_DIR` / `CSP_REPO_DIR` /
`CSP_BUILD_DIR` environment variables. Point them at a **patched** checkout
built as above:

```bash
CSP_REPO_DIR=/path/to/patched/libcsp cargo build
```

### Unit tests / stub builds — no patch needed

The stub build (`cmake -DCSPCL_USE_SYSTEM_CSP=OFF ..`) uses the built-in stubs,
which provide `csp_rdp_wait_acked()` natively. Tests can simulate a peer that stops
acknowledging via the injection hook:

```c
extern int g_csp_rdp_wait_acked_result;  /* stubs/csp_stub.c */

g_csp_rdp_wait_acked_result = CSP_ERR_TIMEDOUT;  /* peer dead   */
g_csp_rdp_wait_acked_result = CSP_ERR_RESET;     /* conn closed */
g_csp_rdp_wait_acked_result = CSP_ERR_NONE;      /* normal      */
```

See `test_send_bundle_ack_timeout` in `tests/test_cspcl.c` for a full example.

### FreeRTOS / embedded targets

Apply the same patch to the libcsp sources in your firmware tree. The patched
code only uses portable CSP primitives (`csp_get_ms`, `csp_bin_sem_wait`,
`csp_queue_size`), so it builds unchanged on FreeRTOS. The ack wait sleeps on a
semaphore in slices of ≤ 100 ms; the calling task blocks for at most
`CSPCL_ACK_TIMEOUT_MS`.

## Tuning

| Knob | Default | Where | Guidance |
| --- | --- | --- | --- |
| `CSPCL_ACK_TIMEOUT_MS` | 5000 ms | `src/cspcl_config.h` (compile-time) | Size for the RTT of **one hop** (bus, radio link), not the end-to-end path. On a CAN bus, even 1–2 s is generous. |
| RDP `conn_timeout` | 10 000 ms | `csp_rdp_set_opt()` | Drives the peer timeout: how long a connection with pending data survives without hearing from the peer. |
| `CSPCL_MAX_CONN_AGE_MS` | unset | env var | Optional maximum age for pooled connections. |

Do **not** stretch these timeouts to cover long-delay links (deep space,
minutes of RTT). RDP cannot operate at such delays — its handshake and packet
timers give up long before an ack could return. The DTN answer is to make the
long-delay link its own hop between two BP nodes and recover failures at the
bundle layer (custody transfer / dispatcher re-scheduling); the CSPCL ack only
needs to be honest about the local hop.

## Do newer libcsp versions make the patch obsolete?

Checked against upstream (July 2026):

| Capability | v1.6 + patch | v2.0 | v2.1 | develop |
| --- | --- | --- | --- | --- |
| Liveness timestamp on RX | ✓ | ✗ | ✓ | ✓ |
| Dead `RDP_OPEN` conn detected | ✓ (auto, only with pending data) | ✗ | manual (`csp_rdp_conn_is_active()`) | ✓ (auto, any idle conn) |
| Wait until data acked | ✓ (`csp_rdp_wait_acked`) | ✗ | ✗ | ✗ |

The peer-timeout half of the patch was progressively adopted upstream (v2.1
adds monitoring, develop auto-closes). The **ack-wait half exists in no
upstream version** — `csp_send()` still returns once packets are queued — so it
must be ported if CSPCL ever migrates to libcsp v2.x. Note that CSPCL currently
requires v1.6; the v2 API is incompatible.
