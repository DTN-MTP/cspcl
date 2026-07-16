# CSPCL Peer Liveness & Bundle Resend on Recovery — Design

**Date:** 2026-07-16
**Status:** Approved design, pending implementation plan
**Repos touched:** `cspcl` (C library), `cspcl-server/rust-bindings` (`cspcl-sys`, `cspcl`, `hardy-cspcl`)

## Problem

When a bundle is forwarded to a remote CSPCL peer that is down, the send fails
(the application-level ACK never arrives, so `cspcl_send_bundle` times out and
the CLA returns `Err`). The hardy BPA correctly parks the bundle as `Waiting`
(via `reset_peer_queue`, `bpa/src/dispatcher/forward.rs:79-85`). **But when the
peer comes back online, the bundle is never resent.**

### Root cause

Two facts combine:

1. **The `hardy-cspcl` CLA has no peer-liveness detection.** It registers every
   configured peer exactly once at startup
   (`Cla::on_register → register_peers → sink.add_peer`,
   `rust-bindings/hardy-cspcl/src/lib.rs:97-123`) and never removes a dead peer
   or re-adds a recovered one. `cspcl-bindings` exposes no liveness primitive
   (only `send_bundle`, `inbound`, `is_initialized`), and the
   `HEARTBEAT_INTERVAL` hinted at in `config.rs:79` is unimplemented.

2. **The BPA re-drives `Waiting` bundles only on RIB changes, not on a timer.**
   `poll_waiting` runs only when `poll_waiting_notify` fires, and that fires only
   on a RIB add/remove via `notify_updated` (`bpa/src/routing/rib.rs:129,462`).
   `forward_bundle`'s failure path does **not** fire it.

Therefore, when the peer recovers, nothing changes in hardy's RIB, so
`poll_waiting` never runs, so the parked bundle sits in `Waiting` until its TTL
expires (reaped as `LifetimeExpired`) or the process restarts. Resend is not
broken — nothing ever tells hardy to try again.

## Goal

Detect when a peer goes down and when it recovers, and on recovery trigger the
existing hardy re-drive path so parked `Waiting` bundles are resent — without
changing the CSPCL wire protocol in a way that breaks interoperability.

## Key decisions (from brainstorming)

| Decision | Choice | Rationale |
|---|---|---|
| Recovery-detection mechanism | **CSP-level liveness** via `csp_ping` | True liveness signal decoupled from bundle traffic; libcsp-native, so interoperable with any libcsp node. |
| Monitoring model | **Reactive** (probe only after a send failure) | No probe traffic while healthy. The probe loop detects recovery. |
| Down threshold | **N consecutive failures** (configurable) | Tolerates transient blips without churning the RIB. |
| Orchestration | **On-demand per-peer recovery task** (Approach 1) | Least machinery; tasks exist only for down peers; each self-manages its timer. |
| Style | **As low-level as possible** | Liveness primitive in C; minimal, lock-free Rust glue (plain atomics + `spawn_blocking`); no channels/actors. |

## Interoperability constraint

**This feature assumes every remote CSPCL peer is ACK-aware** (runs a build that
sends the application-level ACK after `csp_sfp_recv`). Under that assumption:

- **Recovery detection is inherently interoperable.** `csp_ping` uses libcsp's
  built-in CSP ping service (CMP/echo handler on the `CSP_PING` port). Any node
  running libcsp with the standard service handler answers it automatically —
  nothing CSPCL-specific is required on the remote.
- **Down detection is consistent** because send-failure (ACK timeout) is a valid
  delivery signal only when the remote actually sends the ACK.

A **non-ACK** remote would break this: `cspcl_send_bundle` would always time out
waiting for an ACK that never comes, so every send would fail and the peer would
be perpetually (and falsely) flagged down. Supporting non-ACK peers would require
a different design (`csp_ping` as the *primary* liveness signal with proactive
monitoring, and the ACK-wait gated per peer). That is explicitly **out of scope**
here.

## Architecture

The mechanism spans three layers, adding the minimum at each.

### Layer 1 — CSPCL C library: `cspcl_ping()`

New public function in `cspcl.c` / `cspcl.h`:

```c
cspcl_error_t cspcl_ping(cspcl_t *cspcl, uint8_t dest_addr, uint32_t timeout_ms);
```

- Thin wrapper over libcsp's `csp_ping(dest_addr, timeout_ms, size, opts)`
  (`include/csp/csp.h:353`) with a small `size` and default `opts`.
