# Phase 2 - Basic Queued Order Markers

Goal: make queued movement understandable to players with owner-only visual feedback.

## Scope

- Snapshot/projection:
  - Expose capped owner-only queued point markers for selected own units.
  - Do not expose enemy target ids or hidden positions through queued data.
  - Keep compact snapshot encoding bounded and documented.
- Client state:
  - Decode queued marker data into entity views.
  - Render basic queued path markers for selected own units:
    - line from current position or active destination to queued points
    - small point markers for each queued stage
    - attack-move stages visually distinct from plain move stages
- Feedback:
  - Shift-click command feedback should indicate append behavior.
  - Normal command feedback should continue to look like replacement behavior.

## Design Notes

Markers can be approximate. The first version does not need to show exact A* paths. Intent points
are enough for trust: players need to see that the server accepted the queued stages and understand
their order.

Prefer owner-only snapshot fields over client-only predicted markers. Client prediction will drift
when invalid orders are skipped, queue caps are hit, or the server clamps/sanitizes input.

## Tests

- Queue marker data is visible only to the owning player.
- Hidden enemy positions are not serialized through queued marker data.
- Compact snapshot decoding tolerates missing queued marker fields.
- Selected units render queued points without overlapping existing rally markers or command
  feedback.

## Done

- A player can Shift-click several move/attack-move stages and see the accepted queue for selected
  units.
- Markers disappear when the queue is cleared, consumed, or the unit dies.

