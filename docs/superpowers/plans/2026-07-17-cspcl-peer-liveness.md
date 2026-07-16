# CSPCL Peer Liveness & Bundle Resend on Recovery — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect a downed CSPCL peer via N consecutive send failures, detect its recovery with a reactive `csp_ping` probe loop, and re-drive parked `Waiting` bundles by removing then re-adding the peer route.

**Architecture:** Three layers. (1) A new `cspcl_ping()` C function wraps libcsp's `csp_ping`. (2) A thin Rust FFI binding exposes `Cspcl::ping`. (3) The `hardy-cspcl` CLA tracks per-peer failure counts (lock-free atomics); on threshold it spawns a per-peer recovery task that calls `sink.remove_peer`, probes with `ping` until the node answers, then `sink.add_peer` — which triggers hardy's `poll_waiting` re-drive.

**Tech Stack:** C11 + libcsp 1.6; Rust (`bindgen`/`cspcl-sys`, `cspcl`, `hardy-cspcl`); `tokio`; `hardy-bpa` CLA `Sink` API.

**Design doc:** `docs/superpowers/specs/2026-07-16-cspcl-peer-liveness-design.md`

## Global Constraints

- All changes live in the **`cspcl` repo** (working tree: `cspcl-server`, branch `feat/ack-bundle-transmission`). No changes to `~/code/hardy`.
- Pinned to **libcsp v1.6** APIs (`csp_ping(node, timeout, size, opts)` returns `>= 0` RTT on success, `< 0` on failure).
- C public API is `cspcl_*`-prefixed; the `cspcl-sys` bindgen allowlist is `.allowlist_function("cspcl_.*")` — a `cspcl_`-prefixed function is bound automatically, no `build.rs` change.
- **Interop assumption:** every remote CSPCL peer is ACK-aware, so `send_bundle` failure is a valid "down" signal. Non-ACK peers are out of scope.
- Rust workspace root is `rust-bindings/`; run cargo from there. Package names: `cspcl-sys`, `cspcl`, `hardy-cspcl`.
- C stub unit-test build: `-DCSPCL_USE_SYSTEM_CSP=OFF` (compiles `stubs/csp_stub.c`, defines `USING_CSP_STUBS`). Real build needs `-DCSP_REPO_DIR=/home/hugo/code/libcsp`.
- Follow existing patterns: C tests return `int` (0 pass / 1 fail) with `ASSERT_EQ`/`TEST_PASS`/`TEST_FAIL`; failure injection via a global `g_*_fail` flag in `stubs/csp_stub.c`.

---

### Task 1: C `cspcl_ping()` + stub + unit test

**Files:**
- Modify: `src/cspcl.h` (add declaration after `cspcl_send_bundle`, ~line 256)
- Modify: `src/cspcl.c` (add implementation after `cspcl_send_bundle`, ~line 530)
- Modify: `stubs/csp_stub.c` (add `csp_ping` stub + `g_csp_ping_fail` flag)
- Modify: `tests/test_cspcl.c` (add tests + register in `tests[]` array, ~line 601)

**Interfaces:**
- Produces (C): `cspcl_error_t cspcl_ping(cspcl_t *cspcl, uint8_t dest_addr, uint32_t timeout_ms);`
  - Returns `CSPCL_OK` if the node replied, `CSPCL_ERR_TIMEOUT` if not, `CSPCL_ERR_INVALID_PARAM` / `CSPCL_ERR_NOT_INITIALIZED` on misuse.
- Produces (stub): `extern int g_csp_ping_fail;` in `stubs/csp_stub.c`.

- [ ] **Step 1: Write the failing tests** in `tests/test_cspcl.c`

Add these four test functions just above the `tests[]` array (~line 600). They compile in the stub build (`USING_CSP_STUBS`), where `csp_ping` is the injectable stub added in Step 3.

```c
static int test_ping_null_param(void)
{
  ASSERT_EQ(cspcl_ping(NULL, 2, 1000), CSPCL_ERR_INVALID_PARAM);
  TEST_PASS();
  return 0;
}

static int test_ping_not_initialized(void)
{
  cspcl_t cspcl = {0};
  /* initialized flag is 0 -> must reject before touching CSP */
  ASSERT_EQ(cspcl_ping(&cspcl, 2, 1000), CSPCL_ERR_NOT_INITIALIZED);
  TEST_PASS();
  return 0;
}

static int test_ping_reachable(void)
{
  cspcl_t cspcl = {0};
  cspcl.local_addr = 1;
  cspcl.csp_port = 10;
  cspcl.iface_type = CSP_IFACE_LOOPBACK;
  ASSERT_EQ(cspcl_init(&cspcl), CSPCL_OK);

#ifdef USING_CSP_STUBS
  extern int g_csp_ping_fail;
  g_csp_ping_fail = 0;
#endif
  ASSERT_EQ(cspcl_ping(&cspcl, 2, 1000), CSPCL_OK);

  cspcl_cleanup(&cspcl);
  TEST_PASS();
  return 0;
}

static int test_ping_unreachable(void)
{
  cspcl_t cspcl = {0};
  cspcl.local_addr = 1;
  cspcl.csp_port = 10;
  cspcl.iface_type = CSP_IFACE_LOOPBACK;
  ASSERT_EQ(cspcl_init(&cspcl), CSPCL_OK);

#ifdef USING_CSP_STUBS
  extern int g_csp_ping_fail;
  g_csp_ping_fail = 1;
  ASSERT_EQ(cspcl_ping(&cspcl, 2, 1000), CSPCL_ERR_TIMEOUT);
  g_csp_ping_fail = 0;
#endif

  cspcl_cleanup(&cspcl);
  TEST_PASS();
  return 0;
}
```

Register them in the `tests[]` array (after the `test_send_*` entries, ~line 635):