- Returns `CSPCL_OK` if `csp_ping() >= 0`, else a timeout/unreachable error code.
- Named `cspcl_*`, so the `cspcl-sys` bindgen allowlist
  (`rust-bindings/cspcl-sys/build.rs:122`, `.allowlist_function("cspcl_.*")`)
  picks it up automatically — no allowlist change, no raw `csp_*` in the sys
  layer.
- `dest_addr` is the CSP node address (peer `addr`); ping is node-level, which is
  the right granularity for liveness.

### Layer 2 — `cspcl` Rust binding: `Cspcl::ping`

```rust
pub fn ping(&self, dest_addr: u8, timeout: Duration) -> Result<()>;
```

- Direct FFI call to `cspcl_sys::cspcl_ping`.
- Blocking (synchronous C) — callers invoke it via `spawn_blocking`.
- `Transport` gains a matching `ping()` delegate so the CLA never touches raw
  FFI directly.

### Layer 3 — `hardy-cspcl` CLA: liveness + recovery orchestration

Per-peer state, built once at construction from the static peer config
(peers are known upfront → the map itself is immutable, only its contents are
atomic; no lock on the hot path):

```rust
struct PeerLiveness {
    consecutive_failures: AtomicU32,
    recovering: AtomicBool,
}
// held as: HashMap<CspAddress, Arc<PeerLiveness>>
```

- **`forward()`** (`cla.rs`): only *reports* liveness; it never mutates the RIB.
  - `send_bundle` **Ok** → `consecutive_failures.store(0)`.
  - `send_bundle` **Err** →
    `let n = consecutive_failures.fetch_add(1) + 1;`
    `if n >= failure_threshold && !recovering.swap(true) { spawn recovery task }`;
    return `Err` as today.
- **Recovery task** (one per down peer, guarded by the `recovering` CAS) performs
  all RIB mutations, off the egress path.

**Component boundary:** `forward()` reports, the recovery task mutates the RIB.
This is what avoids the mid-forward reentrancy hazard — `forward()` runs on
hardy's egress poller *for that peer*, so calling `remove_peer` inline would
mutate/close the peer while it is mid-send.

## Data flow & state model

```
[Up] --send Ok--> reset failures=0, stay Up
[Up] --send Err--> n = ++failures
        n <  N  → stay Up (transient tolerated)
        n >= N && CAS(recovering: false→true) → spawn recovery task ─┐
                                                                     ▼
[Down] recovery task:
   1. sink.remove_peer(cla_addr)
        // rib.remove → reset_peer_queue(peer) + notify_updated
        // route dropped; in-flight bundle parked as Waiting; NOT re-driven (no route)
   2. loop:
        sleep(heartbeat_interval)
        if spawn_blocking(|| cspcl.ping(addr, ping_timeout)).is_ok() → break
        // honor cancel token → exit WITHOUT re-adding on shutdown
   3. sink.add_peer(cla_addr, node_ids)
        // rib.add → changed=true → notify_updated → poll_waiting → re-drives Waiting bundles
   4. failures=0; recovering=false   // back to Up
```

### Verified hardy behavior this relies on

- `remove_peer → remove_forward → rib.remove` calls both `reset_peer_queue(peer)`
  **and** `notify_updated` (`bpa/src/routing/rib.rs:292-296`). Step 1 clears the
  route *and* parks the in-flight bundle as `Waiting`; with no route it won't be
  re-driven to the dead peer.
- `add_peer → add_forward → rib.add` sets `changed=true` (Forward action,
  `rib.rs:237-238`) → `notify_updated` → `poll_waiting` (`rib.rs:251-252`). Step 3
  is exactly what re-drives the parked bundle, now routed to the freshly
  registered `peer_id`.

### Concurrency guarantees

- The `recovering` CAS ensures **exactly one** recovery task per peer, even under
  concurrent `forward()` calls from multiple egress queues.
- While `Down`, the route is gone, so `forward()` is never called for that peer →
  the failure counter cannot advance → no duplicate spawns.
- `ping` is blocking C, always run via `spawn_blocking`, so it never stalls the
  tokio runtime and runs on a thread separate from RX/TX (satisfies the
  RX/TX-separation deadlock hazard from the ACK design notes).

## Configuration

Finishes the intended-but-unimplemented `HEARTBEAT_INTERVAL`:

