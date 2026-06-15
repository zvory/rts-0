# Phase 10 - Cleanup, Docs, and Regression Coverage

Status: Not Started.

## Goal

Consolidate the 0.1 ability foundation, remove obsolete special-case paths, and document the new
runtime contracts. This is a cleanup and hardening phase, not a feature expansion phase.

## Scope

- Update design docs:
  - `docs/design/server-sim.md` for ability runtime, tick order, and command/recast semantics
  - `docs/design/protocol.md` for ability objects, events, and compact snapshot fields
  - `docs/design/client-ui.md` for projected ability objects and richer previews
  - `docs/design/balance.md` for factual Ekat ability behavior and fun-test caveats
- Refresh any affected context capsules if section structure changed.
- Remove or isolate obsolete one-off Ekat teleport/line-shot hooks that are no longer used.
- Ensure ability metadata remains Rust-authoritative and client catalog parity still covers Ekat
  command-card descriptors.
- Add or consolidate regression coverage for:
  - replay keyframe cloning and seek behavior with active ability runtime state
  - fog-filtered ability objects and projectile events
  - dash return validity
  - anchor destruction lockout
  - dual-origin projectile composition
  - client decode/render/preview contracts
- Add developer diagnostics only where they help future ability debugging, such as compact debug
  logs or self-play fixture notes; avoid noisy production logs.
- Collect factual patch-note bullets for player-facing Ekat behavior.

## Expected Deliverables

- Design docs describe the actual implemented ability foundation.
- Obsolete Ekat special paths are removed or clearly marked as compatibility/test-only.
- Focused regression coverage exists for the highest-risk server, protocol, and client seams.
- The plan can be handed to a final review pass with known follow-ups rather than hidden
  architecture debt.

## Out of Scope

- New abilities.
- Balance tuning beyond factual documentation.
- Full art, sound, animation, or AI support.
- Broad rewrites of mortar, artillery, smoke, combat, pathing, or prediction.

## Verification

- Run focused tests added or touched by the phase.
- Run protocol parity checks.
- Run client architecture checks if client module boundaries changed during cleanup.
- Run the sim architecture check if server module boundaries changed.
- Let the ordinary commit hook provide broader coverage when the phase is ready to merge.

## Manual Testing Focus

Run one Ekat fun-test match and exercise dash/return, line projectile, anchor placement, anchor
destruction, and anchor-enhanced line projectile. Confirm no obvious console errors, hidden object
leaks, or stuck cooldown/lockout states appear during the core loop.

## Handoff Expectations

The handoff must summarize final docs, tests, removed special cases, patch-note bullets, and the
remaining follow-ups that should be considered outside the 0.1 system foundation.
