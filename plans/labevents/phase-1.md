# Phase 1 - Server Event Projection Contract

Status: planned.

## Goal

Make lab transient event delivery match lab visibility. A lab full-world recipient should receive
the union of transient events for all active lab players while preserving the existing full-world
snapshot body, and lab team/teams recipients should receive event unions for only the selected
visible players.

## Scope

- Introduce an explicit event-projection decision in the room snapshot fanout path. Prefer a small
  enum or struct that separates snapshot body selection from event bucket selection, for example
  player-only, selected-player union, and full visible-player union.
- Update lab snapshot projection construction so:
  - `FullWorld` lab vision uses a full-world snapshot body with all active lab player event buckets.
  - `Team` lab vision uses the selected team's player event buckets.
  - `Teams` lab vision uses the selected teams' player event buckets.
- Preserve current normal live active-player behavior: active player snapshots still attach only the
  active player's event bucket.
- Preserve current replay/team-union behavior unless the new abstraction exposes an existing
  destructive-read bug. If changed, document the intended behavior in the phase handoff.
- Avoid consuming event buckets for projections that can be shared by multiple recipients. Multiple
  lab viewers in the same full-world or team view must receive identical transient events.
- Add focused server tests. At minimum cover:
  - A default full-world lab operator issuing `issueCommandAs` for P2 mortar fire receives the
    P2 `mortarLaunch` event before impact.
  - Two default full-world lab viewers both receive the same P2 `mortarLaunch` event.
  - A lab operator with Team 2 vision receives P2 launch events, while Team 1 vision does not
    receive owner-only P2 launch events unless normal visibility rules would include them.
  - Existing normal live mortar privacy expectations still pass.
- Update `docs/design/protocol.md` if the lab event-visibility contract is not already stated
  clearly enough. Refresh `docs/context/protocol.md` only if the section map or capsule summary
  needs to change.

## Expected Touch Points

- `server/src/lobby/projection.rs`
- `server/src/lobby/live_tick.rs`
- `server/src/lobby/room_task/lab.rs`
- `server/src/lobby/room_task/tests/lab.rs` or `server/src/lobby/room_task/tests/lab_timeline.rs`
- `server/crates/sim/src/game/tests/smoke_mortar_tests.rs` only if an existing sim-level privacy
  test needs to be clarified
- `docs/design/protocol.md` if the event projection contract changes textually

## Constraints

- Do not change the `Event::MortarLaunch` wire shape.
- Do not move mortar warning responsibility to the client or to local command feedback.
- Do not make lab full-world impersonate P2 for all player-scoped snapshot fields. The snapshot body
  can keep the current lab view-player behavior while event projection becomes a union.
- Keep fog/privacy guardrails for normal live matches. This phase should not make hidden enemy
  mortar launch events visible outside lab-authorized projections.
- Keep the implementation small. If a full projection-layer rewrite becomes tempting, stop and
  choose the smallest abstraction that removes the lab inconsistency.

## Verification

- Run the focused Rust tests added or changed for room/lab projection.
- Run the existing mortar privacy tests that cover manual and autocast launch delivery.
- Run `cargo fmt` for touched Rust files.
- If protocol docs changed, run the cheapest relevant doc/contract check already used for protocol
  touch points, or explain why no protocol parity command was needed.

## Manual Testing Focus

- In lab default full-world mode, issue Mortar Team fire as P2 and confirm the persistent warning
  circle appears before the explosion.
- Repeat with P2 mortar autocast enabled and confirm the warning circle appears.
- In a normal live match, confirm a player does not gain hidden enemy mortar launch warning markers
  outside the documented visibility rules.

## Handoff

After implementation, mark this phase done in this file and summarize the final event-projection
abstraction, the exact tests added, and whether any protocol text changed. The next agent should
use that summary to add or adjust the externally observable regression in Phase 2.
