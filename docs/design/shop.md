# Cultivator Shop

## Phase 0: building brief — complete

The user approved the gourd Shop concept and explicitly authorized implementation. The Shop is
an inert Cultivator building for now: it has no goods, production, research, abilities,
aura, resource storage, supply, attack, or rally point. Its future purpose is deferred. Building it
currently trades resources and space for a structure; opponents can attack and destroy it.

UI description: “A sturdy gourd-shaped shop. No goods or services are available yet.”

Approved art: the upper part of a giant gourd recently driven into the earth, surrounded by
fractured rocks. A covered open service window has no door. The side bears 愛 (love). The cork,
neck rope and canopy are white team-paint regions. Use the approved tall concept and existing
Cultivator camera, painterly outlines, timber and stone styling.

## Phase 1: rules and balance — complete

User-specified: 200 steel, 200 oil, 40 seconds of construction, three tiles wide by four tiles
deep, no functionality. The Shop has **98 HP**, matching the Engineering Complex per the
latest balance request.

- Cultivator Serfs construct it through their normal build card (slot Z in the grid preset).
  No additional prerequisite or build limit. It is available in normal matches and Lab.
- Authoritative construction is 1,200 ticks at 30 Hz. Footprint is 3×4, with ordinary placement,
  collision, terrain restrictions, line-of-sight blocking and selection geometry.
- Armored, one-tile sight, zero weapon damage/range, zero supply used/provided. No producer or
  production-anchor registration. Other factions cannot construct it.
- Existing construction progress, cancellation/refund, repair behavior, enemy damage, destruction,
  cleanup, fog filtering, remembered-building rendering, checkpoint and replay rules apply.
  No capture, upgrade, transform, special death effect or special repair rule.
- Ordinary construction/damage/destruction presentation and generic audio apply. Bespoke animation,
  audio, autonomous AI construction and all shop interactions are deferred.
- Wire kind `shop`, compact kind code 34. No new snapshot fields, commands or durable state.

## Delivery checklist

- [x] Brief and rules recorded before implementation; user authorized implementation.
- [x] Rules, protocol, build card, art and source-of-truth mirrors updated.
- [x] Construction, affordability, faction restriction and inert behavior validated.
- [x] In-game appearance checked with the actual Shop kind.

Focused checks and the owned PR record the final verification and merge outcome.
