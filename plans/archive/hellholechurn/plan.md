# Hellhole Churn Plan

## Purpose

Make the canonical 300-supply Hellhole exercise sustained unit death, population churn, command
admission, formation planning, and changing path destinations while remaining deterministic across
the live Lab room, isolated server harness, checked-in snapshot stream, and Lab replay/seek flows.
The central players will lose god mode and replace missing units at the following pre-tick boundary,
so one outgoing frame may show the temporary population drop. The shuttle players will retain god
mode and issue a fresh unqueued move command every second for a deterministic random half of their
units toward a deterministically jittered tile in their active destination corridor.

## Overall Constraints

- Preserve server authority and the public `Game` seam. The room task remains the sole owner of its
  `Game`; scripted scenario behavior must use public Lab operations and Lab command APIs.
- Keep all automated scenario actions deterministic across fresh runs, Lab seeks, replay export,
  the direct server harness, and snapshot-stream generation. Prefer stateless hashing keyed by the
  fixed scenario seed, player, command epoch, and entity id over mutable driver RNG state.
- Reuse the existing replay representations for `IssueCommandAs` and `SpawnEntities`. Do not add a
  wire-protocol operation unless current replay serialization proves unable to express a resolved
  scripted action.
- Apply respawns at the next pre-tick boundary after a death. A one-frame population and supply drop
  is intentional stress evidence; do not add a post-tick callback or move snapshot fanout solely to
  hide it.
- Respawn only missing units belonging to central players 1 and 2. Their buildings are allowed to
  take damage and are not automatically replaced; shuttle players 3 and 4 retain god mode for both
  units and buildings.
- Keep respawn placement bounded and simple. Search deterministic candidate positions nearest the
  center, accept the first valid body-aware placement, batch the resolved spawns once per pre-tick,
  and report or skip an unplaceable deficit without blocking or panicking the room.
- Select exactly 43 of each shuttle player's 85 live units at every 30-tick command boundary. The
  selection and destination must be deterministic, `queued:false`, and use the existing Lab option
  that ignores normal command-list limits.
- Destination jitter must change the integer goal tile within the appropriate endpoint corridor;
  sub-tile pixel jitter is insufficient because the path cache keys on integer start and goal tiles.
- Preserve the canonical initial setup: four 300-supply rosters, 470 isolated stone tiles, existing
  central attack/setup orders, building rings, map, camera, and full-vision observer presentation.
- Keep the isolated server benchmark's default timing representative. Extra diagnostics may count
  scripted actions and churn, but must not enable heavyweight per-path tracing in the default gate.
- Regenerate derived scenario and snapshot-stream assets only from their checked-in generators; do
  not hand-edit generated JSON or binary bytes.
- Update the relevant `docs/design/server-sim.md`, `docs/design/testing.md`, and
  `docs/perf-tracing.md` source-of-truth sections when behavior or benchmark interpretation changes.
- Each implementation phase must land on its own `zvorygin/` branch, be pushed as an owned PR with
  auto-merge armed, and wait for a definite merge with its head reachable from `origin/main` before
  the next phase starts.
- When a phase is complete, mark its phase document done in that phase's implementation commit.
  After implementing each phase, provide a handoff describing what the next agent should do and the
  core features a human should manually test.

## Phase Summaries

### [Phase 1 - Replay-Safe Scripted Lab Actions](phase-1.md)

Generalize the scenario driver and room executor so a bundled scenario can emit both commands and
ordinary replay-serializable Lab mutations at a pre-tick boundary. Route scripted mutations through
one apply-and-record path that preserves timeline ordering, seek reconstruction, replay export, and
failure isolation without changing the current Hellhole behavior. Add focused tests proving a
scripted spawn batch is recorded once, survives seek reconstruction, and replays through existing
`SpawnEntities` operations.

### [Phase 2 - Deterministic Hellhole Churn](phase-2.md)

Use the phase 1 action path to remove god mode from central players, replace their missing units on
the next pre-tick, and command a deterministic random half of each shuttle army every second toward
changing destination tiles. Regenerate the Lab checkpoint and offline snapshot stream, strengthen
the structural/replay/performance assertions, and document how to interpret the intentional
one-frame population drops and churn counters. Run the isolated benchmark and one authoritative
Interact inspection so the final handoff includes both measured behavior and a clean visual review.

## Phase Index

1. [Phase 1 - Replay-Safe Scripted Lab Actions](phase-1.md)
2. [Phase 2 - Deterministic Hellhole Churn](phase-2.md)

## Deferred Backlog

- Optimize or replace the bounded nearest-center placement search only if measurement shows it is a
  material benchmark bottleneck.
- Add deeper path-cache hit/miss aggregation to the default isolated harness only if changing goal
  tiles and existing optional performance tracing are insufficient to diagnose a future regression.
- Generalize scenario-runtime configuration into checkpoint metadata only if a second bundled
  scenario needs the same recurring-action machinery.

## Implementation Process

Implement and merge one phase at a time. Do not start phase 2 until phase 1's PR is merged and its
head is reachable from `origin/main`. Each phase executor must leave a compact handoff covering the
completed scope, focused verification, player-facing impact, what the next executor should know, and
the core manual test focus.
