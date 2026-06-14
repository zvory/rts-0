# Phase 11 - Second Faction Combat and Signature Ability Slice

Status: Blocked until Phase 10 lands an approved hero-centric Ekat slice.

## Objective

Extend the approved Ekat hero combat loop. This phase must build on the new hero-centric spec,
not the purged RTS-style baseline-unit plus specialist-unit design.

## Scope

- Add only the approved hero combat, progression, ability, or objective interactions.
- Do not add the purged Conscript, Signal Team, Workshop, or Mark Target content.
- Use existing ability registry and effect hooks where possible; add only tightly scoped hooks if
  the approved ability cannot be implemented cleanly.
- Add fog-safe events, notices, cooldown/charge projection, approved resource/progression costs, and
  client visuals for hero abilities.
- Keep AI blocked for the new faction unless explicitly approved.
- Keep prediction disabled for the new faction unless WASM support is intentionally implemented.
- Update the lifecycle matrix if the new combat or ability slice changes replay, branch, spectator,
  fog, prediction, or dev scenario behavior.
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

- Focused Rust tests for approved hero stats, progression/economy, commands, and legality.
- Rust ability tests for carrier eligibility, target mode, costs, cooldowns, charges, events, and
  wrong-faction rejection.
- Fog/security regression tests for every new event or reveal.
- Protocol parity tests for every new kind, ability, event, or upgrade id.
- Client command-card/control descriptor tests for the hero combat and ability surface.
- Targeted client smoke or dev scenario test for rendering and command issuance.
- Balance/design docs updated with player-facing hero stats and ability behavior.

## Manual Testing Focus

Play a short local/dev match as Ekat and verify hero combat readability, ability targeting,
cooldown/charges, approved resource/progression costs, fog behavior, defeat/win behavior, and
Kriegsia regression.

## Handoff Expectations

The handoff must include patch-note bullets, implemented hero/ability details, verification
commands/results, lifecycle matrix updates, known balance risks, and the next approved items
proposed for Phase 12.

## Player-Facing Outcome

Ekat has a small but playable hero-combat identity suitable for focused playtesting.
