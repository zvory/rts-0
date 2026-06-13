# Phase 6 - Ability Effect Hooks

Status: Designed, not implemented.

## Objective

Add only the reusable ability effect hooks needed by known current abilities or architecture
fixtures. This phase must not depend on the real second-faction brief, which is approved later in
Phase 8, and it should avoid building a generic scripting engine before real mechanics prove the
needed shapes.

## Scope

- Identify from current ability parity and fixture needs which reusable effect classes are required.
- Add focused hooks only when current parity or fixture coverage needs concrete patterns such as:
  - self buff
  - targeted world effect
  - delayed projectile or delayed impact
  - area effect
  - toggle/autocast
  - limited charges
- Steel/Oil-consuming activation
- Keep complex one-off implementations acceptable when they remain clearer than generic hooks.
- Ensure hooks receive faction/player context so wrong-faction effects cannot trigger.
- Ensure every effect event remains fog-safe and does not reveal hidden enemy entities or positions.
- Keep `Game::tick()` panic-free when ability definitions are missing, stale caster ids are used, or
  target positions are invalid.
- Update ability docs with the split between registry metadata and effect implementation.

## Expected Touch Points

- `server/crates/sim/src/game/ability.rs`
- `server/crates/sim/src/game/services/ability_orders.rs`
- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/sim/src/game/services/combat/`
- `server/crates/sim/src/game/smoke.rs`
- `server/crates/sim/src/game/mortar.rs`
- `server/crates/sim/src/game/artillery.rs`
- `server/crates/sim/src/game/snapshot.rs`
- `server/crates/protocol/src/lib.rs`
- `client/src/protocol.js`
- `client/src/config.js`
- `client/src/hud_command_card.js`
- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `docs/design/balance.md`

## Verification

- Rust tests for each reusable hook that is added.
- Rust regression tests proving existing ability effects remain behaviorally unchanged.
- Fog/security tests for every event or reveal produced by the hooks.
- Command tests for stale ids, invalid target positions, wrong-faction effects, and missing
  definitions.
- Client descriptor tests if any hook changes projection, charges, cooldowns, or event rendering.

## Manual Testing Focus

Use debug mode to execute every current ability and inspect that visuals, cooldowns, resource costs,
autocast state, and fog behavior are unchanged.

## Handoff Expectations

The handoff must list the hooks that exist, the effect code intentionally left one-off, and the
specific extension points Phase 10 may use if the approved second-faction signature ability needs
additional tightly scoped hooks.

## Player-Facing Outcome

No intended current-faction balance change. The ability system has practical extension points, but
second-faction-specific hooks wait until the faction brief is approved and Phase 10 needs them.
