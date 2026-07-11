# Checkpoint Backed Starts and Replays Plan

> [!WARNING]
> **POTENTIALLY STALE SUBDIVISION - DO NOT IMPLEMENT YET.**
> This lab-replay subdivision depends on assumptions that may change when
> `plans/archive/game-state/plan.md` lands. Re-evaluate this subplan and its phase files before
> implementation.

## Purpose

Use serialized checkpoints as the start state for normal matches, labs, imported setups, and
replays. This stage deliberately breaks old replay compatibility and replaces initializer recipes
with `ReplayArtifact { start: GameCheckpoint, actions }`. It should run after checkpoint
serialization and guards are in place.

## Phase Summaries

### [Phase 1 - Normal Match Start Checkpoints](phase-1.md)

Make normal match setup produce a tick-zero checkpoint from the existing map, player, loadout, and
spawn rules. Runtime behavior should remain equivalent while the generated checkpoint becomes the
source of truth for the started game. This makes future replay starts independent from later spawn
rule changes.

### [Phase 2 - Lab Start and Import Checkpoints](phase-2.md)

Make blank labs, catalog labs, and imported lab setups produce or consume the same checkpoint type.
"Lab Scenario" may remain player-facing UI/catalog copy, but the persisted setup payload should be
checkpoint-backed rather than a separate legacy setup contract. Lab baseline resets should store
a new baseline checkpoint plus a fresh current-branch action log.

### [Phase 3 - Game Construction From Checkpoint](phase-3.md)

Move game-start paths onto a narrow `Game` construction API that accepts a validated checkpoint.
Normal matches and labs may still generate checkpoints through different producers, but live games
should start by importing checkpoint state. This phase should retire construction paths that bypass
checkpoint validation.

### [Phase 4 - Replay Artifact Schema Break](phase-4.md)

Replace the old replay artifact schema with a checkpoint-backed schema. Old artifacts should fail
with a clear unsupported-schema message rather than being migrated. Dev artifact loading, replay
room launch, and match-history replay affordances should use only the new shape.

### [Phase 5 - Match Capture and History Integration](phase-5.md)

Capture ended matches as checkpoint-backed replay artifacts. The start checkpoint should represent
the actual initial authoritative state, and the timeline should initially contain normal player
commands. Match history should store or expose the new artifact and hide replay launch for old rows
that cannot play.

## Overall Constraints

- A map is not a checkpoint. Checkpoints may reference map identity/hash or embed validated map data,
  but they must still carry concrete game state.
- Preserve player ids, teams, entity ids, resources, and RNG state from the generated checkpoint.
- Do not add lab replay action support in this stage except where lab start checkpoints require it.
- Old replay compatibility is intentionally not required.
- Short beta replay dead zones between independently deployed phase PRs are acceptable, but each
  broken path must fail clearly and the final stage must restore newly captured replay launch.
- The replay viewer and runtime should remain shared.

## Handoff Requirements

Every phase handoff must name the old construction or artifact path that was retired, the new
checkpoint path that replaced it, and the manual start/replay flow to test.
