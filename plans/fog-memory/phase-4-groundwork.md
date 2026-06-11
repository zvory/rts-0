# Phase 4 Groundwork: Perspective Pathing Semantics

This note inventories current movement/combat behavior and narrows the design questions in
`phase-4.md` into implementable defaults. It is intentionally documentation-only.

## Current Mechanics That Matter

- Pathfinding currently uses full live building occupancy. Terrain and all building footprints are
  considered blocked by `Occupancy::build`, and `PathingService` keys its cache by the live static
  fingerprint.
- A* returns a best-effort route toward the closest reachable tile when the exact goal is blocked
  or search budget is exhausted. `MovePhase::PathFailed` is used when the coordinator receives no
  useful route, not every time the original destination is impossible.
- Movement execution is already authoritative. A unit cannot step, slide, rotate, or be pushed into
  live static occupancy even if its path says to go there.
- Static-obstacle repath already exists for `Move`, `AttackMove`, and ability movement. If a unit
  repeatedly fails its next path step against live terrain/building occupancy for
  `STATIC_BLOCKED_REPATH_TICKS`, movement clears the stale path and marks the unit
  `AwaitingPath`; the coordinator recomputes later under the normal per-tick A* budget.
- Local steering and scout-car reverse recovery are traffic/recovery tools, not knowledge tools.
  They should not create player memory or silently solve hidden wall-offs.
- Attack-move is aggressive combat. It can acquire visible enemies within aggro range and chase
  them; ordinary units clear their path while firing, but tanks, scout cars, and upgraded riflemen
  can keep firing while moving.
- Units prefer enemy units over enemy buildings during acquisition, then fall back to nearest enemy
  in range. A visible blocker building can be attacked by attack-move today, but moving-fire units
  may continue along their route while shooting.
- Explicit attack preserves a target id while valid and visible. Combat may request chase paths to
  the target or a standoff point, and falls back to acquisition only if the explicit target is gone.
- Gather workers ignore nearby enemies while gathering. Build and gather paths use interaction
  routing, not vehicle-clearance movement routing.
- Building memory is server-only and per player. It stores visible enemy buildings, keeps hidden
  destroyed buildings stale until their footprint is scouted, and ignores lingering death vision.

## Decision Inventory

### Move Orders

Accepted direction: newly discovered blockers should cause repathing. If no owner-perspective path
exists after the blocker is known, the order should fail rather than repeatedly walking into the
same footprint.

Implementation implication: phase 6 can reuse static-obstacle repath as the detection debounce, but
phase 5 must make the recomputed path use the owner's perspective. If the newly visible/remembered
building makes the destination unreachable, the existing `PathFailed` promotion behavior should run.

### Gather And Build Orders

Accepted direction: gather and build should repath around newly discovered blockers. Workers should
not auto-attack blockers as part of gather/build.

Implementation implication: gather/build repath needs an order-specific re-request path, because
the existing static-obstacle repath only marks `Move`, `AttackMove`, and ability movement as
`AwaitingPath`. Build should keep the placement intent if a staging path can be found; if no legal
staging path exists after perspective update, clear/fail the build the same way current build
planning fails.

### Remembered Buildings As Blockers

Accepted direction: remembered buildings block future planning even if not currently visible.
Never-seen enemy buildings do not block owner-perspective planning. Destroyed-but-still-remembered
buildings also block until the remembered footprint is scouted and memory is removed.

Implementation implication: the perspective occupancy fingerprint must include owned buildings,
currently visible enemy buildings, and remembered enemy footprints, including remembered records
whose live entity no longer exists.

### Knowledge Propagation And Repath Cost

Accepted direction: once a building is known to a player, all that player's units can plan around
it. Existing active paths should not all be recomputed immediately.

Recommended default: lazy invalidation. New path requests use the updated perspective immediately,
but existing moving units keep their current path until normal triggers fire: next-step live block,
next-waypoint known-blocked precheck, chase/gather/material-goal refresh, or a fresh user order.

Rationale: eager global repath would require tracking which active paths cross each newly
remembered footprint or scanning every moving unit when memory changes. Lazy invalidation preserves
the current bounded path budget and avoids a new path dependency graph.

### Blocked Detection

Options:

- Immediate collision/path invalidation: responsive, but risks invalidating paths from transient
  body-orientation or wall-slide checks and can overreact near corners.
- Stuck-tick threshold: robust against unit traffic, but slow and conflates mobile congestion with
  static blockers.
- Failed local steering: useful for infantry only, but misses vehicles and treats steering as
  authoritative knowledge.
- Failed next-waypoint/landing validation: closest match to the current static-obstacle repath
  model; directly observes live static occupancy blocking the planned route.

Recommended default: combine failed next-step static validation with a short debounce, and add a
cheap immediate precheck only when the next planned tile is already known blocked in the owner's
perspective. Do not use generic stuck ticks as the primary hidden-building detector.

