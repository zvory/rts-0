# Panzerfaust Implementation Plan

## Purpose

Implement the Panzerfaust requirements from [checklist.md](checklist.md) as a normal Kriegsia
infantry unit without exposing an unfinished combat path in live matches. The unit is a
Training Centre-unlocked Barracks infantry option with one short-range anti-tank shot, then it
converts into a Rifleman. Phase 0 and Phase 1 are already captured in the checklist as the
requirements gate; the implementation phases below should still be run one at a time only after
explicit approval to proceed beyond that gate.

## Product Input

- [checklist.md](checklist.md) is the active Panzerfaust brief, rules, balance, and deferred-item
  source.
- [docs/new-unit-checklist.md](../../../docs/new-unit-checklist.md) remains the workflow checklist for
  adding the unit.

## Phase Summaries

### [Phase 2 - Hidden Vocabulary, Balance, And Protocol Contract](phase-2.md)

Add the shared Panzerfaust vocabulary, hidden rules data, and protocol contract before any normal
match can train the unit. This phase should introduce the unit kind, mirrored balance-visible
metadata, and any fog-safe event or snapshot vocabulary needed by later one-shot runtime work. It
must keep the Panzerfaust out of normal production, command cards, AI build plans, and player-facing
matches until server runtime and client readability are complete.

### [Phase 3 - Hidden Server Runtime And Conversion](phase-3.md)

Implement the authoritative one-shot Panzerfaust behavior for spawned Panzerfaust entities while
keeping the unit hidden from normal production. The server should own target legality, windup,
travel, recovery, damage, cancellation, order continuation, and conversion into a Rifleman with the
same entity id. This phase proves the unit can be spawned, ordered, fire safely, fail safely, and
convert in simulation tests before player UI exposes it.

### [Phase 4 - Client State, Visuals, And Lab Inspection](phase-4.md)

Teach the client to parse and present the hidden unit and its one-shot feedback well enough for
human inspection. Add a distinguishable loaded Panzerfaust infantry visual, fog-safe launch/impact
feedback, state handling around same-id conversion, and a lab or dev-scenario inspection path. This
phase should make spawned Panzerfaust units readable in a local inspection flow, but still not
trainable in normal matches.

### [Phase 5 - Production Exposure And Command Card](phase-5.md)

Expose the completed Panzerfaust through normal Kriegsia production. Barracks should gain the
Panzerfaust train button after a completed Training Centre, with the approved cost, supply, build
time, hotkey, tooltip, disabled states, queue behavior, cancel/refund behavior, and mirrored catalog
data. AI should continue not training Panzerfaust units in the first pass, while AI-owned spawned
Panzerfaust units can use the already-tested target acquisition behavior.

### [Phase 6 - Audio And Feedback Polish](phase-6.md)

Add the first intentional audio and feedback polish pass for the Panzerfaust shot and conversion.
The unit should not ship with misleading Tank, Rifleman, artillery, or debug-only sounds or effects
for its unique launch, travel, impact, recovery, and conversion moments. This phase should keep the
visual and audio treatment modest but deliberate, with deferred final polish recorded rather than
silently skipped.

### [Phase 7 - Integration Regression And Balance Readiness](phase-7.md)

Add the final focused regression matrix for the fully integrated unit. Cover production, order
queues, cancellation, target filters, fog projection, death cleanup, conversion continuity,
Methamphetamines, Entrenchment, lab/dev inspection, and client feedback. This phase should leave a
small set of reliable local and CI checks plus manual playtest notes for judging whether the
approved numbers need later tuning.

### [Phase 8 - Docs, Patch Notes, And Review Package](phase-8.md)

Close documentation drift and prepare the final review package for the Panzerfaust feature. Contract
docs still belong in the same phase as each contract change, so this phase is for final alignment,
patch-note cleanup, generated surfaces, known deferred items, and review ergonomics. A reviewer
should be able to answer what changed, how to try it, what was tested, and what remains intentionally
deferred without reconstructing the feature from code.

## Phase Index

2. [Phase 2 - Hidden Vocabulary, Balance, And Protocol Contract](phase-2.md)
3. [Phase 3 - Hidden Server Runtime And Conversion](phase-3.md)
4. [Phase 4 - Client State, Visuals, And Lab Inspection](phase-4.md)
5. [Phase 5 - Production Exposure And Command Card](phase-5.md)
6. [Phase 6 - Audio And Feedback Polish](phase-6.md)
7. [Phase 7 - Integration Regression And Balance Readiness](phase-7.md)
8. [Phase 8 - Docs, Patch Notes, And Review Package](phase-8.md)

## Overall Constraints

- Keep [checklist.md](checklist.md) as the product behavior source. If implementation discovers a
  conflict with the checklist, stop as blocked instead of inventing new Panzerfaust behavior.
