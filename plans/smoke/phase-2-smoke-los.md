# Phase 2: Smoke World Model and Authoritative LOS

## Objective

Introduce active smoke clouds as authoritative world state and make fog/combat respect them. This
phase proves the gameplay rule that smoke blocks line of sight and suppresses vision from units
inside it.

## Server World Model

- Add a `SmokeCloud` store owned by `Game` or a small world-effects module.
- Fields should include stable id, center x/y, radius px or tiles, spawned tick, and expires tick.
- Smoke has no owner.
- Clouds expire independently.
- Smoke should not participate in pathing, collision, scoring, supply, or targeting as an entity.

## Dynamic LOS

Extend the existing LOS seam rather than adding ad hoc checks in combat or fog:

- `LineOfSight` currently raycasts static terrain.
- Add a dynamic blocker input or companion service that checks active smoke discs.
- Fog stamping should treat smoke as opaque except for the origin's own smoke state rules.
- Combat target acquisition and firing should use the same smoke-aware LOS query.

## Vision Rules

Implement the special cases explicitly:

- A unit inside smoke contributes no vision. It should not stamp fog for its owner.
- Friendly units inside smoke remain visible to their owner/allies via ownership/friendly
  projection, but they do not reveal outside tiles.
- Enemy units inside smoke are not visible through fog and cannot be targeted.
- Units inside smoke do not acquire new targets and should drop/ignore retained targets.
- A target behind smoke is not fireable because smoke blocks LOS.

## Projection Rules

- Project active smoke clouds only to players who can currently see the cloud according to the
  decided visibility rule from Phase 0.
- Ensure `target_id`, attack reveal events, death events, and positional notices do not reveal units
  hidden by smoke.
- Spectator/dev full-world snapshots may include all smoke clouds.

## Done

- A deployed smoke cloud blocks fog and combat LOS.
- Units do not fire through smoke unless another clear, non-smoke LOS path exists; AT gun to tank is
  the core regression case.
- Enemy units inside smoke are withheld from normal player snapshots.
- Friendly units inside smoke remain in owner snapshots but do not expand visible fog.
- Smoke expiration restores normal LOS.

## Verification

- `cd server && cargo test`
- Add focused fog and combat unit tests around generic firing LOS, AT gun vs tank, units inside
  smoke, and overlapping expiration.
