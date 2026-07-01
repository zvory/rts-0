# Phase 3 - GameState Aggregate Shell

Status: Done.

## Scope

After Phase 1 has landed a complete ownership registry and Phase 2 has separated rebuildable fields
into a private `DerivedState` shell, introduce a private durable `GameState` aggregate under `Game`.
This phase should move fields classified by Phase 1 as `authoritative/serialized` or
`compatibility metadata` out of the top-level `Game` struct and into `GameState`, while preserving
all public behavior.

The intended end state is a private ownership tree, not a checkpoint schema:

```text
Game
  GameState      authoritative/serialized state plus compatibility metadata
  DerivedState   rebuildable cache and performance state from Phase 2
```

Use the Phase 1 registry as the source of truth for the exact field set. For the current code shape,
that likely means `GameState` owns durable fields such as `map`, `entities`, `fog`,
`building_memory`, `players`, `pending`, `command_log`, `tick`, `lingering_sight`,
`firing_reveals`, `smokes`, `trenches`, `ability_runtime`, `mortar_shells`, `artillery_shells`,
`seed`, `starting_loadouts`, `map_metadata`, `active_construction_sites`,
`lab_god_mode_players`, `starting_loadout`, and `rng`, unless Phase 1 explicitly classified a field
differently. Do not move Phase 2 `DerivedState` fields back into durable state, and do not invent
new transient or compatibility categories during implementation.

This is a behavior-preserving private field move and borrow-shaping phase. Public `Game` API
signatures and visible behavior must remain stable for lobby, replay, lab, AI, server, snapshots,
tests, and match-history callers. Private helper signatures may change as needed, but they should
make ownership clearer rather than expose a broad mutable state bag.

`systems::run_tick` can keep receiving split borrows into `GameState` fields and services. It does
not need to accept a whole `GameState`, and this phase should avoid broad mutable getters such as a
crate-wide `state_mut()` that would let unrelated modules bypass service invariants. Prefer narrow
helpers or module-local split borrows for map/entities/players/fog/effects/RNG/service state, and
keep service-owned invariants behind the same focused service APIs that protect them today.

If Phase 1 left an unresolved ownership blocker for any field proposed for `GameState`, or if Phase
2 did not land the `DerivedState` boundary, stop and resolve the earlier phase result before moving
fields by assumption.

Explicit non-goals:

- Do not add durable checkpoint DTOs, cold checkpoint import/export, checkpoint JSON, or serde
  schema work.
- Do not change public Rust APIs, wire protocol shapes, replay artifact schemas, lab scenario
  schemas, snapshot DTOs, or client/server protocol contracts.
- Do not change replay or lab behavior. Existing clone-based replay keyframes and lab scenario
  import/export should keep their current semantics.
- Do not move room/session state into `Game`; replay cursors, lab timeline history, sockets,
  participants, room lifecycle state, selected spectator vision, and AI controller memory remain
  outside this aggregate.
- Do not add architecture guardrails yet unless a touched code path requires a targeted allowlist or
  baseline adjustment. Still run the sim architecture check because this phase reshapes
  `rts-sim::game`.

## Expected Touch Points

- `server/crates/sim/src/game/mod.rs`: introduce the private `GameState` owner, keep `Game` public,
  and move Phase 1 durable/compatibility fields under the new aggregate.
- `server/crates/sim/src/game/state.rs` or an equivalent private module if that keeps
  `mod.rs` readable. Keep the module private to `game` unless a narrower visibility is required.
- `server/crates/sim/src/game/setup.rs`: construct `GameState` and preserve initial supply, fog,
  building-memory, trench-memory, RNG, map metadata, loadout metadata, command-log, pending-command,
  and lab compatibility initialization.
- `server/crates/sim/src/game/snapshot.rs`: read through the new private state owner while keeping
  `snapshot_for*` behavior and the Phase 2 final-spatial access unchanged.
