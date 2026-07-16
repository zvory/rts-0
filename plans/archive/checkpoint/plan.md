# Checkpoint Serialization And Migration Plan

## Status

Active implementation plan for turning the internal `GameState` checkpoint proof from
`plans/game-state/` into a versioned, embeddable text payload and then migrating starts, replays,
and lab setup containers onto that shared payload. This supersedes the stale checkpoint portions of
`plans/lab-replay/`, which should remain historical reference only. Each phase below should land
through the normal owned-PR workflow before the next phase starts.

## Purpose

Make authoritative simulation state a durable, validated, versioned text contract that can be
embedded in multiple artifact types. A checkpoint payload, combined with the exact map supplied by
its containing match start, replay artifact, checkpoint-backed lab setup, or debug document, should
be able to restore a `Game`, rebuild derived state, continue ticking, and match the current
authoritative world with the same semantic and projection accuracy proven by `plans/game-state/`.
Once that is true, normal match starts, replay artifacts, and lab setup assets can converge on one
start-state payload instead of maintaining separate setup formats.

## Phase Summaries

### [Phase 1 - Checkpoint Payload Contract](phase-1.md)

Define the public checkpoint payload contract before moving behavior onto it. This phase should add
a versioned `GameCheckpointV1` embeddable text schema, container rules, field map, serde policy,
validation model, compatibility metadata, and explicit bounds without changing normal match,
replay, or lab behavior. It should end with a precise implementation handoff for the first real
payload round-trip.

### [Phase 2 - Payload Round Trip Proof](phase-2.md)

Implement `Game + exact supplied map -> GameCheckpointV1 -> text bytes -> exact supplied map ->
Game` for all current checkpointed state fields except the container-owned map data itself. This
phase should replace the cfg-test clone-shaped checkpoint proof with explicit DTO conversion,
validation, serde, derived-state rebuild, and payload round-trip tests that reuse the existing
semantic comparator. It may include a test/debug document wrapper to prove disk I/O, but it should
still expose no player-facing route, command, UI, replay schema, or lab setup migration.

### [Phase 3 - Checkpoint-Backed Starts](phase-3.md)

Make ordinary game and non-scenario lab construction compile their setup inputs into a tick-zero
start payload: the exact map plus a `GameCheckpointV1` payload bound to that map. Existing public
constructors and room flows should keep their signatures while the implementation proves
checkpoint-backed starts are behaviorally identical to direct setup. The phase should leave replay
artifacts and committed lab setup files on their current formats until the later lab phases.

### [Phase 4 - Replay Artifact Migration](phase-4.md)

Introduce a replay artifact version whose start state is the replay map binding plus a tick-zero
`GameCheckpointV1` and the existing recorded command stream. New captures should save the tick-zero
map-plus-checkpoint composition at match launch, attach duration/final score/command-stream facts at
match end, and write the checkpoint-backed artifact while old replay artifacts remain loadable or
fail with an intentional, documented compatibility reason. Every replay writer, including lobby,
crash, dev/self-play, AI self-play, and match-history paths, must use a launch-time start
composition rather than deriving start state from the final `Game`. Replay seek and branch behavior
should keep working while keyframe internals are migrated only where the phase explicitly proves
parity.

### [Phase 5 - Lab Setup Checkpoint Adapter](phase-5.md)

Add side-by-side lab setup adapters so legacy setup assets can be converted into a setup container
with map data/binding, authoring metadata, and `GameCheckpointV1`. New lab exports can optionally
emit the checkpoint-backed setup shape behind an internal option. This
phase should preserve today's lab import/export UI behavior, id-remap responses, validation, and
authoring metadata while proving the checkpoint path reaches the same restored lab state. It should
not rewrite the catalog assets yet.

### [Phase 6 - Lab Asset Cutover](phase-6.md)

Migrate the bundled lab catalog and lab export path to checkpoint-backed setup
containers. Keep compatibility readers for old lab setup files during the transition, or explicitly
remove them only after every bundled and persisted caller has a tested replacement. This phase is
the first one that may intentionally change the lab setup JSON shape.

### [Phase 7 - Public Surface Cleanup And Release Audit](phase-7.md)

Remove obsolete setup, replay, and lab scenario paths only after checkpoint-backed starts are the
default everywhere. Tighten docs, architecture checks, fixture generation, and compatibility
messages so new authoritative state cannot bypass the payload contract. Finish with a release audit
covering gameplay parity, replay compatibility, lab compatibility, privacy, persistence, and
operational rollback.

## Overall Constraints

