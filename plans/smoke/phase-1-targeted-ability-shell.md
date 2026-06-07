# Phase 1: Reusable Targeted Ability Shell

## Objective

Add the generic ability data model, cooldown projection, HUD affordance model, and targeted command
plumbing needed by Smoke, while keeping Smoke's actual LOS effect minimal or disabled until Phase 2.
The model must also cover existing Rifleman Charge as a self-activated cooldown ability.

## Server Work

- Add `AbilityKind`, covering existing `Charge` and new `Smoke`.
- Add an ability definition table that answers:
  - carrier unit kinds;
  - target mode (`Self` for Charge, `WorldPoint` for Smoke);
  - range in tiles when applicable;
  - cooldown ticks;
  - resource cost when applicable;
  - tech requirement;
  - whether the ability may be queued.
- Add `SimCommand::UseAbility { ability, units, x, y, queued }`.
- Add protocol translation for generic targeted `useAbility`.
- Preserve the existing `charge` wire command in this phase, but route its validation and cooldown
  behavior through the same ability definitions/cooldown helpers used by Smoke where practical.
- Reuse `dedupe_cap_units` for ability unit lists.
- Add per-entity ability cooldown state keyed by `AbilityKind`.
- Tick ability cooldowns in a generic service or in an existing per-tick entity state step.
- Project owner-only ability cooldown data in snapshots. During migration, either bridge Charge
  into the generic `abilities` list while retaining `chargeCooldownLeft`, or move it fully once all
  protocol decoders and HUD logic are updated in the same change.
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
  projected cooldowns, not hard-coded rifleman or scout-car branches.
- Preserve existing `move`, `attack`, and `setupAtGuns` behavior.

## Reuse Requirements

- Do not add `scoutCarSmokeCooldown` fields.
- Do not branch command routing on scout cars except in the Smoke ability definition's carrier list.
- Do not duplicate a separate Charge cooldown system while adding Smoke. Charge may keep its legacy
  protocol command/field during migration, but the simulation-side ability definition and cooldown
  logic should be shared.
- Do not make the HUD know server-only launch selection rules. The HUD only knows whether the
  selected group has at least one apparent carrier and enough local info to show an affordance.

## Done

- The protocol can carry a generic targeted ability command.
- The server can parse, validate, log, and replay ability commands deterministically.
- Owner-only ability cooldowns can be projected and decoded.
- Client command-card and hotkey plumbing can arm a generic targeted ability mode.
- Existing Rifleman Charge behavior is preserved while sharing the new ability metadata/cooldown
  model where practical.
- No smoke LOS gameplay is required yet.

## Verification

- `cd server && cargo test`
- Client protocol decode/unit checks if present.
- Manual command-card smoke button can be stubbed behind no-op launch only if useful for validating
  UX wiring.