- Do not expose the Panzerfaust in normal production until Phase 5. Earlier phases may add hidden
  vocabulary, tests, internal spawn paths, lab/dev inspection support, and parser fallbacks, but
  normal players should not be able to train a half-finished unit.
- Preserve the approved unit contract: Barracks training after completed Training Centre, hotkey
  `E`, 60 steel / 15 oil, 1 supply, 400-tick build time, 45 HP, 8-tile sight, 9 px radius,
  1.44 px/tick loaded speed, 3-tile loaded range, one 60-damage Tank-only armor-piercing shot,
  15-tick windup, 15-tick travel, 15-tick recovery, then same-id conversion into a Rifleman.
- Preserve Methamphetamines and Entrenchment interactions from the checklist. Methamphetamines
  speeds loaded movement and setup/recovery but does not allow loaded moving fire; active trench
  occupation extends loaded range to 4 tiles through the existing Entrenchment range rule.
- Keep server authority. The server owns target legality, attack acquisition, windup cancellation,
  launch consumption, projectile resolution, conversion, production prerequisites, refunds, fog
  projection, and AI behavior.
- Keep `Game::tick()` panic-free. Stale ids, dead targets, hidden targets, illegal target kinds,
  interrupted orders, disconnected clients, impossible positions, and non-finite values must be safe
  no-ops or recoverable errors.
- Maintain fog guarantees. Panzerfaust target ids, launch points, travel paths, impact positions,
  conversion state, death events, and feedback events must not reveal hidden entities or positions.
- Keep protocol mirrors synchronized across `server/crates/protocol/src/lib.rs`,
  `server/src/protocol.rs`, `server/crates/sim/src/protocol.rs`, `client/src/protocol.js`, compact
  snapshot metadata if touched, and [docs/design/protocol.md](../../../docs/design/protocol.md).
- Keep balance and catalog mirrors synchronized across Rust rules, faction catalog exports,
  `client/src/config.js` and its internal mirrors, generated wiki/stats surfaces, and
  [docs/design/balance.md](../../../docs/design/balance.md).
- Respect client architecture. New client state, renderer, HUD, audio, and lab inspection paths
  should use existing dependency-injection patterns and must clean up listeners, timers, textures,
  sounds, and GPU resources.
- Do not broaden the first-pass target filter beyond Tanks, add hull-facing multipliers, add a
  player-activated ability, add reloads, or teach AI to train Panzerfaust units unless the checklist
  is explicitly revised first.
- Collect factual patch-note bullets during gameplay phases: train source and unlock, cost/supply,
  build time, one-shot damage/range/timing, conversion to Rifleman, Methamphetamines interaction,
  Entrenchment interaction, UI affordance, and expected playtest watch points.
- Each implementation phase must land on its own `zvorygin/` branch, be pushed as an owned PR with
  auto-merge armed, and wait for a definite merge with the phase head reachable from `origin/main`
  before the next phase starts.
- After implementing each phase, the implementing agent must provide a handoff message describing
  what changed, what the next agent should do, and what should be manually tested. Manual testing
  notes should name core gameplay scenarios, not an exhaustive test matrix.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Required Verification Themes

Each phase should run the smallest relevant subset of:

- focused Rust tests for rules definitions, faction catalogs, production prerequisites, combat
  target legality, windup/travel/recovery timing, conversion continuity, order queues, fog
  projection, Methamphetamines, Entrenchment, death cleanup, and AI build exclusions
- `node tests/protocol_parity.mjs` and focused protocol contract tests after unit-kind, snapshot,
  event, compact transport, or parser changes
- `node scripts/check-faction-catalog-parity.mjs` and `node scripts/check-wiki.mjs` after visible
  rules, catalog, command-card, generated stats, or wiki surface changes
- focused client contract tests for HUD command cards, config mirrors, state event handling,
  rendering feedback, rig runtime, audio mapping, lab spawn/inspection, and input/command behavior
- `node scripts/check-client-architecture.mjs` after client module, renderer, HUD, lab, or audio
  wiring changes
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` after
  sim service boundary or `rts-sim::game` module-ownership changes
- `node scripts/check-docs-health.mjs` for docs-heavy phases
- `git diff --check`

## Suggested Execution

Implement one phase at a time from a clean worktree. Do not start a later phase from an assumed
merge; wait for the owned PR to merge and verify reachability from `origin/main`.

```bash
scripts/phase-runner.sh --plan panzerfaust 2 --pr --wait
scripts/phase-runner.sh --plan panzerfaust 3 --pr --wait
scripts/phase-runner.sh --plan panzerfaust 4 --pr --wait
scripts/phase-runner.sh --plan panzerfaust 5 --pr --wait
scripts/phase-runner.sh --plan panzerfaust 6 --pr --wait
scripts/phase-runner.sh --plan panzerfaust 7 --pr --wait
scripts/phase-runner.sh --plan panzerfaust 8 --pr --wait
```