- `server/crates/sim/src/game/lab.rs`: route lab mutations, restore, god mode, scenario export, and
  repair through the aggregate without changing lab scenario behavior or exposing broad state
  mutation.
- `server/crates/sim/src/game/replay.rs`: keep replay artifact capture and command-log replay
  behavior stable while reading metadata and command history from `GameState`.
- `server/crates/sim/src/game/invariants.rs`: update invariant reads to the new owner and keep
  panic-free tick-path assumptions unchanged.
- `server/crates/sim/src/game/systems.rs`: change only what is needed for borrow shaping. The tick
  orchestrator may continue receiving split field borrows rather than a whole aggregate.
- Focused tests under `server/crates/sim/src/game/**` if the field move requires new helper coverage
  or adjustments to the Phase 0.5/Phase 2 state-boundary harness.
- `docs/design/server-sim.md` and `docs/context/server-sim.md` if the private `GameState` boundary
  or section references become part of the documented simulation contract.
- `plans/game-state/phase-3.md`: mark complete only in the implementation commit that lands this
  phase.

Implementation Rust/JS outside `rts-sim::game` should be treated as out of scope unless compiler
errors prove a caller depends on a private helper that must be reshaped. Client code and wire
protocol files should not need changes.

## Verification

- Confirm every Phase 1 `authoritative/serialized` and `compatibility metadata` field is either
  moved into `GameState` or explicitly documented as intentionally left outside because Phase 1
  classified it differently.
- Confirm Phase 2 `DerivedState` fields remain derived and are not serialized or mixed back into
  `GameState`.
- Confirm all public `Game` methods listed in `docs/design/server-sim.md` keep the same signatures
  and observable behavior.
- Confirm `systems::run_tick` still advances the same state in the same order and receives only
  narrow split borrows or focused service handles.
- Confirm replay artifact capture, command-log replay, lab scenario export/restore, lab mutations,
  snapshot projection, invariant checks, and derived-state wipe/rebuild coverage still pass with the
  aggregate shell in place.
- Run the sim architecture check because this phase touches the `rts-sim::game` ownership shape:

```bash
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture
```

Suggested focused commands:

```bash
cargo fmt --manifest-path server/Cargo.toml
cargo test --manifest-path server/Cargo.toml -p rts-sim game_state
cargo test --manifest-path server/Cargo.toml -p rts-sim derived_state
git diff --check -- server/crates/sim/src/game docs/design/server-sim.md docs/context/server-sim.md plans/game-state/phase-3.md plans/game-state/plan.md
```

If the final test names do not include `game_state` or `derived_state`, use the narrowest equivalent
filters that cover the aggregate move, the derived-state wipe/rebuild harness, lab repair, replay
metadata capture, snapshots, and invariants. No broad Node suite or full local test bundle is
expected unless the implementation changes escape the sim crate or alter public protocol-facing
behavior; the PR `./tests/run-all.sh` gate remains the authoritative full-suite check.

## Manual Testing Focus

No broad gameplay manual pass is expected for this private ownership-shell phase. If a manual check
is useful, run one local match or lab scenario that exercises start payloads, one or more ticks,
snapshots, a lab spawn/move/delete or scenario restore, and replay artifact capture enough to confirm
the refactor did not change visible behavior.

## Handoff

The implementation handoff must name:

- the final private `GameState` shape and exactly which former top-level `Game` fields moved into
  it;
- any former `Game` field not moved into `GameState`, with the Phase 1 classification that explains
  why;
- how Phase 2 `DerivedState` remains separate from durable state;
- how public `Game` API signatures were preserved;
- how `systems::run_tick` and lab/snapshot/replay helpers were borrow-shaped without broad mutable
  getters;
- whether any service invariants, architecture allowlists, or documented section references changed;
- the exact focused Rust test commands, archcheck command, and `git diff --check` command that
  passed;
- the core manual testing focus and any unresolved blocker before durable checkpoint DTO work begins.