- Build on the completed `plans/game-state/` ownership tree. Do not re-litigate whether current
  `GameState` fields are authoritative unless implementation evidence proves a registry row is
  wrong.
- Do not serialize `Snapshot` or any fog-filtered client projection as authoritative state.
  Snapshots are verification outputs, not checkpoint inputs.
- Do not `serde` private `GameState` or `Entity` internals directly as the stable persisted
  contract.
  Use explicit DTOs so Rust refactors do not silently become schema breaks.
- `GameCheckpointV1` is the canonical embeddable payload, not a product-specific file format.
  Replays, lab setups, match-start artifacts, and any test/debug files are containers around the
  same payload.
- A map should remain a map. The checkpoint payload should carry a map binding such as stable name,
  schema version, content hash, and any other validation facts needed to prove it is being restored
  against the exact map supplied by the containing artifact or setup path. A standalone debug
  document may include map data beside the checkpoint payload, but map JSON must not become a
  checkpoint container in disguise. Phase 1 must reconcile this public payload policy with the
  current `docs/design/server-sim.md` registry row that classifies runtime `GameState.map` as a
  full internal checkpoint field; after that decision, Phase 2 import should construct the live
  `GameState.map` from the container-supplied map, not from a duplicated map body hidden inside
  `GameCheckpointV1`.
- Preserve stable entity ids, store allocator/high-water state, trench ids, ability runtime ids,
  smoke/shell ids, tick count, RNG draw-stream state, pending commands, command-log metadata,
  entity-local active orders, queued order intents, selected movement paths/waypoints/path goals,
  active and pending smoke clouds, scheduled mortar/artillery impacts, and active ability runtime
  projectiles/world objects according to the server-sim registry.
- Rebuild `DerivedState` from authoritative DTOs on import. `final_spatial` and pathing
  cache/search data must not be serialized.
- Import must validate before constructing a live `Game`: schema version, map identity/hash,
  player ids/teams, owner references, entity ids, command/order references, coordinates, counts,
  timers, queues, resource values, per-payload size caps, and any container-specific caps.
- Lab scenario import/export remains a public, untrusted JSON surface once it accepts
  checkpoint-backed scenario containers. That is distinct from adding a generic live-match
  checkpoint upload command, but it still requires the same schema validation, map binding checks,
  payload/container byte limits, entity/count caps, path allowlists, and protocol/client/doc mirrors
  as any other client-supplied artifact.
- AI controller memory remains outside the checkpoint contract. Checkpoints preserve AI player
  slots and authoritative world state; replay determinism comes from recorded actions.
- Keep old replay and lab assets compatible until a phase explicitly proves and documents a
  replacement or a deliberate rejection policy.
- Replay artifact migration must inventory and test every existing load surface: dev/self-play
  files, crash replay artifacts, match-history database rows, and any committed fixtures. Database
  decoding must use the same versioned compatibility/rejection policy as file loading.
- Replay artifact migration must inventory and test every existing write surface: post-match lobby
  capture, shutdown/crash capture, dev replay saves, AI/self-play artifact writers, scripted
  self-play failure artifacts, match-history attachment, and committed fixture generation. The
  artifact construction API should make final-state-derived start checkpoints impossible by requiring
  a launch-time start composition.
- Replay artifact migration must capture the replay start checkpoint at tick zero before commands
  mutate the match, then append the authoritative command stream and end-of-match metadata when the
  artifact is finalized. Do not derive a replay start checkpoint from the final post-match `Game`.
- Lab scenario migration must treat import/export JSON as protocol-visible unless the phase proves
  the change is server-internal only. Any `LabScenario` message or file-shape change must update the
  mirrored protocol docs/code and run protocol parity checks in the same phase.
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
- No public network protocol command for uploading arbitrary live-match checkpoint payloads in this
  plan. Checkpoint-backed lab scenario import/export is still allowed only through the existing lab
  scenario surface and must be treated as untrusted scenario JSON, not as a generic restore-any-game
  checkpoint endpoint.
- No database migration for historical match rows unless the replay phase explicitly scopes it.
- No guarantee of cross-version checkpoint migration until the payload contract phase defines and tests
  a migration policy.

## Relationship To Older Plans

`plans/lab-replay/` was written before the `GameState`/`DerivedState` ownership work landed and is
marked stale. Use this plan as the current roadmap for checkpoint payload serialization, checkpoint
starts, replay migration, and lab scenario migration. If an older lab-replay phase has useful
detail, copy the current evidence into the relevant phase here rather than executing the stale phase
file directly.
