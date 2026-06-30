# Phase 5 - Queued Planning, Minimap Targeting, And Reconciliation

## Phase Status

Status: pending.

## Objective

Make queued artillery fire planning and minimap targeting match the new server semantics. Players
should be able to queue movement, setup, Point Fire, and Blanket Fire with previews that use planned
origins where possible, and stale local previews should clear when authoritative state disagrees.

## Scope

- Extend client queued fire previews so a queued Point Fire or Blanket Fire after movement uses the
  selected artillery piece's future queued position when the client can infer it.
- Reuse or extend the frozen queued setup planning preview for artillery so Shift-queued setup
  provides a stable future origin/facing for later Point Fire and Blanket Fire targeting.
- Make queued fire command feedback mark the client-computed locked effective point when available.
  Do not store or display the raw clicked point as the accepted target when a lock is known.
- Keep queued fire terminal per artillery in the client affordance layer, matching the server.
  Later queued commands should not appear to append behind accepted Point Fire or Blanket Fire for
  the same gun.
- Make minimap world-coordinate targeting issue the same `pointFire` and `blanketFire` command
  semantics as ordinary world targeting, including Shift queueing and command feedback where the
  minimap has enough data.
- If minimap hover cannot show the full per-gun cone or blanket preview, keep issued commands
  correct and avoid drawing misleading simplified feedback.
- Reconcile local planned/frozen previews with authoritative owner-only `orderPlan` snapshots.
- Clear stale previews when affected units are deselected, Stop/Hold is issued, Escape or explicit
  cancel is used, unqueued replacement commands are sent, match teardown occurs, a unit is no
  longer owned/visible to the command owner, or server `orderPlan` no longer contains the matching
  queued setup/fire stage.
- Update `docs/design/client-ui.md`, `docs/design/server-sim.md`, and `docs/design/protocol.md` for
  queued planning, minimap targeting, and reconciliation behavior.

## Expected Touch Points

- `client/src/client_intent.js`
- `client/src/input/commands.js`
- `client/src/input/index.js`
- `client/src/minimap.js`
- `client/src/command_composer.js`
- `client/src/state.js`
- `client/src/hud_command_card.js`
- `client/src/renderer/feedback.js`
- `client/src/renderer/feedback_view_model.js`
- `tests/client_contracts/state_input_contracts.mjs`
- `tests/client_contracts/input_contracts.mjs`
- `tests/client_contracts/command_composer_contracts.mjs`
- `tests/minimap_input_contracts.mjs`
- `tests/client_contracts/renderer_feedback_contracts.mjs`
- `tests/tri_state/scenarios/queued_move_order_stages_survive_replay.mjs` or a nearby queued-order
  scenario if it is the smallest integration fit
- `docs/design/client-ui.md`
- `docs/design/server-sim.md`
- `docs/design/protocol.md`

## Edge Cases To Cover

- `move -> pointFire` and `move -> blanketFire` previews use the future move destination when known.
- `move -> setup -> pointFire` and `move -> setup -> blanketFire` previews use the frozen setup
  origin/facing when known.
- Shift remains held after queued setup or queued fire without leaving the wrong target mode armed.
- Re-arming setup or fire replaces stale planned previews for the same artillery instead of stacking
  contradictory cones/radii.
- Minimap Shift-click fire commands preserve queued semantics and ability ids.
- Stop/Hold, immediate move/attack/build/deconstruct/ability replacement, selection changes, death,
  loss of ownership, or server order-plan mismatch clears affected local previews.
- Reconnect or replay snapshots show accepted `orderPlan` markers without requiring the original
  local frozen preview state.
- Mixed Artillery/Rifleman or Artillery/Anti-Tank Gun selections show understandable previews and
  issue only compatible fire orders.
- Client-side terminal affordances do not prevent compatible later queued orders for other selected
  units that did not accept the terminal fire order.

## Verification

- Focused client contract tests for queued preview origin selection, frozen setup lifetime,
  minimap fire commands, stale preview cleanup, and order-plan reconciliation.
- Focused tri-state or integration scenario only if unit-level client contracts cannot cover the
  owner-only `orderPlan` behavior.
- `node scripts/check-client-architecture.mjs`
- `node tests/protocol_parity.mjs` if order-plan docs or compact metadata change.
- `git diff --check`

## Manual Test Focus

In a local match, queue artillery movement, Shift-queue setup, then Shift-queue Point Fire and
Blanket Fire from both world view and minimap. Confirm previews use the planned origin, command
feedback marks locked effective points, Stop clears the plan, and the gun eventually fires only
after reaching and setting up at the planned position.

## Handoff Expectations

Explain how local queued previews choose future origins and how they reconcile with `orderPlan`.
Call out any minimap hover limitations and provide patch-note bullets for queued fire planning and
targeting from planned positions.
