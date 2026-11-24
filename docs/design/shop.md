# Cultivator Shop

## Phase 0: building brief — complete

The user approved the gourd Shop concept and explicitly authorized implementation. The Shop is
a Cultivator building selling targeted goods as specified below. It has no production, research,
aura, resource storage, supply, attack, or rally point. Opponents can attack and destroy it.

UI description describes the Shop’s healing potions and escape treasures.

Approved art: the upper part of a giant gourd recently driven into the earth, surrounded by
fractured rocks. A covered open service window has no door. The side bears 愛 (love). The cork,
neck rope and canopy are white team-paint regions. Use the approved tall concept and existing
Cultivator camera, painterly outlines, timber and stone styling.

## Phase 1: rules and balance — complete

User-specified: 200 steel, 200 oil, 20 seconds of construction, three tiles wide by four tiles
deep, no functionality. The Shop has **200 HP** per the latest balance request.

- Cultivator Serfs construct it through their normal build card (slot Z in the grid preset).
  No additional prerequisite or build limit. It is available in normal matches and Lab.
- Authoritative construction is 600 ticks at 30 Hz. Footprint is 3×4, with ordinary placement,
  collision, terrain restrictions, line-of-sight blocking and selection geometry.
- Armored, one-tile sight, zero weapon damage/range, zero supply used/provided. No producer or
  production-anchor registration. Other factions cannot construct it.
- Existing construction progress, cancellation/refund, repair behavior, enemy damage, destruction,
  cleanup, fog filtering, remembered-building rendering, checkpoint and replay rules apply.
  No capture, upgrade, transform, special death effect or special repair rule.
- Ordinary construction/damage/destruction presentation and generic audio apply. Bespoke animation,
  audio and autonomous AI construction are deferred. Targeted shop interactions are specified below.
- Wire kind `shop`, compact kind code 34. Goods extend generic ability targeting and visible unit state.

## Delivery checklist

- [x] Brief and rules recorded before implementation; user authorized implementation.
- [x] Rules, protocol, build card, art and source-of-truth mirrors updated.
- [x] Construction, affordability, faction restriction and inert behavior validated.
- [x] In-game appearance checked with the actual Shop kind.

Focused checks and the owned PR record the final verification and merge outcome.

## Targeted goods

Completed Shops sell Healing Potion for **50 oil** and Escape Treasure for **100 oil**.
Select the Shop, click an item, then click a living friendly unit (including allied units).
Buildings and enemies are invalid targets. Purchases require the target to be visible and within
**30 tiles** of the supplying Shop at purchase time. Invalid purchases spend nothing.
Both items travel directly from the Shop toward the target at **10 tiles per second**;
green circles represent healing potions and purple circles represent escape treasures.
The server owns delivery and effects; the client renders fog-filtered projectile positions.

Healing Potion restores the target to full HP on arrival. This follows the user's final
“heals the unit fully” instruction, superseding the earlier 75 HP description.
Escape Treasure grants one lethal-damage escape, returning the unit to its supplying Shop
with **1 HP**, regardless of the unit's distance when it takes damage. The applied effect is
shown only to the unit’s owner and allies as a purple square with a white **E**, centered on the unit; both dimensions equal twice
the health bar height, even when health bars are hidden. Enemies receive no protection flag
even when they see the unit.

Cultivator units reserve **R** for their single carried item. An empty inventory leaves R empty.
A delivered Escape Treasure exposes **Escape** in R, with autocast enabled by default. Lethal
damage triggers the automatic escape at 1 HP; manual activation consumes the same treasure and
returns the unit immediately while preserving its current HP. Right-click or Alt+R toggles
autocast; manual use remains available when autocast is disabled. Only owned living carriers
can activate or toggle their item. Healing Potion is consumed on delivery rather than carried;
there is never more than one carried item.

A destroyed supplying Shop cancels in-flight goods and removes its applied escape protection.
Repeated escape purchases do not stack. Delivery tracks a moving target directly; disappearing or
dead targets cancel delivery. Oil is paid at launch, with no refund for interrupted delivery.

Escape selects a deterministic legal exit around the supplying Shop, searching farther only when
nearby exits are blocked. If no legal position exists anywhere on the map, the rescue cannot occur.
Deliveries expire after 60 seconds if a moving target cannot be reached.
