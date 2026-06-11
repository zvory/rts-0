# Phase 4: Design Client-Perspective Pathing Semantics

Status: planned

## Goal

Resolve gameplay semantics before changing A* so the implementation has clear answers for hidden
blockers, discovery, and order transitions.

## Required User Prompts

Ask the user to decide or confirm:

- Should move orders always repath around a newly discovered hidden blocker, or should they stop if
  the route is fully blocked? User answer: if some waypoints are blocked, i guess we have to repath, otherwise they will get stuckm trying to reach a waypoint in the middle of a building. 
- Should attack-move attack an enemy building that blocks the path, repath around it, or prefer one
  based on distance/threat? Depends on existing attack move beahviour. I believe attack move currently will halt to attack buildings, in this case, I guess the problem resolves itself as buildings get destroyed by halted units. But this doesn't work for units with move while shooting, because they will still keep moving and now their path is impossible because it's under buildings. Ideally, units decide to "bull through and destroy the buildings" but maybe that invites a lot of tricky edge cases. I think it's better to just repath with attack move? But then they'll start autoattacking the building since it's in vision, destroy it, and then _path away_ to the new path. Fuck! We need ot have a deeper conversation about the design space here.
- Should explicit attack orders attack a blocking enemy building if it prevents reaching the
  original target, or should they keep trying to reach the original target? See answer above, need to do a deep dive.
- Should gather/build orders repath around hidden blockers, fail with feedback, or allow workers to
  attack blockers? GATHER/BUILd should just repath when blockers are found. 
- When should a unit consider itself blocked by an unseen structure: immediate collision/path
  invalidation, stuck ticks threshold, failed local steering, or failed next-waypoint validation? I don't know, I don't know what the trade-offs are for each option, something to talk about.
- Does scouting a blocker elsewhere on the map immediately update future pathing for all of that
  player's units? Yes, all units can become awre, except the only problem I see here is that uhh, every time we discover a building, do we have to recompute every path? That could get expensive. we could have a lookup table for paths that pass through this point, but that sounds like a complex data structure to maintain. Another thing to think about! So mayhbe we repath only on next waypoint (or next waypoint in vision) is blocked? Idk.
- Should remembered buildings continue to block perspective pathing after they are no longer
  visible, or should only currently visible buildings block pathing? Recommended default: remembered
  buildings block planning because the player believes they are there, while never-seen buildings do
  not. remembered buildings block planning, never seen buildings do not. if a remembered building is destroyed, but is still remembered, it should also block planning. 

## Recommended Baseline

- Move orders: when live movement discovers a hidden blocker, update memory if visible, then request
  a new perspective path. If no path exists, enter `PathFailed` and surface existing command
  feedback if available.
- Attack-move: if the discovered blocker is hostile and attackable, attack it when it materially
  blocks progress; otherwise repath.
- Explicit attack: keep intent on the original target, but allow attacking a hostile blocker only if
  it is directly preventing path progress and the original target is not currently fireable.
- Gather/build: repath first; fail rather than auto-attack unless the user explicitly wants worker
  aggression.
- Repath trigger: use a small stuck/blocked threshold on movement against live occupancy, plus
  immediate invalidation when the next planned tile is now known blocked.

## Expected Touch Points

- `docs/design/server-sim.md`
- Possibly a new short design note under `plans/fog-memory/`
- No gameplay code unless the user explicitly asks to combine design and implementation

## Verification

- Documentation review only.
- Confirm the accepted semantics are specific enough to implement tests before code starts.

## Manual Testing Focus

No manual gameplay testing is required in this design-only phase. The handoff should list the
manual scenarios that phase 6 will need to cover.

## Handoff

The handoff should quote or summarize the accepted user decisions and identify any unresolved
questions. It should tell the next agent which order transitions to implement and which tests must
be written first.
