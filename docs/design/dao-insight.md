# Disciple insight and Dao evolution

Phase 0/1 briefs and rules (completed before implementation): each path is a permanent
specialization of the existing Disciple, retaining its entity identity, orders, cost,
supply (2), sight, size, and ordinary ground pathing. Base, War, and Lightness Disciples use 0.75-tile sword reach and five-tile automatic engagement with melee pursuit. Magic replaces its sword with a five-tile ranged primary; its engagement range equals that five-tile attack range. No additional
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

- Lightness: mobile melee skirmisher; 1.15 times scout-car base speed (2.7025 px/tick), 1.5 attacks per original attack
  interval (cooldown divided by 1.5), existing Ekat Dash with optional return unchanged.
  Uses a sword at the base Disciple’s 0.75-tile reach, dealing the same 23 damage with 50% armor penetration and a 43-tick cooldown. Plain Move keeps its destination while striking legal targets within melee reach; automatic Idle/Attack Move retains five-tile acquisition and closes to sword reach. Speed and Ekat Dash with optional return are unchanged. The old archer source and strip are preserved for reuse. Selected candidate B uses a thin katana and a single shoulder scarf: resting close to the back at idle, trailing while running, and following four melee swing poses. Classic evolution key L; grid Z.
- War: armored melee damage dealer; 1.5 damage, +20 maximum HP, 50% armor
  reduction using the existing penetration mechanic (full AP bypasses it), keeps the Disciple's existing
  50% armor penetration. A centered neon-red sword with black outline identifies it, rotated 90 degrees and lengthened 20%. Classic W; grid X.
- Magic: battlefield support; 1.1 times the buffed base movement speed (2.024 px/tick), shared Ekat pull field (renamed Anchor), with doubled inward pull (0.70 movement-speed idle drag and up to 1.70x movement toward its center; radial falloff and weight resistance still apply) and a new Shield, uses the approved Magic C hooded spellcaster, with arms swept backward and flowing mantle while running. Its five-tile orbs deal the same 23 damage with 50% armor penetration and 64-tick cooldown. It stops to fire normally and has no secondary sword. Classic C; grid C. War keeps its existing sword art and red sword icon. Lightness chevrons and the Magic pentagram are removed; insight, readiness and advancement overlays remain. Bespoke audio remains deferred.

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
cooldown, replaces the caster’s prior shield, and expires after 15 seconds or earlier on
depletion, replacement, or caster death. In-range casts preserve movement and queued orders;
out-of-range immediate casts replace the current order with an approach-and-cast order. The pool, fixed world center, and absolute expiry tick live on the caster entity and survive
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

Magic uses shot-triggered windup, release, follow-through and recovery frames over 400 ms. Lightness uses four event-triggered melee swing poses from candidate B. Projectile presentation starts at the release frame (100 ms); the existing immediate authoritative damage timing is unchanged. Arrows use one shared generated texture and a bounded reusable sprite pool. Orbs are about seven world pixels across, with a small halo and one shimmering cross drawn into the shared feedback graphics batch: no emitter, per-orb display objects, blur, filters or retained particles. Retained arrow presentation support and Magic orbs consume only authorized attack endpoints and expire within 600 ms. Changing specialization preserves remaining reload and clears obsolete automatic melee pursuit without canceling explicit Move orders.
