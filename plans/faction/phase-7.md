# Phase 7 - Integration, AI, Rollout, and Documentation

Status: Designed, not implemented.

## Objective

Harden multi-faction play after the second faction exists. This phase verifies mixed-faction
matches, decides AI scope, updates documentation, and prepares the feature for normal lobby
selection.

## Scope

- Add lobby selection UX for faction choice if it was not exposed earlier.
- Verify all supported match shapes with mixed factions.
- Decide AI support:
  - keep AI current-faction-only with validation and UI messaging, or
  - add faction-aware AI profiles/actions for the new faction.
- Update self-play/dev scenario tools so faction choice is explicit.
- Audit replay, branch, match history, spectator, debug mode, quickstart, and dev scenario flows.
- Run fog/security checks for every new ability and event.
- Review performance impact of extra catalog lookups, ability systems, and renderer additions.
- Update design docs and context capsules so future faction work starts from the new architecture.

## Expected Touch Points

- `server/src/lobby/`
- `server/crates/ai/src/`
- `server/crates/sim/src/game/replay.rs`
- `server/crates/sim/src/game/setup/dev_scenarios/`
- `client/src/lobby.js`
- `client/src/match_history.js`
- `client/src/replay_viewer.js`
- `tests/`
- `docs/context/`
- `docs/design/`
- `plans/faction/`

## Verification

- Server integration tests for faction selection and mixed-faction starts.
- Replay/branch tests preserving faction identity and catalogs.
- AI integration tests if AI faction support is enabled.
- Client smoke test for lobby faction selection and in-match HUD.
- Fog/security regression tests for mixed-faction ability events.
- Architecture checks for client and sim if module boundaries changed.
- Targeted performance or self-play tests if new mechanics add heavy tick work.

## Manual Testing Focus

Create a mixed-faction lobby, start a match, verify each player receives the right faction start,
play through basic production and combat, surrender or finish the match, then inspect replay or
post-match flow. If AI is restricted, verify the lobby clearly prevents unsupported AI faction
choices.

## Handoff Expectations

The handoff must state whether the feature is ready for normal selection, whether AI support is
enabled or intentionally blocked, and which docs/context capsules were updated. It should include
remaining balance risks and recommended playtest scenarios.

## Player-Facing Outcome

Faction choice becomes a supported game feature rather than a dev-only path. Mixed-faction matches,
replays, and documentation should be stable enough for regular playtesting.

