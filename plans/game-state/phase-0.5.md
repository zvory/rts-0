# Phase 0.5 - Derived-State Wipe Harness

Status: Done.

## Scope

Add the first executable confidence check for the `GameState` / `DerivedState` split before durable
checkpoint DTO work begins. The phase should prove that derived simulation state can be cleared and
rebuilt at a tick boundary without changing future authoritative behavior.

Implement a narrow test-only or crate-private harness that can run two equivalent games from the same
setup and command stream:

- baseline game: continue ticking without intervention;
- wipe/rebuild game: clear all currently classified derived state at a tick boundary, rebuild it from
  authoritative state, then continue ticking under the same commands.

The first implementation does not need a complete `GameState` struct or serialized
`GameCheckpoint`. It should instead establish the comparator and the clear/rebuild seam that later
checkpoint import will use.

## Derived-State Classification

Treat these as derived candidates for this phase:

- pathfinding cache and search bookkeeping inside `PathingService`;
- spatial indexes and other phase-local read models that can be rebuilt from map/entities;
- perf diagnostics and telemetry that must not feed back into simulation.

Treat these as authoritative unless proven otherwise:

- entity ids and allocator/high-water state;
- unit orders, queued orders, movement phase, selected waypoints, path goal, and path throttling
  fields;
- pending commands and command log state;
- player resources, upgrades, supply, scores, and team/faction metadata;
- fog-relevant memory, building/trench memory, lingering sight, firing reveals, smoke clouds, shell
  stores, ability runtime state, RNG state, and lab god mode.

If clearing a candidate derived field changes future semantic state or fog-filtered projections, move
that field into the authoritative list or implement a deterministic rebuild path before continuing.

## Expected Touch Points

- `server/crates/sim/src/game/mod.rs`
- `server/crates/sim/src/game/systems.rs`
- `server/crates/sim/src/game/services/pathing.rs`
- `server/crates/sim/src/game/snapshot.rs`
- focused tests under `server/crates/sim/src/game/**`
- `docs/design/server-sim.md` and `docs/context/server-sim.md` if the clear/rebuild seam becomes a
  documented simulation contract

## Verification

- Add a focused Rust test or test helper that clears/rebuilds derived state at a tick boundary and
  verifies both paired games remain equivalent after additional ticks.
- Compare semantic authoritative state rather than raw struct bytes. Include per-player
  fog-filtered snapshots anywhere visibility, memory, smoke, or projection could diverge.
- Include at least one pathing-heavy scenario where a warm path cache exists before the wipe and
  future movement still matches after rebuild.
- Prefer a small scenario first; expand coverage only enough to exercise movement/order state,
  pathing cache rebuild, and snapshot/fog equivalence.

Suggested focused command:

```bash
cargo test --manifest-path server/Cargo.toml -p rts-sim derived_state
```

If the final test names do not include `derived_state`, use the narrowest equivalent `cargo test`
filter and record it in the handoff.

## Manual Testing Focus

No broad gameplay manual testing is expected for this phase. If a local manual check is useful, use
one pathing-heavy dev or lab setup and confirm units continue moving normally after the code path that
clears/rebuilds derived state runs in tests.

## Handoff

The handoff must name:

- which fields were cleared/rebuilt by the derived-state seam;
- which fields were deliberately treated as authoritative;
- the exact focused Rust test command that passed;
- any state category that still lacks coverage and should be covered before durable checkpoint DTOs
  are introduced.
