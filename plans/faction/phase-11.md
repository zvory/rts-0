# Phase 11 - Second Faction Combat and Signature Ability Slice

Status: Done.

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
commands/results, lifecycle matrix updates, known balance risks, and the next roster/progression
items proposed for Phase 12.

## Player-Facing Outcome

The new faction has a small but playable combat identity suitable for focused playtesting.

## Executor Notes

Patch-note bullets to carry into the handoff:

- Ekaterina Workshop now trains Signal Teams: 90 steel / 25 oil, 2 supply, 420 ticks (~14s), 42 HP,
  2 damage, 4-tile attack range, 24-tick attack cooldown, 1.45 px/tick movement, and 9 sight.
- Signal Team Mark Target is a queued world-point ability on hotkey `D`: 15 steel, 8-tile range,
  750-tick (~25s) cooldown, immediate marker, 60-tick (~2s) delayed pulse, 1.25-tile radius, and 20
  normal damage to units only.
- Mark Target intentionally includes friendly fire, does not damage buildings, and uses fog-filtered
  marker events with optional caster ids so visible target pings do not reveal hidden Signal Teams.
- The client command card, targeted ability cursor, compact protocol, state storage, renderer
  feedback, and parity checks now include `ekaterina_signal_team` / `markTarget`.
- Ekaterina remains dev-scenario-only; normal lobby selection, AI, prediction, self-play, replay
  branch launch, and match-history replay are still Kriegsia-only until later phases opt in.
