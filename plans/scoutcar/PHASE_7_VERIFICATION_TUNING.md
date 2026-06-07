# Phase 7 - Verification and Tuning

## Goal

Turn the new scout-car movement model from mechanically correct into player-reliable.

## Verification Matrix

Run and keep fixtures for:

- open-field long moves;
- tight building corners;
- two-tile lanes;
- diagonal pinches;
- wall-parallel movement;
- side-by-side building adjacency followed by a turn-away command;
- near-goal reverse;
- far-behind retarget;
- mixed army movement;
- scout-car groups;
- attack-move while moving;
- live server smoke around factories and base traffic.

## Tuning Knobs

Tune conservatively:

- capsule length/width/clearance;
- hard and preferred clearance thresholds;
- clearance cost scale;
- turn cost scale;
- local primitive curvature set;
- reverse penalty;
- wall-response tangent limit;
- no-progress/repath thresholds.

Avoid tuning by hiding bugs. If a legal route exists but the planner repeatedly chooses contact, fix
the scoring or swept legality rather than increasing recovery frequency.

## Documentation

Update:

- `docs/design/server-sim.md` scout-car movement section;
- `docs/context/server-sim.md` if section pointers shift;
- client config comments if authoritative body metadata changes;
- final patch-note bullets for player-facing movement impact.

## Acceptance Criteria

- Scout cars route around buildings with fewer blocked static steps than the Phase 0 baseline.
- Scout cars do not clip terrain or building occupancy in invariant checks.
- Scout cars traverse intended tight spaces without repeated reverse-recovery loops.
- Scout cars avoid wall-hugging in open alternatives.
- Scout cars legally side-by-side with buildings can turn away without the old rectangular
  rear-corner clipping behavior and without accepting any swept capsule overlap.
- Scout cars preserve their identity: fast, light, wheeled, no tank pivot, no sideways infantry
  sliding.

## Suggested Test Commands

- `cd server && cargo test`
- `node tests/server_integration.mjs`
- `node tests/regression.mjs`
- `node tests/ai_integration.mjs`
- `cd tests && npm install && node client_smoke.mjs` when client visuals/advisory previews change

## Done When

- The implementation is covered by deterministic tests and at least one manual/local smoke scenario.
- Patch notes can honestly say scout cars now avoid walls and navigate tight base spaces more
  reliably, without overstating that every traffic jam is impossible.
