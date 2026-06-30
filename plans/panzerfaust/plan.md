# Panzerfaust Plan

The Panzerfaust effort is currently in the new-unit planning gate only. Phase 0 and Phase 1 are
captured in [checklist.md](checklist.md) for user review before any implementation files are
edited. Implementation must not begin until the brief and rules/balance spec are approved.

## Phase Summary

Phase 0 defines the unit identity: an infantry anti-tank ambusher with one Panzerfaust shot that
then becomes a Rifleman. It records the player-facing role, counters, unusual interactions, initial
exposure, and draft patch notes.

Phase 1 defines the reviewable rules and numbers: cost, supply, build source, Training Centre
unlock, HP, speed, sight, range, one-shot timing, target filters, Methamphetamines and Entrenchment
interactions, and AI scope. It keeps unresolved decisions visible instead of letting implementation
infer them.

Later phases cover protocol, simulation, client UI, visuals, audio, tests, and review packaging.
Those phases should be written only after Phase 0/1 are approved or revised by the user.

## Constraints

- Follow [docs/new-unit-checklist.md](../../docs/new-unit-checklist.md).
- Do not edit Rust, JavaScript, protocol, art, generated config, or test implementation files until
  Phase 0 and Phase 1 are approved.
- Balance-visible values must eventually update Rust rules and the client mirror together.
- Any snapshot, event, order, projectile, or conversion state added later must update the protocol
  mirrors and fog-gating documentation in the same implementation change.
- Every implementation phase must be committed on its own `zvorygin/` branch, pushed as an owned PR
  with auto-merge armed, and waited through `scripts/wait-pr.sh` before claiming completion.

## Handoff Requirement

Each future implementation handoff must name the manually testable core behavior for that phase,
including how to train or spawn the Panzerfaust, how to fire the one-shot weapon, how conversion to
Rifleman is observed, and which fog or protocol behavior needs special review.
