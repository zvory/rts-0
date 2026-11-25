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

User-specified: 100 steel, 100 oil, 20 seconds of construction, three tiles wide by four tiles
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
Buildings and enemies are invalid targets. Purchases require the target to be visible.
Healing Potion has **unlimited range**; Escape Treasure requires the target to be within
**30 tiles** of the supplying Shop at purchase time. Invalid purchases spend nothing.
Both items travel directly from the Shop toward the target at **10 tiles per second**;
green circles represent healing potions and purple circles represent escape treasures.
The server owns delivery and effects; the client renders fog-filtered projectile positions.

Healing Potion restores the target to full HP on arrival. This follows the user's final
“heals the unit fully” instruction, superseding the earlier 75 HP description.
Escape Treasure grants one lethal-damage escape, returning the unit to its supplying Shop
with **1 HP**, regardless of the unit's distance when it takes damage. The applied effect is
shown only to the unit’s owner and allies as a purple **ESC** label below the health bar,
even when health bars are hidden. Enemies receive no protection flag even when they see the unit.

Carried items have no total or category limit and no RFV command-card controls. All distinct
passive goods can coexist with Escape Treasure. Duplicate copies of the same item are rejected,
including a copy already in flight, without spending oil. Escape automatically activates on lethal
damage by default. Healing Potion and Healing Seal are consumed rather than carried.

A destroyed supplying Shop cancels in-flight goods and removes its applied escape protection.
Escape Treasure has a **60-second purchase cooldown shared across all Shops owned by the purchasing player**, starting only on successful launch. This cooldown survives building destruction, new Shops, checkpoints and replays. Repeated escape purchases do not stack. Delivery tracks a moving target directly; disappearing or
dead targets cancel delivery. Oil is paid at launch, with no refund for interrupted delivery.

Escape selects a deterministic legal exit around the supplying Shop, searching farther only when
nearby exits are blocked. If no legal position exists anywhere on the map, the rescue cannot occur.
Escape Treasure deliveries expire after 60 seconds if a moving target cannot be reached.
Healing Potions have no flight timeout; distant targets take longer to reach.


### Healing Seal placement

The Shop also sells [Healing Seal](healing-seal.md) for **150 oil** (S). Choose the good,
then click visible ground within 30 tiles. It appears immediately as a selectable,
non-blocking 3×3 green circle. Friendly ground units inside heal 3 HP/s each from a shared
400 HP budget. Its green capacity bar and selection text show healing remaining; it
disappears when empty. It is ground magic, not a carried item or projectile.

### Carried attack and defense goods

Shops also deliver Haste (**100 oil**, Z), Double Damage (**150 oil**, X), and Armor Treasure (**100 oil**, D) to visible friendly units within 30 tiles, using the same delivery speed and timeout as Escape Treasure. Attack and defense describe item roles only. A delivery reserves its distinct item type immediately, preventing duplicate purchases. Invalid purchases spend nothing.

Attack items are permanent passives applied immediately on delivery, with no activation, consumption, or expiry. Haste doubles primary weapon attack speed by halving its reload (minimum one tick); Double Damage doubles primary weapon damage before normal mitigation. Double Damage also affects primary tank splash and manual mortar/artillery shells; launched shells retain the bonus through impact. Haste also reduces manual mortar and artillery reload. Neither affects spells, rockets, disposable panzerfaust shots, or secondary weapons. Armor Treasure is a permanent passive item: delivery immediately applies an additional **50% damage reduction**, rounded upward per hit, including units without native armor. It is never activated or consumed. All four permanent passive effects can coexist; identical copies do not stack. Escape may coexist with all of them.

Carried passive items use existing entity ability-use state; the owner and privileged diagnostic perspectives see equipped items through private ability entries. Enemy snapshots do not expose inventory identities. Already-visible Cultivator fighters expose only the public visual strength (distinct item count saturated at three, `itemRadianceTier`, 0–3) for their golden glow; hidden fighters expose nothing. Checkpoint/replay restore preserves carried items and purchase cooldowns.

### Vitality Treasure

Vitality Treasure is a **100 oil** defense item, purchased from a completed Shop with **F** and can coexist with Armor Treasure. It uses the same visible friendly-unit targeting, 30-tile purchase range, delivery, same-item reservation, and non-stacking rules. Delivery immediately grants **+150 maximum HP and +150 current HP permanently**, preserving any pre-existing missing HP. It remains carried and passive, with no activation, consumption, or expiry. For example, 30/58 HP becomes 180/208 HP. Dao evolution and Master advancement retain the flat 150 bonus without multiplying it. Carried passive state, including the health pair, survives checkpoints. Destroying the supplying Shop after delivery does not remove Armor or Vitality.

Shop command rows mirror the carried categories: top row movement (Escape Q); middle row defense
(Healing Potion A, Healing Seal S, Armor D, Vitality F); bottom row attack (Haste Z, Double Damage X).
Carried goods appear below the health bar as ESC, ARM, VIT, HST, and DD rather than command-card tiles.
Rejected duplicate purchases send the purchasing player an info Notice:
“Item already equipped or being delivered”, without spending resources.
