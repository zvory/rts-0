# AI PLAN

This file is the detailed implementation plan for the gameplay AI and AI-driven self-play.

Use `PLAN.md` as the top-level dependency map. Use this file when implementing AI work so smaller
models do not have to reconstruct the intended architecture from code comments or scattered TODOs.

## Goal

Build a decent but simple AI system that:

- supports at least three maintainable strategies:
  - `rifle_flood_fast`
  - `rifle_flood_full_saturation`
  - one tech-tree strategy that exercises oil, prerequisites, and tank production
- shares game knowledge across live AI and self-play
- stays on the ordinary command path
- stays deterministic under replay
- is easy to update when balance, unlocks, or economy rules change

Non-goals for this plan:

- advanced micro
- scouting logic
- adaptive opponent modeling
- machine learning
- a large behavior-tree framework
- perfect play

The target is maintainability first, then decent coverage and believable behavior.

## Main Architectural Decision

Do not build three separate bots that each know how to play the whole game.

Instead, build:

1. one shared AI knowledge layer
2. one shared action-synthesis layer
3. one small decision loop
4. several thin strategy profiles
5. two adapters:
   - live gameplay AI adapter
   - self-play harness adapter

This keeps "how the game works" in one place and keeps "which style to prefer" in small,
replaceable strategy profiles.

## Why This Approach

The current maintenance problem is duplicated RTS knowledge.

Today, `server/src/game/ai.rs` already has a useful state-derived AI shape, while
`server/src/game/selfplay.rs` contains several bespoke strategy scripts with duplicated economy,
tech, worker assignment, and attack logic. That makes feature changes expensive because unlocking a
tank, saturating a patch, or deciding when to attack must be updated in multiple places.

This plan chooses a smaller architecture instead of a more ambitious one:

- No big behavior-tree framework yet.
- No planner that searches deep future branches.
- No script zoo where each test owns custom game logic.

Trade-off:

- We give up some expressiveness now.
- In return we get an AI system that is much easier to keep correct while the game is evolving.

## Dependency Chain

Implement AI work in this order. Do not skip downward in the chain unless the prerequisites are
already complete and the skipped layer is truly unnecessary.

### A. Shared World Queries and Definitions

Purpose:
- Give AI one authoritative way to ask game questions.

Examples:
- how many workers are mining steel
- what is the saturation target for this base
- which building unlocks tanks
- whether supply is blocked
- whether a building of a certain kind is complete, in progress, or only intended
- what combat units exist and are free to join an attack

Depends on:
- `PLAN.md` Phase 2.2 world helpers
- `PLAN.md` Phase 3.1 definitions

Notes:
- The AI should not own hard-coded copies of unlock chains or unit costs.
- If a game mechanic changes, AI should learn it by asking defs/helpers again, not by editing
  three separate strategies.

### B. Shared AI Knowledge Layer

Purpose:
- Centralize all reusable "RTS common sense" logic.

This layer should answer:
- do I need more workers
- do I need supply now
- what should I build to unlock a target unit
- can I afford this now
- which worker should build
- where is a safe/valid place to build near the base
- which resource node should this idle worker take
- is this army large enough to move out

Examples of helpers to centralize here:
- worker saturation target
- supply pressure calculation
- building prerequisite resolution
- worker pool selection
- pending-build tracking rules
- attack readiness checks

This layer is where "change one piece of code and all strategies inherit it" should become true.

### C. Shared Action-Synthesis Layer

Purpose:
- Turn AI decisions into ordinary `Command`s.

This should own:
- train worker
- train combat unit
- build structure
- assign gather order
- issue attack-move
- reserve local spending inside one think step

Rules:
- stay on the same command path as human clients and replay
- centralize budget reservation logic
- centralize common anti-duplication protections such as one pending depot builder

Depends on:
- shared AI knowledge layer
- `PLAN.md` Phase 2.3 formal command processor

