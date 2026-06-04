# Phase 4 - Collision, Traffic, and Local Avoidance for Heavy Vehicles

## Goal

Make dynamic traffic respect tanks as heavy oriented vehicles instead of resolving every overlap as
circle-center displacement. This phase addresses the remaining sideways drift caused by collision
and local steering after locomotion.

## Steps

1. Update `resolve_collisions` to work through the shared body API from Phase 3.
2. Keep deterministic pair ordering and bounded pass counts.
3. Preserve footing resistance, but make displacement choices vehicle-aware:
   - infantry may be pushed more freely;
   - tanks should prefer braking, reversing, or pivoting over being shoved sideways;
   - blocked collision pushes should not rotate or move tanks into illegal hull positions.
4. Replace tank local steering with controller-compatible avoidance:
   - reduce throttle for frontal obstruction;
   - bias turn rate for reachable open space;
   - optionally reverse when boxed in;
   - avoid injecting perpendicular sidestep waypoints for tanks.
5. Add simple traffic priority rules:
   - moving tanks have priority over idle soft infantry;
   - deployed/braced weapons hold ground;
   - tank-vs-tank conflicts slow or stop rather than both sliding past each other.
6. Add tests for tank/infantry overlap, tank/tank head-on conflict, tank near wall, tank near
   building corner, and mixed group movement.

## Plain-Language Explanation

Even if the tank drives correctly, the post-movement collision system can still shove it sideways.
This phase makes traffic resolution match the vehicle model: tanks should slow, push lighter units,
or pivot, not slide like a puck.

## Expected Code Touches

- `server/src/game/services/movement.rs`
- `server/src/game/services/spatial.rs` if broad-phase support needs shape bounds
- `server/src/game/services/geometry.rs`
- `server/src/game/invariants.rs`
- `DESIGN.md` for the updated collision contract

## Refactor Depth

High. This is the second deep refactor. It depends on Phase 3 unless a limited circle-based
prototype is chosen first.

## Done When

- Collision no longer creates obvious tank sideways drift.
- Heavy vehicles can still be blocked in cramped traffic.
- Infantry can be displaced by tanks without becoming pass-through.
- Replay determinism and invariant checks still hold.

