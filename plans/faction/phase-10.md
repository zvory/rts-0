# Phase 10 - Second Faction Combat and Signature Ability Slice

Status: Designed, not implemented.

## Objective

Add the second faction's first combat loop: one baseline combat unit plus one signature
ability-heavy unit. This phase should make the faction mechanically legible in a short match without
trying to complete the whole roster.

## Scope

- Add the approved baseline combat unit with stats, cost, supply/capacity use, production path,
  renderer data, command-card affordances, and tests.
- Add the approved signature ability-heavy unit and its ability or abilities.
- Use existing ability registry and effect hooks where possible; add only tightly scoped hooks if
  the approved ability cannot be implemented cleanly.
- Add fog-safe events, notices, cooldown/charge projection, Steel/Oil costs, and client visuals for
  the signature ability.
- Keep AI blocked for the new faction unless explicitly approved.
- Keep prediction disabled for the new faction unless WASM support is intentionally implemented.
- Collect factual patch-note bullets for stats, economy costs, ability behavior, UI, and expected
  playtest watch points.

## Expected Touch Points

- `server/crates/rules/src/`
- `server/crates/sim/src/game/`
- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `client/src/protocol.js`
- `client/src/config.js`
- `client/src/hud_command_card.js`
- `client/src/renderer/`
- generated or checked client catalog artifacts/scripts
- `docs/design/balance.md`
- `docs/design/protocol.md`
- `docs/design/client-ui.md`
- `docs/design/server-sim.md`

## Verification

- Focused Rust tests for new unit stats, production, costs, supply/capacity, and command legality.
- Rust ability tests for carrier eligibility, target mode, costs, cooldowns, charges, events, and
  wrong-faction rejection.
- Fog/security regression tests for every new event or reveal.
- Protocol parity tests for every new kind, ability, event, or upgrade id.
- Client command-card descriptor tests for the new combat and ability units.
- Targeted client smoke or dev scenario test for rendering and command issuance.
- Balance docs updated with player-facing stats and ability behavior.

## Manual Testing Focus

Play a short local/dev match as the new faction and verify production, combat readability, ability
targeting, cooldown/charges, Steel/Oil cost, fog behavior, defeat/win behavior, and current-faction
regression.

## Handoff Expectations

The handoff must include patch-note bullets, implemented unit/ability details, verification
commands/results, known balance risks, and the next roster/progression items proposed for Phase 11.

## Player-Facing Outcome

The new faction has a small but playable combat identity suitable for focused playtesting.
