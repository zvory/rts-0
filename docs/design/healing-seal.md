# Healing Seal

Status: **Removed from the Shop.** The ability id has no live carriers and no command-card
entry. The original specification below records the authored/checkpointed field behavior;
purchases are no longer available. Current availability is owned by [Shop](shop.md#healing-seal-placement).

## Phase 0: building brief

Healing Seal is the final requested name. A Cultivator Shop sells a stationary healing field:
a green circle with a green plus sign, a soft glow, and a slow pulse. It occupies a 3×3-tile
visual area. Friendly units inside recover 3 HP per second each, drawing from a shared
400 HP healing budget. It costs 150 oil and disappears when that budget is exhausted.

UI description: “Place a Healing Seal. Friendly units inside recover 3 HP per second.
Expires after restoring 400 HP in total.”

The seal provides a place to recover between engagements. Compared with the Shop's 50-oil
full-heal potion, its tradeoff is a larger upfront cost and time spent standing in a fixed
area, in exchange for healing multiple units and repeated visits. Enemies can contest the
area or force wounded units away. Buildings receive no healing.

It is purchased and placed from a completed Cultivator Shop, rather than constructed by a
Serf or carried in the unit's R item slot. It is intended for normal matches and Lab.
Fog, placement, allied ownership, overlapping fields, depleted-budget cleanup, checkpoints,
and replay seeks need explicit implementation coverage.

## Phase 1: rules and balance specification

### Requested values

| Property | Specification |
| --- | --- |
| Faction / source | Cultivators / completed Shop |
| Price | 0 steel, 150 oil per seal |
| Size | 3×3 tiles; circular field with radius 1.5 tiles |
| Healing | 3 HP per second per eligible friendly unit |
| Shared capacity | 400 HP actually restored across all recipients |
| Expiry | Immediately after the last healing point is spent |
| Appearance | Green circle, green plus, soft glow, slow pulse |
| Capacity bar | Green, full at 400, empty at 0 |
| Selection detail | “275 / 400 healing left” using the current remaining amount |

### Defaults for unspecified behavior

These defaults resolve details not specified by the user; implementation was explicitly authorized.

- Select a completed Shop, choose Healing Seal, then click a visible ground position within
  30 tiles of that Shop, matching its current goods' purchase range. Reserve S in the Shop
  command card defense row, alongside Regen (A), Armor (D), and Vitality (F). Placement is immediate;
  there is no delivery projectile, builder, construction time, cooldown, or research prerequisite.
- Snap the center to the center tile of a 3×3 placement area. Require the whole area to be
  in map bounds and on ordinary traversable ground; reject static blocking structures and
  invalid terrain. Units and other seals do not obstruct placement. Invalid placement,
  insufficient funds, stale Shop ids, and unauthorized purchases spend nothing.
- The seal is walk-through ground magic. It does not block movement, projectiles, or sight;
  existing seals do not reserve footprints against subsequent building construction.
  it is selectable but cannot be attacked, repaired, captured, upgraded, or reclaimed.
  It has no combat HP or armor; its green bar represents healing capacity only.
- It has no weapon, resource storage, production, research, supply, or independent vision.
  No arbitrary time limit or per-player build limit is added; existing object admission
  limits still apply. No cancellation refund after a successful purchase.
- A unit qualifies when alive, wounded, friendly by the authoritative team relation, and
  its center lies within or on the circular boundary. Owner and allied ground units of any
  faction qualify. Air sorties and buildings do not. Healthy units consume no capacity;
  dead units cannot be revived.
- Restore one integer HP every 10 simulation ticks at 30 Hz. On each healing pulse,
  process seals and eligible units in stable id order. Deduct only the HP actually restored;
  clamp to missing HP and remaining capacity. No accumulated burst healing when a unit
  enters after waiting outside. Overlapping seals stack, each paying for its own healing.
- Placed seals persist independently if the supplying Shop is destroyed. They expire only
  on depletion or match teardown. Exhaustion removes the seal with no damage,
  wreckage, resource payout, or positional event sent through fog.
- Use a two-second visual pulse driven by the render clock. Keep circle size fixed so the
  boundary accurately shows the healing area. Place the capacity bar at the field's edge
  without covering the plus. Show the remaining-capacity text in the bottom selection panel.
- Apply ordinary authoritative positional visibility. Hidden seals and their remaining
  capacity must not leak through snapshots, minimap, selection, or depletion effects.
  Use current visible state only; do not retain a permanent remembered-building ghost for
  a consumable field. Visible opponents may inspect the same capacity bar and selection text.
- Lab must support creating a seal and damaged friendly/enemy units for inspection. AI can
  receive healing through the same simulation rules; autonomous AI purchase strategy and
  bespoke audio are deferred. Use ordinary UI purchase/selection feedback without an ambient loop.

## Implementation contract and verification checklist

- [x] Phase 0 brief and Phase 1 rules written before implementation.
- [x] User explicitly instructed implementation after delivery of this brief/spec.
- [x] Choose an authoritative selectable field representation that supports ordinary unit
  selection without creating building collision, combat targeting, victory eligibility,
  tech prerequisites, or fog-memory side effects.
- [x] Add purchase ability/catalog metadata, admission checks, mirrored constants, and
  healing behavior. Persist remaining capacity and pulse timing in the existing checkpoint
  and replay ownership model; keep `Game::tick()` panic-free.
- [x] Update Rust/JS wire mirrors, compact vocabulary, and
  [protocol design](protocol.md) for the chosen field representation and capacity projection.
- [x] Update [balance](balance.md), [Shop](shop.md), and
  [client UI](client-ui.md) documentation with implemented behavior; this proposal must not
  be presented as currently available goods until implementation lands.
- [x] Add the Shop command, placement preview, circle/plus/glow/pulse, capacity bar,
  selection text, minimap behavior, and Lab inspection support with normal teardown.
- [x] Test 150-oil admission and invalid placement; faction/source restrictions; exact
  3 HP/s; circle boundary; owner/allied/enemy/building/air/dead/full-health filtering;
  actual-healing-only spending; multiple recipients; partial final pulse; overlapping
  seals; Shop destruction; fog filtering; checkpoint/replay continuation and selection.
- [x] Run focused simulation, protocol/catalog parity, client architecture, wiki checks,
  and actual Interact visual inspection before the authoritative PR gate.
- [x] Stage factual patch notes for the player-facing implementation.
- PR delivery: the owned PR records focused checks, the authoritative CI gate, and merge status.

Staged patch note: “Cultivator Shops sell Healing Seals for 150 oil.
Friendly units inside the glowing green circle recover 3 HP per second, up to 400 HP total.
Select a seal to see its remaining healing.”

Workflow source: [New Building Checklist](../new-building-checklist.md), Mandatory Workflow Gate.


## Implementation and review evidence

`healing_seal` reuses a selectable entity's checkpointed `hp` / `maxHp` pair for remaining
healing / total capacity. Damage and repair reject the kind. It has no static/LOS collision,
weapon, supply, independent sight, survival-building eligibility, or enemy fog memory.
No new durable state owner or snapshot field is introduced. Purchases use `healingSeal` on
a completed Shop; rendering and selection use existing graphics pools and entity teardown.

Focused coverage is in `game/tests/healing_seal_tests.rs` and
`tests/client_contracts/healing_seal_contracts.mjs`, with the existing Shop regression tests.
Interact review used four wounded Novices around a seal purchased from a Shop with exactly
150 oil. Fixed-step video demonstrates healing and the pulsing field; a normal-HUD still
verifies the selected `400 / 400 healing left` detail. Bespoke audio and AI purchase strategy
remain deferred as specified above.
