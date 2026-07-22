---
layout: default
title: Delivery Acknowledgement
nav_order: 6
permalink: /delivery-ack/
---

# Delivery Acknowledgement

CSPCL confirms that a bundle actually reached its destination itself, at the
application level, using nothing beyond stock CSP connection-oriented
sockets (`csp_send()` / `csp_read()`). It requires **no changes to libcsp** —
any unmodified libcsp v1.6 build works.

## Why this is needed

`csp_send()` and `csp_sfp_send()` only report that data was handed to the
local CSP stack (queued in the transmit window on an RDP connection). They
return success as soon as that queuing succeeds, not once the peer has
received the data. Without an explicit confirmation, a bundle sent to a
dead or unreachable peer looks identical, from the sender's point of view,
to one that was actually delivered — it is reported as sent and discarded
by the Bundle Protocol layer, and is lost.

## How it works

After `csp_sfp_send()` completes, the receiver's `cspcl_recv_bundle()` /
`cspcl_recv_bundle_from_conn()` — once it has successfully reassembled the
bundle via `csp_sfp_recv()` — sends a small one-byte ack packet back on the
*same connection*. The sender, right after its `csp_sfp_send()` call,
blocks on `csp_read()` for that ack (up to `CSPCL_ACK_TIMEOUT_MS`) before
reporting the send as successful.

| Step | Side | Call |
| --- | --- | --- |
| 1 | Sender | `csp_sfp_send()` — fragments and queues the bundle |
| 2 | Receiver | `csp_sfp_recv()` — reassembles the bundle |
| 3 | Receiver | `csp_send()` — sends a 1-byte ack back on the same connection |
| 4 | Sender | `csp_read()` — blocks (up to `CSPCL_ACK_TIMEOUT_MS`) for that ack |

If no ack arrives in time, `cspcl_send_bundle()` invalidates the pooled
connection, reconnects, and retries once; if that also fails it returns:

| cspcl error | Cause | ud3tn CLA behavior |
| --- | --- | --- |
| `CSPCL_ERR_TIMEOUT` | No ack within `CSPCL_ACK_TIMEOUT_MS` | Logs `TX TIMEOUT - No ACKs received`, reports `UD3TN_FAIL` to the BP, tears down the link |
| `CSPCL_ERR_CSP_SEND` | `csp_sfp_send()` itself failed (e.g. out of buffers) | Logs the error, reports `UD3TN_FAIL`, tears down the link |

This means the bundle processor is informed about the **first** bundle that
fails after a peer dies, instead of silently losing it — verified end to end
against a genuinely unmodified libcsp v1.6 build: killing a peer mid-session
causes the very next send to report `CSPCL_ERR_TIMEOUT` within
`CSPCL_ACK_TIMEOUT_MS`, and the link recovers once the peer comes back.

## Why not rely on RDP's own acknowledgements?

An earlier version of this mechanism waited on RDP's internal
acknowledgement state instead, which required a small patch to libcsp's RDP
transport (a public API to block until a connection's outstanding data was
acked, plus a peer-liveness timeout). That worked, but it meant every build
of cspcl depended on a modified libcsp — anyone with a vendor-supplied or
already-qualified libcsp binary couldn't use cspcl at all, and every libcsp
upgrade required re-applying and re-verifying the patch.

The application-level ack achieves the same guarantee — the sender learns
within a bounded time whether the bundle actually arrived — using only
public, stable CSP connection APIs that exist unchanged across libcsp
versions. The cost is one small extra packet's round trip per bundle, which
in practice is negligible next to the SFP fragments already being
exchanged.

## Tuning

| Knob | Default | Where | Guidance |
| --- | --- | --- | --- |
| `CSPCL_ACK_TIMEOUT_MS` | 5000 ms | `src/cspcl_config.h` (compile-time) | Size for the RTT of **one hop** (bus, radio link), not the end-to-end path. On a CAN bus, even 1–2 s is generous. |
| `CSPCL_MAX_CONN_AGE_MS` | unset | env var | Optional maximum age for pooled connections. |

Do **not** stretch `CSPCL_ACK_TIMEOUT_MS` to cover long-delay links (deep
space, minutes of RTT) — a connection-level ack cannot operate at such
delays. The DTN answer is to make the long-delay link its own hop between
two BP nodes and recover failures at the bundle layer (custody transfer /
dispatcher re-scheduling); the CSPCL ack only needs to be honest about the
local hop.

## Known limitation

Sending the ack back is best-effort: if the ack packet itself is lost after
the bundle was successfully received, the sender will time out and retry,
which can result in the same bundle being delivered twice. This is an
accepted trade-off for a best-effort local convergence layer — true
end-to-end exactly-once delivery is the Bundle Protocol layer's job (custody
transfer), not the CLA's.

## Testing

The stub build (`cmake -DCSPCL_USE_SYSTEM_CSP=OFF ..`) simulates the ack
round trip via a failure-injection flag:

```c
extern int g_csp_ack_should_arrive; /* stubs/csp_stub.c, default 1 */

g_csp_ack_should_arrive = 0; /* simulate a peer that never acks (dead/unreachable) */
g_csp_ack_should_arrive = 1; /* normal operation */
```

See `test_send_bundle_ack_timeout` in `tests/test_cspcl.c` for a full
example.
