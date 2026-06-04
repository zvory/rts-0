# Phase 3 - Vehicle Body Geometry Refactor

## Goal

Replace tank-as-circle physical occupancy with an oriented vehicle body. This is the major refactor
phase and is the most likely requirement if tanks still feel like they have a huge invisible bubble.

## Steps

1. Introduce a general body enum in `services::geometry`, for example:

   ```rust
   enum UnitBody {
       Circle(CircleBody),
       OrientedBox(OrientedBoxBody),
       Capsule(CapsuleBody),
   }
   ```

2. Keep infantry as circles. Give tanks an oriented rectangle or capsule using hull facing,
   length, width, and a small clearance margin.
3. Update static standability:
   - world bounds;
   - terrain tile intersection;
   - building rectangle intersection;
   - resource-node/build-site intersection.
4. Update swept/segment standability so path simplification and lookahead checks can prove a tank
   hull can move from one point to another without clipping.
5. Update spawn legality and building-site clearing so production and construction understand the
   new body shape.
6. Update invariants to report body-shape failures clearly.
7. Keep broad-phase queries conservative by using a bounding radius around the oriented body.
8. Add geometry tests for tank front/side clearance against buildings, rocks, world edges, and
   narrow corridors.

## Plain-Language Explanation

The tank should occupy something shaped like a tank, not a big circle. A circle makes the tank feel
too wide at the corners and too padded near its front and rear. This phase changes the physical
model so collision and legal-position checks match the visual hull more closely.

## Expected Code Touches

- `server/src/game/services/geometry.rs`
- `server/src/game/services/standability.rs`
- `server/src/game/services/movement.rs`
- `server/src/game/services/production.rs`
- `server/src/game/services/construction.rs` if build-site checks need new body helpers
- `server/src/game/invariants.rs`
- `server/src/config.rs` or `server/src/rules/defs.rs` for tank length/width/clearance stats
- `client/src/config.js` only if render/selection size hints change
- `DESIGN.md` for the new body-geometry contract

## Refactor Depth

High. This touches every place that assumes `unit_body(...) -> CircleBody`. Do this as its own
phase, with focused tests, instead of mixing it into locomotion changes.

## Done When

- Tanks use oriented body legality for movement and static checks.
- Infantry circle behavior is unchanged.
- Building placement, production spawn, collision, and invariants use one shared body API.
- The tank's effective physical footprint matches the rendered hull closely enough that players do
  not perceive a large invisible bubble.

