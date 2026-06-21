# Hotspot Cleanup Execution Plan

## Purpose

Turn the ranked hotspot backlog from `plans/hotspots/extraction-candidates.md` into executable
cleanup phases. The goal is to reduce review context and future merge risk without changing
gameplay, wire protocol behavior, balance values, UI affordances, or test meaning.

This plan handles the cleanup candidates that are appropriate for phase-runner execution. CSS
modularization and room-task runtime extraction remain deferred because the hotspot triage marked
them as requiring visual workflow support or active room-runtime ownership before automated cleanup.

## Phase Summaries

### [Phase 1 - Client Contract Foundation](phase-1.md)

Create the shared helper shape for `tests/client_contracts.mjs` and move a small set of low-risk
contract sections into imported modules under `tests/client_contracts/`. Keep
`node tests/client_contracts.mjs` as the stable command and preserve every existing assertion. This
phase proves the split pattern before moving the rest of the top hotspot.

### [Phase 2 - Client Contract Domain Split](phase-2.md)

Finish splitting the remaining `tests/client_contracts.mjs` sections by contract area. The top-level
runner should become a short dependency-free importer while helpers and domain files carry the
existing assertions. This makes later HUD, state, match, protocol, and config cleanup easier to
review because their contract coverage is smaller and easier to load.

### [Phase 3 - Command Service Guards and Tests](phase-3.md)

Split command-service tests by behavior family and extract pure input guard helpers from
`server/crates/sim/src/game/services/commands.rs`. Preserve `apply_commands`, command ordering,
receipt ordering, resource mutation order, replay determinism, and queued-order behavior. This
reduces context in the third-ranked hotspot without changing gameplay command semantics.

### [Phase 4 - Broad Sim Game Test Split](phase-4.md)

Convert `server/crates/sim/src/game/tests.rs` into a test module root split by behavior family. Move
shared fixtures first, then move assertions without rewriting them or changing public `Game` setup
APIs. This phase is test organization only and should keep fog, replay determinism, and observer
analysis checks plainly auditable.

### [Phase 5 - HUD Helper Extraction](phase-5.md)

Extract focused HUD helpers for resource display, control-group rendering, and command-card DOM
assembly behind the existing `HUD` public surface. Keep command ids, descriptors, affordability,
cooldowns, command issuer calls, and `ClientIntent` usage stable. This is the first runtime-client
cleanup because it has strong contract coverage and clear local helper patterns.

### [Phase 6 - GameState Helper Extraction](phase-6.md)

Extract narrow `GameState` helper modules for transient visual-effect buffers and read/query helpers.
Keep snapshot application order, interpolation semantics, public data shape, and the separation
between server state and browser-local `ClientIntent` unchanged. This phase should avoid prediction
or optimistic-overlay moves unless the first helper split makes that obviously mechanical.

### [Phase 7 - Match Shell Collaborators](phase-7.md)

Extract small collaborators from `client/src/match.js` for net-report or ping management, combat
audio event handling, and settings action wiring. `Match` remains the composition shell with explicit
injected dependencies and unchanged lifecycle ordering. This phase must preserve frame order,
teardown, replay/lab/live wiring, and room-time behavior.

### [Phase 8 - AI Self-Play Test Split](phase-8.md)

Split AI self-play tests and harness helpers by domain while preserving quick-test defaults and
`RTS_FULL_AI_TESTS=1` gates. Replay artifact schema and failure artifact usefulness must stay stable.
This phase reduces a high-churn test hotspot without changing AI behavior.

### [Phase 9 - Protocol Mirror Cleanup Gate](phase-9.md)

Create a design-first protocol cleanup handoff rather than moving protocol code immediately. The
phase should read the current protocol design, inspect Rust and JS mirror boundaries, and write a
separate phase-runner-ready protocol cleanup plan only if it can preserve every tag, field, compact
code, version, and exported API. If no safe plan exists, mark the gate done with a clear no-go
decision and the evidence.

