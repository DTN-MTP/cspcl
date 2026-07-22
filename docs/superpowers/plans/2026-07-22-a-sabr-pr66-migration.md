# A-SABR PR #66 Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Adapt the `hardy-a-sabr` crate to the rebuilt A-SABR routing/pathfinding API from PR #66, migrating its internal time model from `f64` seconds to `i64` milliseconds.

**Architecture:** Unchanged shape. `compute_first_hop` builds a fresh generativity-guarded `a_sabr::utils::Router` per call (SPSN + hybrid-parenting + SABR distance), routes a representative `Bundle` to one destination node, extracts the first-hop node id, and discards the router. The async `Scheduler` drives these queries at contact boundaries / on a safety tick and installs `RouteAction::Via` routes via the `RoutingSink`.

**Tech Stack:** Rust (edition 2024, nightly toolchain — already in use), `a_sabr` (git dependency), `hardy-bpa`/`hardy-bpv7`, `tokio`, `tracing`.

## Global Constraints

- All A-SABR-facing time/duration values are **i64 milliseconds**; `rate` is **i64 bps**; `size` is **i64** (bundle volume). These replace the previous `f64` seconds model throughout `hardy-a-sabr`.
- `NodeMapping.asabr_node_id` stays `u16`; cast to `a_sabr` `NodeID` (a `usize` wrapper) at the boundary via `(x as usize).into()`.
- `ShadowEngineConfig` carries only `max_entries: usize`. Priority handling is the fixed `PRIO_COUNT = 1` const in the SPSN alias.
- Destination type parameter is `a_sabr::multigraph::RoutableNodeRef` (unicast only; no multicast).
- `a_sabr` is pinned to rev `16ffed523bf862ba770bf1b07adc8ee533066f1a` (A-SABR `main` HEAD, post-merge of PR #66).
- Work happens on the current branch `feat/hardy-sabr-routing`. Commit after every task.
- Node ids are assumed contiguous `0..N`; guarded by `debug_assert!` in `build_contact_plan`.

---

### Task 1: Pin the `a_sabr` dependency to the PR #66 merge

**Files:**
- Modify: `rust-bindings/hardy-a-sabr/Cargo.toml:12`

**Interfaces:**
- Consumes: nothing.
- Produces: a resolvable `a_sabr` dependency exposing the new API (`a_sabr::utils::Router`, `a_sabr::pathfinding::top_level::aliases::SpsnHybridParenting`, `a_sabr::contact_plan::RealNode`, etc.).

- [ ] **Step 1: Update the dependency line**

In `rust-bindings/hardy-a-sabr/Cargo.toml`, change line 12 from:

```toml
a_sabr = { git = "https://github.com/DTN-MTP/A-SABR.git" }
```

to:

```toml
a_sabr = { git = "https://github.com/DTN-MTP/A-SABR.git", rev = "16ffed523bf862ba770bf1b07adc8ee533066f1a" }
```

- [ ] **Step 2: Refresh the lockfile for `a_sabr` only**

Run: `cd rust-bindings && cargo update -p a_sabr --precise 16ffed523bf862ba770bf1b07adc8ee533066f1a`
Expected: Cargo reports updating `a_sabr` from the old rev `1277e360…` to `16ffed52…` and pulls new transitive deps (`generativity`, `itertools`, `ringbuffer`, `replace_with`).

- [ ] **Step 3: Confirm the dependency resolves and the new API is present**

Run: `cd rust-bindings && cargo build -p hardy-a-sabr 2>&1 | head -40`
Expected: The build **fails to compile** — but with errors about the *old* API in our source (e.g. `unresolved import a_sabr::routing`, `a_sabr::vertex`, `no variant lazy_get_for_unicast`, `Bundle` fields `source`/`destinations`). This confirms the new `a_sabr` is wired in and our code is what needs migrating. Errors about `a_sabr` internals failing to compile would instead indicate a toolchain problem — the toolchain is nightly (`rustc --version` shows `1.98.0-nightly`), which satisfies `a_sabr`'s edition-2024 + `Box::new_zeroed_slice` requirements.

- [ ] **Step 4: Commit**

```bash
cd /home/hugo/code/cspcl/cspcl-a-sabr
git add rust-bindings/hardy-a-sabr/Cargo.toml rust-bindings/Cargo.lock
git commit -m "build(hardy-a-sabr): pin a_sabr to PR #66 merge rev"
```

---

### Task 2: Retype the time model to i64 milliseconds

This task is a pure `f64 → i64` retype across the model files. The crate will **not** compile green at the end of this task — `engine.rs` still uses the old API and is fixed in Task 3. The checkpoint here is that the model files have the new types and `cargo build` no longer reports `f64`/`f64`-mismatch errors in these files (only the `engine.rs` API errors remain).

**Files:**
- Modify: `rust-bindings/hardy-a-sabr/src/topology.rs`
- Modify: `rust-bindings/hardy-a-sabr/src/projection.rs:14-33` (the `RepresentativeBundle` struct + its `Default`) and `:46-52` (the `project_routes` signature)
- Modify: `rust-bindings/hardy-a-sabr/src/refresh.rs:36` (`now` param)
- Modify: `rust-bindings/hardy-a-sabr/src/scheduler.rs` (fields + `now`/`next_boundary_delay`)
- Modify: `rust-bindings/hardy-a-sabr/src/router.rs` (field + builder)
- Modify: `rust-bindings/hardy-a-sabr/src/config.rs:11` (`start_time`)

**Interfaces:**
- Produces:
  - `topology::ContactWindow { tx_node_id: u16, rx_node_id: u16, start: i64, end: i64, rate: i64, delay: i64 }`
  - `topology::TopologySnapshot::next_boundary_after(&self, now: i64) -> Option<i64>`
  - `projection::RepresentativeBundle { size: i64, priority: i8, expiration_horizon: i64 }`
  - `projection::project_routes(topology, engine_config, config, source: u16, now: i64) -> Result<Vec<ProjectedRoute>, ASABRError>`
  - `scheduler::Scheduler::now(&self) -> i64`, `start_time: i64`
  - `router::Router::with_start_time(self, start_time: i64) -> Self`, field `start_time: i64`
  - `config::RuntimeConfig::start_time: i64`

- [ ] **Step 1: Retype `ContactWindow` and `next_boundary_after` in `topology.rs`**

Replace the `ContactWindow` struct definition with:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ContactWindow {
    pub tx_node_id: u16,
    pub rx_node_id: u16,
    pub start: i64,
    pub end: i64,
    pub rate: i64,
    pub delay: i64,
}
```

Replace the `next_boundary_after` method body with:

```rust
    pub fn next_boundary_after(&self, now: i64) -> Option<i64> {
        self.contacts
            .iter()
            .flat_map(|contact| [contact.start, contact.end])
            .filter(|boundary| *boundary > now)
            .min()
    }
```

(`NodeMapping` and `hardy_eid_for` are unchanged.)

- [ ] **Step 2: Retype `RepresentativeBundle` and `project_routes` in `projection.rs`**

Replace the `RepresentativeBundle` struct and its `Default` impl with:

```rust
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RepresentativeBundle {
    pub size: i64,
    pub priority: i8,
    pub expiration_horizon: i64,
}

impl Default for RepresentativeBundle {
    fn default() -> Self {
        Self {
            size: 1,
            priority: 0,
            expiration_horizon: 3_600_000,
        }
    }
}
```

Change the `project_routes` signature's `now` parameter from `now: f64` to `now: i64`. The function body is otherwise unchanged.

In `refresh.rs`, change the `refresh_routes` signature's `now` parameter (line 36) from `now: f64` to `now: i64`. The body — which forwards `now` to `project_routes` — is unchanged.

- [ ] **Step 3: Retype the clock in `scheduler.rs`**

In `struct Scheduler`, change the field `start_time: f64` to `start_time: i64`. In `Scheduler::new`, change the parameter `start_time: f64` to `start_time: i64` (the struct-init line is unchanged).

Replace the `now` method with:

```rust
    fn now(&self) -> i64 {
        self.start_time + self.started_at.elapsed().as_millis() as i64
    }
```

Replace `next_boundary_delay` with:

```rust
    fn next_boundary_delay(&self, now: i64) -> Option<Duration> {
        self.topology
            .next_boundary_after(now)
            .map(|boundary| Duration::from_millis((boundary - now).max(0) as u64))
    }
```

(`next_wakeup_delay`, which combines `next_boundary_delay` with `self.safety_tick` via `.min`, is unchanged — both are `Duration`.)

- [ ] **Step 4: Retype `start_time` in `router.rs`**

In `struct Router`, change `pub(crate) start_time: f64` to `pub(crate) start_time: i64`. In `Router::new`, change the initializer `start_time: 0.0` to `start_time: 0`. Change the builder to:

```rust
    pub fn with_start_time(mut self, start_time: i64) -> Self {
        self.start_time = start_time;
        self
    }
```

- [ ] **Step 5: Retype `start_time` in `config.rs`**

In `struct RuntimeConfig`, change `pub start_time: f64` to `pub start_time: i64`. (The `#[serde(default)]` attribute is unchanged; `i64` defaults to `0`.)

- [ ] **Step 6: Confirm the model files type-check (engine errors expected)**

Run: `cd rust-bindings && cargo build -p hardy-a-sabr 2>&1 | grep -E "topology.rs|projection.rs|refresh.rs|scheduler.rs|router.rs|config.rs" | head`
Expected: **No** errors referencing these five files. Remaining errors all point at `engine.rs` (old A-SABR API) — that is fixed in Task 3.

- [ ] **Step 7: Commit**

```bash
cd /home/hugo/code/cspcl/cspcl-a-sabr
git add rust-bindings/hardy-a-sabr/src/topology.rs rust-bindings/hardy-a-sabr/src/projection.rs rust-bindings/hardy-a-sabr/src/refresh.rs rust-bindings/hardy-a-sabr/src/scheduler.rs rust-bindings/hardy-a-sabr/src/router.rs rust-bindings/hardy-a-sabr/src/config.rs
git commit -m "refactor(hardy-a-sabr): migrate time model to i64 milliseconds"
```

---

### Task 3: Rewrite `engine.rs` for the new A-SABR API

This task fixes the remaining compile errors and brings the crate green. It ends with a passing unit test for `compute_first_hop`.

**Files:**
- Modify (full rewrite of contents): `rust-bindings/hardy-a-sabr/src/engine.rs`
- Test: appended `#[cfg(test)] mod tests` in `rust-bindings/hardy-a-sabr/src/engine.rs`

**Interfaces:**
- Consumes: `topology::TopologySnapshot`, `topology::NodeMapping`, `topology::ContactWindow`, `projection::RepresentativeBundle` (from Task 2).
- Produces:
  - `engine::ShadowEngineConfig { max_entries: usize }` (+ `Default` → `10`)
  - `engine::build_contact_plan(&TopologySnapshot) -> Result<ContactPlan<NoManagement, EVLManager>, ASABRError>`
  - `engine::compute_first_hop(topology, config, source: u16, destination: u16, now: i64, representative: &RepresentativeBundle) -> Result<Option<u16>, ASABRError>`

- [ ] **Step 1: Write the failing test**

Append this module to the end of `rust-bindings/hardy-a-sabr/src/engine.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::topology::{ContactWindow, NodeMapping, TopologySnapshot};
    use hardy_bpv7::eid::NodeId;

    fn node(asabr_node_id: u16, eid: &str) -> NodeMapping {
        NodeMapping {
            asabr_node_id,
            hardy_node_id: eid.parse::<NodeId>().expect("valid node id"),
        }
    }

    fn contact(tx: u16, rx: u16, start: i64, end: i64) -> ContactWindow {
        ContactWindow {
            tx_node_id: tx,
            rx_node_id: rx,
            start,
            end,
            rate: 1_000_000,
            delay: 1,
        }
    }

    #[test]
    fn first_hop_follows_linear_plan() {
        // 0 -> 1 -> 2, all contacts open for the whole horizon.
        let topology = TopologySnapshot {
            nodes: vec![node(0, "ipn:0.0"), node(1, "ipn:1.0"), node(2, "ipn:2.0")],
            contacts: vec![contact(0, 1, 0, 100_000), contact(1, 2, 0, 100_000)],
        };
        let config = ShadowEngineConfig::default();
        let representative = RepresentativeBundle::default();

        let hop = compute_first_hop(&topology, &config, 0, 2, 0, &representative)
            .expect("routing succeeds");
        assert_eq!(hop, Some(1));
    }

    #[test]
    fn no_path_returns_none() {
        // 0 -> 1 only; node 2 is unreachable.
        let topology = TopologySnapshot {
            nodes: vec![node(0, "ipn:0.0"), node(1, "ipn:1.0"), node(2, "ipn:2.0")],
            contacts: vec![contact(0, 1, 0, 100_000)],
        };
        let config = ShadowEngineConfig::default();
        let representative = RepresentativeBundle::default();

        let hop = compute_first_hop(&topology, &config, 0, 2, 0, &representative)
            .expect("routing succeeds");
        assert_eq!(hop, None);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails to compile**

Run: `cd rust-bindings && cargo test -p hardy-a-sabr first_hop 2>&1 | head -30`
Expected: FAIL — compile errors from the old A-SABR API still present in the non-test part of `engine.rs`.

- [ ] **Step 3: Replace the non-test part of `engine.rs`**

Replace everything in `rust-bindings/hardy-a-sabr/src/engine.rs` *above* the `#[cfg(test)] mod tests` block with:

```rust
use a_sabr::{
    bundle::Bundle,
    contact::{Contact, ContactInfo},
    contact_manager::legacy::evl::EVLManager,
    contact_plan::{ContactPlan, RealNode},
    errors::ASABRError,
    multigraph::RoutableNodeRef,
    node::{Node, NodeInfo},
    node_manager::none::NoManagement,
    pathfinding::top_level::aliases::SpsnHybridParenting,
    utils::{Router, make_guard},
};

use crate::{projection::RepresentativeBundle, topology::TopologySnapshot};

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ShadowEngineConfig {
    pub max_entries: usize,
}

impl Default for ShadowEngineConfig {
    fn default() -> Self {
        Self { max_entries: 10 }
    }
}

pub fn build_contact_plan(
    topology: &TopologySnapshot,
) -> Result<ContactPlan<NoManagement, EVLManager>, ASABRError> {
    let mut nodes = topology
        .nodes
        .iter()
        .filter_map(|node| {
            Node::try_new(
                NodeInfo {
                    id: (node.asabr_node_id as usize).into(),
                    name: node.hardy_node_id.to_string().into(),
                    excluded: false,
                },
                NoManagement {},
            )
            .map(|inode| (node.asabr_node_id, RealNode::Inode(inode)))
        })
        .collect::<Vec<_>>();

    nodes.sort_by_key(|(id, _)| *id);

    debug_assert!(
        nodes
            .iter()
            .enumerate()
            .all(|(index, (id, _))| index as u16 == *id),
        "asabr_node_id values must be contiguous starting at 0"
    );

    let realnodes = nodes.into_iter().map(|(_, node)| node).collect::<Vec<_>>();

    let contacts = topology
        .contacts
        .iter()
        .filter_map(|contact| {
            Contact::try_new(
                ContactInfo::new(
                    (contact.tx_node_id as usize).into(),
                    (contact.rx_node_id as usize).into(),
                    contact.start,
                    contact.end,
                ),
                EVLManager::new(contact.rate, contact.delay),
            )
        })
        .collect::<Vec<_>>();

    Ok(ContactPlan::new(realnodes, Vec::new(), contacts))
}

pub fn compute_first_hop(
    topology: &TopologySnapshot,
    config: &ShadowEngineConfig,
    source: u16,
    destination: u16,
    now: i64,
    representative: &RepresentativeBundle,
) -> Result<Option<u16>, ASABRError> {
    let contact_plan = build_contact_plan(topology)?;

    make_guard!(id);
    let mut router = Router::<_, _, SpsnHybridParenting<1, _, _, _>, RoutableNodeRef>::build(
        id,
        contact_plan,
        (config.max_entries, ()),
    )?;

    let bundle = Bundle {
        priority: representative.priority,
        size: representative.size,
        expiration: now + representative.expiration_horizon,
    };

    let Some(source_ref) = router.node_id_ref((source as usize).into())?.internal() else {
        return Ok(None);
    };
    let destination_ref = router
        .node_id_ref((destination as usize).into())?
        .routable()?;

    let output = router.route(destination_ref, now, source_ref, &bundle, None)?;

    Ok(output.map(|(_path, first_hop)| {
        usize::from(
            first_hop
                .rx_node
                .internal()
                .expect("A-SABR first-hop rx_node is always an internal node"),
        ) as u16
    }))
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd rust-bindings && cargo test -p hardy-a-sabr 2>&1 | tail -20`
Expected: PASS — `first_hop_follows_linear_plan` and `no_path_returns_none` both pass; crate compiles with no errors.

- [ ] **Step 5: Commit**

```bash
cd /home/hugo/code/cspcl/cspcl-a-sabr
git add rust-bindings/hardy-a-sabr/src/engine.rs
git commit -m "refactor(hardy-a-sabr): route via new A-SABR utils::Router API"
```

---

### Task 4: Update the sample server config to i64 milliseconds

**Files:**
- Modify: `rust-bindings/hardy-a-sabr-server/examples/a-sabr-router.yaml`

**Interfaces:**
- Consumes: the `config::RuntimeConfig` / `engine::ShadowEngineConfig` / `topology`/`projection` shapes from Tasks 2–3.
- Produces: a YAML config that deserializes cleanly under the new types.

- [ ] **Step 1: Rewrite the sample config**

Replace the entire contents of `rust-bindings/hardy-a-sabr-server/examples/a-sabr-router.yaml` with (times in ms, `start`/`end`/`delay` scaled ×1000, `rate` unchanged as bps, `expiration_horizon` = 1 h in ms, and the removed `engine` sub-keys dropped):

```yaml
grpc_addr: "http://[::1]:51051"
agent_name: "a-sabr"

runtime:
  source: 0
  start_time: 0
  safety_tick_secs: 10

  engine:
    max_entries: 10

  topology:
    nodes:
      - asabr_node_id: 0
        hardy_node_id: "ipn:0.1.0"
      - asabr_node_id: 1
        hardy_node_id: "ipn:1.2.0"
    contacts:
      - tx_node_id: 0
        rx_node_id: 1
        start: 0
        end: 60000
        rate: 1000000
        delay: 1000

  projection:
    bundle:
      size: 1
      priority: 0
      expiration_horizon: 3600000
    destinations:
      - pattern: "ipn:1.2.*"
        asabr_destination: 1
        route_priority: 100
```

- [ ] **Step 2: Verify the server builds and the config parses**

Run: `cd rust-bindings && cargo build -p hardy-a-sabr-server 2>&1 | tail -10`
Expected: PASS — the server compiles against the migrated crate. (Config parsing is exercised by `serde_yaml` at runtime; a compile is sufficient for this task since the server has no config-parse unit test.)

- [ ] **Step 3: Commit**

```bash
cd /home/hugo/code/cspcl/cspcl-a-sabr
git add rust-bindings/hardy-a-sabr-server/examples/a-sabr-router.yaml
git commit -m "docs(hardy-a-sabr-server): update sample config to i64 milliseconds"
```

---

### Task 5: Full workspace verification

**Files:** none (verification only).

**Interfaces:** none.

- [ ] **Step 1: Build the whole workspace**

Run: `cd rust-bindings && cargo build 2>&1 | tail -15`
Expected: PASS — all crates (`cspcl-sys`, `cspcl`, `hardy-cspcl`, `hardy-a-sabr`, `hardy-a-sabr-server`) compile.

- [ ] **Step 2: Run all tests**

Run: `cd rust-bindings && cargo test 2>&1 | tail -20`
Expected: PASS — including the two `engine.rs` tests from Task 3.

- [ ] **Step 3: Run clippy on the migrated crate**

Run: `cd rust-bindings && cargo clippy -p hardy-a-sabr -p hardy-a-sabr-server 2>&1 | tail -20`
Expected: No errors. Warnings that predate this work (if any) are acceptable; new warnings introduced by the migration should be fixed.

- [ ] **Step 4: Verify the serde feature also builds**

Run: `cd rust-bindings && cargo build -p hardy-a-sabr --features serde 2>&1 | tail -10`
Expected: PASS — the `Serialize`/`Deserialize` derives on the retyped structs compile.

- [ ] **Step 5: Final commit (only if clippy fixes were made)**

```bash
cd /home/hugo/code/cspcl/cspcl-a-sabr
git add -A rust-bindings/hardy-a-sabr rust-bindings/hardy-a-sabr-server
git commit -m "chore(hardy-a-sabr): clippy cleanup after PR #66 migration"
```

(Skip this commit if Steps 1–4 produced no changes.)

---

## Self-Review notes

- **Spec coverage:** engine rewrite (Task 3), i64 model incl. topology/projection/scheduler/router/config (Task 2), `ShadowEngineConfig`→`max_entries` (Task 3), `asabr_node_id: u16` retained (Task 2/3), dependency pin (Task 1), sample-config update (Task 4), node-id `debug_assert!` (Task 3), tests + workspace build/test/clippy/serde (Tasks 3, 5). Risk #1 (toolchain) is resolved — nightly confirmed in Task 1 Step 3. Risk #2 (`make_guard!` re-export) is exercised the moment Task 3 compiles; if `use a_sabr::utils::make_guard;` fails to resolve, add `generativity = "1.2"` to `hardy-a-sabr/Cargo.toml` `[dependencies]` and import `use generativity::make_guard;` instead. Risk #3 (first-hop extraction) is pinned by `first_hop_follows_linear_plan`.
- **Placeholder scan:** none — every code step contains full content.
- **Type consistency:** `compute_first_hop(topology, config, source: u16, destination: u16, now: i64, representative: &RepresentativeBundle) -> Result<Option<u16>, ASABRError>` and `ShadowEngineConfig { max_entries: usize }` are used identically in the Task 3 code and the Task 3 tests; `now: i64` is consistent across `project_routes`, `Scheduler::now`, and `compute_first_hop`.
