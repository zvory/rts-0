# Vector Field

Magic Adepts and Magic Masters replace Anchor with the distinct `vectorField` ability on C.
Ekat retains Magic Anchor unchanged, including its line-shot origin and recast behavior.

The first press/click fixes the center. Drag-release or a second click sets the direction.
Escape/right-click cancel. The square is 4 by 4 tiles for an Adept, 5 by 5 for a Master.
Rank is captured when placed; later rank changes do not resize an existing field. Public size and
direction are projected only with a visible field, so enemies and allies render identical geometry
without needing visibility of its caster. It lasts ten seconds and casts up to 7.5 tiles away.

Every allied or hostile ground unit inside, except actively entrenched infantry, receives an additive displacement equal to half its own effective movement speed, independent of footing resistance.
The rate includes movement upgrades (including Methamphetamines and Tank+), active speed effects,
and terrain modifiers; it does not depend on whether the unit is currently moving.
Slope speed is evaluated against the combined field direction. Static terrain and enemy-body admission remain.
Active trench occupation grants immunity to Vector Field displacement, including overlapping fields;
dig-in progress alone grants no immunity, and leaving the trench ends it immediately.
Normal commanded movement continues. Displacement does not rotate bodies. Overlapping fields
sum their directional vectors without normalization: matching directions stack, opposite directions
cancel, and perpendicular directions combine diagonally. For any unit moving directly against the field on uniform terrain, one opposing field halves its
progress and two cancel its movement, including Scout Cars, Tanks, and normal or meth-boosted Riflemen. Placement is
immediate and preserves the caster's active order and queued orders. A successful cast starts a 30-second cooldown for Magic Adepts or a 20-second cooldown for Magic Masters.
Promotion preserves a cooldown already in progress.

Preview and placed field share a green arrow, square outline, and green particles. Particles move
at 30 world pixels per second in field direction, using the renderer clock so fixed captures and
pause behavior follow the existing presentation timing. They fade at the entry/exit edges.

The server validates finite center/direction, normalizes a nonzero vector, deduplicates/caps unit
ids, and applies normal owner, carrier, faction, readiness, cost, cooldown and range guards.
Vector Field has its own ability id/code and world-object kind, independent balance constants,
and preserves Magic Dao death Insight for hostile unit deaths within its square.
