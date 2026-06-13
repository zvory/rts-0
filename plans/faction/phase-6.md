# Phase 6 - Second Faction Vertical Slice

Status: Designed, not implemented.

## Objective

Implement the approved second faction as a playable vertical slice using the new faction, economy,
catalog, ability, and client surfaces. This phase begins only after the faction brief and rules/
balance spec are approved.

## Scope

- Complete or reference the faction brief and rules/balance spec before implementation.
- Add the faction id, catalog data, starting loadout, resource model, and first production path.
- Implement the minimum playable roster first:
  - one core/base structure or equivalent anchor
  - one builder/producer path or equivalent mechanic
  - one baseline combat unit
  - one signature ability-heavy unit
  - enough economy/progression to sustain a short match
- Add units/buildings/upgrades/abilities incrementally after the vertical slice works.
- Add art/rendering that is readable enough for playtesting; avoid blank placeholders in normal
  gameplay.
- Collect patch-note bullets as stats, economy, combat behavior, and UI affordances are added.
- Keep AI disabled or restricted for the new faction unless explicitly implemented.

## Expected Touch Points

- `server/crates/rules/src/`
- `server/crates/sim/src/game/`
- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `client/src/protocol.js`
- `client/src/config.js`
- `client/src/hud_command_card.js`
- `client/src/renderer/`
- `docs/design/balance.md`
- `docs/design/protocol.md`
- `docs/design/client-ui.md`
- `docs/design/server-sim.md`

## Verification

- Focused Rust tests for every new unit/building/ability rule.
- Protocol parity tests for every new kind, ability, upgrade, event, or resource id.
- Client command-card descriptor tests for the new faction.
- Server integration test for a mixed-faction match start and basic production/ability use.
- Fog/security regression tests for signature abilities.
- Targeted client smoke test for rendering and command issuance.
- Balance docs updated in the same change as player-facing stats.

## Manual Testing Focus

Play a short local match or dev scenario as the new faction and verify start, resource/progression,
building/production path, signature ability use, combat readability, and defeat/win behavior.
Also play a current-faction match to confirm it was not regressed.

## Handoff Expectations

The handoff must include patch-note bullets, the implemented roster/progression list, tests run,
known balance risks, and what remains for the next implementation slice. It should also state
whether AI can select the new faction or remains blocked.

## Player-Facing Outcome

Players can try the new faction's first playable slice. The faction should feel mechanically
distinct enough to validate the architecture, even if final balance and roster depth are deferred.