### D. Small Decision Loop

Purpose:
- Decide what to do this think tick using the shared knowledge/actions.

Recommended shape:
- cheap periodic think loop
- collect current facts
- evaluate a small set of candidate macro actions
- emit a few commands in priority order

This does not need to be a full behavior tree.

A ranked checklist or utility-style scorer is enough, as long as it is:
- deterministic
- easy to inspect
- easy to parameterize by strategy profile

### E. Strategy Profiles

Purpose:
- Express different personalities without forking AI mechanics.

Each profile should mostly be data and thresholds, not custom control flow.

Candidate fields:
- target worker count behavior
- rush vs eco preference
- minimum free supply buffer
- desired barracks count curve
- desired tech timing
- attack size threshold
- whether to save for tech before continuing rifle production
- preferred composition priorities

The first required profiles are:

#### `rifle_flood_fast`

Intent:
- pressure quickly
- cut worker greed earlier
- build early rifle production
- attack with a smaller army threshold

Use for:
- proving early aggression still works
- replacing fragile worker-rush-adjacent scripts with a more game-realistic early attack bot

#### `rifle_flood_full_saturation`

Intent:
- saturate the starting steel economy first
- then scale rifle production harder
- attack later with a larger/more stable wave

Use for:
- proving the economy-first opening still transitions into pressure
- catching regressions where worker assignment or supply planning breaks macro play

#### `tech_tree`

Intent:
- exercise oil gathering
- build the prerequisite chain
- reach tank production
- attack with a mixed army after teching

Use for:
- testing prerequisite logic
- testing oil economy
- testing tank unlock/progression changes

### F. Perception Adapters

Purpose:
- share the AI brain while preserving the gameplay-vs-selfplay boundary.

Two adapters are expected:

#### Live AI Adapter

- reads authoritative state
- keeps the current gameplay AI contract
- may use fuller knowledge because it is server-side

#### Self-Play Adapter

- reads the same snapshot-style view allowed by the self-play harness contract
- uses the same shared decision machinery underneath

Important:
- These adapters may differ in what they can observe.
- They should not differ in game-mechanics knowledge.

## Rollout Plan

Implement in small, reviewable phases.

### Phase AI-1: Document and Freeze the AI Boundary

Deliverables:
- document the intended shared-layer architecture
- name the first three strategy profiles
- document that self-play should migrate away from bespoke scripts toward shared personalities

Why first:
- avoids coding toward two different mental models

### Phase AI-2: Extract Shared Knowledge from Existing Code

- [x] Centralize deterministic near-base build-site selection so live AI and self-play stop
  carrying separate placement heuristics.
- [x] Centralize worker saturation targeting, local spend reservation, and attack-wave selection
  helpers so both AI entry points share the same small rules.

Deliverables:
- identify duplicated logic in `ai.rs` and `selfplay.rs`
- move reusable economy/build/attack knowledge into shared helpers

Expected duplicated areas:
- worker saturation logic
- supply logic
- build placement
- pending build intent tracking
- unit training affordability
- worker assignment to steel/oil
- attack readiness

Success condition:
- at least one meaningful AI rule can be changed in one place and observed by more than one
  strategy consumer

### Phase AI-3: Introduce Strategy Profiles

Deliverables:
- add a profile/config object for AI personality selection
- port the first three strategies onto the shared decision loop

Rules:
- avoid separate per-strategy script files that copy whole decision trees
- allow small, explicit per-strategy overrides only when a profile field is not enough

Success condition:
- `rifle_flood_fast`, `rifle_flood_full_saturation`, and `tech_tree` all run off the same core

### Phase AI-4: Move Live AI onto the Shared Core

Deliverables:
- live AI uses the shared knowledge/action system
- current basic AI behavior remains deterministic

Rules:
- do not break the lobby/gameplay AI feature while improving architecture
- keep one think cadence and shared command path semantics

Success condition:
- live AI can select one of the new strategy profiles without duplicating mechanics

