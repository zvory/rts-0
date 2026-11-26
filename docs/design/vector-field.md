# Vector Field

Magic Adepts and Magic Masters replace Anchor with the distinct `vectorField` ability on C.
Ekat retains Magic Anchor unchanged, including its line-shot origin and recast behavior.

The first press/click fixes the center. Drag-release or a second click sets the direction.
Escape/right-click cancel. The square is 4 by 4 tiles for an Adept, 5 by 5 for a Master.
Rank is captured when placed; later rank changes do not resize an existing field. Public size and
direction are projected only with a visible field, so enemies and allies render identical geometry
without needing visibility of its caster. It lasts ten seconds and casts up to 7.5 tiles away.

Every allied or hostile ground unit inside receives an additive 3-world-pixel-per-tick displacement
(90 px/s at 30 Hz), independent of footing. Static terrain and enemy-body admission remain.
Normal commanded movement continues. Displacement does not rotate bodies. Overlapping fields
select the first active field rather than stacking. Placement is immediate and preserves the caster's active order and queued orders. A successful
cast starts a 30-second cooldown for Magic Adepts or a 20-second cooldown for Magic Masters.
Promotion preserves a cooldown already in progress.

Preview and placed field share a green arrow, square outline, and green particles. Particles move
at 90 world pixels per second in field direction, using the renderer clock so fixed captures and
pause behavior follow the existing presentation timing. They fade at the entry/exit edges.

The server validates finite center/direction, normalizes a nonzero vector, deduplicates/caps unit
ids, and applies normal owner, carrier, faction, readiness, cost, cooldown and range guards.
Vector Field has its own ability id/code and world-object kind, independent balance constants,
and preserves Magic Dao death Insight for hostile unit deaths within its square.
