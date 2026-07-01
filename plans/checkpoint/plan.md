# Checkpoint Serialization And Migration Plan

## Status

Active implementation plan for turning the internal `GameState` checkpoint proof from
`plans/game-state/` into a versioned file format and then migrating starts, replays, and lab
scenarios onto that format. This supersedes the stale checkpoint portions of `plans/lab-replay/`,
which should remain historical reference only. Each phase below should land through the normal
owned-PR workflow before the next phase starts.

## Purpose

Make authoritative simulation state a durable, validated, versioned file contract. A checkpoint
file should be able to restore a `Game`, rebuild derived state, continue ticking, and match the
current authoritative world with the same semantic and projection accuracy proven by
`plans/game-state/`. Once that is true, normal match starts, replay artifacts, and lab scenario
assets can converge on one start-state contract instead of maintaining separate setup formats.

## Phase Summaries

### [Phase 1 - Checkpoint File Contract](phase-1.md)

Define the public checkpoint file contract before moving behavior onto it. This phase should add a
versioned `GameCheckpointV1` envelope, field map, serde policy, validation model, compatibility
metadata, and explicit bounds without changing normal match, replay, or lab behavior. It should end
with a precise implementation handoff for the first real file round-trip.

### [Phase 2 - File Round Trip Proof](phase-2.md)

Implement `Game -> GameCheckpointV1 -> bytes/file -> Game` for all current `GameState` fields. This
phase should replace the cfg-test clone-shaped checkpoint proof with explicit DTO conversion,
validation, serde, derived-state rebuild, and file round-trip tests that reuse the existing
semantic comparator. It should still expose no player-facing route, command, UI, replay schema, or
lab scenario migration.

### [Phase 3 - Checkpoint-Backed Starts](phase-3.md)

Make ordinary game and lab construction compile their setup inputs into a tick-zero checkpoint, then
restore the live `Game` from that checkpoint. Existing public constructors and room flows should
keep their signatures while the implementation proves checkpoint-backed starts are behaviorally
identical to direct setup. The phase should leave replay artifacts and committed lab scenario files
on their current formats.

### [Phase 4 - Replay Artifact Migration](phase-4.md)

Introduce a replay artifact version whose start state is `GameCheckpointV1` plus the existing
recorded command stream. New captures should write the checkpoint-backed artifact while old replay
artifacts remain loadable or fail with an intentional, documented compatibility reason. Replay seek
and branch behavior should keep working while keyframe internals are migrated only where the phase
explicitly proves parity.

### [Phase 5 - Lab Scenario Checkpoint Adapter](phase-5.md)

Add side-by-side lab scenario adapters so current `LabScenarioV1` assets can be converted into
checkpoint starts and new lab exports can optionally emit checkpoints. This phase should preserve
today's lab import/export UI behavior, id-remap responses, validation, and authoring metadata while
proving the checkpoint path reaches the same restored lab state. It should not rewrite the catalog
assets yet.

### [Phase 6 - Lab Asset Cutover](phase-6.md)

Migrate the bundled lab catalog and lab submission/export path to checkpoint-backed scenario assets.
Keep compatibility readers for old `LabScenarioV1` files during the transition, or explicitly
remove them only after every bundled and persisted caller has a tested replacement. This phase is
the first one that may intentionally change the on-disk lab scenario format.

### [Phase 7 - Public Surface Cleanup And Release Audit](phase-7.md)

Remove obsolete setup, replay, and lab scenario paths only after checkpoint-backed starts are the
default everywhere. Tighten docs, architecture checks, fixture generation, and compatibility
messages so new authoritative state cannot bypass the file contract. Finish with a release audit
covering gameplay parity, replay compatibility, lab compatibility, privacy, persistence, and
operational rollback.

## Overall Constraints

- Build on the completed `plans/game-state/` ownership tree. Do not re-litigate whether current
  `GameState` fields are authoritative unless implementation evidence proves a registry row is
  wrong.
- Do not serialize `Snapshot` or any fog-filtered client projection as authoritative state.
  Snapshots are verification outputs, not checkpoint inputs.
- Do not `serde` private `GameState` or `Entity` internals directly as the stable file contract.
  Use explicit DTOs so Rust refactors do not silently become schema breaks.
- Preserve stable entity ids, store allocator/high-water state, trench ids, ability runtime ids,
  smoke/shell ids, tick count, RNG draw-stream state, pending commands, and command-log metadata
  according to the server-sim registry.
- Rebuild `DerivedState` from authoritative DTOs on import. `final_spatial` and pathing
  cache/search data must not be serialized.
- Import must validate before constructing a live `Game`: schema version, map identity/hash,
  player ids/teams, owner references, entity ids, command/order references, coordinates, counts,
  timers, queues, resource values, and per-file size caps.
- AI controller memory remains outside the checkpoint contract. Checkpoints preserve AI player
  slots and authoritative world state; replay determinism comes from recorded actions.
- Keep old replay and lab assets compatible until a phase explicitly proves and documents a
  replacement or a deliberate rejection policy.
- Any phase touching protocol, client, replay, lab, or room behavior must read the relevant context
  capsule and design section before implementation.
- Run focused tests for the touched surface in each phase; the PR `./tests/run-all.sh` gate remains
  the authoritative full-suite check.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, then waited on until GitHub reports the PR merged and the phase head SHA is
  reachable from `origin/main`.
- After implementing each phase, the implementing agent must provide a handoff message naming what
  changed, what the next agent should do, focused verification that passed, and the core manual
  testing focus. Manual testing notes should cover the main product surface affected by that phase,
  not an exhaustive matrix.

## Non-Goals

- No gameplay or balance changes.
- No client-side save/load UI until a later product plan asks for it.
- No public network protocol command for uploading arbitrary checkpoint files in this plan.
- No database migration for historical match rows unless the replay phase explicitly scopes it.
- No guarantee of cross-version checkpoint migration until the file contract phase defines and tests
  a migration policy.

## Relationship To Older Plans

`plans/lab-replay/` was written before the `GameState`/`DerivedState` ownership work landed and is
marked stale. Use this plan as the current roadmap for checkpoint file serialization, checkpoint
starts, replay migration, and lab scenario migration. If an older lab-replay phase has useful
detail, copy the current evidence into the relevant phase here rather than executing the stale phase
file directly.
