# Cultivator Shop

## Phase 0: building brief — complete

The user approved the gourd Shop concept and explicitly authorized implementation. The Shop is
a Cultivator building selling targeted goods as specified below. It has no production, research,
aura, resource storage, supply, attack, or rally point. Opponents can attack and destroy it.

UI description describes the Shop’s Regen items and escape treasures.

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

Completed Shops sell Regen for **50 oil** and Escape Treasure for **100 oil**.
Select a completed Shop, click an item, then click a living friendly unit (including allied units).
Alternatively, select one or more Novices, Adepts or Masters and press **R** to open the
**Buy menu** without changing selection (Serfs have no Buy menu). Each item click/key buys
one copy for an eligible selected owned fighter. The server ranks eligible recipients by
Master > Adept > Novice, then descending Insight, then descending carried-plus-in-flight item
count, then ascending entity id. Full inventories and duplicate equipped/in-flight items are
skipped. This lets repeated purchases distribute copies without waiting for delivery.
The client chooses the lowest-id completed owned Shop as source; a missing or destroyed
source cannot purchase. The menu stays open until Return (V), Escape, or selection change.
Throw Sword remains on E. While the Buy menu is open, dotted lines connect its Shop to all
selected owned fighters, including full carriers. Existing resource costs, shared Escape
cooldown, visibility and command admission limits still apply.
Buildings and enemies are invalid targets. Purchases require the target to be visible.
All unit-targeted goods have **unlimited range**. Invalid purchases spend nothing.
All targeted items travel directly from the Shop toward the target at **7.5 tiles per second**;
deliveries use the same colored icons displayed on equipped units, in the world and minimap.
The server owns delivery and effects; the client renders fog-filtered projectile positions.

Regen is a permanent carried passive that restores **3 HP per second** after arrival
(one HP every ten simulation ticks), capped at maximum HP. It does not instantly heal,
expire, or stack; it occupies one carried slot and survives supplying-Shop destruction.
Escape Treasure grants one lethal-damage escape, returning the unit to its supplying Shop
with **1 HP**, regardless of the unit's distance when it takes damage. The applied effect is
shown only to the unit’s owner and allies as a purple **ESC** label below the health bar,
even when health bars are hidden. Enemies receive no protection flag even when they see the unit.

Novices carry one item, Adepts carry up to three, and Masters carry up to six, without category limits and no RFV command-card controls. Distinct passive goods can coexist with Escape Treasure within the rank limit. In-flight deliveries reserve slots; Healing Seal does not use slots; Regen uses one. Other unit kinds retain their existing behavior. Duplicate copies of the same item are rejected,
including a copy already in flight, without spending oil. Escape automatically activates on lethal
damage by default. Healing Seal is placed rather than carried.

A destroyed supplying Shop cancels in-flight goods and removes its applied escape protection.
Escape Treasure has a **60-second purchase cooldown shared across all Shops owned by the purchasing player**, starting only on successful launch. This cooldown survives building destruction, new Shops, checkpoints and replays. Repeated escape purchases do not stack. Delivery tracks a moving target directly; disappearing or
dead targets cancel delivery. Oil is paid at launch, with no refund for interrupted delivery.

Escape selects a deterministic legal exit around the supplying Shop, searching farther only when
nearby exits are blocked. If no legal position exists anywhere on the map, the rescue cannot occur.
All unit-targeted deliveries have no flight timeout; distant targets take longer to reach.
Flying goods have **10 HP**, are unarmored and targetable by enemy weapons, and never block
ground movement or contribute sight, supply, or army control. Destroying a good cancels its
effect and releases its reserved slot with **no refund**. Each has a tiny 12×1-pixel HP fill.
With a completed Shop selected, hovering an eligible friendly unit draws a thin dotted
Shop-to-unit line.


### Healing Seal placement

Healing Seal is no longer sold or exposed on the Shop command card. Its reserved ability id,
authored fields, healing, rendering and checkpoint behavior remain available for Lab/replays.

### Carried attack and defense goods

Shops also deliver Haste (**100 oil**, Z), Double Damage (**150 oil**, X), and Armor Treasure (**100 oil**, S) to visible friendly units at any distance, using the same delivery speed and lifetime as Escape Treasure. Attack and defense describe item roles only. A delivery reserves its distinct item type immediately, preventing duplicate purchases. Invalid purchases spend nothing.

Attack items are permanent passives applied immediately on delivery, with no activation, consumption, or expiry. Haste increases movement speed by **15%**, increases primary weapon attack speed by **25%** (reload divided by 1.25, rounded upward to a whole tick, minimum one tick), and permits attacks while moving; Double Damage doubles primary weapon damage before normal mitigation. Double Damage also affects primary tank splash and manual mortar/artillery shells; launched shells retain the bonus through impact. Haste also reduces manual mortar and artillery reload. Automatic direct primary fire may bypass deployment while carrying Haste; ordinary weapon targeting restrictions and indirect-fire deployment rules still apply. Neither affects spells, rockets, disposable panzerfaust shots, or secondary weapons. Armor Treasure is a permanent passive item: delivery immediately applies an additional **50% damage reduction**, rounded upward per hit, including units without native armor. It is never activated or consumed. Within the rank slot limit, permanent passive effects can coexist; identical copies do not stack. Escape occupies one slot alongside the passive items.

Carried passive items use existing entity ability-use state; the owner and privileged diagnostic perspectives see equipped items through private ability entries. Enemy snapshots do not expose inventory identities. Already-visible Cultivator fighters expose only the public visual strength (distinct item count saturated at six, `itemRadianceTier`, 0–6) for their golden glow; hidden fighters expose nothing. Checkpoint/replay restore preserves carried items and purchase cooldowns.

### Vitality Treasure

Vitality Treasure is a **100 oil** defense item, purchased from a completed Shop with **D** and can coexist with Armor Treasure. It uses the same visible friendly-unit targeting, unlimited purchase range, delivery, same-item reservation, and non-stacking rules. Delivery immediately grants **+150 maximum HP and +150 current HP permanently**, preserving any pre-existing missing HP. It remains carried and passive, with no activation, consumption, or expiry. For example, 30/58 HP becomes 180/208 HP. Dao evolution and Master advancement retain the flat 150 bonus without multiplying it. Carried passive state, including the health pair, survives checkpoints. Destroying the supplying Shop after delivery does not remove Armor or Vitality.

Both Shop and Buy menu cards attach a small tab to the existing left edge of each row, without extending the panel: **Util** (Escape Q), **Defense**
(Regen A, Armor S, Vitality D; F empty), **Offense** (Haste Z, Double Damage X). Buy menu Return occupies V.
Item glyph/subtitle pairs are `ESC` / Escape, `HP` / +3 per second, `ARM` / +50% base armor,
`+HP` / +150 HP, `HST` / Haste, and `DD` / 2x damage. Armor's subtitle describes the existing
50% mitigation effect; no armor or other item balance value changes.
Carried goods appear as a rosette of colored icons at the unit center: a green cross for Regen,
a purple return diamond for Escape, a shield for Armor, a heart for Vitality, a bolt for Haste,
and a sword for Double Damage. Flying goods and minimap markers use the same icons.
Rejected duplicate purchases send the purchasing player an info Notice:
“Item already equipped or being delivered”, without spending resources.
