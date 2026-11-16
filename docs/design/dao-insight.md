# Disciple insight and Dao evolution

Phase 0/1 briefs and rules (completed before implementation): each path is a permanent
specialization of the existing Disciple, retaining its entity identity, orders, cost,
supply (2), sight, size, melee range and ordinary ground pathing. No additional
training source or build time. Evolution is immediate and free after 60 actual hostile
HP damage dealt, capped at 60; misses, friendly fire, invulnerability and overkill grant
no extra insight. One evolution per unit. Player research persists after Pavilion loss.
All three existing Pavilion research slots retain 200 steel / 100 oil / 20 seconds,
serial research queue and existing prerequisite/faction gates.

- Lightness: mobile skirmisher; scout-car base speed, 1.5 attacks per original attack
  interval (cooldown divided by 1.5), existing Ekat Dash with optional return unchanged.
  A centered yellow left-facing double chevron with black outline identifies it. Classic evolution key L; grid Z.
- War: armored melee damage dealer; 1.5 damage, +20 current/max HP, 50% armor
  reduction using the existing penetration mechanic (full AP bypasses it), keeps the Disciple's existing
  50% armor penetration. A centered neon-red sword with black outline identifies it, rotated 90 degrees and lengthened 20%. Classic W; grid X.
- Reality: battlefield support; 1.1 base movement speed, existing Ekat Magic Anchor
  unchanged, a centered green pentagram with black outline identifies it. Classic R; grid C. Further support design,
  bespoke rigs and audio remain deferred for this prototype.

Each path has a separate researched unlock. No automatic AI evolution in this pass;
AI can encounter and damage evolved units. Insight uses a blue fill over red background
under the HP bar and exact insight/Dao appears in selected-unit details. Evolved units
keep the full insight bar. No new targeting or hidden-position information is exposed.
State is checkpointed with the entity and snapshots use appended compact slots.
