# Scout Car Movement Plan

Goal: make scout cars feel reliable in tight bases and narrow lanes without turning them into tanks
or sideways-sliding infantry. The current implementation already has route lookahead, bounded yaw,
traffic throttle, tolerant arrival, and reverse recovery, but it still lets scout cars choose routes
and local steps that ride walls too closely. This plan resets the approach around clearance and
car-like motion primitives.

## Design Principles

- Scout cars are pose-driven vehicles: `(x, y, facing)` is the movement state.
- Static geometry should be avoided before contact, not recovered from after contact.
- Hard collision uses the authoritative server body; visuals may remain truck-like.
- Global routing provides a corridor, not an exact spline or collision guarantee.
- Local movement chooses from legal car-like candidate motions each tick.
- Reverse exists as a straight, bounded recovery/control primitive, not a general steering mode.
- Determinism and replay stability matter more than physically perfect vehicle simulation.

## Research Basis

- Recast-style navigation uses agent radius / area erosion so paths are generated in space where an
  agent can exist, not directly against source collision geometry.
- Clearance-aware navmesh work calls out the exact failure mode we see here: waypoints without
  guaranteed clearance cause agents to move too close to static geometry, slide along walls, or get
  stuck.
- Hybrid A* for car-like robots searches `(x, y, theta)` with forward/reverse vehicle motions. It is
  the right conceptual model, but the RTS implementation should use a bounded local version first.
- Naive obstacle potential fields can make narrow passages effectively untraversable. Any wall
  avoidance cost must taper based on local clearance so scout cars prefer open space without
  refusing legitimate chokepoints.

Useful references:

- https://recastnav.com/md_Docs_2__1__Introduction.html
- https://dev.epicgames.com/documentation/unreal-engine/API/Runtime/Navmesh/rcErodeWalkableArea
- https://www.cs.upc.edu/~npelechano/MIG2013_Oliva.pdf
- https://ai.stanford.edu/~ddolgov/papers/dolgov_gpp_stair08.pdf
- https://ics-websites.science.uu.nl/docs/vakken/mcrws/papers_new/Reynolds%20-%201999%20-%20Steering%20behaviors%20for%20autonomous%20characters.pdf

## Phases

- [Phase 0 - Baseline and Debug Harness](PHASE_0_BASELINE.md)
- [Phase 1 - Capsule Geometry](PHASE_1_CAPSULE_GEOMETRY.md)
- [Phase 2 - Static Clearance Field](PHASE_2_CLEARANCE_FIELD.md)
- [Phase 3 - Clearance-Aware Route Planning](PHASE_3_CLEARANCE_ROUTE_PLANNING.md)
- [Phase 4 - Motion Primitive Local Planner](PHASE_4_MOTION_PRIMITIVE_LOCAL_PLANNER.md)
- [Phase 5 - Swept Collision and Wall Response](PHASE_5_SWEPT_COLLISION_WALL_RESPONSE.md)
- [Phase 6 - Traffic and Group Movement](PHASE_6_TRAFFIC_GROUP_MOVEMENT.md)
- [Phase 7 - Verification and Tuning](PHASE_7_VERIFICATION_TUNING.md)
