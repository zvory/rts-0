# Projection audit checklist

Use this checklist when adding a snapshot field, transient event, observer mode, replay control, or
lab command path. The projection contract is intentionally explicit: normal players use
authoritative team fog, selected observer views use selected real player ids, and full-world views
use a separate diagnostic/full-world policy.

## Recipient and issuer

- Name the recipient policy: active player, live spectator, replay selected player(s), lab
  all-team, lab selected team, dev full-world, or replay branch live seat.
- If the action can issue gameplay commands, name the issuer separately from the viewer. Lab
  operators use `issueCommandAs` for the single selected owner; mixed-owner selections must stay
  rejected instead of being partitioned.
- Confirm command-card resources, faction requirements, upgrades, rally/order feedback, range
  overlays, setup wedges, audio categories, and right-click enemy classification all read from the
  command/control owner when one exists.

## Snapshot data

- For normal active players, verify entity visibility and `visibleTiles` come from living-team
  current fog. Exact-owner fields remain exact-owner-only: resources, supply, upgrades, command
  authority, ability affordances, order plans, rally plans, and private controls.
- For replay, live spectator, and Lab team/all-team views, pass the selected real player ids through
  the same snapshot body, remembered-building memory, resource rows, and event-union path.
- For full-world dev views, use the full-world snapshot body and the deterministic full-world event
  union. Do not approximate full-world behavior by choosing viewer id `0`.
- For remembered buildings, document whether the view uses one player's memory, a selected union, or
  no memory. Union memory should dedupe by building id with newest `observedTick` winning.
- For `playerResources`, expose rows only for the real player ids selected by the observer view;
  full-world/all-player views expose all active rows.

## Events

- Decide whether the event is owner-only, team-visible, selected-player-union, full-world, or global
  gameplay information. Put that policy in `docs/design/protocol.md`.
- Position, entity id, target id, `reveal`, and `toPos` data must be fog-gated unless the event is a
  documented global gameplay rule.
- Live spectator all-player unions filter position-free, info-severity private notices such as
  command rejections. Replay/lab selected-player perspectives keep private notices for the selected
  real players.
- `artilleryFiring` is intentionally global visual gameplay information. It may carry the firing
  owner, firing position, and facing, but not the shooter entity id, target point, terrain,
  exploration, or hidden entity visibility.

## Regression coverage

- Add the smallest test at the owning seam: sim projection for fog/memory, lobby room-task fanout for
  observer mode selection, or client contract tests for control-owner/render/input behavior.
- Cover both the allowed recipient and at least one denied recipient when privacy is the point.
- For client control-owner changes, include a contract that proves lab P2 control does not fall back
  to raw `state.playerId`.
