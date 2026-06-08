# Phase 5 - Visual Feedback and Planned-Order Projection

Status: Planned.

Goal: make the richer command-intent system understandable to players through owner-only feedback
without leaking hidden information.

## Scope

- Extend owner-only `orderPlan` projection only if needed to represent the new staged intent kinds:
  - self-ability stages may need a marker or may remain omitted if no safe world point exists
  - AT setup facing may need a marker that carries a safe point/facing representation
  - queued smoke should remain represented as a safe smoke target point
- Keep target ids and hidden enemy positions out of `orderPlan`.
- Show command feedback for:
  - queued vs immediate clicks
  - repeated ability targets
  - queue-full notices
  - AT setup facing intent where owner-visible
- Optionally add projected AT cones after queued move/setup if enough safe information exists:
  - base cone on current unit position for immediate setup
  - base cone on known queued destination only when the projection is unambiguous
  - avoid drawing misleading cones when no safe projected position exists

## Non-Goals

- Do not reveal fog-hidden enemy target positions.
- Do not require exact path simulation on the client.
- Do not block implementation on projected AT cones; they are useful but optional.

## Tests

- Compact snapshot encoding/decoding remains mirrored if `orderPlan` shape changes.
- Owner sees accepted queued smoke/move/attack markers.
- Non-owner and spectator fog views do not receive private order-plan data.
- Queue-full notice renders without breaking command targeting.
- Client smoke test covers no text/marker overlap after the new feedback.

## Done

- Players can understand the planned sequence well enough to use smoke walls, queued Charge, and AT
  setup without guessing.
- Fog and protocol mirror tests pass.
