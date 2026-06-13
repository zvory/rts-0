# Phase 2 - Scripted AI Skeleton

Status: not started

## Goal

Introduce the new AI architecture behind a non-default profile id while leaving the existing
`rifle_flood_full_saturation` AI untouched. This phase should produce a debuggable skeleton:
phase selection, manager interfaces, goal blockers, and command traces, but only minimal behavior.

## Scope

- Add a new profile id for the 1.0 opponent, for example `launch_standard`.
- Keep `rifle_flood_full_saturation` available as the baseline and rollback profile.
- Add a strategic director that chooses phases from durable progress:
  - completed buildings
  - worker/saturation progress
  - attack waves launched
  - expansion started/completed
  - elapsed-time fallback
  - army/production milestones
- Add typed phase targets for economy, buildings, production, research, and attacks.
- Add manager interfaces that report:
  - desired target
  - current progress
  - blocker, if any
  - selected high-level action or emitted command, if any
- Reuse the existing `AiActionContext` / `ai_core::actions` seam for executable work instead of
  introducing a second command-emission path.
- Add an AI decision trace structure that can be used by tests now and exposed in debug surfaces
  later.
- Add parity tests proving the old live AI profile still runs and remains selectable.

## Expected Touch Points

- `server/crates/ai/src/ai_core/profiles.rs`
- `server/crates/ai/src/live.rs`
- New or existing modules under `server/crates/ai/src/ai_core/`
- `docs/design/ai.md`

## Verification

- Pure tests for phase unlock ordering and fallback behavior.
- Pure tests for trace generation and stable sorting.
- Existing live AI tests proving the current default/baseline profile is still valid.
- A scenario-harness test that runs the new profile skeleton for a few decisions and confirms it
  emits explainable blockers instead of panicking or over-reserving resources.

## Manual Testing Focus

- Start a local AI self-play/watch run with the old saturation profile and confirm it still works.
- If a dev-only profile selector exists, confirm the new profile can be selected without becoming
  the normal live-lobby default.

## Handoff

The handoff should describe the phase model, the trace format, the profile id names, and which
manager behaviors are still stubbed for Phase 3. It should also call out any new action helper
added to `ai_core::actions` so later managers use the shared reservation and command path.
