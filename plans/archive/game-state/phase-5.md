# Phase 5 - Movement And Economy Checkpoint Coverage

Status: Done.

## Scope

After Phase 4 has landed the first internal `Game -> GameCheckpoint -> Game` cold-restore proof,
expand that same internal checkpoint path and semantic comparator over durable movement, order, and
economy state. This phase is behavior-preserving coverage work: add or extend internal DTO fields,
import/export helpers, canonical comparator views, and focused Rust tests so the checkpoint proof
captures state that can change future movement, resource, production, construction, deconstruction,
score, command-log, or snapshot results.

Reuse the Phase 4 checkpoint harness rather than creating another restore mechanism. The baseline
game should continue normally, while the restored game should still be rebuilt by exporting an
internal `GameCheckpoint`, importing it into fresh `GameState`, rebuilding `DerivedState`, then
ticking both games forward under the same subsequent command stream. Compare semantic
authoritative state after additional ticks, and compare per-player snapshots anywhere movement
debug paths, order plans, player resources, supply, visible construction progress, or other
movement/economy projections could diverge.

Coverage must include these durable state families where they exist in the current code:

- entity ids and allocator/high-water state, including the next id allocated after restore;
- unit active orders, queued order intents, order execution phases, movement phases, selected
  paths/waypoints, path goals, and movement throttling or recovery fields such as repath,
  stuck/progress, sidestep, scout-car recovery, static-blocked, oil-starvation, breakthrough, and
  facing state that changes future movement;
- pending commands and command-log behavior at checkpoint boundaries, including commands queued for
  the next tick and commands already stamped into the authoritative log;
- player steel/oil, upgrades, supply used/cap, score counters, units-lost-by-kind analysis support,
  player id, team id, faction id, name/color, AI flag, and start tile metadata;
- gather, build, train, research, rally, production, construction, and Tank Trap deconstruction
  state, including progress counters, phases, waiting-at-site timers, queued production/research,
  rally plan stages, active construction site projection state, resource-node remaining amounts,
  extractor progress, and worker/resource-node reservations;
- worker carried-resource fields and reserved drop-off fields, even if they are currently future
  compatibility state, so later round-trip harvesting cannot silently fall outside the checkpoint
  contract;
- tick and RNG continuity where relevant, without adding new gameplay randomness.

Add DTO/comparator coverage for missing fields before broadening scenarios. If a restored game
diverges, first treat it as evidence that a durable field is missing from the internal checkpoint
or comparator, or that import failed to rebuild derived state from durable state. Do not repair a
Phase 5 failure by changing movement rules, economy rules, build placement policy, production
timing, command validation, order promotion, pathfinding behavior, unit stats, costs, supply, or
projection policy.

Preconditions:

- Phase 4's internal checkpoint export/import path exists and does not use
  `Game::clone_for_replay_keyframe`.
- Phase 4's semantic comparator and per-player snapshot comparison are reusable by additional
  focused scenarios.
- Phase 1 through Phase 4 left no unresolved ownership blocker for the movement/order/economy fields
  covered here.

Explicit non-goals:

- No public checkpoint schema, JSON format, wire protocol, endpoint, client, snapshot DTO, or public
  `Game` API change.
- No replay keyframe replacement, replay artifact migration, lab timeline migration, lab scenario
  migration, or replay/lab product behavior change.
- No balance or gameplay changes.
- No full combat, projectile, smoke, ability-runtime, trench/building/fog-memory, lab god mode, or
  observer-analysis coverage yet, except the incidental state needed for movement/economy scenarios
  and per-player snapshot equivalence.
- No promise that AI controller decisions after restore are deterministic; use deterministic non-AI
  command streams for the new checkpoint tests.

## Expected Touch Points

- `server/crates/sim/src/game/state.rs` or the Phase 4 equivalent checkpoint module: add internal
  DTO fields and import/export handling for the movement/order/economy durable fields listed in
  this phase.
- `server/crates/sim/src/game/mod.rs`: add or adjust private/crate-private checkpoint test helpers
  only if needed to construct boundary states, restore from the internal checkpoint, or inspect
  canonical semantic views. Keep public `Game` API signatures stable.
- `server/crates/sim/src/game/entity/{entity.rs,order.rs,state.rs,store.rs}`: add narrowly scoped
  internal DTO/comparator accessors or derives only if the checkpoint path cannot otherwise capture
  entity ids, allocator state, orders, movement fields, production/construction state, worker state,
  resource reservations, and resource-node/extractor state.
- `server/crates/sim/src/game/services/movement/**`: read-only evidence or focused test fixtures for
  path/waypoint/throttle continuity unless a small helper is required for canonical comparison.
- `server/crates/sim/src/game/services/{commands.rs,order_queue.rs,economy.rs,production.rs,construction.rs}`:
  read-only evidence or test fixtures for pending-command, queued-order, gather, build,
  production, research, rally, construction, and deconstruction scenarios.
- Focused tests under `server/crates/sim/src/game/**`, preferably beside the Phase 4 checkpoint
  harness/comparator so movement/economy coverage extends the existing proof.
- `docs/design/server-sim.md` and `docs/context/server-sim.md` only if the implementation changes
  the internal checkpoint policy or moves documented section anchors. No doc update is needed for a
  pure test/DTO coverage expansion that preserves the Phase 4 contract.
