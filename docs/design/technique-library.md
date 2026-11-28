# Technique Library

## Phase 0: approved building brief

The user named and authorized implementation of a Cultivator Technique Library: a Serf-built
research structure that moves combat abilities out of the starting kit. UI description:
“Research combat techniques for Disciples and Dao Adepts.” Investing in techniques competes
with unit production and Dao research; opponents can delay unfinished research by destroying
this vulnerable structure. Completed techniques are permanent player-wide unlocks, including
future units and Masters, and survive losing the library.

## Phase 1: rules and balance specification

Use the Three Dao Pavilion baseline: 100 steel, 25 oil, 560 construction ticks (18⅔ seconds),
120 HP, armored, no weapon, one tile sight, 3×3 footprint, no supply. Serfs build it after a
completed Nexus and Portal; no build limit. Normal placement, blocking, repair, construction,
cancellation/refund, research queue, destruction and fog rules apply. Worker build-card slot 6
(hotkey D) fills an existing empty slot. Standard minimap, selection, construction and damage
presentation apply; white roof is player-tinted, timber/stone/parchment remain fixed.

Three independent researches cost 100 steel and 100 oil each:

| Research | Time | Unlock |
| --- | --- | --- |
| Charge | 20 seconds | Automatic Charge for Disciples/Novices, War Adepts and War Masters |
| Shield | 20 seconds | Shield for Magic Adepts and Magic Masters |
| Throw Sword | 20 seconds | Throw Sword for War Adepts and War Masters |

Blink remains available to Wind Adepts and Wind Masters without research.
Vector Field remains available to Magic Adepts and Magic Masters without research. Existing
ability strength, cooldowns, charge recharge, and advancement rules stay unchanged. Technique
research never uses the Pavilion's escalating Dao prices. Command admission, queued execution,
automatic Charge, client cards, owner-only research projection and replay vocabulary must agree.
AI can encounter/build/research through its catalog; autonomous technique purchasing policy is
not added in this pass. Existing construction/research audio is reused; bespoke audio is deferred.

The request supplies Phase 0/1 direction and explicit implementation authorization. Remaining
baseline choices above follow the Pavilion and existing research architecture. Visual approval
is required on the open PR before auto-merge is enabled.

## Delivery checklist

- [x] Phase 0/1 brief and rules recorded before implementation.
- [x] Building, research catalog and wire mirrors implemented.
- [x] Server gates manual, queued and automatic ability use.
- [x] Focused rules, simulation and client contract validation.
- [x] ImageGen source, reproducible atlas and in-game visual inspection.
- [x] PR opened with auto-merge disabled for requested visual approval.