### Phase AI-5: Move Self-Play onto the Shared Core

Deliverables:
- self-play matchup configuration can choose AI personalities
- old bespoke scripts are reduced or removed where equivalent shared-profile coverage exists

Rules:
- keep the self-play harness on the public `Game` seam
- keep artifact logging, milestones, and replay checks

Success condition:
- matchup tests use shared AI personalities rather than owning separate strategy logic

### Phase AI-6: Replace Brittle Scripted Coverage with Matchups

Deliverables:
- personality-vs-personality tests
- milestone assertions tuned to each matchup

Required initial matchups:
- `rifle_flood_fast` vs `rifle_flood_full_saturation`
- `rifle_flood_fast` vs `tech_tree`
- `rifle_flood_full_saturation` vs `tech_tree`

Possible assertions:
- fast flood attacks before tech-tree reaches tanks often enough to matter
- full-saturation flood reaches stronger economy milestones before first committed push
- tech-tree reliably gathers oil, builds prerequisites, and produces tanks

Rules:
- prefer milestone and outcome assertions over exact tick-perfect command sequences
- do not require pixel-perfect or queue-perfect behavior

### Phase AI-7: Broaden Coverage as Mechanics Expand

Future work after the first three strategies:
- machine-gunner-aware profiles
- faction-specific profiles
- AI surrender/GG behavior
- richer composition rules

Do not start these until the initial shared architecture is stable.

## Test Strategy

The AI system must be tested at multiple levels.

### Unit-Level AI Helper Tests

Test small deterministic helpers such as:
- saturation targets
- supply pressure
- prerequisite resolution
- free-army selection
- build-site selection filters
- local spend reservation

These tests should be fast and should not require a full live match.

### Live AI Behavior Tests

Keep or replace current focused gameplay AI tests with assertions such as:
- trains workers beyond start
- builds supply before deadlock
- produces riflemen
- reaches tech building goals when using the tech strategy
- eventually damages or pressures an opponent

### Self-Play Matchup Tests

Use shared personalities and check:
- economy milestones
- tech milestones
- combat-event milestones
- replay determinism
- no stalls before required goals

### Replay and Determinism Tests

Replay correctness is a hard requirement.

The AI must:
- emit ordinary commands only
- remain deterministic under stable world iteration
- keep replay output identical to live output

If any AI change introduces nondeterminism, fix that before adding more strategy complexity.

## Implementation Rules

- Keep AI, replay, and human players on the same command path.
- Avoid copying game-rule constants into strategy code when defs/helpers can answer the question.
- Prefer small pure helper functions over large monolithic `think()` bodies.
- Prefer deterministic ordering over hash-order-dependent scans.
- Keep per-strategy differences visible and inspectable.
- A strategy profile should say "prefer this timing/composition", not redefine mining, building,
  and attack semantics from scratch.

## File-Ownership Direction

This is guidance, not a strict final module map.

Likely ownership split:
- `server/src/game/ai.rs`
  - live AI adapter
  - profile selection wiring
- new shared AI module(s)
  - shared knowledge
  - shared action synthesis
  - shared decision loop
  - strategy profile definitions
- `server/src/game/selfplay.rs`
  - self-play adapter
  - matchup configuration
  - milestones and artifact/reporting logic

Important:
- the self-play harness should still own test orchestration
- it should not keep owning separate RTS mechanics if the shared AI core can do that job

## Exit Criteria for the First AI Refactor

This AI refactor is successful when all of the following are true:

- there are at least three working strategy profiles:
  - `rifle_flood_fast`
  - `rifle_flood_full_saturation`
  - `tech_tree`
- live AI and self-play share the same AI knowledge/action core
- changing a tech prerequisite or economy rule usually requires changing one shared helper, not
  several strategy implementations
- matchup tests exercise the strategies without relying on fragile scripted command sequences
- replay determinism remains intact
