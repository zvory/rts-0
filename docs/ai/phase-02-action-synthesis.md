# AI-2: Shared Action Synthesis

Status: Done.

Build one deterministic way for AI decisions to turn facts into ordinary commands.

This phase should centralize command construction, local resource reservation, worker reservation,
and duplicate-command prevention. It should still avoid changing strategy behavior beyond the
minimum needed to use the shared action layer.

## Goal

Create shared action helpers that can be used by live AI and self-play:

- train units
- build structures
- assign workers to resources
- stage combat units
- launch attack-move waves
- reserve local spending inside one think step

The output is still only `Command`.

## Suggested Files

- Add `server/src/game/ai_core/actions.rs`.
- Extend `server/src/game/ai_core/mod.rs`.
- Reuse or migrate `SpendBudget` from `server/src/game/ai_shared.rs`.
- Touch `server/src/game/ai.rs` and `server/src/game/selfplay.rs` only enough to prove reuse.

## Core Types

Suggested shape:

- `AiActionContext`
  - profile-independent facts
  - mutable local budget
  - reserved worker ids
  - reserved resource node ids
  - skipped build tiles, when supplied by self-play
  - emitted commands
- `AiReservations`
  - workers reserved this think
  - resource nodes reserved this think
  - buildings/production reservations if needed
- `BuildPlacementRequest`
  - building kind
  - start tile
  - search radius/options
  - skip tiles
  - placement predicate callback

Keep the action layer stateless where possible. Persistent state such as wave size and failed build
spots should stay in the adapter/controller until a later phase proves it belongs in shared state.

## Subtasks

### AI-2.1 Centralize Local Spend Reservation

Move `SpendBudget` into the shared core or make it the canonical budget type from `ai_shared`.

It must support:

- `can_afford_unit`
- `reserve_unit`
- `can_afford_building`
- `reserve_building`
- `free_supply`
- committed steel already reserved by en-route builders

The action layer should reserve locally before emitting a command so one think tick does not queue
more spending than the AI can afford.

### AI-2.2 Centralize Worker Selection

Create one helper for selecting build workers.

Rules to preserve:

- prefer idle workers
- fall back to gatherers when appropriate
- do not reuse a worker twice in one think
- avoid workers assigned to special roles such as oil if the caller marks them reserved
- keep deterministic ordering

Live AI and self-play currently each do variants of this. They should converge.

### AI-2.3 Centralize Build Commands

Create a shared `try_build` helper.

It should:

- check local affordability
- select and reserve a worker
- find a valid build spot through a supplied placement predicate
- reserve local building cost
- emit `Command::Build`

The helper must not directly mutate entities or assume command success. The server still validates
on apply.

Self-play-specific failed build spot tracking can remain outside the helper initially. The helper
should accept a `skip` set so self-play can pass those tiles in.

### AI-2.4 Centralize Train Commands

Create shared helpers for:

- training workers from idle industrial centers
- training combat units from production buildings
- shallow queue policy

The helper should accept policy inputs rather than hard-code one strategy:

- max queue depth
- unit kind priorities
- whether to save for tech
- max count for a unit kind

### AI-2.5 Centralize Worker Resource Assignment

Create a helper that assigns idle workers to resources.

Initial scope:

- steel assignment for current live AI behavior
- optional oil assignment when requested by a tech profile
- distinct node reservation in one think

Do not add expansion logic in this phase.

### AI-2.6 Centralize Attack Commands

Create shared helpers for:

- selecting ready combat units
- issuing `AttackMove`
- reissuing pressure for already committed units

Keep current rally-line behavior if migrating live AI. More advanced squad logic belongs in later
phases.

### AI-2.7 Add Action Tests

Add tests for:

- a build action reserves the worker and cost
- a second build action cannot reuse the same worker
- unit training respects local budget and supply
- resource assignment picks distinct nodes
- attack command unit order is deterministic

Use small fixtures. Avoid whole-match tests unless necessary.

## Non-Goals

- No new profiles yet.
- No live AI profile UI.
- No self-play script replacement yet.
- No expansion bases.
- No advanced micro.

## Validation

Run targeted Rust tests. If live AI is partially migrated, also run the existing AI behavior test
that proves the AI grows economy, builds supply, produces riflemen, and damages the opponent.

## Done Criteria

AI-2 is done when:

- live AI and at least one self-play script use shared action helpers
- local budget and worker reservation logic exists in one place
- common build/train/gather/attack commands are synthesized by shared code
- behavior remains deterministic

Implemented in `server/src/game/ai_core/actions.rs`. Live AI and the self-play scripts now use
shared helpers for local spend reservation, build-worker reservation, build placement callbacks,
training queue policy, resource-node reservation, and deterministic attack-move command emission.
