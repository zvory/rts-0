# Dao Masters — local animation review

## Brief and authorization

The user selected Lightness A (twin scarves), War B (forked spear), and Magic C
(broad casting sleeves), and explicitly authorized implementing all three with the
simple advancement rules below. This is a second rank of the existing Warrior/Dao
entity, not a new production catalog. Each path remains permanent.
Implementation and focused checks are authorized; PR creation is held until the user
has reviewed the in-game animation preview. This document records the Phase 0/1
decisions before implementation.

## Lightness Master: Phase 0/1

Mobile melee skirmisher; twin flowing scarves and the existing single thin katana.
Description: “Automatically advances at 120 insight, fully heals, and gains greater
size, speed, damage and health.” Same battlefield role and counters as Lightness
Adept; no new targeting, movement, ability or economy mechanic. Automatically earned
in normal games, including AI-owned units. No cost, build source, hotkey, build time,
research or additional supply: advancement of an existing Adept only. Retain supply
2, ground pathing, sight, armor, range, cooldown, penetration, Dash behavior and all
orders/cooldowns. Relative to Lightness Adept: render and collision radius ×1.2,
movement ×1.1 (2.97275 px/tick), ordinary attack damage ×1.2 (23→28, nearest integer),
maximum HP ×1.4 (115→161). No ability-damage changes.

## War Master: Phase 0/1

Armored melee damage dealer; swept helmet crest, pointed shoulders and forked yari.
Description, automatic availability, free advancement, supply and unchanged properties
as above, retaining War armor, Throw Sword, and 1.75-tile spear reach. Relative to War
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

At the end-of-tick advancement pass, a living Adept at 120 insight becomes its matching
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

Patch-note draft: Adepts automatically become Masters at 120 insight, fully heal, and
gain 20% size/damage, 10% speed and 40% maximum HP, with distinct path-specific art.

## Local review evidence

The following Interact captures use the real Pixi renderer and authoritative Lab state.
Links were published for at least 24 hours; captured media remains outside Git.

- [Standing: Adepts above, Masters below](http://olympus.tail6943f.ts.net:8091/p/w9YpITSN4ppbQXPSUT0p2bPQ/dao-master-standing-final-2026-11-23T00-08-35-129Z.png).
- [Walking, all three paths](http://olympus.tail6943f.ts.net:8091/p/c7bWaXOtkcXwtGjpkyJT7cY6/dao-master-walking-smooth.mp4).
- [Lightness attacks](http://olympus.tail6943f.ts.net:8091/p/sK3oPRFpncsvhnLcMtVv1ZYu/lightness-master-attack-review.mp4).
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
