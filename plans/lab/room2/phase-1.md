# Phase 1 - Capability Baseline

## Phase Status

- [x] Done.

## Objective

Record the current room, replay, dev scenario, replay branch, and lab behavior as explicit capability
bundles before changing runtime code. This phase should turn the architectural diagnosis from
`plan.md` into a concrete current-state baseline with focused tests that future phases can preserve.

## Work

- Audit latest `main` for mode-shaped decisions in room policy, tick control, projection, start
  payload composition, snapshot diagnostics, client controls, settings, replay/dev protocol messages,
  lab operation routing, and docs.
- Create `plans/lab/room2/capability-matrix.md` with one row per product path: normal lobby, normal
  live match, live spectator, post-match replay, persisted replay room, saved replay artifact,
  replay branch staging, replay branch live, dev scenario, lab operator, and lab read-only viewer.
- For each row, record state source, lifecycle/joining, clock, authority, mutation, visibility,
  diagnostics, persistence/export, start payload affordances, and manual-smoke focus.
- Add or tighten characterization tests where a later phase would otherwise be guessing. Prioritize
  lab mutation classification, dev pause/step, replay speed/seek/vision, debug path availability,
  spectator projection, branch seat aliases, and normal-match isolation.
- List every known product-mode leak to remove later, including `debugMode`, `devWatch.kind`,
  replay-named time controls, replay-named time state, `DebugHuman`-driven diagnostics, and any lab
  behavior hidden inside authority or visibility instead of mutation policy.
- Do not rename protocol fields or move runtime behavior in this phase.

## Expected Touch Points

- `plans/lab/room2/capability-matrix.md`
- `plans/lab/room2/phase-1.md`
- `server/src/lobby/session_policy.rs`
- `server/src/lobby/tick_control.rs`
- `server/src/lobby/projection.rs`
- `server/src/lobby/room_task.rs`
- `server/crates/sim/src/game/snapshot.rs`
- `client/src/match.js`
- `client/src/state.js`
- `client/src/replay_controls.js`
- focused tests under `server/src/lobby/tests.rs`, `server/src/lobby/room_task.rs`, and client
  contract tests as needed

## Implementation Checklist

- [x] Add the capability matrix with all current product paths.
- [x] Add an inventory section for mode-shaped leaks and the phase expected to remove each leak.
- [x] Add focused tests for lab mutation classification or policy coverage if missing.
- [x] Add focused tests for diagnostic availability if missing.
- [x] Add focused tests for current replay/dev time-control behavior if missing.
- [x] Mark this phase as done in this file.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server session_policy`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `cargo test --manifest-path server/Cargo.toml -p rts-server replay`
- `cargo test --manifest-path server/Cargo.toml -p rts-server dev`
- `node tests/client_contracts.mjs` if client behavior tests are touched
- `git diff --check`

If a filtered command matches zero tests, add or use exact test names before counting it as
verification.

## Manual Test Focus

No live browser smoke is required unless this phase unexpectedly changes runtime code. If it does,
manually check one normal match, one replay with vision selection, one dev scenario pause/step flow,
and one lab operator setup flow.

## Handoff Expectations

Summarize the capability matrix, list the exact leaks that Phase 2 should address first, name the
tests added or tightened, and state whether the next phase can safely begin policy changes.
