# Phase 1: Reusable Targeted Ability Shell

## Objective

Add the generic command, data model, cooldown projection, HUD affordance model, and queue plumbing
needed by targeted abilities, while keeping Smoke's actual LOS effect minimal or disabled until
Phase 2.

## Server Work

- Add `AbilityKind`, starting with `Smoke`.
- Add an ability definition table that answers:
  - carrier unit kinds;
  - target mode (`WorldPoint`);
  - range in tiles;
  - cooldown ticks;
  - resource cost;
  - tech requirement;
  - whether the ability may be queued.
- Add `SimCommand::UseAbility { ability, units, x, y, queued }`.
- Add protocol translation for generic `useAbility`.
- Reuse `dedupe_cap_units` for ability unit lists.
- Add per-entity ability cooldown state keyed by `AbilityKind`.
- Tick ability cooldowns in a generic service or in an existing per-tick entity state step.
- Project owner-only ability cooldown data in snapshots.
- Add `Order::UseAbility` and `OrderIntent::UseAbility` only if out-of-range/queued execution needs
  persistent state in this phase. If phased smaller, direct in-range launch can be supported first,
  but the order shape should still be designed here.

## Client Work

- Add generic command builder:

```text
cmd.useAbility(ability, units, x, y, queued)
```

- Add generic command-target state that can represent `ability:smoke` without adding one-off
  strings for every ability.
- Extend command-card data/signature logic so ability buttons are driven by ability definitions and
  projected cooldowns, not hard-coded scout-car branches.
- Preserve existing `move`, `attack`, and `setupAtGuns` behavior.

## Reuse Requirements

- Do not add `scoutCarSmokeCooldown` fields.
- Do not branch command routing on scout cars except in the Smoke ability definition's carrier list.
- Do not make the HUD know server-only launch selection rules. The HUD only knows whether the
  selected group has at least one apparent carrier and enough local info to show an affordance.

## Done

- The protocol can carry a generic targeted ability command.
- The server can parse, validate, log, and replay ability commands deterministically.
- Owner-only ability cooldowns can be projected and decoded.
- Client command-card and hotkey plumbing can arm a generic targeted ability mode.
- No smoke LOS gameplay is required yet.

## Verification

- `cd server && cargo test`
- Client protocol decode/unit checks if present.
- Manual command-card smoke button can be stubbed behind no-op launch only if useful for validating
  UX wiring.
