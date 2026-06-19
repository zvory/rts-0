# Phase 5 - Entity Delta And Keyframe Protocol

## Phase Status

- [ ] Tentative. Needs Phase 3 design approval before implementation.

## Objective

Implement stateful entity deltas and periodic keyframes if the measurement and encoding phases show
that format-level savings are not enough. This placeholder is not runner-ready.

## Tentative Scope

- Entity add/update/remove deltas for the compact entity schema.
- Optional-section deltas for production, rally, order plans, abilities, setup, debug path, and build
  state.
- Delta handling for ability objects, remembered buildings, upgrades, transient events, player
  resources, and net status.
- Versioned keyframes that reset the client baseline.
- Client-side reconstruction into the current semantic snapshot shape before `GameState.applySnapshot`.
- Recovery from missed, stale, duplicate, unsupported, or out-of-order delta frames.
- Tests that prove fog-gated data is not leaked through retained baselines or removed visibility.

## Required Follow-up Before Execution

Do not implement this phase until Phase 3 is rewritten into an approved design and Phase 4 either
lands or is intentionally skipped. The rewritten phase should split entity deltas into smaller
implementation chunks if needed; this work is likely too broad for one safe PR.
