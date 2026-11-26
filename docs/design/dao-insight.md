# Disciple insight and Dao evolution

Phase 0/1 briefs and rules (completed before implementation): each path is a permanent
specialization of the existing Disciple, retaining its entity id, orders,
sight, and ordinary ground pathing. War alone has a specialization-specific collision radius. Base and Wind Disciples use 0.75-tile sword reach; War uses 1.75-tile spear reach. All three retain five-tile automatic engagement with melee pursuit. Magic replaces its sword with a five-tile ranged primary; its engagement range equals that five-tile attack range. Combat evolution needs no additional
training source or build time. A Dao button assigns its path to every selected, owned, unevolved Disciple, even before
insight is ready. Selecting another path replaces the pending choice. This
entity-owned intent is independent of orders: moving, attacking, Stop, and queued orders
do not cancel it. No automatic research purchase, Portal default, or army-wide promotion
is added. At full insight, a Novice without a pending choice automatically receives one of the
three Taos with equal probability using the authoritative, checkpointed simulation RNG.
An explicitly selected path always takes precedence. Evolution is free and commits during the tick once a legal evolved body fits and the unit has 60 insight, capped at 60; misses, friendly fire, invulnerability and overkill grant
no extra insight. Actual hostile unit HP loss grants one insight per HP removed (two for War Adepts damaging Tanks); building HP loss grants a fixed quarter rate. Novice, Wind, War, and Magic ordinary attacks and eligible ability projectiles share this rule, including both legs of Throw Sword. Advancement clears all existing insight. Successful advancement fully restores HP to the evolved unit’s maximum,
after applying any path-specific maximum-HP bonus. Novices have 58 maximum HP; Wind and Magic Adepts have 150; War Adepts have 176. Novice evolution uses the existing bounded body-growth clearance search before committing the larger Adept body. If no nearby legal pose exists, keep the chosen Dao, insight and HP and retry next tick; do not reroll an automatic choice or heal until evolution can commit. Novice collision radius is 17 world pixels; its artwork and 19.5-pixel direct/outer click radius remain unchanged. Adept collision radii stay independent at 26 pixels for Wind/Magic and 18.2 for War, with the existing Master ×1.2 body scaling. Base moves at 2.0 px/tick; War and Magic move at 2.1 px/tick. Choosing a pending path or repeating
an evolution request does not heal; dead units cannot advance. One chosen-path evolution per unit, followed by one automatic Master advancement. Player research persists after Pavilion loss.
The choice clears on evolution; evolved units cannot change paths. Pending choices persist
through checkpoints and are visible only to teammates and privileged observers. The command
card highlights chosen paths with an assigned/eligible count; tooltips show missing
insight, and unit details show the pending path. Automatic evolution runs after combat,
research production, and ability runtime each tick, skipping dead units.
Pavilion research unlocks direct Portal production of the matching Adept for 150 steel,
35 oil, three supply and ten seconds, without a Novice input. It also unlocks paid
one-for-one training in the [Proving Grounds](proven-grounds.md),
with the existing shared research price tiers, serial queue and prerequisite/faction gates.
Combat-earned evolution requires neither research nor a Pavilion.

Novices, War Adepts, and War Masters have **Charge**, an automatic passive shown as a disabled command-card tile with the
autocast swirl and authoritative cooldown clock. It has no hotkey, cast action, or autocast toggle. Against a living
hostile unit selected by normal combat targeting (explicit attack, attack-move, or automatic
engagement), Novices gain 1.5× movement speed for 30 ticks (one second); War Adepts and Masters gain 2× speed for up to 120 ticks (four seconds). War Charge ends on the first committed primary attack, including a miss, loss of a legal combat target, or replacement of the engagement order. It grants no moving attacks. All ranks retain a 450-tick
(15-second) cooldown from activation. It only triggers within five tiles of center distance, at least two tiles away,
or at shorter distance when the target's actual movement has a positive component away
from the carrier. Nearby stationary, sideways-moving, or approaching targets do not trigger it.
Buildings and friendly targets never trigger it. Movement duration and cooldown survive
checkpoints; evolution into War retains the speed benefit, while Wind or Magic removes it.
Charge, Master movement bonuses, and Haste multiply together.

