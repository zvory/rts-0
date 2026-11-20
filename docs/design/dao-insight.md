# Disciple insight and Dao evolution

Phase 0/1 briefs and rules (completed before implementation): each path is a permanent
specialization of the existing Disciple, retaining its entity identity, orders, cost,
supply (2), sight, size, and ordinary ground pathing. All Disciples share a 0.75-tile melee reach (increased from 0.5 by 0.25 tiles to better cover the extended sword animation). No additional
training source or build time. A Dao button assigns its path to every selected, owned, unevolved Disciple, even before
research or insight are ready. Selecting another path replaces the pending choice. This
entity-owned intent is independent of orders: moving, attacking, Stop, and queued orders
do not cancel it. No automatic research purchase, Portal default, or army-wide promotion
is added. Evolution is immediate and free once the chosen research is complete and the unit has 60 actual hostile
HP damage dealt, capped at 60; misses, friendly fire, invulnerability and overkill grant
no extra insight. Successful advancement fully restores HP to the evolved unit’s maximum,
after applying any path-specific maximum-HP bonus. Choosing a pending path or repeating
an evolution request does not heal; dead units cannot advance. One evolution per unit. Player research persists after Pavilion loss.
The choice clears on evolution; evolved units cannot change paths. Pending choices persist
through checkpoints and are visible only to teammates and privileged observers. The command
card highlights chosen paths with an assigned/eligible count; tooltips show missing research
and insight, and unit details show the pending path. Automatic evolution runs after combat,
research production, and ability runtime each tick, skipping dead units.
All three existing Pavilion research slots retain 200 steel / 100 oil / 20 seconds,
serial research queue and existing prerequisite/faction gates.

- Lightness: mobile skirmisher; scout-car base speed, 1.5 attacks per original attack
  interval (cooldown divided by 1.5), existing Ekat Dash with optional return unchanged.
  A centered yellow left-facing double chevron with black outline identifies it. Classic evolution key L; grid Z.
- War: armored melee damage dealer; 1.5 damage, +20 maximum HP, 50% armor
  reduction using the existing penetration mechanic (full AP bypasses it), keeps the Disciple's existing
  50% armor penetration. A centered neon-red sword with black outline identifies it, rotated 90 degrees and lengthened 20%. Classic W; grid X.
- Magic: battlefield support; 1.1 base movement speed, existing Ekat pull field (renamed Anchor) and a new Shield, a centered green pentagram with black outline identifies it. Classic C; grid C. Bespoke rigs and audio remain deferred for this prototype.

Each path has a separate researched unlock. No automatic AI evolution in this pass;
AI can encounter and damage evolved units. Insight uses a blue fill over red background
under the HP bar and exact insight appears in selected-unit details for teammates and privileged observers only. Enemy snapshots omit exact insight. Evolved units
keep the full insight bar. No new targeting or hidden-position information is exposed.
State is checkpointed with the entity and snapshots use appended compact slots.

Readiness radiance pulses around actionable Dao research/evolution buttons and around a
Disciple at maximum insight before evolution. Unit readiness uses the approved Tight
Corona shader: a warm, softly blooming, animated corona centered about 13.6 world pixels
from the unit (roughly half the previous ray effect radius), with no geometric rays.
Labels split after “Dao of”. Visible enemies
see the unit radiance, with ordinary fog filtering. Realm advancement projects a 15-tick
(half-second) expanding gold corona reaching about one tile, independently of insight privacy.

Magic Disciples can place Shield within five tiles: a shimmering circle with the same
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
Kriegsia with no Dao research. Setup imports that change player metadata resend
authoritative start payloads to every viewer so faction/team rosters and command cards
refresh together with the restored simulation. Imports with unchanged rosters retain
the existing match and snapshot sequence. Mixed-faction Lab support is outside this change.

War Disciples gain **Throw Sword** (E): a world-point aimed spinning sword using
Line Shot's six-tile range, 0.6-tile width, eight pixels per tick travel, free cost,
and 20-second cooldown. Each enemy targetable can be hit once per leg: 40 damage
outbound and 80 returning, with the existing terrain and Shield reductions. The sword
tracks its living caster on return and expires on caster death or the shared projectile
lifetime limit. Other Dao paths and unevolved Disciples cannot cast it. Its separate
`throwSword` identity uses the existing fog-filtered line-projectile projection; no caster
lookup is needed to display the generated sword sprite. Ekat's Line Shot is unchanged.
