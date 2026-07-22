# A-SABR PR #66 API migration — design

**Date:** 2026-07-22
**Crate:** `rust-bindings/hardy-a-sabr` (+ `hardy-a-sabr-server` config files)
**Upstream:** [DTN-MTP/A-SABR PR #66](https://github.com/DTN-MTP/A-SABR/pull/66) (merged to `main`)

## Goal

Adapt `hardy-a-sabr` to the rebuilt A-SABR routing/pathfinding public interface introduced by PR #66. The crate's role is unchanged: build an ephemeral "shadow" contact-graph per query, compute the first hop toward each projected destination, and install the results as `RouteAction::Via` routes through the async `Scheduler`. Only the A-SABR call surface and the numeric time model change.

## Summary of upstream API changes

The A-SABR crate moved to a workspace layout (package `a_sabr` now lives in the `asabr/` subdirectory), `edition = "2024"`, and `#![no_std]` (using `alloc`). The routing surface was rebuilt:

| Old (current `hardy-a-sabr`) | New (PR #66) |
|---|---|
| `a_sabr::routing::Router`, `a_sabr::routing::aliases::SpsnHybridParenting` | `a_sabr::utils::Router`, `a_sabr::pathfinding::top_level::aliases::SpsnHybridParenting` — alias gains a `PRIO_COUNT` const generic and a destination type param `D` |
| `a_sabr::vertex::Vertex` (`INode`/`ENode`/`VNode`) | **removed** → `a_sabr::contact_plan::RealNode` (`Inode`/`Enode`) + `a_sabr::multigraph` node refs |
| `Bundle { source, destinations, priority, size, expiration }` | `Bundle { priority, size, expiration }` — `source` and `destinations` removed; source & destination are passed to `route()` instead |
| `TreeCache::new(check_size, check_priority, max_entries)` built by hand | built implicitly from pathfinder args `(max_entries, ())`; `check_priority` → the `PRIO_COUNT` const; `check_size` knob removed |
| `router.route(source_u16, &bundle, now, &excluded)` → `out.lazy_get_for_unicast(dest)` → `contact.borrow().info.rx_node_id` | `router.route(dest_ref, now, source_ref, &bundle, prune)` → `Option<(PathIterator, PathFragment)>`; the returned `PathFragment` is the **first hop**, `frag.rx_node` is its receiving node |
| `ContactPlan::new(vertices, contacts, None)` | `ContactPlan::new(realnodes, vnodes, contacts)`; `Contact::try_new(...)` now returns `Option<(Contact, tx_idx, rx_idx)>` — the exact tuple `contacts` wants |
| `EVLManager::new(rate: f64, delay: f64)` | `EVLManager::new(rate: i64, delay: i64)` (`DataRate` / `Duration`) |

Two structural shifts drive the design:

1. **Generativity lifetime `'id`.** `Multigraph`/`Router` carry an invariant lifetime bound to a `make_guard!` stack token, so they cannot be stored in a struct field or held across `.await` — they must be built, used, and dropped inside one stack frame. `compute_first_hop` already follows this "build → route → discard" pattern, so no structural rework is required.

2. **Numeric types `f64` → `i64`.** `Date`, `Duration`, `Volume`, `DataRate` are all `i64` now (recommended unit: milliseconds / bytes / bps). `NodeID` is a `usize` wrapper (was `u16`). We migrate the entire `hardy-a-sabr` model to **i64 milliseconds** to match natively, with no conversion layer.

## Architecture

Unchanged. Per `compute_first_hop` call: build a fresh `ContactPlan` from the `TopologySnapshot`, wrap it in a generativity-guarded `Router` using the SPSN / hybrid-parenting / SABR-distance pathfinder, route a representative `Bundle` to a single destination node, extract the first-hop node id, then discard the router. The `Scheduler` drives these queries at contact boundaries and on a safety tick, diffs the resulting routes, and installs them via the `RoutingSink`. `route()` mutates (schedules tx on) the graph, which is harmless because the graph is rebuilt and discarded on every call — this preserves the existing shadow semantics.

## Component-level changes

### `engine.rs` (core rewrite)

- **Imports:** add `utils::{Router, make_guard}`, `pathfinding::top_level::aliases::SpsnHybridParenting`, `contact_plan::{ContactPlan, RealNode}`, `multigraph::RoutableNodeRef`. Remove `vertex::Vertex`, `routing::*`, `route_storage::cache::TreeCache`, `contact::*` stays.
- **`build_contact_plan`:** produce `Vec<RealNode<NoManagement>>` via `RealNode::Inode(Node::try_new(NodeInfo { id: (asabr_node_id as usize).into(), name: hardy_node_id.to_string().into(), excluded: false }, NoManagement {}))`, sorted by `asabr_node_id`. `vnodes` is empty. Contacts: `Contact::try_new(ContactInfo::new((tx as usize).into(), (rx as usize).into(), start, end), EVLManager::new(rate, delay))` — `filter_map` yields the `(Contact, tx_idx, rx_idx)` tuples directly. Return `ContactPlan::new(realnodes, Vec::new(), contacts)`.
- **Delete** `new_route_cache`, `build_shadow_router`, and the `ShadowRouter` type alias.
- **`compute_first_hop(topology, config, source: u16, destination: u16, now: i64, representative) -> Result<Option<u16>, ASABRError>`:**
  1. `let contact_plan = build_contact_plan(topology)?;`
  2. `make_guard!(id);`
  3. `let mut router = Router::<_, _, SpsnHybridParenting<1, _, _, _>, RoutableNodeRef>::build(id, contact_plan, (config.max_entries, ()))?;`
  4. `let bundle = Bundle { priority: representative.priority, size: representative.size, expiration: now + representative.expiration_horizon };`
  5. Resolve `source_ref` via `router.node_id_ref((source as usize).into())?.internal()`; return `Ok(None)` if the source node is not internal.
  6. Resolve `dest_ref` via `router.node_id_ref((destination as usize).into())?.routable()?`.
  7. `let out = router.route(dest_ref, now, source_ref, &bundle, None)?;`
  8. `Ok(out.map(|(_, first_hop)| usize::from(first_hop.rx_node.internal().expect("first hop rx_node is internal")) as u16))`.
- **`ShadowEngineConfig`** reduces to `{ max_entries: usize }`. `check_size` is removed (no API equivalent); `check_priority` is removed and replaced by the fixed `PRIO_COUNT = 1` const in the alias (matching the old `false` default). `Default` keeps `max_entries: 10`.

### `topology.rs`
- `ContactWindow`: `start: i64`, `end: i64`, `delay: i64` (ms), `rate: i64` (bps).
- `NodeMapping.asabr_node_id`: **stays `u16`** (compact domain id, cast to `NodeID` at the A-SABR boundary).
- `next_boundary_after(now: i64) -> Option<i64>` (integer comparison, `min` instead of `total_cmp`).

### `projection.rs`
- `RepresentativeBundle { size: i64, expiration_horizon: i64 }`; `Default` → `size: 1`, `priority: 0`, `expiration_horizon: 3_600_000` (1 hour in ms).
- `project_routes(..., now: i64)`.

### `scheduler.rs`
- `start_time: i64` (ms).
- `now() = self.start_time + self.started_at.elapsed().as_millis() as i64`.
- `next_boundary_delay`: `Duration::from_millis((boundary - now).max(0) as u64)`.
- `safety_tick` stays a `std::time::Duration` (wall-clock sleep budget).

### `router.rs`
- `start_time: f64 → i64`; `with_start_time(i64)`. `safety_tick: Duration` unchanged.

### `config.rs`
- `RuntimeConfig.start_time: f64 → i64`. `safety_tick_secs: u64` unchanged (still a real-seconds wall-clock budget).

### `hardy-a-sabr-server`
- No Rust changes expected — it deserializes `RuntimeConfig` from YAML. **Existing/sample YAML config files must be updated** so contact times, delays, `expiration_horizon`, and `start_time` are i64 milliseconds (and `rate` is i64 bps). Any sample configs committed in the repo are updated as part of this work.

### `hardy-a-sabr/Cargo.toml`
- Pin `a_sabr` to a specific merged-`main` `rev` for reproducibility (currently unpinned). The `git` dependency by package name `a_sabr` still resolves despite the `asabr/` subdirectory move.
- Add `generativity` as a direct dependency **only if** the `make_guard!` re-export through `a_sabr::utils` fails to resolve at the call site.

## Invariants & assumptions

- **Node-id contiguity:** `asabr_node_id` values are assumed to be `0..N` contiguous, so a node's position in the id-sorted `realnodes` vector equals its id. `Contact::try_new` derives contact endpoint indices from raw node ids, which is only correct under this assumption. This assumption is already present in the current code; it is preserved and guarded with a `debug_assert!` in `build_contact_plan`.
- **Unit convention:** all A-SABR-facing time/duration values are milliseconds; volumes are in the same arbitrary unit the caller uses for bundle `size`; rates are bps. The convention only needs to be internally consistent across the contact plan, bundle, and `now`.

## Error handling

- `compute_first_hop` continues to return `Result<Option<u16>, ASABRError>`: `Err` on A-SABR construction/routing failure, `Ok(None)` when no path exists (including source == destination, which `route()` reports as no first hop), `Ok(Some(hop))` otherwise.
- `project_routes` propagates `ASABRError` and skips destinations with no first hop or no reverse EID mapping, as today.
- The `Scheduler` logs and swallows refresh/withdraw errors via `tracing::warn!`, unchanged.

## Risks

1. **Toolchain.** A-SABR is now `edition = "2024"` (Rust ≥ 1.85) and `destination.rs` uses `Box::new_zeroed_slice` (nightly `new_zeroed_alloc`). The **first implementation step is a trial `cargo build`** against the pinned dependency to surface any toolchain/nightly requirement (e.g. a `rust-toolchain.toml`) before writing migration code.
2. **`make_guard!` re-export.** If the macro doesn't resolve through `a_sabr::utils`, fall back to a direct `generativity` dependency (see Cargo.toml note).
3. **First-hop extraction correctness.** The returned `PathFragment` is the fragment immediately after the source (verified against `PathIterator::commit` and the `inter-regional_routing` example). A unit test pins this behaviour.

## Testing

- `cargo build` and `cargo test` for the whole `rust-bindings` workspace against the real A-SABR dependency.
- A `compute_first_hop` unit test over a small 3-node linear contact plan (`0 → 1 → 2`) asserting the next hop from `0` toward `2` is `1`, and that an unreachable destination yields `Ok(None)`.
- Verify `hardy-a-sabr-server` still loads its (updated) YAML config.

## Out of scope

- Multicast (`Dest::AllNodes` / anycast) — not implemented upstream for `NoManagement`; we route unicast to a single `RoutableNodeRef` destination only.
- Any change to the scheduler's contact-boundary/safety-tick logic beyond the `f64 → i64` retype.
- Virtual nodes / external nodes in the topology model.