- Wind: mobile melee skirmisher; 1.15 times scout-car base speed (2.7025 px/tick), 1.5 attacks per original attack
  interval (cooldown divided by 1.5), twelve-tile Blink without a return recast.
  Uses a sword at the base Disciple’s 0.75-tile reach, dealing the same 23 damage with 25% armor penetration (50% at Master rank) and a 61-tick cooldown. Plain Move keeps its destination while striking legal targets within melee reach. Explicit Attack and automatic melee pursuit also retain their current approach path while striking; the approach still ends at sword reach, so a fleeing target can cause short stop/repath cycles as it leaves range; automatic Idle/Attack Move retains five-tile acquisition and closes to sword reach. Blink reaches twelve tiles; Ekat retains five tiles and its optional return. Wind charge and recharge rules are described below. The old archer source and strip are preserved for reuse. Selected candidate B uses a thin katana and a single shoulder scarf: resting close to the back at idle, trailing while running, and following four melee swing poses. Classic evolution key L; grid Z.
- War: armored melee damage dealer; 1.05 times base movement speed (2.1 px/tick), 1.5 damage, +26 maximum HP relative to the other Adepts, 50% armor
  reduction using the existing penetration mechanic (full AP bypasses it), keeps the Disciple's existing
  100% primary armor penetration at both ranks. Uses the approved larger armored candidate B Yari silhouette, with idle, two walking poses and four thrust poses. Its sprite scale is 0.56, halfway from the former 0.8 toward the base Disciple’s 0.32. The collision/interaction circle shrinks by the same 30%, from 13.5 to 18.2 world pixels. Its 1.75-tile weapon range plus that radius and four-pixel range slack matches the smaller spear. Vehicle hulls and building footprints retain their existing contact geometry. The 400 ms attack clip anticipates, drives, extends and recovers, with a smooth 11.2-world-pixel presentation-only forward step; collision, authoritative position and damage timing do not move with the artwork. Classic W; grid X.
- Magic: battlefield support; 1.05 times base movement speed (2.1 px/tick), Vector Field and Shield. Vector Field is a centered 4-by-4-tile square (5 by 5 at Master rank) that adds 3 world pixels per tick in the selected direction to every enemy ground unit, independent of weight. Ekat retains her separate radial Magic Anchor. Magic uses the approved Magic C hooded spellcaster, with arms swept backward and flowing mantle while running. Its five-tile orbs deal the same 23 damage with 25% armor penetration (50% at Master rank) and 91-tick cooldown. It stops to fire normally and has no secondary sword. Classic C; grid C. All three Dao silhouettes replace their legacy glyphs; insight, readiness and advancement overlays remain. Bespoke audio remains deferred.

Each paid training path has a separate researched unlock; combat-earned evolution is unrestricted. No automatic AI evolution in this pass;
AI can encounter and damage evolved units. Insight uses a blue fill over red background
under the HP bar for everyone who can see the unit, including enemies. Exact insight is public in visible-unit snapshots and selected-unit details. Evolved units
reset to zero insight and have a cap of 200. Wind, War, and Magic continue earning insight from
actual hostile HP damage; Magic also earns Vector Field insight as described below; reaching 200 automatically advances living Adepts to their matching
[Master rank](dao-masters.md), fully healing to 1.4 times their Adept maximum HP. Masters
are 1.2 times Adept size and 1.1 times speed. Wind and Magic deal 1.2 times ordinary attack damage (23→28); War doubles its rounded rank-scaled damage (35→42→84). Wind and Magic primary penetration rises from 25% to 50%; War retains 100%. They
reset insight to zero and stop earning it; no further advancement or repeat healing. Adept bars
and selected-unit details use the new cap. Tournament winners follow the same rules. No new targeting or hidden-position information is exposed.
State is checkpointed with the entity and snapshots use appended compact slots.

Readiness radiance pulses around actionable Dao research/evolution buttons and around a
Disciple at maximum insight before evolution. Unit readiness uses the approved Tight
Corona shader: a warm, softly blooming, animated corona centered about 13.6 world pixels
from the unit (roughly half the previous ray effect radius), with no geometric rays.
Labels split after “Dao of”. Visible enemies
see the unit radiance, with ordinary fog filtering. Realm advancement projects a 15-tick
(half-second) expanding gold corona reaching about one tile, alongside the public insight bar.

