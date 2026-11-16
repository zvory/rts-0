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
- Reality: battlefield support; 1.1 base movement speed, existing Ekat pull field (renamed Anchor) and a new Shield, a centered green pentagram with black outline identifies it. Classic R; grid C. Bespoke rigs and audio remain deferred for this prototype.

Each path has a separate researched unlock. No automatic AI evolution in this pass;
AI can encounter and damage evolved units. Insight uses a blue fill over red background
under the HP bar and exact insight appears in selected-unit details for teammates and privileged observers only. Enemy snapshots omit exact insight. Evolved units
keep the full insight bar. No new targeting or hidden-position information is exposed.
State is checkpointed with the entity and snapshots use appended compact slots.

Readiness radiance pulses around actionable Dao research/evolution buttons and around a
Disciple at maximum insight before evolution. Labels split after “Dao of”. Visible enemies
see the unit radiance, with ordinary fog filtering. Realm advancement projects a 15-tick
(half-second) expanding gold burst reaching about two tiles, independently of insight privacy.

Reality Disciples can place Shield within five tiles: a shimmering circle with the same
three-tile radius as Anchor, covering allied units (including the caster). One shared pool
absorbs 300 hostile damage after normal armor/terrain reductions; overflow damages the
unit, and depletion removes the field immediately. Friendly fire, environmental damage,
buildings, invulnerable targets and enemy units do not consume the pool. Overlapping
fields consume pools in stable caster-id order. Shield costs no resources, has a ten-second
cooldown, replaces the caster’s prior shield, and persists until depletion, replacement,
or caster death. The pool and fixed world center live on the caster entity and survive
checkpoints; the existing ability-runtime allocator supplies its unique visual id.
All visible viewers see the field; owner/privileged viewers receive its remaining pool.

Lab commands continue to use the selected owner’s faction and research. A Disciple
spawned for a Kriegsia player can move but cannot evolve; the Dao preview’s P2 is
Kriegsia with no Dao research. Setup imports now resend authoritative start payloads
to every viewer so faction/team rosters and command cards refresh together with the
restored simulation. Mixed-faction Lab support is outside this change.
