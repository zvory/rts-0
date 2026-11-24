# Dao Masters — local animation review

## Brief and authorization

The user selected Wind A (twin scarves), War B (forked spear), and Magic C
(broad casting sleeves), and explicitly authorized implementing all three with the
simple advancement rules below. This is a second rank of the existing Warrior/Dao
entity (now an authoritative path-specific Adept kind), not a new production catalog. Each path remains permanent.
The user reviewed the animation and corrected War Master scale, then authorized merging. This document records the Phase 0/1
decisions before implementation.

## Wind Master: Phase 0/1

Mobile melee skirmisher; twin flowing scarves and the existing single thin katana.
Description: “Automatically advances at 200 insight, fully heals, and gains greater
size, speed, damage and health.” Same battlefield role and counters as Wind
Adept; no new targeting, movement, ability or economy mechanic. Automatically earned
in normal games, including AI-owned units. No cost, build source, hotkey, build time,
research or additional supply: advancement of an existing Adept only. Retain supply
2, ground pathing, sight, armor, range, cooldown, penetration and all
orders/cooldowns. Relative to Wind Adept: render and collision radius ×1.2,
movement ×1.1 (2.97275 px/tick), ordinary attack damage ×1.2 (23→28, nearest integer),
maximum HP ×1.4 (115→161). No ability-damage changes.

## War Master: Phase 0/1

Armored melee damage dealer; swept helmet crest, pointed shoulders and forked yari.
Description, automatic availability, free advancement, supply and unchanged properties
as above, retaining War armor and 1.75-tile spear reach, and reducing Throw Sword cooldown from 40 to 20 seconds. Advancement preserves an existing cooldown. Relative to War
Adept: render and collision radius ×1.2, movement ×1.1 (2.31 px/tick), ordinary attack
damage ×1.2 (35→42, after existing rounded War damage bonus), maximum HP ×1.4
(135→189). Existing counters and penetration rules remain. No ability-damage changes.

## Magic Master: Phase 0/1

Ranged support caster; broad bell sleeves and three trailing mantle panels.
Description, automatic availability, free advancement, supply and unchanged properties
as above, retaining five-tile orb primary, Shield and Anchor. Relative to Magic Adept:
render and collision radius ×1.2, movement ×1.1 (2.31 px/tick), ordinary attack damage
×1.2 (23→28), maximum HP ×1.4 (115→161). Existing counters remain. Shield capacity,
Anchor strength, spell ranges and all cooldowns stay unchanged.

## Shared advancement and persistence

At the end-of-tick advancement pass, a living Adept at 200 insight becomes its matching
Master once, resets insight to zero, and heals to its new maximum HP. No choice,
research or order is needed; the Novice's existing chosen-Dao advancement is unchanged.
Masters stop accumulating insight and cannot advance or heal this way again. No heal
for dead entities. Retain entity id, owner, target/order queue, ability state and attack
reload. Reuse the existing 15-tick advancement corona. Rank is durable entity state,
checkpointed and fog-filtered with the visible entity. Exact insight remains private.
HUD/portraits/minimap use the Master label and matching art. Lab review uses normal
combat advancement or a valid checkpoint; no network command grants free advancement.

## Animation and review

Per path: one standing frame, three walk/run poses, four attack poses. Use the selected
generated design as an explicit image reference. Fixed overhead camera and crown/body
scale, white/off-white tint regions, charcoal accents, keyed magenta matte. Retain
shot-driven attack timing and ordinary movement cadence. Normalize torso size to the
current Adept before applying the 1.2 rank scale; do not size by flowing cloth bounds.
Inspect standing, moving and attacking in the real Pixi renderer and capture stills and
video for user review. Bespoke audio remains deferred; existing audio behavior stays.

Patch-note draft: Adepts automatically become Masters at 200 insight, fully heal, and
gain 20% size/damage, 10% speed and 40% maximum HP, with distinct path-specific art.

## Local review evidence

The following Interact captures use the real Pixi renderer and authoritative Lab state.
Links were published for at least 24 hours; captured media remains outside Git.