Magic Disciples can place Shield within five tiles: a shimmering circle with a
2.1-tile radius, covering allied units (including the caster).
The field has 150 HP and grants a separate 75% armor layer against hostile attacks entering
from outside; attacks originating inside bypass its armor and do not drain its HP. Existing
unit armor, facing, cover, and terrain reductions apply first. Shield armor then uses the ordinary
armor-penetration formula: `damage × (1 − 0.75 × (1 − penetration))`, rounded to the nearest HP.
An otherwise 100-damage hit deals 25 HP at 0% piercing, 63 HP at 50% piercing, and 100 HP at
100% piercing. Regardless of piercing or the unit's defenses, the shield loses the attack's full
pre-reduction damage (area hits use their distance-falloff damage). Even fully piercing hits drain it.
A field active at impact protects the entire hit, including the hit that depletes it; excess durability
cost does not become damage to the unit. Depletion removes the field immediately for later hits.
Overlapping fields never multiply armor or charge the same hit twice: the first eligible field in
stable caster-id order supplies the armor and pays the full hit, even when that hit breaks it.
This applies to direct weapons, Throw Sword (per leg), Line Shot, and each area-damage victim;
tank HE splash is non-piercing. Mortar/rocket and artillery checks use recorded firing positions,
which survive attacker death and checkpoint restore. Friendly fire, environmental damage,
buildings, invulnerable targets, and enemy units receive no shield armor and do not drain it.
Shield costs no resources, has a ten-second
cooldown, replaces the caster’s prior shield, and expires after 5 seconds (10 seconds for Magic Masters, whose shield also has 300 HP) or earlier on
depletion, replacement, or caster death. In-range casts preserve movement and queued orders;
out-of-range immediate casts replace the current order with an approach-and-cast order. The pool, fixed world center, and absolute expiry tick live on the caster entity and survive
checkpoints; the existing ability-runtime allocator supplies its unique visual id.
All visible viewers see a gray lifetime bar on a black background at the top of the field,
six pixels below its HP-bar position. It drains from full to empty using the server-provided
remaining ticks and the mirrored 5-second duration, independently of damage received.
All viewers who can see the field receive its remaining HP, including enemies and allies.

Lab commands continue to use the selected owner’s faction and research. A Disciple
spawned for a Kriegsia player can move but cannot evolve; the Dao preview’s P2 is
Kriegsia with no Dao research. Setup imports that change player metadata resend
authoritative start payloads to every viewer so faction/team rosters and command cards
refresh together with the restored simulation. Imports with unchanged rosters retain
the existing match and snapshot sequence. Mixed-faction Lab support is outside this change.

War Adepts and War Masters can cast **Throw Sword** (E). It is a world-point aimed spinning sword using
a 7.5-tile range, 0.9-tile hitbox width, 100/11 pixels per tick travel, free cost,
and a 40-second cooldown for Adepts or 20 seconds for Masters. Advancement preserves an existing cooldown. Each enemy targetable can be hit once per leg: 20 damage
outbound and 40 returning before protection. Throw Sword has zero armor penetration:
armored targets take one quarter damage (5 outbound, 10 returning). Dao armor and
normal direct-hit entrenchment protection apply before the existing map-tile cover
and Shield reductions. Small/unarmored targets retain full base damage. The sword
tracks its living caster on return and expires on caster death or the shared projectile
lifetime limit (round-trip travel time plus one second). At maximum range the flight
targets 1.76 seconds, 10% above the original 1.6 seconds. Each leg rounds up
to 27 simulation ticks, so the observed full flight is 54 ticks (1.8 seconds at 30 Hz).
The spinning sprite is 48 world pixels long,
1.5 times its original dimensions. Other Dao paths and unevolved Disciples cannot cast it. Its separate
`throwSword` identity uses the existing fog-filtered line-projectile projection; no caster
lookup is needed to display the generated sword sprite. Ekat's Line Shot is unchanged.