- Extend `PeerConfig` with an optional 4th CSV field:
  `NODE_ID,ADDR,PORT[,HEARTBEAT_INTERVAL]` → `heartbeat_interval: Option<u32>`
  (seconds). Today `config.rs:55-56` *rejects* a 4th field; replace that with
  parsing it. The existing test `config.rs:113-114` (`ipn:3.0,3,1,60`) already
  expects this and becomes a real assertion.
- Three new tunables on `Config`, each with a compile-time default:
  - `failure_threshold: u32` — N consecutive failures → down (default `3`).
  - `ping_timeout_ms: u32` — per-ping timeout (default `1000`).
  - `default_heartbeat_interval_s: u32` — used when a peer omits its own
    (default `5`).

## Error handling

- **Ping fails in the loop** — expected; keep looping. Not logged per-attempt
  (too noisy); log once at `debug` on entering Down, once at `info` on recovery.
- **`send_bundle` Err below threshold** — return `Err` as today; the BPA parks
  the bundle as `Waiting`; no state transition.
- **`add_peer` returns `Err`/`false` on recovery** — log `warn` and retry a
  bounded number of times on subsequent intervals before giving up. This is the
  one path that could strand a bundle, so it must not silently no-op.
- **Shutdown mid-recovery** — the `cancel_dispatcher` child token cancels the
  sleep/ping loop; the task exits *without* re-adding. Bundles stay `Waiting`
  (correct for shutdown).

## Edge cases

- **Flapping** (up/down/up) — bounded: one task per down, state reset per
  recovery; worst case a remove/probe/re-add cycle per flap, self-correcting
  within one `heartbeat_interval`.
- **ACK timeout is itself ~10s** (RDP `conn_timeout` floor, per the ACK design
  notes) — with `N=3`, declaring down takes ~30s of real send attempts. A tuning
  note, not a correctness issue.
- **Peer dies with nothing queued** — not detected until the next send attempt.
  Accepted consequence of the reactive model.

## Testing

Bottom-up, matching the layering:

1. **C layer — `cspcl_ping()`** (CSPCL C test suite, alongside `test_cspcl*`):
   ping a reachable loopback node → `CSPCL_OK` with a sane RTT; ping an address
   with no responder → timeout error within `timeout_ms`.
2. **Config parsing** (`config.rs` unit tests): 4-field form parses
   `heartbeat_interval = Some(60)`; 3-field form yields `None`; malformed interval
   is a clean parse error. Upgrade `config.rs:113-114` to assert the parsed value.
3. **Liveness state machine** (pure unit tests, no CSP): factor the decision into
   a pure function, e.g. `fn on_send_result(&PeerLiveness, ok: bool) -> Transition`,
   and test: N-1 failures stay Up; the Nth flips `recovering` exactly once;
   concurrent failures spawn one task (CAS); success resets the counter.
4. **Recovery integration** (decisive end-to-end, using the 2-node harness implied
   by `config-1.yaml`/`config-2.yaml`): send to a dead peer → assert bundle parked
   `Waiting` and `remove_peer` fired; bring the peer up → assert `csp_ping`
   succeeds → `add_peer` fires → bundle re-driven and delivered. This reproduces
   and closes the original bug. Provide a seam to observe `add_peer`/`remove_peer`
   (mock `Sink` in the CLA's tests, or assert via hardy peer/RIB state in the
   harness).

## Risks to validate during implementation

- **R1 (highest priority — validate before building Layer 3): the remote answers
  `csp_ping`.** The recovery path assumes the remote libcsp node serves the CSP
  ping service (CMP handler on the `CSP_PING` port). If it does not respond to
  ping, this approach cannot detect recovery and must be reconsidered. Confirm
  first, standalone (Layer 1 test against the real remote), before investing in
  the CLA orchestration.
- **R2: `csp_ping` thread-safety** alongside concurrent SFP send/recv on the same
  CSP stack. libcsp 1.6 handles ping replies in the router task, so it should be
  safe; confirm no shared-connection contention with the ACK-wait.
- **R3: `add_peer` re-drive actually reaches `poll_waiting`** in the deployed
  hardy version. Traced in current source (`rib.rs:237-252`); the integration
  test (#4) proves it end-to-end rather than by code-reading.

## Out of scope

- Supporting non-ACK-aware remote peers (would require ping-primary proactive
  monitoring and per-peer gating of the ACK-wait).
- Proactive/continuous liveness monitoring of healthy peers.
- Periodic re-drive of `Waiting` bundles in the hardy BPA core (the rejected
  Option B).
- Detecting a silently-dead peer that has no bundles queued.
