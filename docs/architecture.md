---
layout: default
title: Architecture
nav_order: 5
permalink: /architecture/
---

# Architecture

## Protocol Stack

```
+----------------------------------+
|    Application / BP Agent        |
|  (aap2-send, custom code)        |
+----------------------------------+
              |  BP7 Bundle (serialized CBOR)
              v
+----------------------------------+
|        uD3TN / BP7               |
|      Bundle Processor            |
+----------------------------------+
              |  raw bundle bytes
              v
+----------------------------------+
|            CSPCL                 |  <- this library
|  cspcl_send/recv_bundle()        |
+----------------------------------+
              |  CSP connection + SFP fragments
              v
+----------------------------------+
|           libcsp                 |
|  csp_sfp_send / csp_sfp_recv     |
+----------------------------------+
              |  CSP packets
              v
+----------------------------------+
|   Physical / Virtual Link        |
|  CAN  |  SocketCAN  |  ZMQHUB   |
+----------------------------------+
```

---

## SFP Fragmentation

CSP has a small MTU (typically 256 bytes on CAN). BP7 bundles can be several kilobytes.
CSPCL delegates fragmentation entirely to CSP's **Simple Fragmentation Protocol (SFP)**.

### How SFP works

Each SFP fragment appends an 8-byte trailer:

```
+---------------------------+------------+-----------+
|     payload (<=248 B)     |  offset    | totalsize |
|                           |  (4 bytes) |  (4 bytes)|
+---------------------------+------------+-----------+
```

- `offset` — byte position of this fragment within the full message
- `totalsize` — total reassembled message size

The receiver allocates a single buffer of `totalsize` on first fragment arrival and
fills it as fragments arrive in order. An offset mismatch or size inconsistency causes
the transfer to abort.

### MTU budget

| Layer | Bytes |
| --- | --- |
| CSP MTU | 256 |
| SFP header | 8 |
| **Usable payload** | **248** |

A 10 KB bundle requires ceil(10240 / 248) = **42 fragments**.

---

## Connection Model

CSPCL uses **one CSP connection per bundle transfer**:

```
Sender                        Receiver
  |                               |
  |-- csp_connect() ------------>|
  |                               |  csp_accept()
  |-- SFP fragment 0 ----------->|
  |-- SFP fragment 1 ----------->|
  |        ...                    |
  |-- SFP fragment N ----------->|
  |                               |  bundle reassembled
  |-- csp_close() ------------->|
  |                               |  csp_close()
```

The receiver socket stays open across transfers; only the per-bundle connections are
opened and closed.

---

## Reliability

SFP is **not** reliable by itself — it does not retransmit lost fragments. Reliability
is provided at other layers:

| Layer | Mechanism |
| --- | --- |
| Physical (CAN) | Hardware CRC + ack |
| CSP | RDP enabled by default via `CSPCL_CSP_CONN_OPTIONS` / `CSPCL_CSP_SOCKET_OPTIONS` |
| BP7 custody transfer | End-to-end bundle retransmission |
| CSPCL | Surfaces CSP/SFP failures as `CSPCL_ERR_TIMEOUT` or `CSPCL_ERR_SFP` |

If you need best-effort delivery, override `CSPCL_CSP_CONN_OPTIONS` to `CSP_O_NONE` and
`CSPCL_CSP_SOCKET_OPTIONS` to `CSP_SO_NONE`.

---

## Address Translation

CSPCL maps BP endpoint IDs to 8-bit CSP node addresses:

| BP Endpoint | CSP Address |
| --- | --- |
| `ipn:5.0` | `5` |
| `ipn:5.42` | `5` |
| `dtn://node7/sink` | `7` |

The mapping is intentionally simple: use the node number directly. For networks where
these do not coincide, a lookup table can be introduced in `cspcl_endpoint_to_addr()`.

---

## File Layout

```
cspcl/
+-- src/
|   +-- cspcl.h            Public C API
|   +-- cspcl.c            Implementation
|   +-- cspcl_config.h     Compile-time constants
+-- rust-bindings/
|   +-- cspcl-sys/         Raw FFI bindings (bindgen)
|   +-- cspcl/             Safe Rust wrapper crate
+-- ud3tn-integration/
|   +-- src/
|       +-- cla_csp.c      uD3TN CLA implementation
|       +-- cla_csp.h
+-- stubs/                 CSP stubs for unit testing without libcsp
+-- tests/                 C unit tests (CTest)
+-- tools/
    +-- zmqhub_broker.py   ZMQ hub for local testing
    +-- simple_bundle_transfer.sh
```