```c
    {"test_ping_null_param", test_ping_null_param},
    {"test_ping_not_initialized", test_ping_not_initialized},
    {"test_ping_reachable", test_ping_reachable},
    {"test_ping_unreachable", test_ping_unreachable},
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /home/hugo/code/cspcl/cspcl-server
cmake -S . -B build-stubs -DCSPCL_USE_SYSTEM_CSP=OFF -DCSPCL_BUILD_TESTS=ON
cmake --build build-stubs 2>&1 | tail -20
```
Expected: **compile/link failure** — `implicit declaration of 'cspcl_ping'` and/or undefined reference to `cspcl_ping` and `g_csp_ping_fail`.

- [ ] **Step 3: Add the `csp_ping` stub** to `stubs/csp_stub.c`

Next to `g_csp_sfp_send_fail` (~line 52) add the flag:

```c
/**
 * Set to non-zero to make csp_ping return a failure (unreachable node).
 * Reset to 0 to simulate a reachable node. Used for failure-injection tests.
 */
int g_csp_ping_fail = 0;
```

Add the stub function (anywhere among the other CSP API stubs):

```c
int csp_ping(uint8_t node, uint32_t timeout, unsigned int size, uint8_t opts)
{
  (void) node;
  (void) timeout;
  (void) size;
  (void) opts;
  /* Real csp_ping returns round-trip time in ms (>= 0), or < 0 on failure. */
  return g_csp_ping_fail ? -1 : 1;
}
```

- [ ] **Step 4: Declare `cspcl_ping` in `src/cspcl.h`**

After the `cspcl_send_bundle` declaration (~line 256):

```c
/**
 * @brief Probe reachability of a remote CSP node via CSP ping (CMP echo).
 *
 * Transport-level liveness check, independent of bundle traffic. Any libcsp
 * node running the standard service handler answers automatically, so this is
 * interoperable across CSPCL implementations.
 *
 * @param cspcl       Initialized CSPCL instance.
 * @param dest_addr   Destination CSP node address.
 * @param timeout_ms  Time to wait for the echo reply, in milliseconds.
 * @return CSPCL_OK if the node replied; CSPCL_ERR_TIMEOUT if it did not in
 *         time; CSPCL_ERR_INVALID_PARAM / CSPCL_ERR_NOT_INITIALIZED on misuse.
 */
cspcl_error_t cspcl_ping(cspcl_t *cspcl, uint8_t dest_addr, uint32_t timeout_ms);
```

- [ ] **Step 5: Implement `cspcl_ping` in `src/cspcl.c`**

After the `cspcl_send_bundle` function body (~line 530). `csp.h` (with `CSP_O_NONE` via `csp_types.h`) is already included at the top of the file.

```c
cspcl_error_t cspcl_ping(cspcl_t *cspcl, uint8_t dest_addr, uint32_t timeout_ms)
{
  if (cspcl == NULL) {
    return CSPCL_ERR_INVALID_PARAM;
  }

  if (!cspcl->initialized) {
    return CSPCL_ERR_NOT_INITIALIZED;
  }

  /* csp_ping returns the round-trip time in ms (>= 0) on success, < 0 on
   * failure/timeout. A 1-byte payload is enough to confirm the node answers. */
  int rtt = csp_ping(dest_addr, timeout_ms, 1, CSP_O_NONE);
  if (rtt < 0) {
    return CSPCL_ERR_TIMEOUT;
  }

  return CSPCL_OK;
}
```

- [ ] **Step 6: Build and run tests to verify they pass**

```bash
cd /home/hugo/code/cspcl/cspcl-server
cmake --build build-stubs 2>&1 | tail -5
ctest --test-dir build-stubs -R test_cspcl --output-on-failure
```
Expected: PASS — output includes `[PASS] test_ping_null_param`, `test_ping_not_initialized`, `test_ping_reachable`, `test_ping_unreachable`, and `Failed: 0`.

- [ ] **Step 7: Commit**

```bash
cd /home/hugo/code/cspcl/cspcl-server
git add src/cspcl.h src/cspcl.c stubs/csp_stub.c tests/test_cspcl.c
git commit -m "feat(cspcl): add cspcl_ping() liveness probe wrapping csp_ping

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Rust FFI binding — `Cspcl::ping`

**Files:**
- Modify: `rust-bindings/cspcl-sys/src/primitive.rs` (add `ping_ptr`; extend `use`)
- Modify: `rust-bindings/cspcl-sys/src/types.rs` (add `ping_ptr` returning `Result<()>`)
- Modify: `rust-bindings/cspcl/src/instance.rs` (add `Cspcl::ping`)
- Create: `rust-bindings/cspcl/tests/ping.rs` (integration test, own process)

**Interfaces:**
- Consumes: `cspcl_sys::cspcl_ping` (bindgen-generated from Task 1); `cspcl::error::{Error, Result}`.
- Produces (Rust): `cspcl_sys::primitive::ping_ptr(cspcl: *mut cspcl_t, dest_addr: u8, timeout_ms: u32) -> cspcl_error_t`; `cspcl_sys::types::ping_ptr(...) -> Result<()>`; `cspcl::Cspcl::ping(&self, dest_addr: u8, timeout: std::time::Duration) -> Result<()>`.

- [ ] **Step 1: Write the failing integration test** — create `rust-bindings/cspcl/tests/ping.rs`

Runs as its own process (clean global CSP state). A loopback instance pinging a non-existent node returns `Err(Timeout)` (the stub-free real build routes nowhere → `csp_ping` fails).

```rust
use std::time::Duration;

use cspcl_bindings::{Cspcl, CspAddress, Error, Interface};

