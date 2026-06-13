# Phase 6 - Shared Current Vision

Status: planned.

## Goal

Make authoritative current line of sight shared across living teammates. This phase should focus on
fog grids and `visibleTiles`, leaving allied projection details and transient event privacy to the
next phase.

## Scope

- Recompute authoritative current fog by team, or stamp each entity's sight into every living
  teammate's player grid.
- Keep neutral resource nodes from granting vision.
- Preserve smoke blocking and lingering death sight semantics under team vision.
- Ensure `visibleTiles` sent to each player reflects team current vision.
- Ensure a living player with no own entities but living teammates still receives team current
  vision.
- Ensure defeated/disconnected/eliminated teammates stop contributing live sight.
- Keep command authority and resource/upgrades snapshots local-player-only.
- Do not broaden allied entity detail projection, remembered buildings, target tracers, or
  support-fire marker event fanout in this phase except where needed to prove fog grids.

## Expected Touch Points

- `docs/design/server-sim.md`
- `docs/design/protocol.md`
- `docs/design/hardening.md`
- `server/crates/sim/src/game/teams.rs`
- `server/crates/sim/src/game/fog.rs`
- `server/crates/sim/src/game/snapshot.rs`
- `server/crates/sim/src/game/mod.rs`
- `tests/team_integration.mjs`
- `tests/regression.mjs`

## Verification

```bash
cd server && cargo test fog --workspace
cd server && cargo test team --workspace
node tests/team_integration.mjs
node tests/regression.mjs
```

Required automated scenarios:

- Ally scout reveals an enemy to a teammate's snapshot.
- Enemy outside all allied sight is absent from every teammate snapshot.
- Player with no own entities but living allies still receives team `visibleTiles`.
- Defeated/eliminated teammate sight no longer contributes to team current vision.
- Smoke still blocks non-owner/team visibility under team fog.
- Lingering death sight remains snapshot-only and follows the documented team rules.
- Shared `visibleTiles` updates cause client explored history to accumulate in a headless or
  smoke-testable path.

## Acceptance Criteria

- Server-authoritative shared current vision works for every living teammate.
- Hidden enemies remain hidden from the whole opposing team when no living teammate sees them.
- Resource, supply, upgrades, and command authority remain local-player-only.

## Manual Testing Focus

None expected unless the explored-history behavior needs a visual sanity check. Prefer a scripted
scenario URL over manual multi-tab setup.

## Handoff Requirements

The phase handoff must explain the team-fog implementation choice, define defeated/disconnected
vision behavior, and list any projection or event surfaces intentionally deferred.
