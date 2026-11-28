# Barbed Wire

Phase 0 brief and Phase 1 rules specification are complete. The user explicitly authorized
implementation and reasonable assumptions in the task request.

- Engineer-built anti-infantry field obstacle, available after a completed Training Centre.
- Costs 4 steel, no oil; each 1×1 tile segment takes 3 seconds (90 ticks).
- Any team's infantry (Engineer, Warrior, Rifleman, Panzerfaust, Machine Gunner, Ekat) and
  Golem and Mortar Team take 10 direct HP damage on entering a completed segment's tile and move at
  70% speed while their centre is on that tile. Includes the builder. Leaving and re-entering
  triggers damage again; standing still does not. Other terrain speed multipliers multiply.
- Assumptions: 2 crossing HP, Small armor, no weapon, sight, supply, production, research or income.
  Completed wire becomes neutral like Tank Traps, never keeps a player alive, does not block
  movement or sight, and is untargetable and immune to weapon damage. Tanks destroy a completed segment immediately on tile entry. Other ground units consume one
  crossing HP on entry; wire disappears after two such entries.
  Vehicles and Anti-Tank Guns take no damage or slowdown.
  Scaffolds have no hazard effects. Completing wire beneath a unit does not count as entry.
- Standard engineer construction, cancellation, refund and placement terrain rules.
  Placement allows infantry overlap; no overlapping structures. No dedicated deconstruction
  command or AI construction strategy in this pass; AI units encounter the same hazard rules.
- Click-drag produces a contiguous rasterized straight line including both endpoints (cardinal bridges on diagonals), with no
  deliberate gaps; invalid placement tiles stop the queued line. Existing build queue limits apply.
- Uses the existing build command, snapshot and checkpoint structures with kind `barbed_wire`.
  Ordinary fog and neutral-obstacle damage/death projection rules apply; no new events.
- Uses the selected tangled-barrier PNG with neutral steel/wood colors. Dedicated audio remains
  deferred; standard construction audio is reused.
- Player note: Engineers can build Barbed Wire after a Training Centre for 4 steel per tile in
  3 seconds; it slows infantry and mortars by 30% and deals 10 damage on entry, including allies.