#[test]
fn ping_unreachable_node_times_out() {
    let cspcl = Cspcl::new(
        CspAddress { addr: 1, port: 10 },
        Interface::Loopback,
    )
    .expect("init loopback cspcl");

    // Address 42 has no route on the loopback interface -> no reply.
    let result = cspcl.ping(42, Duration::from_millis(200));
    assert!(
        matches!(result, Err(Error::Timeout)),
        "expected Err(Timeout), got {result:?}"
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cd /home/hugo/code/cspcl/cspcl-server/rust-bindings
CSP_REPO_DIR=/home/hugo/code/libcsp cargo test -p cspcl --test ping 2>&1 | tail -20
```
Expected: **compile failure** — `no method named 'ping' found for struct 'Cspcl'`.

- [ ] **Step 3: Add the raw FFI wrapper** in `rust-bindings/cspcl-sys/src/primitive.rs`

Add `cspcl_ping` to the `use crate::{...}` list at the top, then add:

```rust
pub unsafe fn ping_ptr(cspcl: *mut cspcl_t, dest_addr: u8, timeout_ms: u32) -> cspcl_error_t {
    unsafe { cspcl_ping(cspcl, dest_addr, timeout_ms) }
}
```

- [ ] **Step 4: Add the checked wrapper** in `rust-bindings/cspcl-sys/src/types.rs`

After `send_bundle_ptr` (~line 143):

```rust
pub unsafe fn ping_ptr(cspcl: *mut cspcl_t, dest_addr: u8, timeout_ms: u32) -> Result<()> {
    ok_or_err(unsafe { primitive::ping_ptr(cspcl, dest_addr, timeout_ms) })
}
```

- [ ] **Step 5: Add `Cspcl::ping`** in `rust-bindings/cspcl/src/instance.rs`

Inside `impl Cspcl` (e.g. after `close_rx_socket`). Mirrors `send_bundle`'s lock-release-before-FFI pattern so inbound accept keeps polling.

```rust
    /// Probe reachability of a remote CSP node (transport-level liveness).
    ///
    /// Returns `Ok(())` if the node replied within `timeout`, `Err(Error::Timeout)`
    /// if not. Blocking — call from a blocking context (e.g. `spawn_blocking`).
    pub fn ping(&self, dest_addr: u8, timeout: std::time::Duration) -> Result<()> {
        let inner_guard = self.inner();
        let inner = (&*inner_guard as *const cspcl_sys::cspcl_t).cast_mut();
        drop(inner_guard);

        let timeout_ms = timeout.as_millis().min(u32::MAX as u128) as u32;
        unsafe { cspcl_sys::types::ping_ptr(inner, dest_addr, timeout_ms) }.map_err(Error::from_raw)
    }
```

- [ ] **Step 6: Run the test to verify it passes**

```bash
cd /home/hugo/code/cspcl/cspcl-server/rust-bindings
CSP_REPO_DIR=/home/hugo/code/libcsp cargo test -p cspcl --test ping 2>&1 | tail -20
```
Expected: PASS — `test ping_unreachable_node_times_out ... ok`.

- [ ] **Step 7: (R1 gate) Manually smoke-test ping against a real remote**

Per the design's highest-priority risk, confirm the actual remote answers CSP ping **before** building the CLA layer. With the real remote CSPCL node running, write a throwaway `main` or extend the test to `cspcl.ping(REMOTE_ADDR, Duration::from_secs(1))` and confirm `Ok(())`. If the remote does **not** answer ping, STOP and revisit the design (the recovery loop cannot work). Document the result in the PR.

- [ ] **Step 8: Commit**

```bash
cd /home/hugo/code/cspcl/cspcl-server
git add rust-bindings/cspcl-sys/src/primitive.rs rust-bindings/cspcl-sys/src/types.rs \
        rust-bindings/cspcl/src/instance.rs rust-bindings/cspcl/tests/ping.rs
git commit -m "feat(cspcl-bindings): expose Cspcl::ping over cspcl_ping FFI

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: CLA config — heartbeat interval + tunables

**Files:**
- Modify: `rust-bindings/hardy-cspcl/src/config.rs` (`PeerConfig` field + `FromStr`; `Config` fields + `Default`; tests)

**Interfaces:**
- Produces: `PeerConfig.heartbeat_interval: Option<u32>`; `Config.failure_threshold: u32`; `Config.ping_timeout_ms: u32`; `Config.default_heartbeat_interval_s: u32`.

- [ ] **Step 1: Write the failing tests** — extend the `tests` module in `config.rs`

Update the existing `parses_repeated_peer_arguments` test to assert the new field, and add a malformed-interval test:

```rust
    #[test]
    fn parses_heartbeat_interval() {
        let config = Config::parse_from([
            "hardy-cspcl",
            "--interface", "loopback",
            "--interface-name", "loopback",
            "--peer", "ipn:2.0,2,0",
            "--peer", "ipn:3.0,3,1,60",
        ]);

        assert_eq!(config.peers[0].heartbeat_interval, None);
        assert_eq!(config.peers[1].heartbeat_interval, Some(60));
    }

    #[test]
    fn rejects_malformed_heartbeat_interval() {
        let err = "ipn:2.0,2,0,abc".parse::<PeerConfig>().unwrap_err();
        assert!(err.contains("heartbeat interval"), "unexpected error: {err}");
    }

    #[test]
    fn config_has_liveness_defaults() {
        let config = Config::default();
        assert_eq!(config.failure_threshold, 3);
        assert_eq!(config.ping_timeout_ms, 1000);
        assert_eq!(config.default_heartbeat_interval_s, 5);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /home/hugo/code/cspcl/cspcl-server/rust-bindings
cargo test -p hardy-cspcl config:: 2>&1 | tail -20
```
Expected: compile failure — `no field 'heartbeat_interval' on type 'PeerConfig'` / `no field 'failure_threshold'`.

- [ ] **Step 3: Add the `PeerConfig` field**

In `config.rs`, add to the struct (after `port`):

```rust
    pub heartbeat_interval: Option<u32>,
```

Add to the `Default for PeerConfig` impl:

```rust
            heartbeat_interval: None,
```

- [ ] **Step 4: Parse the optional 4th CSV field** in `impl FromStr for PeerConfig`

Replace the trailing `if parts.next().is_some() { ... }` block and the `Ok(Self { ... })` return with:

```rust
        let heartbeat_interval = match parts.next() {
            Some(s) => Some(
                s.parse()
                    .map_err(|err| format!("invalid heartbeat interval: {err}"))?,
            ),
            None => None,
        };

        if parts.next().is_some() {
            return Err("expected NODE_ID,ADDR,PORT[,HEARTBEAT_INTERVAL]".to_string());
        }

        Ok(Self {
            node_id,
            addr,
            port,
            heartbeat_interval,
        })
```

- [ ] **Step 5: Add the `Config` tunables (flatten-safe defaults)**

`hardy-cspcl-server` deserializes this struct with `#[serde(flatten)]` (see Task 6), where container-level `#[serde(default)]` is unreliable — so give each new field a **field-level** serde default. In the `Config` struct (after `peers`):

```rust
    #[arg(long, default_value = "3")]
    #[cfg_attr(feature = "serde", serde(default = "default_failure_threshold"))]
    pub failure_threshold: u32,
    #[arg(long, default_value = "1000")]
    #[cfg_attr(feature = "serde", serde(default = "default_ping_timeout_ms"))]
    pub ping_timeout_ms: u32,
    #[arg(long, default_value = "5")]
    #[cfg_attr(feature = "serde", serde(default = "default_heartbeat_interval_s"))]
    pub default_heartbeat_interval_s: u32,
```

Add the default helper functions (gated so they don't warn when `serde` is off), e.g. just above the `Config` struct:

```rust
#[cfg(feature = "serde")]
fn default_failure_threshold() -> u32 {
    3
}
#[cfg(feature = "serde")]
fn default_ping_timeout_ms() -> u32 {
    1000
}
#[cfg(feature = "serde")]
fn default_heartbeat_interval_s() -> u32 {
    5
}
```

In `impl Default for Config` (after `peers: Vec::new()`):

```rust
            failure_threshold: 3,
            ping_timeout_ms: 1000,
            default_heartbeat_interval_s: 5,
```

- [ ] **Step 6: Run tests to verify they pass**

```bash
cd /home/hugo/code/cspcl/cspcl-server/rust-bindings
cargo test -p hardy-cspcl config:: 2>&1 | tail -20
```
Expected: PASS — `parses_heartbeat_interval`, `rejects_malformed_heartbeat_interval`, `config_has_liveness_defaults`, and the existing peer-parsing test all ok.

- [ ] **Step 7: Commit**

```bash
cd /home/hugo/code/cspcl/cspcl-server
git add rust-bindings/hardy-cspcl/src/config.rs
git commit -m "feat(hardy-cspcl): parse HEARTBEAT_INTERVAL + liveness tunables

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: Liveness state machine

**Files:**
- Create: `rust-bindings/hardy-cspcl/src/liveness.rs`
- Modify: `rust-bindings/hardy-cspcl/src/lib.rs` (add `mod liveness;`)

**Interfaces:**
- Produces: `hardy_cspcl::liveness::PeerLiveness` with `new()`, `on_send_success()`, `on_send_failure(threshold: u32) -> bool` (true exactly once, on the call that wins the threshold-crossing race), `reset()`, and `is_recovering() -> bool`.

- [ ] **Step 1: Write the failing tests** — create `rust-bindings/hardy-cspcl/src/liveness.rs` with only the state machine's tests first (add the struct in Step 3):

```rust
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

// (PeerLiveness impl added in Step 3)

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn stays_up_below_threshold() {
        let p = PeerLiveness::new();
        assert!(!p.on_send_failure(3)); // 1
        assert!(!p.on_send_failure(3)); // 2
        assert!(!p.is_recovering());
    }

    #[test]
    fn crosses_threshold_exactly_once() {
        let p = PeerLiveness::new();
        assert!(!p.on_send_failure(3)); // 1
        assert!(!p.on_send_failure(3)); // 2
        assert!(p.on_send_failure(3)); //  3 -> true, wins recovery
        assert!(p.is_recovering());
        // Already recovering: further failures never re-trigger.
        assert!(!p.on_send_failure(3));
    }

    #[test]
    fn success_resets_failure_count() {
        let p = PeerLiveness::new();
        p.on_send_failure(3);
        p.on_send_failure(3);
        p.on_send_success();
        assert!(!p.on_send_failure(3)); // count restarted at 1
        assert!(!p.is_recovering());
    }

    #[test]
    fn reset_returns_to_up() {
        let p = PeerLiveness::new();
        assert!(p.on_send_failure(1)); // threshold 1 -> immediate
        assert!(p.is_recovering());
        p.reset();
        assert!(!p.is_recovering());
        // Fresh cycle possible after reset.
        assert!(p.on_send_failure(1));
    }

    #[test]
    fn only_one_thread_wins_the_threshold() {
        let p = Arc::new(PeerLiveness::new());
        // Prime to threshold-1 so the next failure crosses.
        p.on_send_failure(2);
        let wins = Arc::new(AtomicU32::new(0));

        let mut handles = Vec::new();
        for _ in 0..16 {
            let p = p.clone();
            let wins = wins.clone();
            handles.push(std::thread::spawn(move || {
                if p.on_send_failure(2) {
                    wins.fetch_add(1, Ordering::SeqCst);
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(wins.load(Ordering::SeqCst), 1);
    }
}
```

Add `mod liveness;` to `rust-bindings/hardy-cspcl/src/lib.rs` (near the other `mod` lines at the top).

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /home/hugo/code/cspcl/cspcl-server/rust-bindings
cargo test -p hardy-cspcl liveness:: 2>&1 | tail -20
```
Expected: compile failure — `cannot find type 'PeerLiveness' in this scope`.

- [ ] **Step 3: Implement `PeerLiveness`** — add above the `#[cfg(test)]` module in `liveness.rs`:

```rust
/// Per-peer liveness state. Lock-free; safe under concurrent `forward()` calls.
pub struct PeerLiveness {
    consecutive_failures: AtomicU32,
    recovering: AtomicBool,
}

impl PeerLiveness {
    pub fn new() -> Self {
        Self {
            consecutive_failures: AtomicU32::new(0),
            recovering: AtomicBool::new(false),
        }
    }

    /// A send succeeded: clear the failure streak. Leaves `recovering` untouched.
    pub fn on_send_success(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
    }

    /// A send failed. Returns `true` exactly once — on the call that both
    /// reaches `threshold` and wins the race to flip `recovering` false->true.
    /// The winner is responsible for starting the recovery task.
    pub fn on_send_failure(&self, threshold: u32) -> bool {
        let n = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
        if n >= threshold {
            self.recovering
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        } else {
            false
        }
    }

    /// Recovery finished: return to the Up state.
    pub fn reset(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
        self.recovering.store(false, Ordering::Release);
    }

    pub fn is_recovering(&self) -> bool {
        self.recovering.load(Ordering::Acquire)
    }
}

impl Default for PeerLiveness {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd /home/hugo/code/cspcl/cspcl-server/rust-bindings
cargo test -p hardy-cspcl liveness:: 2>&1 | tail -20
```
Expected: PASS — all five liveness tests ok.

- [ ] **Step 5: Commit**

```bash
cd /home/hugo/code/cspcl/cspcl-server
git add rust-bindings/hardy-cspcl/src/liveness.rs rust-bindings/hardy-cspcl/src/lib.rs
git commit -m "feat(hardy-cspcl): add PeerLiveness state machine

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Recovery orchestration + wire into the CLA

**Files:**
- Modify: `rust-bindings/hardy-cspcl/src/transport.rs` (add `Transport::ping` + `Error::Ping`)
- Modify: `rust-bindings/hardy-cspcl/src/liveness.rs` (add `run_recovery` + orchestration test)
- Modify: `rust-bindings/hardy-cspcl/src/lib.rs` (`PeerRuntime`, `Cla` fields, build `peers` map, `spawn_recovery`)
- Modify: `rust-bindings/hardy-cspcl/src/cla.rs` (liveness hooks in `forward`)
- Modify: `rust-bindings/hardy-cspcl/Cargo.toml` (ensure tokio `rt`/`macros` features)

**Interfaces:**
- Consumes: `Cspcl::ping` (Task 2); `PeerLiveness` (Task 4); `Config` fields (Task 3); hardy `cla::Sink::{add_peer, remove_peer}`.
- Produces: `Transport::ping(&self, dest_addr: u8, timeout: Duration) -> Result<(), Error>`; `hardy_cspcl::liveness::run_recovery<P, Fut>(sink: Arc<dyn cla::Sink>, cla_addr: cla::ClaAddress, node_ids: Vec<NodeId>, liveness: Arc<PeerLiveness>, heartbeat: Duration, cancel: hardy_async::CancellationToken, probe: P)` where `P: Fn() -> Fut + Send + Sync`, `Fut: Future<Output = bool> + Send`; `Cla::spawn_recovery(&self, csp_addr: CspAddress, peer: &PeerRuntime)`.

- [ ] **Step 1: Write the failing orchestration test** — add a second test module to `liveness.rs`:

```rust
#[cfg(test)]
mod recovery_tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    use bytes::Bytes;
    use hardy_async::async_trait;
    use hardy_bpa::cla::{self, ClaAddress};

    #[derive(Default)]
    struct MockSink {
        removed: AtomicU32,
        added: AtomicU32,
    }

    #[async_trait]
    impl cla::Sink for MockSink {
        async fn unregister(&self) {}
        async fn dispatch(
            &self,
            _bundle: Bytes,
            _peer_node: Option<&hardy_bpv7::eid::NodeId>,
            _peer_addr: Option<&ClaAddress>,
        ) -> cla::Result<()> {
            Ok(())
        }
        async fn add_peer(
            &self,
            _cla_addr: ClaAddress,
            _node_ids: &[hardy_bpv7::eid::NodeId],
        ) -> cla::Result<bool> {
            self.added.fetch_add(1, Ordering::SeqCst);
            Ok(true)
        }
        async fn remove_peer(&self, _cla_addr: &ClaAddress) -> cla::Result<bool> {
            self.removed.fetch_add(1, Ordering::SeqCst);
            Ok(true)
        }
    }

    #[tokio::test]
    async fn recovery_removes_probes_then_readds_and_resets() {
        let sink = Arc::new(MockSink::default());
        let liveness = Arc::new(PeerLiveness::new());
        // Simulate forward() having claimed recovery.
        assert!(liveness.on_send_failure(1));
        assert!(liveness.is_recovering());

        let cla_addr = ClaAddress::Private(Bytes::from_static(&[2, 0]));
        let node_ids = vec!["ipn:2.0".parse().unwrap()];
        let cancel = hardy_async::CancellationToken::new();

        // probe: false twice, then true (peer recovers on 3rd attempt).
        let calls = Arc::new(AtomicU32::new(0));
        let calls_probe = calls.clone();
        let probe = move || {
            let calls = calls_probe.clone();
            async move { calls.fetch_add(1, Ordering::SeqCst) >= 2 }
        };

        run_recovery(
            sink.clone(),
            cla_addr,
            node_ids,
            liveness.clone(),
            Duration::from_millis(1),
            cancel,
            probe,
        )
        .await;

        assert_eq!(sink.removed.load(Ordering::SeqCst), 1);
        assert_eq!(sink.added.load(Ordering::SeqCst), 1);
        assert!(calls.load(Ordering::SeqCst) >= 3);
        assert!(!liveness.is_recovering());
    }

    #[tokio::test]
    async fn recovery_cancelled_does_not_readd() {
        let sink = Arc::new(MockSink::default());
        let liveness = Arc::new(PeerLiveness::new());
        assert!(liveness.on_send_failure(1));
        let cla_addr = ClaAddress::Private(Bytes::from_static(&[2, 0]));
        let node_ids = vec!["ipn:2.0".parse().unwrap()];
        let cancel = hardy_async::CancellationToken::new();
        cancel.cancel(); // cancelled before probing starts

        run_recovery(
            sink.clone(),
            cla_addr,
            node_ids,
            liveness.clone(),
            Duration::from_millis(1),
            cancel,
            move || async { false }, // never succeeds
        )
        .await;

        assert_eq!(sink.removed.load(Ordering::SeqCst), 1);
        assert_eq!(sink.added.load(Ordering::SeqCst), 0); // never re-added
    }
}
```

- [ ] **Step 2: Ensure tokio test features** — in `rust-bindings/hardy-cspcl/Cargo.toml`, set the tokio dependency features to include `rt` and `macros` (needed for `#[tokio::test]`, `tokio::spawn`, `spawn_blocking`):

```toml
tokio = { version = "1.51.1", features = ["rt", "macros", "time"] }
```

Run to verify the new tests fail:

```bash
cd /home/hugo/code/cspcl/cspcl-server/rust-bindings
cargo test -p hardy-cspcl recovery_tests:: 2>&1 | tail -20
```
Expected: compile failure — `cannot find function 'run_recovery' in this scope`.

- [ ] **Step 3: Implement `run_recovery`** — add to `liveness.rs` (above the test modules). Add the imports at the top of the file:

```rust
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use hardy_async::CancellationToken;
use hardy_bpa::cla::{self, ClaAddress};
use hardy_bpv7::eid::NodeId;
use tracing::{info, warn};
```

```rust
/// Drive one peer's down->up recovery: drop the route, probe until the node
/// answers, then re-add the route (which triggers hardy's poll_waiting re-drive
/// of parked Waiting bundles). Runs on its own task; honors `cancel` for
/// shutdown. `probe` returns `true` once the peer is reachable.
pub async fn run_recovery<P, Fut>(
    sink: Arc<dyn cla::Sink>,
    cla_addr: ClaAddress,
    node_ids: Vec<NodeId>,
    liveness: Arc<PeerLiveness>,
    heartbeat: Duration,
    cancel: CancellationToken,
    probe: P,
) where
    P: Fn() -> Fut + Send + Sync,
    Fut: Future<Output = bool> + Send,
{
    // 1. Drop the route: hardy resets this peer's ForwardPending -> Waiting.
    if let Err(e) = sink.remove_peer(&cla_addr).await {
        warn!("remove_peer failed for {cla_addr}: {e}");
    }

    // 2. Probe until the node answers, or we are cancelled.
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("Recovery for {cla_addr} cancelled; leaving bundles Waiting");
                return;
            }
            _ = tokio::time::sleep(heartbeat) => {}
        }
        if probe().await {
            break;
        }
    }

    // 3. Re-add the route -> hardy notify_updated -> poll_waiting re-drives.
    //    Bounded retries: a false/Err here would strand bundles, so don't
    //    silently no-op.
    let mut attempts = 0u32;
    loop {
        match sink.add_peer(cla_addr.clone(), &node_ids).await {
            Ok(_) => break,
            Err(e) => {
                attempts += 1;
                warn!("add_peer failed (attempt {attempts}) for {cla_addr}: {e}");
                if attempts >= 3 {
                    break;
                }
                tokio::time::sleep(heartbeat).await;
            }
        }
    }

    // 4. Back to Up.
    liveness.reset();
    info!("Peer {cla_addr} recovered; route re-added, Waiting bundles re-driven");
}
```

- [ ] **Step 4: Run the orchestration tests to verify they pass**

```bash
cd /home/hugo/code/cspcl/cspcl-server/rust-bindings
cargo test -p hardy-cspcl recovery_tests:: 2>&1 | tail -20
```
Expected: PASS — `recovery_removes_probes_then_readds_and_resets` and `recovery_cancelled_does_not_readd` ok.

- [ ] **Step 5: Add `Transport::ping`** in `rust-bindings/hardy-cspcl/src/transport.rs`

Add `use std::time::Duration;` at the top. Add a variant to the `Error` enum:

```rust
    #[error("transport ping failed: {0}")]
    Ping(#[source] CspclError),
```

Add the method to `impl Transport` (blocking; callers wrap in `spawn_blocking`):

```rust
    pub fn ping(&self, dest_addr: u8, timeout: Duration) -> Result<(), Error> {
        self.cspcl.ping(dest_addr, timeout).map_err(Error::Ping)
    }
```

- [ ] **Step 6: Add `PeerRuntime`, `Cla` fields, and build the map** in `rust-bindings/hardy-cspcl/src/lib.rs`

Add imports: `use std::time::Duration;` and `use hardy_bpv7::eid::NodeId;` (NodeId is already imported). Add the struct and extend `Cla`:

```rust
struct PeerRuntime {
    liveness: Arc<crate::liveness::PeerLiveness>,
    heartbeat: Duration,
    node_id: NodeId,
}

pub struct Cla {
    csp_to_endpoint: HashMap<CspAddress, NodeId>,
    peers: HashMap<CspAddress, PeerRuntime>,
    transport: transport::Transport,
    cancel_dispatcher: CancellationToken,
    sink: Once<Arc<dyn Sink>>,
    failure_threshold: u32,
    ping_timeout: Duration,
}
```

In `Cla::new`, build `peers` alongside the existing `csp_to_endpoint` loop and populate the new fields in the returned `Self`:

```rust
        let mut peers = HashMap::<CspAddress, PeerRuntime>::new();
        for peer in &config.peers {
            let csp_address = CspAddress {
                addr: peer.addr,
                port: peer.port,
            };
            let heartbeat = Duration::from_secs(
                peer.heartbeat_interval
                    .unwrap_or(config.default_heartbeat_interval_s) as u64,
            );
            peers.insert(
                csp_address,
                PeerRuntime {
                    liveness: Arc::new(crate::liveness::PeerLiveness::new()),
                    heartbeat,
                    node_id: peer.node_id.clone(),
                },
            );
        }

        Ok(Self {
            csp_to_endpoint,
            peers,
            transport,
            cancel_dispatcher: CancellationToken::new(),
            sink: Once::new(),
            failure_threshold: config.failure_threshold,
            ping_timeout: Duration::from_millis(config.ping_timeout_ms as u64),
        })
```

Add the `spawn_recovery` helper to `impl Cla` (add `info` to the `tracing` import):

```rust
    fn spawn_recovery(&self, csp_addr: CspAddress, peer: &PeerRuntime) {
        let Some(sink) = self.sink.get().cloned() else {
            warn!(
                "Cannot start recovery for {}:{}: sink not registered",
                csp_addr.addr, csp_addr.port
            );
            return;
        };

        let transport = self.transport.clone();
        let liveness = peer.liveness.clone();
        let heartbeat = peer.heartbeat;
        let node_ids = vec![peer.node_id.clone()];
        let cla_addr = ClaAddress::Private(Into::<Bytes>::into(csp_addr));
        let ping_timeout = self.ping_timeout;
        let dest_addr = csp_addr.addr;
        let cancel = self.cancel_dispatcher.child_token();

        info!(
            "Peer {}:{} down after {} failures; starting recovery probe",
            csp_addr.addr, csp_addr.port, self.failure_threshold
        );

        tokio::spawn(async move {
            crate::liveness::run_recovery(
                sink,
                cla_addr,
                node_ids,
                liveness,
                heartbeat,
                cancel,
                move || {
                    let transport = transport.clone();
                    async move {
                        tokio::task::spawn_blocking(move || {
                            transport.ping(dest_addr, ping_timeout).is_ok()
                        })
                        .await
                        .unwrap_or(false)
                    }
                },
            )
            .await;
        });
    }
```

- [ ] **Step 7: Wire liveness into `forward`** in `rust-bindings/hardy-cspcl/src/cla.rs`

Keep the existing first line of `forward` (the `let ClaAddress::Private(raw_addr) = cla_addr else { return Ok(ForwardBundleResult::NoNeighbour); };` guard). Replace **everything after that guard** (the `let csp_addr = …` line and the `match self.transport.send_bundle(...)` block) with a version that records liveness for configured peers:

```rust
        let csp_addr = CspAddress::try_from(raw_addr.clone())
            .map_err(|e| cla::Error::Internal(Box::new(e)))?;

        let result = self.transport.send_bundle(bundle, csp_addr).await;

        if let Some(peer) = self.peers.get(&csp_addr) {
            match &result {
                Ok(_) => peer.liveness.on_send_success(),
                Err(_) => {
                    if peer.liveness.on_send_failure(self.failure_threshold) {
                        self.spawn_recovery(csp_addr, peer);
                    }
                }
            }
        }

        match result {
            Ok(_) => Ok(ForwardBundleResult::Sent),
            Err(e) => {
                warn!(
                    "Failed to send CSP bundle to {}:{}: {e}",
                    csp_addr.addr, csp_addr.port
                );
                Err(cla::Error::Internal(Box::new(e)))
            }
        }
```

- [ ] **Step 8: Build and run the full CLA test suite**

```bash
cd /home/hugo/code/cspcl/cspcl-server/rust-bindings
cargo test -p hardy-cspcl 2>&1 | tail -25
```
Expected: PASS — all `liveness::`, `recovery_tests::`, and `config::` tests ok; crate builds with no warnings about unused fields.

- [ ] **Step 9: Commit**

```bash
cd /home/hugo/code/cspcl/cspcl-server
git add rust-bindings/hardy-cspcl/src/transport.rs rust-bindings/hardy-cspcl/src/liveness.rs \
        rust-bindings/hardy-cspcl/src/lib.rs rust-bindings/hardy-cspcl/src/cla.rs \
        rust-bindings/hardy-cspcl/Cargo.toml
git commit -m "feat(hardy-cspcl): detect peer down and resend on recovery

On N consecutive send failures, remove the peer route and start a
csp_ping recovery probe; on recovery re-add the peer, triggering hardy
poll_waiting to re-drive parked Waiting bundles.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: `hardy-cspcl-server` — verify flatten-safe config defaults

The server crate deserializes `hardy_cspcl::Config` from YAML via `serde_saphyr` with `#[serde(flatten)]` (`hardy-cspcl-server/src/config.rs:21-22`). Prove that a YAML file omitting the new tunables still yields the intended defaults (not `0`), and that a peer without a heartbeat gets `None`. No production code changes are expected here — `create_cla` already passes the whole `Config` to `Cla::new` — this task is the cross-crate safety net for Task 3's defaults.

**Files:**
- Modify: `rust-bindings/hardy-cspcl-server/src/config.rs` (add a test)

**Interfaces:**
- Consumes: `hardy_cspcl::Config` fields `failure_threshold`, `ping_timeout_ms`, `default_heartbeat_interval_s` (Task 3); `load_server_config`.

- [ ] **Step 1: Write the failing test** in the `tests` module of `hardy-cspcl-server/src/config.rs`

```rust
    #[test]
    fn liveness_tunables_default_when_absent_from_yaml() {
        let path = std::env::temp_dir().join(format!(
            "hardy-cspcl-server-defaults-{}.yaml",
            std::process::id()
        ));
        fs::write(
            &path,
            "\
local-addr: 7
port: 9
interface: loopback
interface-name: loopback
peers:
  - node-id: ipn:2.0
    addr: 2
    port: 1
",
        )
        .expect("write test config");

        let config = load_server_config(&path).expect("load server config");
        let _ = fs::remove_file(&path);

        // Omitted tunables must fall back to the field-level serde defaults,
        // NOT u32::default() (0), which flatten + container-default would give.
        assert_eq!(config.cspcl_config.failure_threshold, 3);
        assert_eq!(config.cspcl_config.ping_timeout_ms, 1000);
        assert_eq!(config.cspcl_config.default_heartbeat_interval_s, 5);
        assert_eq!(config.cspcl_config.peers[0].heartbeat_interval, None);
    }
```

- [ ] **Step 2: Run the test**

```bash
cd /home/hugo/code/cspcl/cspcl-server/rust-bindings
cargo test -p hardy-cspcl-server config:: 2>&1 | tail -20
```
Expected outcomes:
- **PASS** → Task 3's field-level defaults flow correctly through `flatten`; done.
- **FAIL** with values of `0` → `serde_saphyr` is not applying the field-level defaults under `flatten`. Fix in `hardy-cspcl/src/config.rs`: keep the field-level `#[serde(default = "...")]` (already added in Task 3) and, if still failing, wrap the fields so a missing key is unambiguous — change the loader to deserialize into an intermediate `Option` is overkill; instead add `#[serde(default)]` retained at container level *and* confirm the helper fns are reachable (feature `serde` enabled for the `hardy-cspcl` dependency of `hardy-cspcl-server`). Re-run until green.

- [ ] **Step 3: Commit**

```bash
cd /home/hugo/code/cspcl/cspcl-server
git add rust-bindings/hardy-cspcl-server/src/config.rs
git commit -m "test(hardy-cspcl-server): verify liveness config defaults via flatten

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: End-to-end verification (2-node)

This is the decisive real-world test — it reproduces and closes the original bug and validates risks R2/R3. It uses the real build (`CSP_REPO_DIR`) and the existing two-node configs (`rust-bindings/config-1.yaml`, `config-2.yaml`).

**Files:**
- Reference: `rust-bindings/config-1.yaml`, `rust-bindings/config-2.yaml`
- No source changes (verification only). If a repeatable script is desired, create `rust-bindings/scripts/verify-liveness.sh`.

- [ ] **Step 1: Build the real (non-stub) binaries**

```bash
cd /home/hugo/code/cspcl/cspcl-server/rust-bindings
CSP_REPO_DIR=/home/hugo/code/libcsp cargo build -p hardy-cspcl-server 2>&1 | tail -5
```
Expected: builds successfully.

- [ ] **Step 2: Reproduce the bug scenario and verify the fix**

1. Start node 1 (hardy BPA + hardy-cspcl CLA) with `config-1.yaml`, peer 2 configured with a short heartbeat (e.g. `ipn:2.0,2,0,2`) and `--failure-threshold 3 --ping-timeout-ms 1000`.
2. With node 2 **down**, send a bundle destined for `ipn:2.x`.
3. Observe node-1 logs: after 3 failures, `Peer 2:0 down after 3 failures; starting recovery probe`. Enable `RUST_LOG=debug` to also see hardy's `reset_peer_queue`/route removal for peer 2.
4. Start node 2 with `config-2.yaml`.
5. **Verify:** within ~one heartbeat interval, node-1 logs `Peer ...:... recovered; route re-added, Waiting bundles re-driven`, and node 2 receives the bundle (node-2 `New bundle in inbound stream` / `Dispatched inbound CSP bundle`).

Expected: the bundle that was parked while node 2 was down is delivered once node 2 returns — the original bug is fixed.

- [ ] **Step 3: Confirm risks R2/R3**

- R2 (ping thread-safety): during Step 2, keep a steady trickle of bundles to a *live* peer while another peer is in its probe loop; confirm no hangs/panics (ping runs on `spawn_blocking`, separate from RX/TX).
- R3 (`add_peer` reaches `poll_waiting`): confirmed by the bundle actually being delivered in Step 2.5 rather than staying `Waiting`.

- [ ] **Step 4: Record results in the PR**

Note the observed logs (down detection, recovery, delivery) and the R1/R2/R3 outcomes in the PR description. No commit unless a `verify-liveness.sh` script was added, in which case:

```bash
cd /home/hugo/code/cspcl/cspcl-server
git add rust-bindings/scripts/verify-liveness.sh
git commit -m "test(hardy-cspcl): script for 2-node liveness/resend verification

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Notes for the implementer

- **Task order matters:** 1 → 2 gate the C/FFI layer; 3, 4 are independent leaves; 5 depends on 2/3/4; 6 depends on 3 (cross-crate config safety net); 7 (e2e) depends on all. Do not start Task 5 until R1 (Task 2 Step 7) is confirmed — if the remote doesn't answer `csp_ping`, the whole approach must be revisited.
- **Other projects checked:** `~/code/hardy` needs **no changes** — the design uses the existing `Sink::add_peer`/`remove_peer`, which already trigger `reset_peer_queue` + `notify_updated` → `poll_waiting` (verified: `rib.rs:237-252,292-296`). The only cross-crate work is Task 6 (`hardy-cspcl-server` config defaulting). The C-library change (`cspcl_ping`) is purely additive and does not affect other CLA integrations (uD3TN, UniBO).
- **Timing note:** with the ACK-timeout floor (~10s) and `failure_threshold=3`, declaring a peer down takes ~30s of real send attempts. Tune `--failure-threshold` / the ACK timeout for faster reaction if needed.
- **Do not** call `sink.remove_peer`/`add_peer` synchronously inside `forward()` — that path runs on hardy's egress poller for the peer being mutated. All RIB mutation happens on the spawned recovery task (Task 5).
