# Phase 2 - Deterministic Hellhole Churn

## Phase Status

- [ ] Done.

## Objective

Enhance `supply-300-hellhole` with mortal central armies that replenish at the next pre-tick and
invulnerable shuttle armies that receive changing partial-army move commands every second. Keep the
live Lab room, isolated server benchmark, and offline client snapshot stream driven by the same
deterministic scenario logic. Regenerate all canonical assets and finish with measured and visual
evidence.

## Work

- Change the generated checkpoint state and Lab metadata so only players 3 and 4 have god mode.
  Players 1 and 2, including their buildings, must be damageable.
- Preserve a deterministic target unit composition for central players 1 and 2. Before each tick,
  compare their live units by owner and kind with that target and create one spawn request for each
  deficit.
- Resolve deficit spawns with a bounded nearest-center search:
  - use deterministic candidate ordering nearest the canonical center
  - validate terrain and current body occupancy
  - account for earlier resolved spawns in the same batch
  - apply at most one `SpawnEntities` Lab operation per pre-tick
  - skip and report unresolved deficits without panic or unbounded searching
- Accept the intentional one-frame population/supply drop between death removal and the following
  pre-tick replacement. Do not add post-tick mutation or snapshot-fanout special cases.
- Keep spawned replacements fresh and otherwise inert; do not issue replacement-only attack,
  setup, or formation commands unless required for normal unit validity.
- Change shuttle automation for players 3 and 4 so every multiple of 30 ticks it:
  - deterministically ranks the 85 live unit ids using scenario seed, player id, epoch, and entity id
  - selects exactly 43 ids
  - issues one unqueued move command per shuttle player with command limits ignored
  - retains the existing 900-tick endpoint direction changes
  - chooses a deterministic valid goal tile inside a bounded corridor around the active endpoint
  - changes goal tiles across the canonical 900-tick run so repeated commands cannot all reuse one
    path-cache goal
- Keep randomness stateless or fully reconstructable so Lab seeking and fresh runs produce identical
  selections, destinations, spawns, entity ids, events, and snapshots.
- Share canonical Hellhole constants/geometry/composition between the generator and runtime driver
  where doing so prevents drift; do not introduce a generalized metadata registry without a second
  use case.
- Update the isolated harness to count and report at least shuttle commands, selected units, deaths,
  respawn batches, and respawned units. Retain the entity-count invariant at the pre-tick/post-action
  measurement boundary while explicitly allowing the death tick's outgoing snapshot to be lower.
- Strengthen focused tests for:
  - god mode exactly `[3, 4]`
  - deterministic 43-unit selections at 30-tick cadence
  - integer destination-tile variation and corridor bounds
  - central roster restoration by owner and kind
  - one-frame population/supply drops followed by restoration
  - replay seek/export behavior after both commands and respawn batches
  - deterministic snapshot-stream generation with actual death/respawn churn
- Regenerate, do not hand-edit:
  - `server/assets/lab-scenarios/supply-300-hellhole.json`
  - `client/assets/snapshot-streams/supply-300-hellhole.rtsstream`
- Update Hellhole behavior, commands, counters, regeneration steps, and interpretation in
  `docs/design/server-sim.md`, `docs/design/testing.md`, and `docs/perf-tracing.md`.
- Run the isolated 900-tick release harness and record its realtime factor plus churn counters.
- Use the project `interact` skill to open one authoritative Hellhole Lab scene, advance it far
  enough to observe churn, capture a clean Pixi screenshot, inspect the returned artifact exactly
  once, and preserve only its Tailnet Preview URL in the handoff.
- Mark this phase done in this file in the implementation commit.

## Expected Touch Points

- `server/src/bin/generate_supply_300_hellhole.rs`
- `server/src/lobby/lab_scenario_driver.rs`
- a small shared Hellhole specification/helper module if needed
- `server/crates/sim/src/game/lab.rs` and `docs/design/server-sim.md` if a typed bounded Lab query or
  placement helper is added to the public `Game` seam
- `server/src/tools/hellhole_snapshot_stream.rs`
- `server/src/tools/hellhole_perf_harness.rs`
- `server/assets/lab-scenarios/supply-300-hellhole.json`
- `client/assets/snapshot-streams/supply-300-hellhole.rtsstream`
- `server/src/lab_scenarios.rs`
- `server/src/lobby/room_task/tests/lab_scenario_driver.rs`
- snapshot-stream and client-performance contract tests
- `docs/design/server-sim.md`
- `docs/design/testing.md`
- `docs/perf-tracing.md`

## Verification

- Focused Rust tests for the Hellhole generator, driver, room-task replay/seek flow, Lab placement,
  snapshot stream, and isolated harness invariants.
- `node tests/client_contracts/snapshot_stream_contracts.mjs`.
- `node tests/client_contracts/frame_profiler_contracts.mjs`.
- `node scripts/check-lobby-architecture.mjs`.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if the
  public `Game` seam or sim architecture changes.
- `scripts/hellhole-perf-harness.sh --ticks 900 --json` in release mode.
- One clean authoritative Interact Lab screenshot inspected once.
- `git diff --check`.

## Manual Test Focus

Watch the central scrum through several deaths and verify that population can dip for one snapshot,
then returns as replacements appear near the center. Watch both diagonal shuttle armies and confirm
only part of each formation receives visibly changing orders every second while the shuttle units
remain invulnerable. Seek backward across a death/respawn interval and resume to confirm the same
churn repeats without duplicate scripted actions or room failure.

## Handoff Expectations

Report the exact selection count and cadence, destination corridor/jitter rule, respawn search bound,
and treatment of unresolved deficits. Include the 900-tick isolated harness timings and churn
counters, the focused verification commands, the Tailnet Preview URL, player-facing impact, and any
performance caveat that should be watched in later optimization work.
