# Phase 6 - Hardening, Replay, and Cleanup

Status: Planned.

Goal: lock the command-intent system down after the server and client paths are unified.

## Scope

- Raise the unit queue cap from 8 to 16 if playtesting confirms the need.
- Audit command input bounds:
  - unit-list dedupe/cap
  - queue cap notices
  - non-finite coordinates
  - invalid ids and stale target ids
  - fog-hidden direct targets
  - malformed ability target modes
- Add replay coverage for mixed command-intent sequences.
- Ensure command logs preserve any new queued/setup/ability fields needed for deterministic replay.
- Remove obsolete client special cases such as attack-only held-key exceptions once the composer
  owns arming semantics.
- Remove or narrow `#![allow(dead_code)]` on `order_planner.rs` once live command handling uses it.
- Update docs and patch notes for player-facing behavior.

## Regression Scenarios

- Smoke wall then attack-move with more selected scout cars than smoke clicks.
- Two scout cars receiving four queued smokes round-robin, then later attack-move.
- Move, queued Charge, queued attack-move.
- Packed AT guns move, then queued setup facing point.
- Immediate reactive smoke from a moving scout car that already has a destination.
- Queue-full attempts on capped units.
- Hidden/dead attack target stages skipped without panic.

## Tests

- `cargo test -p rts-sim`
- replay determinism for mixed command-intent command logs
- `node tests/server_integration.mjs`
- `node tests/regression.mjs`
- `node tests/ai_integration.mjs`
- `cd tests && npm install && node client_smoke.mjs`

## Done

- Planner, live command service, client composer, protocol docs, and player-visible behavior agree.
- No ad hoc command arming exceptions remain outside the composer.
- Queue and command hardening invariants are documented and tested.