Magic uses shot-triggered windup, release, follow-through and recovery frames over 400 ms. Wind uses four event-triggered melee swing poses from candidate B. Projectile presentation starts at the release frame (100 ms); the existing immediate authoritative damage timing is unchanged. Arrows use one shared generated texture and a bounded reusable sprite pool. Orbs are about seven world pixels across, with a small halo and one shimmering cross drawn into the shared feedback graphics batch: no emitter, per-orb display objects, blur, filters or retained particles. Retained arrow presentation support and Magic orbs consume only authorized attack endpoints and expire within 600 ms. Changing specialization preserves remaining reload and clears obsolete automatic melee pursuit without canceling explicit Move orders.

## Engine unit identity

The existing approved units are distinct authoritative kinds: Novice (`Warrior`, `warrior`),
Wind Adept (`WindAdept`, `wind_adept`), War Adept (`WarAdept`, `war_adept`),
and Magic Adept (`MagicAdept`, `magic_adept`). This is an identity correction for existing units,
not additional content or a balance change. The user authorized implementation of this correction.

Their Phase 0 roles, counters, descriptions, visuals, audio deferrals, and Phase 1 stats,
abilities, keys, acquisition costs/timing/prerequisites, supply, sight, and movement/pathing
remain as specified above and in the Proving Grounds design. Each path is already approved;
the existing Dao researches also unlock their corresponding Portal train buttons.
Direct Portal production costs 150 steel, 35 oil, three supply and ten seconds. Combat evolution changes kind in place, preserving id, ownership, orders, and
reload; paid training and tournament winners use the same transition. Direct Lab spawning of
an Adept initializes its existing full stats and abilities.

Kind is the single source of permanent specialization in simulation and checkpoints. The
snapshot `dao` value is derived from kind for existing presentation consumers, with no second
mutable Dao identity in the engine. Pending `chosenDao` remains base-only private intent.
The new compact kind codes are 35 (Wind), 36 (War), and 37 (Magic); slots are unchanged.
Selection uses authoritative kind directly. Saves/replays from the old shared-kind representation
are not migrated under the repository's latest-version-only policy.

Bundled Lab setups declare simulation schema 7. Their existing non-Disciple entity states are
unchanged; catalog restoration checks validate every refreshed checkpoint with the current engine.

Adepts (and their Master advancement) use three supply; Novices use two. Combat
evolution increases used supply immediately and may exceed the cap. Conversion queues
reserve three supply; Proving Grounds entrants wait if the additional supply is unavailable.

Wind uses the separate `windDash` ability: twelve-tile movement without a return marker or recast. Adepts hold one charge and Masters two, recharging sequentially every eighteen seconds. Promotion grants the additional ready charge while preserving existing recharge progress.


## Combat insight

Novices, Wind Adepts, War Adepts, and Magic Adepts earn one insight per actual hostile
unit HP removed, regardless of attacker or target cost, including zero-cost units.
War Adepts earn double insight (two per HP removed) against Tanks, including both
legs of Throw Sword.
Buildings give 0.25 insight per actual hostile HP removed, except Wind (Dao of Lightness)
earns one insight per HP removed from Pumpjacks and Steel Mines. Damage before the
killing blow counts; overkill grants nothing. There is no killing-blow bonus, and Throw
Sword earns insight at the same target-specific rates. Existing shield, armor, and hostility rules apply.

Arithmetic uses a fixed denominator of four: unit damage and Wind extractor damage
contribute four numerator units per HP, War Adept damage to Tanks contributes eight,
and ordinary building damage contributes one.
Fractional numerators carry between hits and target changes, persist in checkpoints,
and remain server-private. At the insight cap, evolution, or Master advancement,
discard the remainder alongside any excess insight. Masters earn no further insight.


## Magic support insight

Shield grants no insight from casting, durability consumption, piercing hits, expiry, or replacement.

Each enemy unit that dies with its center inside an active Vector Field grants its living Magic
caster 20 insight, regardless of killer or unit cost. Allied deaths, buildings, expired Vector Fields,
and dead casters grant nothing. Each overlapping Vector Field rewards its own caster. Rewards cap
at 200; Masters earn none. No public death information or wire fields are added.