### Attack-Move Into Blocking Buildings

Open design problem: attack-move currently has two competing intents.

- The movement intent says "reach this attack-move destination."
- The combat intent says "engage visible enemies encountered on the way."

For non-moving-fire units, a visible blocker often resolves naturally because firing clears the
path and the unit holds position. For moving-fire units, especially tanks/scout cars and upgraded
riflemen, the unit can keep trying to advance while shooting, so repathing around the building can
make the unit drive away from the blocker it is also attacking.

Recommended implementable default:

- If a hostile visible building materially blocks the next path step and is attackable, attack-move
  should temporarily prioritize destroying that blocker over preserving the current path segment.
- "Materially blocks" should be tied to the same live static-blocked detection used for movement,
  not nearest visible building in sight.
- For non-moving-fire units, this can be the current behavior: acquire the blocker, clear/hold path
  while firing, then resume attack-move destination after the blocker is gone or target is lost.
- For moving-fire units, phase 6 needs an explicit suppression rule: while the blocker is the
  current blocking target and still blocks the next route, do not repath away from it every debounce
  interval. Either hold movement while firing or keep the order in a blocker-engagement substate
  until the blocker dies, becomes nonblocking, or the player issues a new order.

Question to confirm: should moving-fire attack-move units stop to destroy a material blocking
building, or should they try to path around it while opportunistically shooting? The first is more
predictable and avoids the "shoot then path away" failure mode; the second preserves mobility but
creates more edge cases.

### Explicit Attack Into Blocking Buildings

Open design problem: explicit attack has stronger target intent than attack-move. A hidden/visible
building might block the chase path to the ordered target but is not the user's chosen target.

Recommended implementable default:

- Preserve the original explicit target id.
- If the explicit target is currently fireable, ignore the blocker and fire at the target.
- If the explicit target is not fireable and a hostile visible building is materially blocking the
  chase path, allow a temporary blocker engagement.
- After the blocker dies, becomes nonblocking, or no longer blocks progress, resume chase toward
  the original explicit target.
- If the blocker is neutral/friendly/non-attackable, repath around it if possible; otherwise let the
  explicit attack become unreachable under existing unreachable/path-failed behavior.

Question to confirm: should explicit attack ever retarget permanently to a blocker, or only use a
temporary blocker engagement that keeps the original target intent?

## Recommended Phase 5 Contract

- Add an owner-perspective occupancy view for path planning only.
- Keep live occupancy for movement landing legality, collision, placement, construction, spawn,
  damage, and any invariant checks.
- Include perspective identity/fingerprint in path cache keys.
- New path requests should immediately see newly refreshed memory for their owner.
- Existing active paths should not be proactively recomputed when memory changes.

## Recommended Phase 6 Contract

- Detection source: live movement/static validation, optionally plus known-next-waypoint precheck.
- Memory source: only refresh memory through the existing fog-visible building memory rules.
- Move: repath under owner perspective; if no useful path remains, `PathFailed`.
- Attack-move: if materially blocked by a hostile attackable building, engage that blocker first;
  otherwise repath under owner perspective.
- Explicit attack: preserve original target intent; temporarily engage a material hostile blocker
  only while the original target is not fireable.
- Gather/build: repath under owner perspective; fail/clear only if the interaction/staging path is
  still unavailable; never auto-attack.
- Repath throttling: use the existing path budget and `last_repath_tick` throttle, with no global
  scan of all paths when memory changes.

## Tests To Write Before Code

- Move paths through a never-seen enemy building under perspective planning, then fails live
  movement, discovers/refreshes memory when visible, and repaths around it.
- Move into a fully sealed hidden wall-off, discovers/refreshes memory, then reaches `PathFailed`
  instead of looping forever.
- Newly issued move after scouting a wall-off avoids remembered blockers.
- Existing far-away active path is not immediately recomputed when another unit scouts a building;
  it only changes on a normal path trigger.
- Remembered destroyed building blocks planning until its footprint is scouted and memory is
  removed.
- Gather and build repath around a discovered blocker without creating an attack order.
- Attack-move with a non-moving-fire unit stops to destroy a material blocking building, then
  resumes the attack-move destination if the order is still valid.
- Attack-move with a moving-fire unit covers the confirmed design choice: either hold to destroy
  the blocker or repath around it without oscillating.
- Explicit attack keeps the original target id while temporarily handling a material blocking
  building, then resumes chasing the target.

## Remaining Questions For User Confirmation

- For moving-fire attack-move units, should a material blocking building force a stop-and-destroy
  behavior, or should those units keep mobility and repath around it while shooting?
- For explicit attack, should blocker engagement always be temporary with the original target
  preserved, or can the blocker become the new target?
- If a path is only partially blocked by a remembered-but-destroyed building, is the desired player
  experience to require scouting the footprint before units will use the opened route? The current
  accepted memory rule implies yes.