- `plans/game-state/phase-5.md`: mark complete only in the implementation commit that lands this
  phase.

Implementation Rust/JS outside `rts-sim::game` should be out of scope unless compiler errors prove
a private helper must move. Client code, protocol crates, rule/balance crates, server room code,
lab scenario schemas, and replay artifact schemas should not need changes.

## Verification

- Extend the Phase 4 checkpoint comparator so every covered movement/order/economy field is either
  compared in a canonical semantic view or explicitly proven irrelevant because it is
  derived/transient under Phase 1's registry.
- Include at least one checkpoint test with pending commands queued before the next tick. Export,
  import, tick baseline and restored once, and prove pending commands are applied and logged
  exactly once with the same tick stamp and command-log order.
- Include at least one checkpoint test after several commands have already applied. Export/import
  with an existing command log, continue ticking, and prove the log is preserved without duplicate
  or missing entries.
- Include a movement/order scenario with active movement plus future queued stages. It should cover
  active order intent, queued intents, `MovePhase`, selected path/waypoints, `path_goal`,
  repath/throttle fields, and owner-visible movement projection. Compare per-player snapshots with
  movement-path diagnostics enabled where debug path projection is the observable surface.
- Include an economy/resource scenario with at least one worker gather order in progress or
  harvesting, resource-node remaining amount, resource reservation/miner slot, player resource
  totals, and supply/resource rows in snapshots.
- Include a build/construction/deconstruction scenario that crosses a checkpoint while a worker is
  walking to a site, waiting at a site, constructing a scaffold, or deconstructing a Tank Trap.
  Cover build/deconstruct phases, waiting timers, active construction site projection, spawned
  scaffold ids, construction progress, refunds or resource waits, and queued handoff orders where
  practical.
- Include a train/research/production/rally scenario that crosses a checkpoint with queued
  production or research in progress. Cover production/research queue item progress, player
  resource and supply reservation, completed upgrade insertion, rally point/rally queue state, and
  the spawned unit's id/order/queued-rally continuity after additional ticks.
- Prove entity allocator continuity through a post-restore allocation, preferably by completing a
  production or construction spawn after the checkpoint and asserting baseline/restored ids match.
- Preserve tick continuity and compare RNG state if the Phase 4 checkpoint DTO exposes it. If the
  selected movement/economy scenarios do not consume RNG, use a narrow internal comparison rather
  than adding random gameplay behavior.
- Compare semantic authoritative state after additional ticks, not just immediately after import.
  Include per-player fog-filtered snapshots for every player in scenarios where movement/economy
  state can affect order plans, debug paths, resources, supply, production, construction, or visible
  entity state.
- Confirm `Game::clone_for_replay_keyframe`, replay artifact capture/playback, lab timeline
  keyframes, lab scenario import/export, public APIs, wire protocol, client code, and balance values
  did not change.

Suggested focused commands:

```bash
cargo fmt --manifest-path server/Cargo.toml
cargo test --manifest-path server/Cargo.toml -p rts-sim checkpoint
cargo test --manifest-path server/Cargo.toml -p rts-sim movement_economy_checkpoint
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture
git diff --check -- server/crates/sim/src/game docs/design/server-sim.md docs/context/server-sim.md plans/game-state/phase-5.md plans/game-state/plan.md
```

If final test names do not include `checkpoint` or `movement_economy_checkpoint`, use the narrowest
equivalent filters that cover the internal checkpoint path, DTO/comparator field coverage,
pending-command boundary behavior, movement/order continuity, gather/build/train/research
continuity, semantic comparison, and per-player snapshot comparison. No broad Node suite or full
local test bundle is expected unless implementation changes escape the sim crate or alter
protocol-facing behavior; the PR `./tests/run-all.sh` gate remains the authoritative full-suite
check.

## Manual Testing Focus

No broad manual gameplay pass is expected because this phase should expose no public checkpoint or
UI behavior. If a manual check is useful, run one ordinary local match or dev/lab scenario that
issues move/queued-move, gather, build, train, research, rally, and cancel/deconstruct commands,
then confirm visible gameplay, resource rows, supply, construction progress, production queues, and
movement/order projections behave as before.

## Handoff

The implementation handoff must name:

- every internal `GameCheckpoint`/DTO field or canonical comparator view added for
  movement/order/economy state;
- every covered durable field family and any field intentionally excluded because Phase 1 classified
  it as derived or transient;
- the checkpoint boundary tests added for pending commands and existing command logs;
- the movement/order, gather/resource, build/construction/deconstruction, and
  train/research/production/rally scenarios added;
- how semantic authoritative comparison and per-player snapshot comparison run after additional
  ticks, including whether movement debug-path projection is included;
- how entity id and allocator/high-water continuity, tick continuity, and RNG continuity were
  preserved and tested;
- confirmation that public APIs, wire protocol, client code, replay keyframes/artifacts, lab
  timeline keyframes, lab scenario import/export, and balance/gameplay values did not change;
- the exact focused Rust test commands, archcheck command, and `git diff --check` command that
  passed;
- remaining checkpoint coverage gaps, especially combat, projectiles, smoke, ability runtime,
  trench/building/fog memory, lab god mode, observer analysis, and any movement/economy edge case
  that still lacks coverage before public checkpoint, replay, or lab migration work is considered.