- [Standing: Adepts above, Masters below](http://olympus.tail6943f.ts.net:8091/p/w9YpITSN4ppbQXPSUT0p2bPQ/dao-master-standing-final-2026-11-23T00-08-35-129Z.png).
- [Walking, all three paths](http://olympus.tail6943f.ts.net:8091/p/c7bWaXOtkcXwtGjpkyJT7cY6/dao-master-walking-smooth.mp4).
- [Wind attacks](http://olympus.tail6943f.ts.net:8091/p/sK3oPRFpncsvhnLcMtVv1ZYu/wind-master-attack-review.mp4).
- [War attacks](http://olympus.tail6943f.ts.net:8091/p/VH4rTsHVOiC4QWRuopLNxDpT/war-master-attack-review.mp4).
- [Magic attacks](http://olympus.tail6943f.ts.net:8091/p/Qy59ya6mumu5a5IhCFqVRjow/magic-master-attack-review.mp4).

Source art and portable imagegen prompts are under
`client/assets/rigs/disciple-dao/source/`; generated runtime strips are reproducible
with `node scripts/art/disciple-dao-pipeline.mjs --check`.

Focused verification: all 15 existing Warrior tests and three new Master tests pass;
Master compact serialization, protocol parity, sprite/portrait/minimap routing,
selection, health overlays, config surface and presentation-frame checks pass.
Client/sim architecture, crate boundaries, source caps, docs health and faction catalog
parity pass; the 17 wiki regression tests also pass. Strict Clippy for the changed libraries passes. An additional strict
Clippy run including test targets fails on 82 existing warnings in unrelated test code
(for example `entity/tests.rs` argument count and checked division); this draft does
not change those tests. The full PR gate is intentionally deferred until animation
review and PR creation.

Review reproduction: worktree `zvorygin/dao-master-preview`; Lab attack checkpoint
artifact `artifact_752e3d0782334f5fb5a94fe2ddcf156f` is retained locally under the
worktree’s ignored Interact artifacts. Targets and attackers use Lab invulnerability
for repeated attack animation sampling; gameplay damage/advancement is tested separately.
The gameplay server and Interact daemon are shut down after capture.

## War Master scale follow-up

The user accepted the animations and requested a size correction before PR creation.
The initial source normalization used 0.30 for War Master versus 0.31 for War Adept,
but the Master source’s core torso is smaller. Normalize the approved core-torso
landmarks (Adept 112 source pixels, Master 84) using `0.31 * 112 / 84`, then retain
the existing 1.2 Master world scale. The full eight-frame Master strip uses 256×384-pixel
cells to avoid clipping the unchanged spear/attack geometry. Novice and War Adept
art, all gameplay stats and animation cadence stay unchanged. Before/after captures
use the same checkpoint, viewport, camera and unit positions, with Novice, War Adept
and War Master from left to right. The user approved the result and authorized PR delivery.

Follow-up review artifacts (one-hour preview lifetime):

- [Labeled before/after WebM](http://olympus.tail6943f.ts.net:8091/p/uiSTmgZBqBJBN0Q6tcIn16Yr/war-master-before-after.webm).
- [Labeled before/after still](http://olympus.tail6943f.ts.net:8091/p/8zwvw7n3lP7z-JbNVdfCznji/war-master-before-after.png).

The seven-second WebM shows standing and walking before the correction, followed by
the same sequence afterward. Focused sprite contracts check all eight War Master
frames, atlas dimensions and unclipped frame edges; pipeline determinism, client
architecture, source caps and docs health pass. The task-only Lab server is stopped.

Integration with current main retains separate authoritative Adept kinds, Novice
Charge and fifteen-tile Wind Dash. Master rank is the appended public `daoMaster`
boolean on each Adept kind; compact snapshot vocabulary is version 65.

### Safe body growth beside blockers

Master promotion uses the current post-production/death static topology. If growth
needs clearance, it tries deterministic half-pixel local adjustments (bounded by
twice the radius increase plus one pixel). If none fits, insight remains full and
HP unchanged until a legal nearby body position becomes available. Movement, unit
collision pushes, Dash/return and Lab placement validate the entity’s actual radius.
This prevents promotion or movement from leaving the larger body inside a building.

## Wind Dash charges

Wind uses its own fifteen-tile Dash without a return marker or recast. Adepts hold one charge; Masters hold two. Promotion adds one ready charge and preserves an existing recharge timer. Charges recharge sequentially, one every eight seconds; either ready charge can be used immediately. Kat retains its existing Dash and return behavior.
