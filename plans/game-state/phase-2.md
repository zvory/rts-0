# Phase 2 - Explicit DerivedState Shell

Status: Done.

## Scope

Introduce a private `DerivedState` boundary under `Game` for fields that Phase 1 classified as
`derived/rebuildable`, starting with the post-tick `spatial` index and pathing cache/search
bookkeeping. This phase is behavior-preserving code movement: it makes derived ownership explicit
without changing gameplay, command validation, fog projection, replay behavior, or public `Game`
APIs.

The intended shape is a private shell owned by `Game`, for example a `DerivedState` struct or module
that contains the rebuildable final spatial index plus the pathing service/cache boundary. Keep the
existing `systems.rs` phase-local derived state as the tick pipeline authority:

- `PreCommandDerivedState`, `PostMovementDerivedState`, `PreCollisionDerivedState`, and
  `FinalDerivedState` remain local to `systems.rs` unless the implementation needs a narrow helper
  to construct the final snapshot index.
- `systems::run_tick` should still rebuild derived state at the same boundaries and return or write
  the final spatial index that snapshots consume after the tick.
- snapshot code must still have access to the final post-tick spatial index without rebuilding it
  per recipient.
- clearing pathing derived state must not drop `PathingService`'s default budget, cache capacity, or
  other configuration needed for live path requests; only reusable cache/search bookkeeping may be
  reset unless Phase 1 explicitly classified more of the service as rebuildable.

Reuse and extend the Phase 0.5 derived-state wipe/rebuild harness. The proof for this phase should
clear the new `DerivedState` shell through the same seam, rebuild what must be rebuilt from
authoritative state, continue ticking under the same command stream, and compare semantic state plus
per-player fog-filtered snapshots. Do not create a second comparator or a separate confidence
mechanism.

If Phase 1 left any unresolved ownership blocker for `spatial`, pathing cache/search bookkeeping, or
another field proposed for `DerivedState`, stop and resolve the registry first instead of moving the
field by assumption.

Explicit non-goals:

- Do not move durable fields into `GameState` yet.
- Do not add durable checkpoint DTOs or cold checkpoint import/export.
- Do not change the public `Game` API used by lobby, replay, lab, AI, or server code.
- Do not change replay/lab behavior except for repair paths needed after lab mutations to keep
  derived state valid.
- Do not move authoritative selected unit paths, movement phases, waypoints, path goals, order
  queues, pending commands, command logs, fog, memory stores, effect stores, RNG, scores, player
  economy, map metadata, seed, loadouts, or lab god mode into `DerivedState`.

## Expected Touch Points

- `server/crates/sim/src/game/mod.rs`: replace direct `Game` fields for currently derived final
  spatial/pathing state with a private `DerivedState` owner, plus private helpers for snapshot
  access and tick/lab repair.
- `server/crates/sim/src/game/systems.rs`: preserve existing phase-local derived-state rebuild
  timing and the final spatial index handoff to `Game`; avoid making tick systems consume stale
  cross-phase indexes.
- `server/crates/sim/src/game/services/pathing.rs`: add or adjust a narrow clear/rebuild/reset
  method if needed so cache/search bookkeeping can be wiped while preserving default budget and
  cache-capacity configuration.
- `server/crates/sim/src/game/services/spatial.rs`: use existing `SpatialIndex::build` behavior
  unless a small constructor/helper is needed for `DerivedState`.
- `server/crates/sim/src/game/setup.rs` and `server/crates/sim/src/game/setup/dev_scenarios.rs`:
  initialize the new `DerivedState` shell with the same initial spatial index and live pathing
  budget/cache settings used today.
- `server/crates/sim/src/game/lab.rs`: route lab mutation repair through the new derived-state
  repair/rebuild path while preserving existing supply, fog, building-memory, trench-memory, god
  mode, and invariant repairs.
- focused tests under `server/crates/sim/src/game/**`, especially the Phase 0.5 derived-state
  wipe/rebuild harness and any pathing cache tests needed to prove config is preserved.
- `docs/design/server-sim.md` and `docs/context/server-sim.md` only if the new private
  `DerivedState` boundary becomes part of the documented simulation contract or shifts section
  references from Phase 1's registry.
- `plans/game-state/phase-2.md`: mark complete only in the implementation commit that lands this
  phase.

Implementation Rust/JS outside these areas should be treated as out of scope unless required by the
compiler after the private field move. Client code should not need changes.

## Verification

- Extend the Phase 0.5 derived-state wipe/rebuild harness so it wipes the new `DerivedState` shell,
  including the final spatial index and pathing cache/search bookkeeping, then rebuilds the final
  spatial index from `Game`'s authoritative state.
- Keep the comparator semantic rather than byte-for-byte. It should continue comparing
  authoritative state and per-player fog-filtered snapshots after additional ticks, ignoring only
  fields Phase 1 classified as derived or transient.
- Include a pathing-heavy scenario that warms the path cache before the wipe, clears the cache via
  the new `DerivedState` seam, and proves future movement/order results still match.
- Add a focused assertion that clearing pathing derived state preserves the live default pathing
  budget/cache configuration or otherwise proves a cleared cache still uses the same configured
  budget path as before.
- Cover at least one lab mutation repair path if the implementation changes lab repair plumbing, so
  a spawn/move/delete/restore mutation leaves final spatial, fog, supply, and memory repair valid.
- Run the narrowest focused Rust command that covers the updated harness. Suggested command:

```bash
cargo test --manifest-path server/Cargo.toml -p rts-sim derived_state
```

If test names do not include `derived_state`, use the closest narrow filter that covers the harness,
pathing cache/config preservation, and any touched lab repair tests. Also run a docs/whitespace check
for the touched plan/design files:

```bash
git diff --check -- plans/game-state/phase-2.md plans/game-state/plan.md docs/design/server-sim.md docs/context/server-sim.md
```

No broad Node or full-suite local run is expected unless implementation changes escape the sim crate
or the focused Rust tests expose cross-boundary risk. The PR `./tests/run-all.sh` gate remains the
authoritative full-suite check.

## Manual Testing Focus

No broad manual gameplay pass is expected for this ownership-shell phase. If a manual check is
useful, use one pathing-heavy dev or lab scenario and verify units keep moving normally after issuing
move/attack-move commands, then perform one lab spawn/move/delete or scenario restore and confirm
snapshots immediately reflect the repaired world.

## Handoff

The implementation handoff must name:

- the final private `DerivedState` shape and exactly which former `Game` fields moved into it;
- how final spatial access remains available to snapshot code without changing the public `Game`
  API;
- how pathing cache/search bookkeeping is cleared while preserving default budget/cache-capacity
  configuration;
- what changed in lab repair, if anything, and which lab mutation repair test covered it;
- the exact focused Rust test command and `git diff --check` command that passed;
- any Phase 1 registry row or Phase 0.5 harness scenario that still needs follow-up before durable
  `GameState` or checkpoint DTO work begins.