### [Phase 10 - Balance Mirror Cleanup Gate](phase-10.md)

Create a design-first balance/config cleanup handoff rather than moving balance code immediately.
The phase should inspect Rust authoritative rules, client-visible mirrors, parity scripts, and wiki
checks, then write a separate phase-runner-ready balance cleanup plan only if it can guarantee no
numeric drift. If generation or validation is the safer next step, plan that before any manual file
split.

## Phase Index

1. [Phase 1 - Client Contract Foundation](phase-1.md)
2. [Phase 2 - Client Contract Domain Split](phase-2.md)
3. [Phase 3 - Command Service Guards and Tests](phase-3.md)
4. [Phase 4 - Broad Sim Game Test Split](phase-4.md)
5. [Phase 5 - HUD Helper Extraction](phase-5.md)
6. [Phase 6 - GameState Helper Extraction](phase-6.md)
7. [Phase 7 - Match Shell Collaborators](phase-7.md)
8. [Phase 8 - AI Self-Play Test Split](phase-8.md)
9. [Phase 9 - Protocol Mirror Cleanup Gate](phase-9.md)
10. [Phase 10 - Balance Mirror Cleanup Gate](phase-10.md)

## Overall Constraints

- Start every phase from fresh `origin/main` in an isolated `/tmp/rts-worktrees` worktree on a
  `zvorygin/` branch.
- Preserve unrelated dirty state, especially `playtest_notes.md`.
- Each phase must be pushed as an owned PR with auto-merge armed, then waited on until GitHub
  reports the PR merged and the phase head is reachable from `origin/main`.
- When a phase is complete, mark that phase document as done in the same implementation commit.
- Do not change gameplay behavior, wire protocol shape, balance values, command semantics, UI
  affordances, or assertion meaning unless the phase is explicitly rewritten as a behavior plan.
- Treat test splits as moves and helper extraction, not as permission to delete coverage or rewrite
  assertions.
- Preserve stable public commands and APIs, especially `node tests/client_contracts.mjs`,
  `Game::tick`, `Game` public methods, `apply_commands`, `HUD`, `GameState`, and `Match`.
- Keep mirrored contracts synchronized. Protocol and balance surfaces need design-doc and parity
  review before code movement; Phase 9 and Phase 10 are gates, not automatic refactors.
- If a split creates new files that are not already covered by `scripts/hotspot-analysis.mjs`
  architectural groups, update both `scripts/hotspot-analysis.mjs` and
  `plans/hotspots/group-map.md` in the same phase.
- After any phase that materially splits a hotspot, rerun
  `node scripts/hotspot-analysis.mjs --base-ref HEAD --recent-days 14 --limit 0 --output /tmp/rts-hotspots-after.json`
  and summarize whether the logical group stayed trackable.
- Use focused verification during phase work. The PR gate remains the authoritative full
  `./tests/run-all.sh` check.
- Every phase handoff must say what changed, what the next agent should do, which focused commands
  passed, what uncertainty remains, and which core feature should be manually tested.

## Deferred From This Runner Chain

- CSS section modularization is not included yet. It needs an explicit static-load and visual
  verification workflow before selector/file movement is worth automating.
- Room task runtime extraction is not included yet. The hotspots triage marked it read-only because
  active room-runtime plans own that surface.
- Protocol and balance code movement should not be run from this plan. Phase 9 and Phase 10 may
  create separate plans after proving the guardrails.

## Suggested Execution

Run one phase at a time and wait for each PR to merge before starting the next phase:

```bash
scripts/phase-runner.sh --plan hotspotcleanup phase-1 phase-2 phase-3 phase-4 phase-5 phase-6 phase-7 phase-8 phase-9 phase-10 --pr --wait
```

For a smaller first wave, run through the low-risk test and command-service cleanup only:

```bash
scripts/phase-runner.sh --plan hotspotcleanup phase-1 phase-2 phase-3 phase-4 --pr --wait
```
