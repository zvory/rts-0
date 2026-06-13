# Phase 11 - Roster Expansion, Integration, and Rollout

Status: Designed, not implemented.

## Objective

Expand the second faction only as far as the approved brief allows, then harden mixed-faction play
for regular playtesting. This phase decides rollout readiness, AI restrictions, documentation, and
remaining balance risks.

## Scope

- Add approved additional units, buildings, upgrades, abilities, and progression slices
  incrementally, each with targeted tests and patch-note bullets.
- Add lobby selection UX for faction choice if it was not exposed earlier.
- Keep AI current-faction-only with validation and UI messaging unless a separate approved AI scope
  implements second-faction AI.
- Update self-play/dev scenario tools so faction choice is explicit where needed.
- Audit replay, branch, match history, spectator, debug mode, quickstart, dev scenario, and
  post-match flows under the new non-backcompat schema.
- Run fog/security checks for every second-faction ability and event.
- Review performance impact of extra catalog lookups, ability systems, and renderer additions.
- Update design docs and context capsules so future faction work starts from the new architecture.
- Decide whether faction choice is ready for normal selection or should remain dev/local only.

## Expected Touch Points

- `server/src/lobby/`
- `server/crates/ai/src/` only for validation/restriction unless AI support is approved
- `server/crates/sim/src/game/replay.rs`
- `server/crates/sim/src/game/setup/dev_scenarios/`
- `client/src/lobby.js`
- `client/src/match_history.js`
- `client/src/replay_viewer.js`
- `client/src/match.js`
- `tests/`
- `docs/context/`
- `docs/design/`
- `plans/faction/`

## Verification

- Server integration tests for faction selection, AI restriction, and mixed-faction starts.
- Replay/branch tests preserving faction identity and catalogs under the new schema.
- AI integration tests for restriction messaging, or broader AI tests only if AI support is enabled.
- Client smoke test for lobby faction selection and in-match HUD.
- Fog/security regression tests for mixed-faction ability events.
- Architecture checks for client and sim if module boundaries changed.
- Targeted performance or self-play tests if new mechanics add heavy tick work.
- Balance docs and patch notes updated for every player-facing stat/mechanic added.

## Manual Testing Focus

Create a mixed-faction lobby, start a match, verify each player receives the right faction start,
play through basic production and combat, surrender or finish the match, then inspect replay or
post-match flow. Verify unsupported AI faction choices are clearly blocked.

## Handoff Expectations

The handoff must state whether faction selection is ready for normal playtesting, whether AI support
is enabled or intentionally blocked, which docs/context capsules were updated, remaining balance
risks, and recommended playtest scenarios.

## Player-Facing Outcome

Faction choice becomes a supported playtest feature, or remains intentionally dev-gated with clear
remaining work.
